//! The content-addressed synthesis cache: where an entry lives, what it must
//! contain, and when it may be reused.
//!
//! An entry is reused only after its recorded metadata has been validated
//! against this build's canonical audio format and its audio re-hashed to the
//! digest the artifact records. Partial sibling transactions and invalid
//! published entries are moved to collision-free quarantine before synthesis
//! publishes a replacement, as ADR-0001 §§12.6–12.7 require.
//!
//! The sharding layout is stated once, in [`entry_path_elements`], and is
//! private to this crate.

use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE,
    CacheKey, DeterminismClass, LanguageTag, ModelArtifactsHash, PlannedSegment, Revision,
    SynthesisContext, VoiceConditioningHash, WorkerBundleHash, is_blake3_hex,
};
use tempfile::Builder;

use crate::{
    AudioError, AudioFault, BuildError, CacheEntryFault, CacheError, CalibrationSource,
    ConditioningContradiction, EdgeConditioning, MAX_SEGMENT_AUDIO_MS, MAX_TRANSITION_RAMP_MS,
    ManagedPathError, REQUIRED_EDGE_SILENCE_MS, SilenceThreshold, StagedAudioProducer,
    condition_edges,
    durable::{
        DurableFileSystem, RenameOutcome, publish_directory_noreplace, sync_directory_transaction,
        write_json_atomically,
    },
    io_error, locking, managed, measure_edge_silence, samples_for,
};

// Layout version this module accepts for a cache artifact, imported rather
// than declared because ADR-0001 §12.5 also makes it a synthesis-key input:
// `study-tts-core/src/identity.rs` owns it so a change invalidates reuse
// through the key as well as through the check below, and that module names
// this file in return. Independent of the lesson and manifest schema versions
// despite sharing a value today; each versions a different document and moves
// separately. An artifact declaring anything else is refused rather than read
// on the guess that the layout did not change.
use study_tts_core::CACHE_SCHEMA_VERSION;

/// Bytes held in memory at once while hashing a file.
///
/// A 60-minute lesson master is roughly 345 MB of canonical 24 kHz mono float
/// PCM, and `manifest::write` hashes both it and the encoded export. Reading a
/// file whole made peak memory scale with lesson length, which ADR-0001 §17.14
/// rules out for the long-form soak test: "No unbounded resource growth is
/// acceptable."
const HASH_BUFFER_BYTES: usize = 64 * 1024;

/// Characters of the cache key that name the shard directory grouping entries
/// under `segments/`.
///
/// Kept below the key length so the prefix slice in `entry_path_elements` is
/// in bounds, and asserted here rather than trusted: `CacheKey` guarantees the
/// length, and this is where that guarantee stops being a comment and becomes
/// a compile error.
const CACHE_SHARD_WIDTH: usize = 2;
const _: () = assert!(CACHE_SHARD_WIDTH <= CacheKey::LENGTH);

/// Directory grouping every cache entry beneath the cache root.
const SEGMENTS_DIRECTORY: &str = "segments";

/// The audio one cache entry holds.
const AUDIO_RECORD: &str = "audio.wav";

/// The metadata describing that audio.
const ARTIFACT_RECORD: &str = "artifact.json";

/// Prefix marking sibling cache-directory transactions not yet authoritative.
const CACHE_STAGE_PREFIX: &str = ".cache-stage-";

/// Attempts this build makes at one segment before giving up.
///
/// One, and written out because ADR-0001 §12.6 spells the quarantine path
/// `attempt-<attempt>-<request-id>` and a number has to go there. `resolve`
/// runs one transaction and returns; the bounded retry policy of ADR-0001 §11.3
/// is `DELIVERY-PLAN.md` E5-S3's, and this constant is what that story replaces
/// with a real counter rather than a value it has to discover is hard-coded.
const SOLE_ATTEMPT: u32 = 1;

/// One segment's audio as the cache holds it, validated and ready to assemble.
///
/// Produced only by [`crate::CachePublisher::resolve`], so nothing downstream
/// can name a cache entry that has not passed its checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCachedArtifact {
    /// Identity of the segment within its lesson.
    pub(crate) segment_id: String,
    /// The synthesis identity that named this entry.
    pub(crate) cache_key: CacheKey,
    /// The entry directory, which a refusal routes to runtime reconciliation.
    pub(crate) entry_dir: PathBuf,
    /// The validated audio inside that entry.
    pub(crate) audio_path: PathBuf,
    /// Digest of that audio, re-verified before assembly reads it.
    pub(crate) audio_blake3: String,
    /// Frames the audio holds, agreeing with the artifact record.
    pub(crate) frames: u32,
    /// Silence to write after this segment, in milliseconds.
    pub(crate) pause_after_ms: u32,
}

impl ValidatedCachedArtifact {
    /// Identifies the planned segment that produced this validation token.
    pub fn segment_id(&self) -> &str {
        &self.segment_id
    }

    /// Carries the synthesis identity rechecked against `artifact.json`.
    pub fn cache_key(&self) -> &CacheKey {
        &self.cache_key
    }

    /// Locates the immutable entry retained for runtime reconciliation.
    pub fn entry_dir(&self) -> &Path {
        &self.entry_dir
    }

    /// Locates the WAV whose structure and artifact record were rechecked.
    pub fn audio_path(&self) -> &Path {
        &self.audio_path
    }

    /// Carries the digest reverified before this token was issued.
    pub fn audio_blake3(&self) -> &str {
        &self.audio_blake3
    }

    /// Carries the frame count agreed by the WAV and artifact record.
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Preserves the plan's post-segment silence for deterministic assembly.
    pub fn pause_after_ms(&self) -> u32 {
        self.pause_after_ms
    }
}

/// `artifact.json`: what one cache entry declares about the audio beside it.
///
/// `deny_unknown_fields` because a field this build does not know is a field
/// it cannot check, and an entry it cannot fully check is one it must refuse
/// rather than partly trust. The declared format is compared against this
/// build's canonical values before the audio is reused, so an entry written by
/// a differently configured build is a refusal instead of a silent format
/// change part way through a master.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct CacheArtifact {
    schema_version: String,
    cache_key: CacheKey,
    audio_blake3: String,
    sample_rate: u32,
    channels: u16,
    sample_format: String,
    frames: u32,
    edge_conditioning: RecordedConditioning,
    provenance: ArtifactProvenance,
}

/// What conditioning did to this entry's audio, and under which calibration.
///
/// ADR-0001 §11.1 and §13.4 both require the padding and ramp sample counts to
/// be recorded rather than merely applied, so a reviewer can tell audio that
/// needed no work from audio that was rebuilt at both ends.
///
/// Separate from [`EdgeConditioning`], which is what the conditioner returns:
/// this is the durable shape, and it carries the calibration the counts were
/// produced under. `EdgeConditioning` is measured in samples and knows nothing
/// about provenance; flattening it in here is not open either, because
/// `serde(flatten)` and `deny_unknown_fields` cannot both apply, and
/// `deny_unknown_fields` is the rule this record is built on.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct RecordedConditioning {
    leading_padding: u32,
    trailing_padding: u32,
    leading_ramp: u32,
    trailing_ramp: u32,
    calibration_source: CalibrationSource,
}

impl RecordedConditioning {
    /// Records what the conditioner reported, under the threshold it applied.
    fn new(conditioning: EdgeConditioning, threshold: SilenceThreshold) -> Self {
        Self {
            leading_padding: conditioning.leading_padding,
            trailing_padding: conditioning.trailing_padding,
            leading_ramp: conditioning.leading_ramp,
            trailing_ramp: conditioning.trailing_ramp,
            calibration_source: threshold.source(),
        }
    }
}

/// The identities the worker reported for the audio beside this record.
///
/// ADR-0001 §12.5 derives the cache key from exactly these inputs, so recording
/// them is not a duplicate of the key: the key is a digest that can only answer
/// "same or different", while this answers *which* input differs when a
/// reviewer or a later build has to explain an entry. It is written only after
/// [`synthesize_transaction`] has proved that these values recompute to the key
/// the entry is published under, so the record cannot describe other audio.
///
/// `deny_unknown_fields` for the reason [`CacheArtifact`] has it: a field this
/// build cannot check is a field it must not silently accept.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct ArtifactProvenance {
    worker_bundle_hash: WorkerBundleHash,
    model_repository: String,
    model_revision: Revision,
    tokenizer_revision: Revision,
    /// Identity of the model bytes the gate proved for the build that wrote
    /// this entry.
    ///
    /// Recorded because the key is derived from it: without it this record
    /// could not recompute the key the entry is published under, which is the
    /// check `synthesize_transaction` makes before publishing. Required rather
    /// than optional, which is why [`study_tts_core::CACHE_SCHEMA_VERSION`]
    /// takes a major with it.
    model_artifacts_hash: ModelArtifactsHash,
    language: LanguageTag,
    determinism_class: DeterminismClass,
    seed: u64,
    /// Backend generation parameters, by name.
    ///
    /// Through [`crate::distinct_map`] because a repeated name here is not a
    /// duplicate of anything: the map keeps the last binding, the key
    /// recomputes from what it kept, and the entry is reused under a record
    /// that no longer says one thing about what produced its audio.
    #[serde(deserialize_with = "crate::distinct_map::deserialize")]
    generation_parameters: BTreeMap<String, String>,
    /// Voice-conditioning artifact for this segment's speaker, absent until
    /// E1-S2 resolves voice references.
    voice_conditioning_hash: Option<VoiceConditioningHash>,
    /// Voice profile the worker resolved that artifact through.
    ///
    /// The profile identity rather than a digest of the profile record: it
    /// names the directory holding the consent decision a reviewer follows,
    /// which is what this field is read for, and it is a value the worker can
    /// actually produce — `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md`
    /// records why a digest is not.
    voice_profile: String,
    /// Backend build that produced the audio; diagnostic, not an identity
    /// input.
    backend_revision: String,
}

impl ArtifactProvenance {
    /// Rebuilds the synthesis context this record claims was used.
    ///
    /// `speaker` is the segment's, because the record keeps one
    /// voice-conditioning hash — the one for the speaker of the segment it
    /// describes — while a [`SynthesisContext`] carries a map. Rebuilding the
    /// map with that single entry reproduces exactly what
    /// [`SynthesisContext::key_for`] reads, and nothing more.
    ///
    /// `voice_profile` and `backend_revision` are deliberately absent:
    /// ADR-0001 §12.5 does not make either a synthesis-key input, so folding
    /// them in here would derive a key the planner never could.
    fn context(&self, speaker: &str) -> SynthesisContext {
        SynthesisContext {
            worker_bundle_hash: self.worker_bundle_hash.clone(),
            model_repository: self.model_repository.clone(),
            model_revision: self.model_revision.clone(),
            tokenizer_revision: self.tokenizer_revision.clone(),
            model_artifacts_hash: self.model_artifacts_hash.clone(),
            language: self.language.clone(),
            determinism_class: self.determinism_class,
            seed: self.seed,
            generation_parameters: self.generation_parameters.clone(),
            voice_conditioning_hashes: self
                .voice_conditioning_hash
                .clone()
                .map(|hash| BTreeMap::from([(speaker.to_owned(), hash)]))
                .unwrap_or_default(),
        }
    }
}

/// The elements naming one cache entry beneath the cache root, in order.
///
/// The single statement of the sharding layout, kept private to this crate:
/// anything outside it that derived an entry path would be a second copy of
/// the scheme, drifting silently when the shard width changes.
///
/// Total for every [`CacheKey`]: the type guarantees `CacheKey::LENGTH` ASCII
/// characters, so the shard prefix is in bounds and on a character boundary.
/// Taking a `&str` here is what made this a panic reachable from a
/// deserialized plan.
fn entry_path_elements(cache_key: &CacheKey) -> [&str; 3] {
    let key = cache_key.as_str();
    [SEGMENTS_DIRECTORY, &key[..CACHE_SHARD_WIDTH], key]
}

/// Every published entry beneath `cache_root`, keyed by the identity it is
/// filed under.
///
/// A reader of [`entry_path_elements`] rather than a second copy of it: each
/// directory found is parsed back into a [`CacheKey`] and its own path
/// re-derived from that key, so a shard width that changed would fail here
/// instead of quietly enumerating nothing.
///
/// A cache with no entries yet is an empty map rather than an error, and the
/// tree is never created: this is a report, and a report must not make the
/// state it describes.
///
/// # Errors
///
/// [`CacheError::UnrecognizedCacheEntry`] for a directory the cache did not
/// name, [`ManagedPathError::ManagedPathEscape`] for a planted link, otherwise
/// [`IoError::FileSystem`].
pub(crate) fn published_entries(
    cache_root: &Path,
) -> Result<BTreeMap<CacheKey, PathBuf>, BuildError> {
    let segments = managed::directory_candidate(cache_root, SEGMENTS_DIRECTORY)?;
    let mut entries = BTreeMap::new();
    if !segments.is_dir() {
        return Ok(entries);
    }

    for shard in read_directories(&segments)? {
        for entry in read_directories(&shard)? {
            let unrecognized = || {
                BuildError::from(CacheError::UnrecognizedCacheEntry {
                    entry_dir: entry.clone(),
                })
            };
            let key: CacheKey = entry
                .file_name()
                .and_then(OsStr::to_str)
                .ok_or_else(unrecognized)?
                .parse()
                .map_err(|_| unrecognized())?;
            // The layout, checked rather than assumed: a key found under the
            // wrong shard is a tree this build did not write.
            if resolve_entry_location(cache_root, &key)?.1 != entry {
                return Err(unrecognized());
            }
            entries.insert(key, entry);
        }
    }
    Ok(entries)
}

/// The immediate subdirectories of `directory`, in a stable order.
///
/// Shared with `preview`, which walks its own published tree the same way.
///
/// # Errors
///
/// [`IoError::FileSystem`] when the directory or an entry cannot be read.
pub(crate) fn read_directories(directory: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| io_error(directory, error))? {
        let entry = entry.map_err(|error| io_error(directory, error))?;
        // `symlink_metadata`, so a link to a directory elsewhere is not walked
        // into: `managed` refuses one on the resolution path and this is the
        // enumeration path.
        let metadata = entry
            .metadata()
            .map_err(|error| io_error(entry.path(), error))?;
        if metadata.is_dir() {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

/// Resolves one entry's shard and no-replace destination.
///
/// [`entry_path_elements`] states the layout; this walks it one level at a
/// time. A *lexical* escape is already impossible because `CacheKey`
/// guarantees hex, so no element can carry a separator or `..` — but a symlink
/// planted at `segments`, at the shard, or at the entry itself could redirect
/// later reads or publication. Managed resolution refuses each one before use.
///
/// # Errors
///
/// Whatever `managed::subdirectory` reports for the first element that fails:
/// [`ManagedPathError::ManagedPathEscape`] for a planted link or a result
/// outside its parent, otherwise [`IoError::FileSystem`].
fn resolve_entry_location(
    cache_root: &Path,
    cache_key: &CacheKey,
) -> Result<(PathBuf, PathBuf), BuildError> {
    let elements = entry_path_elements(cache_key);
    let segments = managed::subdirectory(cache_root, elements[0])?;
    let shard = managed::subdirectory(&segments, elements[1])?;
    let entry = managed::directory_candidate(&shard, elements[2])?;
    Ok((shard, entry))
}

/// Names the entry a fault was found in.
///
/// Shared with `assembly`, which detects a truncated entry while reading it, so
/// both report the same violated invariant with the same remedy rather than two
/// messages that happen to agree.
pub(crate) fn rejected(entry_dir: &Path, segment_id: &str, fault: CacheEntryFault) -> BuildError {
    CacheError::UnusableCacheEntry {
        entry_dir: entry_dir.to_path_buf(),
        segment_id: segment_id.to_owned(),
        fault: Box::new(fault),
    }
    .into()
}

/// Returns the segment's audio from the cache, publishing a complete directory
/// transaction when the immutable entry is absent.
///
/// A hit is only a hit once the entry's recorded metadata matches this build's
/// canonical format and its audio re-hashes to the recorded digest. A partial
/// transaction or invalid published entry is quarantined before synthesis
/// publishes a replacement.
///
/// # Errors
///
/// [`AudioError::UnusableAudio`] when fresh synthesis produces audio this
/// build cannot use,
/// [`ManagedPathError::ManagedPathEscape`] when a link occupies an entry path,
/// [`crate::DurableStateError::QuarantineFailed`] when a failed attempt cannot
/// be retained, and [`BuildError::Synthesis`] when the staged producer fails.
pub(crate) fn resolve(
    filesystem: &dyn DurableFileSystem,
    cache_root: &Path,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    producer: &mut dyn StagedAudioProducer,
) -> Result<ValidatedCachedArtifact, BuildError> {
    let (_shard, entry_dir) = resolve_entry_location(cache_root, &segment.cache_key)?;
    if entry_dir.is_dir() {
        match load_entry(segment, &entry_dir) {
            Ok(entry) => return Ok(entry),
            Err(error) if is_unusable_cache_entry(&error) => {}
            Err(error) => return Err(error),
        }
    }

    let _key_lock = locking::acquire_cache_key_lock(cache_root, &segment.cache_key)?;
    let (shard, entry_dir) = resolve_entry_location(cache_root, &segment.cache_key)?;
    if entry_dir.is_dir() {
        match load_entry(segment, &entry_dir) {
            Ok(entry) => return Ok(entry),
            Err(error) if is_unusable_cache_entry(&error) => {
                quarantine_invalid_entry(
                    filesystem,
                    quarantine_root,
                    job_id,
                    segment,
                    &entry_dir,
                    error,
                )?;
            }
            Err(error) => return Err(error),
        }
    }

    reconcile_stages(
        filesystem,
        &shard,
        &entry_dir,
        quarantine_root,
        job_id,
        segment,
    )?;
    if entry_dir.is_dir() {
        return load_entry(segment, &entry_dir);
    }

    synthesize_transaction(
        filesystem,
        &shard,
        &entry_dir,
        quarantine_root,
        job_id,
        segment,
        producer,
    )
}

fn is_unusable_cache_entry(error: &BuildError) -> bool {
    matches!(
        error,
        BuildError::Cache(CacheError::UnusableCacheEntry { .. })
    )
}

fn quarantine_invalid_entry(
    filesystem: &dyn DurableFileSystem,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    entry_dir: &Path,
    primary: BuildError,
) -> Result<(), BuildError> {
    match quarantine_transaction(filesystem, quarantine_root, job_id, segment, entry_dir) {
        Ok(_) => Ok(()),
        Err(cleanup) => Err(crate::DurableStateError::QuarantineFailed {
            staging_path: entry_dir.to_path_buf(),
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }
        .into()),
    }
}

fn load_entry(
    segment: &PlannedSegment,
    entry_dir: &Path,
) -> Result<ValidatedCachedArtifact, BuildError> {
    let audio_path = managed::leaf(entry_dir, AUDIO_RECORD)?;
    let artifact_path = managed::leaf(entry_dir, ARTIFACT_RECORD)?;
    load_validated(segment, entry_dir, &audio_path, &artifact_path)
}

fn synthesize_transaction(
    filesystem: &dyn DurableFileSystem,
    shard: &Path,
    entry_dir: &Path,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    producer: &mut dyn StagedAudioProducer,
) -> Result<ValidatedCachedArtifact, BuildError> {
    let stage = Builder::new()
        .prefix(&format!(
            "{CACHE_STAGE_PREFIX}{}-",
            segment.cache_key.as_str()
        ))
        .tempdir_in(shard)
        .map_err(|error| io_error(shard, error))?
        .keep();
    let audio_path = managed::leaf(&stage, AUDIO_RECORD)?;
    let artifact_path = managed::leaf(&stage, ARTIFACT_RECORD)?;

    let report = match producer.produce(&audio_path) {
        Ok(report) => report,
        Err(error) => {
            let primary = BuildError::from(error);
            return Err(quarantine_failed_attempt(
                filesystem,
                quarantine_root,
                job_id,
                segment,
                &stage,
                primary,
            ));
        }
    };
    if let Err(error) = managed::leaf(&stage, AUDIO_RECORD) {
        return Err(quarantine_failed_attempt(
            filesystem,
            quarantine_root,
            job_id,
            segment,
            &stage,
            error,
        ));
    }
    // What the stage *contains* is what gets published: the transaction below
    // renames this directory into place, so a scratch file the worker left
    // beside its audio would be published inside an entry claiming to hold one
    // segment's speech. Checked here, while the producer that made it is still
    // attributable, rather than at the rename where it is only a surprising
    // directory listing.
    if let Err(error) = check_stage_holds_only_audio(&stage, segment) {
        return Err(quarantine_failed_attempt(
            filesystem,
            quarantine_root,
            job_id,
            segment,
            &stage,
            error,
        ));
    }
    let validated = match validate_wav(&audio_path) {
        Ok(validated) => validated,
        Err(fault) => {
            let error = AudioError::UnusableAudio {
                path: audio_path.clone(),
                fault,
            };
            return Err(quarantine_failed_attempt(
                filesystem,
                quarantine_root,
                job_id,
                segment,
                &stage,
                error.into(),
            ));
        }
    };
    if report.sample_rate != CANONICAL_SAMPLE_RATE
        || report.channels != CANONICAL_CHANNELS
        || report.frames != validated_frames(&validated)
    {
        let error = AudioError::SynthesizerReportMismatch {
            segment_id: segment.id.clone(),
            reported_sample_rate: report.sample_rate,
            reported_channels: report.channels,
            reported_frames: report.frames,
            written_sample_rate: CANONICAL_SAMPLE_RATE,
            written_channels: CANONICAL_CHANNELS,
            written_frames: validated_frames(&validated),
        };
        return Err(quarantine_failed_attempt(
            filesystem,
            quarantine_root,
            job_id,
            segment,
            &stage,
            error.into(),
        ));
    }

    // The identity gate. Everything above proved the *file* is what the worker
    // said it was; this proves the worker synthesized what the plan asked for.
    // Recomputing the whole key from the reported inputs is deliberate: a
    // field-by-field comparison would stop covering any input added later,
    // whereas a key that does not match cannot name this audio at all.
    //
    // It is only as strong as the report is independent. Since E1-S2 the
    // voice-conditioning artifact reaches the key, and E1-S3 made the worker
    // report the artifact it actually read rather than echoing the requested
    // one, which is what `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`
    // §Limits this change does not close recorded as owed.
    //
    // The worker reports that artifact twice, and only the copy inside
    // `context` reaches the key. So the two are checked against each other
    // first: a report naming one artifact in `voice_conditioning_hash` and
    // another in `context` would otherwise pass the gate below on the second
    // while publishing provenance built from — and a cache key derived from —
    // values its own top-level field contradicts. The gate cannot see it,
    // because both sides of the comparison come from the same half of the
    // report.
    let in_context = report.context.voice_conditioning_for(&segment.speaker);
    if in_context != Some(&report.voice_conditioning_hash) {
        let error =
            AudioError::ConditioningIdentityContradiction(Box::new(ConditioningContradiction {
                segment_id: segment.id.clone(),
                reported: report.voice_conditioning_hash.to_string(),
                in_context: in_context.map_or_else(
                    || "no conditioning artifact at all".to_owned(),
                    ToString::to_string,
                ),
            }));
        return Err(quarantine_failed_attempt(
            filesystem,
            quarantine_root,
            job_id,
            segment,
            &stage,
            error.into(),
        ));
    }

    let reported_key = report.context.key_for(segment);
    if reported_key != segment.cache_key {
        let error = AudioError::SynthesizerIdentityMismatch {
            segment_id: segment.id.clone(),
            planned: segment.cache_key.clone(),
            reported: reported_key,
        };
        return Err(quarantine_failed_attempt(
            filesystem,
            quarantine_root,
            job_id,
            segment,
            &stage,
            error.into(),
        ));
    }

    // Conditioned here: after the report gates above, which compare the
    // worker's claims against what the worker wrote, and before the hash below,
    // which must cover the bytes that are actually published. `frames` is
    // replaced because conditioning adds zero padding, so the count recorded in
    // the artifact is the conditioned one.
    let staged = match condition_staged_audio(&audio_path, validated) {
        Ok(staged) => staged,
        Err(fault) => {
            let error = AudioError::UnusableAudio {
                path: audio_path.clone(),
                fault,
            };
            return Err(quarantine_failed_attempt(
                filesystem,
                quarantine_root,
                job_id,
                segment,
                &stage,
                error.into(),
            ));
        }
    };

    let audio_blake3 = hash_file(&audio_path)?;
    let artifact = CacheArtifact {
        schema_version: CACHE_SCHEMA_VERSION.to_owned(),
        cache_key: segment.cache_key.clone(),
        audio_blake3,
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
        frames: staged.frames,
        // ADR-0001 §11.1 and §13.4: the counts are recorded, not merely
        // applied. Taken from the conditioner's own report rather than
        // re-derived, so the record cannot describe conditioning other than the
        // conditioning that produced these bytes.
        edge_conditioning: RecordedConditioning::new(
            staged.conditioning,
            SilenceThreshold::provisional(),
        ),
        provenance: ArtifactProvenance {
            worker_bundle_hash: report.context.worker_bundle_hash.clone(),
            model_repository: report.context.model_repository.clone(),
            model_revision: report.context.model_revision.clone(),
            tokenizer_revision: report.context.tokenizer_revision.clone(),
            model_artifacts_hash: report.context.model_artifacts_hash.clone(),
            language: report.context.language.clone(),
            determinism_class: report.context.determinism_class,
            seed: report.context.seed,
            generation_parameters: report.context.generation_parameters.clone(),
            voice_conditioning_hash: report
                .context
                .voice_conditioning_for(&segment.speaker)
                .cloned(),
            voice_profile: report.voice_profile.clone(),
            backend_revision: report.backend_revision.clone(),
        },
    };
    write_json_atomically(filesystem, &artifact_path, &artifact)?;
    sync_directory_transaction(filesystem, &stage, &[&audio_path, &artifact_path])?;

    match publish_directory_noreplace(filesystem, &stage, entry_dir)? {
        RenameOutcome::Published => load_entry(segment, entry_dir),
        RenameOutcome::DestinationExists => {
            let winner = load_entry(segment, entry_dir)?;
            quarantine_transaction(filesystem, quarantine_root, job_id, segment, &stage)?;
            Ok(winner)
        }
    }
}

fn reconcile_stages(
    filesystem: &dyn DurableFileSystem,
    shard: &Path,
    entry_dir: &Path,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
) -> Result<(), BuildError> {
    let prefix = format!("{CACHE_STAGE_PREFIX}{}-", segment.cache_key.as_str());
    let mut stages = Vec::new();
    for entry in fs::read_dir(shard).map_err(|error| io_error(shard, error))? {
        let entry = entry.map_err(|error| io_error(shard, error))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(&prefix) {
            stages.push(name.to_owned());
        }
    }
    stages.sort();

    for stage_name in stages {
        let stage = managed::directory_candidate(shard, &stage_name)?;
        if entry_dir.is_dir() {
            quarantine_transaction(filesystem, quarantine_root, job_id, segment, &stage)?;
            continue;
        }
        let audio = match managed::leaf(&stage, AUDIO_RECORD) {
            Ok(path) => path,
            Err(_) => {
                quarantine_transaction(filesystem, quarantine_root, job_id, segment, &stage)?;
                continue;
            }
        };
        let artifact = match managed::leaf(&stage, ARTIFACT_RECORD) {
            Ok(path) => path,
            Err(_) => {
                quarantine_transaction(filesystem, quarantine_root, job_id, segment, &stage)?;
                continue;
            }
        };
        if audio.is_file()
            && artifact.is_file()
            && load_validated(segment, &stage, &audio, &artifact).is_ok()
        {
            sync_directory_transaction(filesystem, &stage, &[&audio, &artifact])?;
            if publish_directory_noreplace(filesystem, &stage, entry_dir)?
                == RenameOutcome::Published
            {
                continue;
            }
        }
        if stage.exists() {
            quarantine_transaction(filesystem, quarantine_root, job_id, segment, &stage)?;
        }
    }
    Ok(())
}

/// Refuses a staging transaction holding anything but the assigned audio.
///
/// Only the audio, because this runs before the artifact record is written: at
/// this point the producer has returned and nothing else has been staged, so
/// every other name in the directory came from the worker.
///
/// # Errors
///
/// [`CacheError::UncontainedStagedFile`] naming the first unexpected entry, and
/// [`BuildError::Io`] when the stage cannot be read — which is itself a reason
/// not to publish it.
fn check_stage_holds_only_audio(stage: &Path, segment: &PlannedSegment) -> Result<(), BuildError> {
    for entry in fs::read_dir(stage).map_err(|error| io_error(stage, error))? {
        let entry = entry.map_err(|error| io_error(stage, error))?;
        let name = entry.file_name();
        if name != OsStr::new(AUDIO_RECORD) {
            return Err(CacheError::UncontainedStagedFile {
                segment_id: segment.id.clone(),
                unexpected: name.to_string_lossy().into_owned(),
            }
            .into());
        }
    }
    Ok(())
}

/// Moves one failed staging transaction to the path ADR-0001 §12.6 names.
///
/// The layout is the job, the segment, `take-<take>`, and then
/// `attempt-<attempt>-<request-id>-<nonce>`: §12.6's spelling with one
/// addition. **The nonce is not decoration.**
/// §12.6 requires the directory to be collision-free, and an attempt number and
/// a request identity do not make it so on their own: both are derived from the
/// plan, so a second run of the same plan over the same job — a resume, or an
/// operator re-running a build — reproduces them exactly and would publish into
/// a directory that already holds an earlier failure's evidence. `tempdir_in`
/// creates the directory as it names it, so the nonce is proof of exclusivity
/// rather than a guess at one.
///
/// Nothing is overwritten and nothing is deleted: §12.6 keeps quarantined
/// entries for a person to read.
fn quarantine_transaction(
    filesystem: &dyn DurableFileSystem,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    stage: &Path,
) -> Result<PathBuf, BuildError> {
    let source_parent = stage
        .parent()
        .ok_or_else(|| ManagedPathError::UnrootedDestination {
            path: stage.to_path_buf(),
        })?;
    let job = managed::subdirectory(quarantine_root, job_id)?;
    let segment_dir = managed::subdirectory(&job, &segment.id)?;
    let take_dir = managed::subdirectory(&segment_dir, &format!("take-{}", segment.take))?;
    let attempt = Builder::new()
        .prefix(&format!("attempt-{SOLE_ATTEMPT}-{}-", segment.request_id()))
        .tempdir_in(&take_dir)
        .map_err(|error| io_error(&take_dir, error))?
        .keep();
    let destination = attempt.join("cache-entry");
    if publish_directory_noreplace(filesystem, stage, &destination)? != RenameOutcome::Published {
        return Err(crate::DurableStateError::PublicationConflict { path: destination }.into());
    }
    filesystem.sync_directory(source_parent)?;
    filesystem.sync_directory(&take_dir)?;
    Ok(destination)
}

fn quarantine_failed_attempt(
    filesystem: &dyn DurableFileSystem,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    stage: &Path,
    primary: BuildError,
) -> BuildError {
    match quarantine_transaction(filesystem, quarantine_root, job_id, segment, stage) {
        Ok(destination) => error_at_quarantine_destination(primary, &destination),
        Err(cleanup) => crate::DurableStateError::QuarantineFailed {
            staging_path: stage.to_path_buf(),
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }
        .into(),
    }
}

fn error_at_quarantine_destination(error: BuildError, destination: &Path) -> BuildError {
    match error {
        BuildError::Audio(AudioError::UnusableAudio { fault, .. }) => AudioError::UnusableAudio {
            path: destination.join(AUDIO_RECORD),
            fault,
        }
        .into(),
        error => error,
    }
}

/// Accepts a published entry as a hit only once every claim it makes holds.
///
/// The checks run in a fixed order so each failure is reported as itself: the
/// artifact parses, its recorded digest is well formed, its declared format is
/// this build's, its key is the plan's, its recorded provenance derives that
/// key, and only then is the audio re-hashed against the digest. A malformed
/// digest reported as a mismatch would tell an operator their file was tampered
/// with when it was merely written wrong.
///
/// # Errors
///
/// [`CacheError::UnusableCacheEntry`] carrying the [`CacheEntryFault`] that
/// names the violated invariant.
fn load_validated(
    segment: &PlannedSegment,
    entry_dir: &Path,
    audio_path: &Path,
    artifact_path: &Path,
) -> Result<ValidatedCachedArtifact, BuildError> {
    let bytes = fs::read(artifact_path).map_err(|source| {
        rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::UnreadableArtifact {
                path: artifact_path.to_path_buf(),
                source,
            },
        )
    })?;

    // Path 1: the artifact does not parse.
    let artifact: CacheArtifact = serde_json::from_slice(&bytes).map_err(|error| {
        rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::UnparseableArtifact {
                path: artifact_path.to_path_buf(),
                source: error,
            },
        )
    })?;

    // Path 2: the recorded digest is not a digest at all.
    //
    // Checked here rather than at the comparison below, because a malformed
    // record that reaches the comparison is reported as a checksum *mismatch* —
    // telling the operator their audio was tampered with when the artifact was
    // what broke. `VoiceError::MalformedChecksum` draws the same distinction
    // for voice records.
    if !is_blake3_hex(&artifact.audio_blake3) {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::MalformedRecordedDigest {
                recorded: artifact.audio_blake3,
            },
        ));
    }

    // Path 3: the artifact parses but describes audio this build cannot
    // consume.
    if artifact.schema_version != CACHE_SCHEMA_VERSION
        || artifact.sample_rate != CANONICAL_SAMPLE_RATE
        || artifact.channels != CANONICAL_CHANNELS
        || artifact.sample_format != CANONICAL_SAMPLE_FORMAT
    {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::IncompatibleArtifact {
                schema_version: artifact.schema_version,
                sample_rate: artifact.sample_rate,
                channels: artifact.channels,
                sample_format: artifact.sample_format,
                required_schema_version: CACHE_SCHEMA_VERSION,
                required_sample_rate: CANONICAL_SAMPLE_RATE,
                required_channels: CANONICAL_CHANNELS,
                required_sample_format: CANONICAL_SAMPLE_FORMAT,
            },
        ));
    }

    // Path 4: the entry belongs to a different synthesis identity.
    if artifact.cache_key != segment.cache_key {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::CacheKeyMismatch {
                recorded: artifact.cache_key,
                required: segment.cache_key.clone(),
            },
        ));
    }

    // Path 5: the recorded provenance does not derive the key it is filed
    // under.
    //
    // Path 4 only proved that two copies of the key agree. The provenance
    // beside them is the entry's audit record — which model revision, which
    // language, which worker bundle produced this audio — and nothing so far
    // has held it to the key. Editing `model_revision` on disk would otherwise
    // leave the entry reusable while its record described synthesis that never
    // happened. Cheap enough to run before the audio is re-hashed, so a
    // dishonest record is refused without reading a megabyte of WAV.
    let derived = artifact
        .provenance
        .context(&segment.speaker)
        .key_for(segment);
    if derived != artifact.cache_key {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::ProvenanceKeyMismatch {
                recorded: artifact.cache_key,
                derived,
            },
        ));
    }

    // Path 6: the audio itself is unreadable, non-canonical, or does not match
    // the artifact.
    let samples =
        validate_wav(audio_path).map_err(|fault| rejected(entry_dir, &segment.id, fault.into()))?;
    // ADR-0001 §12.6 lists the silence and edge checks among the conditions for
    // *using* an entry, not only for writing one: an entry published by a build
    // that conditioned differently is refused rather than concatenated into a
    // master with a step at its join. The validation pass retains the decoded
    // samples so all audio checks use the bytes it already read.
    check_exposed_endpoints(samples[0], samples[samples.len() - 1])
        .map_err(|fault| rejected(entry_dir, &segment.id, fault.into()))?;
    check_edge_silence(&samples).map_err(|fault| rejected(entry_dir, &segment.id, fault.into()))?;
    check_declared_conditioning(&artifact.edge_conditioning, &samples)
        .map_err(|fault| rejected(entry_dir, &segment.id, fault))?;
    let checksum = hash_file(audio_path)?;
    let frames = validated_frames(&samples);
    if frames != artifact.frames {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::FrameCountMismatch {
                found: u64::from(frames),
                declared: artifact.frames,
            },
        ));
    }
    if checksum != artifact.audio_blake3 {
        return Err(rejected(
            entry_dir,
            &segment.id,
            CacheEntryFault::ChecksumMismatch {
                found: checksum,
                declared: artifact.audio_blake3,
            },
        ));
    }

    Ok(ValidatedCachedArtifact {
        segment_id: segment.id.clone(),
        cache_key: segment.cache_key.clone(),
        entry_dir: entry_dir.to_path_buf(),
        audio_path: audio_path.to_path_buf(),
        audio_blake3: checksum,
        frames,
        pause_after_ms: segment.pause_after_ms,
    })
}

/// Hashes a file through a bounded buffer, so peak memory does not scale with
/// the file.
///
/// The digest is identical to hashing the whole file in one call, because
/// BLAKE3 over a byte sequence does not depend on how that sequence is chunked.
/// Entries and manifests recorded by earlier builds stay valid.
pub(crate) fn hash_file(path: &Path) -> Result<String, BuildError> {
    let mut file = fs::File::open(path).map_err(|error| io_error(path, error))?;
    hash_stream(&mut file).map_err(|error| io_error(path, error))
}

/// Hashes everything `source` yields, through a bounded buffer.
///
/// Retries a read the operating system interrupts. `Interrupted` means a signal
/// arrived mid-call, not that the file is unreadable: the read consumed nothing
/// and is meant to be reissued. Treating it as failure would abandon a build
/// over a signal already handled elsewhere — and would do it most often on the
/// longest file, because the more reads a hash performs the likelier one of
/// them is interrupted.
///
/// Separate from `hash_file` so the retry is reachable from a test: a real file
/// cannot be made to interrupt on demand.
fn hash_stream(source: &mut impl Read) -> io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    // Read straight into the hashing buffer rather than through a `BufReader`,
    // which would hold a second buffer of its own to no purpose.
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];

    loop {
        match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(filled) => hasher.update(&buffer[..filled]),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Writes conditioned samples back over the staged file.
///
/// Written through the same canonical spec the rest of this module asserts, so
/// a conditioned file is indistinguishable in format from the one the worker
/// wrote — only its edges differ.
fn write_canonical_samples(path: &Path, samples: &[f32]) -> Result<(), AudioFault> {
    let specification = hound::WavSpec {
        channels: CANONICAL_CHANNELS,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, specification)?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Refuses audio whose exposed endpoints are not exactly zero.
///
/// ADR-0001 §12.6 makes the edge check a condition of *using* a cache entry,
/// not only of writing one, so this runs on reuse as well as after
/// conditioning: an entry published by a build that conditioned differently is
/// refused rather than concatenated into a master with a step at its join.
fn check_exposed_endpoints(first: f32, last: f32) -> Result<(), AudioFault> {
    for (edge, value) in [("first", first), ("last", last)] {
        if value != 0.0 {
            return Err(AudioFault::ExposedEndpointNotZero { edge, value });
        }
    }
    Ok(())
}

/// Refuses audio whose exposed edges carry less silence than ADR-0001 requires.
///
/// ADR-0001 §13.4 requires at least 10 ms at each exposed edge and §12.6 makes
/// the check a condition of using an entry. Measured through
/// [`measure_edge_silence`] — the same measurement the conditioner pads from —
/// rather than by counting exact zeros: §13.4 measures silence against the
/// audio-profile threshold, so an edge that arrived quiet-but-nonzero holds
/// its 10 ms lawfully and a zero-counting test would refuse it.
///
/// This is a different question from [`check_exposed_endpoints`], and both are
/// ADR-0001 §13.4's: this one is about the *duration* of the edge silence, that
/// one about the *value* at the endpoint. Reading them as one rule is what let
/// `condition_edges` satisfy the duration and leave the endpoint at `4.3e-6`,
/// which refused every real take until the conditioner padded for it.
///
/// # Errors
///
/// [`AudioFault::InsufficientEdgeSilence`] naming the shorter edge.
fn check_edge_silence(samples: &[f32]) -> Result<(), AudioFault> {
    let (leading, trailing) = measure_edge_silence(
        samples,
        CANONICAL_SAMPLE_RATE,
        SilenceThreshold::provisional(),
    );
    check_edge_silence_counts(leading, trailing)
}

fn check_edge_silence_counts(leading: usize, trailing: usize) -> Result<(), AudioFault> {
    let required = samples_for(REQUIRED_EDGE_SILENCE_MS, CANONICAL_SAMPLE_RATE);

    for (edge, measured) in [("first", leading), ("last", trailing)] {
        if measured < required {
            return Err(AudioFault::InsufficientEdgeSilence {
                edge,
                silence_frames: u32::try_from(measured).unwrap_or(u32::MAX),
                required_frames: u32::try_from(required).unwrap_or(u32::MAX),
                required_milliseconds: REQUIRED_EDGE_SILENCE_MS,
            });
        }
    }
    Ok(())
}

/// Refuses a record declaring conditioning ADR-0001 §13.4 does not permit.
///
/// # Errors
///
/// [`CacheEntryFault::ConditioningOutsideRatifiedGeometry`] naming the first
/// count out of range, and
/// [`CacheEntryFault::ConditioningInconsistentWithAudio`] when the recorded
/// ramps cannot describe the decoded samples, and
/// [`CacheEntryFault::ConditionedUnderAnotherCalibration`] when the entry was
/// conditioned against a threshold this build no longer applies.
fn check_declared_conditioning(
    recorded: &RecordedConditioning,
    samples: &[f32],
) -> Result<(), CacheEntryFault> {
    let ramp = ratified_samples(MAX_TRANSITION_RAMP_MS);
    let padding = ratified_samples(REQUIRED_EDGE_SILENCE_MS);

    // Padding is bounded by the silence it completes: `condition_edges`
    // computes it as the shortfall against that requirement, so a record
    // declaring more describes conditioning this project does not perform.
    for (field, declared, permitted, milliseconds) in [
        (
            "leading_ramp",
            recorded.leading_ramp,
            ramp,
            MAX_TRANSITION_RAMP_MS,
        ),
        (
            "trailing_ramp",
            recorded.trailing_ramp,
            ramp,
            MAX_TRANSITION_RAMP_MS,
        ),
        (
            "leading_padding",
            recorded.leading_padding,
            padding,
            REQUIRED_EDGE_SILENCE_MS,
        ),
        (
            "trailing_padding",
            recorded.trailing_padding,
            padding,
            REQUIRED_EDGE_SILENCE_MS,
        ),
    ] {
        if declared > permitted {
            return Err(CacheEntryFault::ConditioningOutsideRatifiedGeometry {
                field,
                declared,
                permitted,
                permitted_milliseconds: milliseconds,
            });
        }
    }

    let applied = SilenceThreshold::provisional().source();
    if recorded.calibration_source != applied {
        return Err(CacheEntryFault::ConditionedUnderAnotherCalibration {
            recorded: recorded.calibration_source.name(),
            required: applied.name(),
        });
    }

    // Every non-silent conditioned segment retains at least the required edge
    // silence. Removing those two regions therefore gives an upper bound on
    // signal length. A nonzero span wider than one sample requires a ramp to
    // contribute one exact-zero sample at each end, so that span plus two gives
    // the corresponding lower bound. The original sample values remain
    // unrecoverable, but a declared ramp outside these bounds cannot have
    // produced this audio.
    let required = samples_for(REQUIRED_EDGE_SILENCE_MS, CANONICAL_SAMPLE_RATE);
    let maximum = samples_for(MAX_TRANSITION_RAMP_MS, CANONICAL_SAMPLE_RATE)
        .min(samples.len().saturating_sub(required.saturating_mul(2)) / 2);
    let first_nonzero = samples.iter().position(|sample| *sample != 0.0);
    let last_nonzero = samples.iter().rposition(|sample| *sample != 0.0);
    let minimum = match (first_nonzero, last_nonzero) {
        (Some(first), Some(last)) if first < last => maximum.min((last - first + 3) / 2),
        _ => 0,
    };
    let minimum = u32::try_from(minimum).unwrap_or(u32::MAX);
    let maximum = u32::try_from(maximum).unwrap_or(u32::MAX);
    for (field, declared) in [
        ("leading_ramp", recorded.leading_ramp),
        ("trailing_ramp", recorded.trailing_ramp),
    ] {
        if !(minimum..=maximum).contains(&declared) {
            return Err(CacheEntryFault::ConditioningInconsistentWithAudio {
                field,
                declared,
                minimum,
                maximum,
            });
        }
    }
    if recorded.leading_ramp != recorded.trailing_ramp {
        return Err(CacheEntryFault::ConditioningInconsistentWithAudio {
            field: "trailing_ramp",
            declared: recorded.trailing_ramp,
            minimum: recorded.leading_ramp,
            maximum: recorded.leading_ramp,
        });
    }
    Ok(())
}

/// One ratified edge duration as a canonical-rate frame count.
fn ratified_samples(milliseconds: u32) -> u32 {
    u32::try_from(samples_for(milliseconds, CANONICAL_SAMPLE_RATE)).unwrap_or(u32::MAX)
}

/// Conditions one staged segment's edges in place and reports the new frames.
///
/// ADR-0001 §12.6 lists duration, silence, and edge checks among the conditions
/// for publishing, and `DELIVERY-PLAN.md` E1-S3 task 4 is "validate **and
/// condition** canonical audio before atomic cache publication". This is the
/// conditioning half.
///
/// **Ordering is load-bearing.** It runs after the worker's reported frame
/// count has been checked against what the worker actually wrote — conditioning
/// adds frames, so checking afterwards would compare the worker's claim against
/// this build's own edit — and before the audio is hashed, so the digest and
/// the recorded frame count describe the bytes that are published.
///
/// Because it adds frames it re-applies the segment ceiling to its own result:
/// see [`check_segment_ceiling`], which runs before the conditioned samples are
/// written back.
///
/// # Errors
///
/// [`AudioFault::ConditionedTooLong`] when the padding would carry the segment
/// past `crate::MAX_SEGMENT_AUDIO_MS`, [`AudioFault::ExposedEndpointNotZero`]
/// when conditioning left an endpoint off zero, and whichever [`AudioFault`]
/// the read or the write-back reports.
///
/// The silence threshold is provisional while ADR-0003 is Proposed; see
/// [`crate::SilenceThreshold`] and
/// `docs/adr/deviations/ADR-0001-D007-provisional-edge-conditioning.md`.
fn condition_staged_audio(
    path: &Path,
    mut samples: Vec<f32>,
) -> Result<ConditionedStage, AudioFault> {
    let frames = u32::try_from(samples.len()).map_err(|_| AudioFault::FrameCountOverflow)?;
    let conditioning = condition_edges(
        &mut samples,
        CANONICAL_SAMPLE_RATE,
        SilenceThreshold::provisional(),
    );
    // Before the write-back, so a ceiling refusal leaves the worker's own bytes
    // in the stage. Refusals below the write retain this build's conditioned
    // bytes in quarantine instead.
    let conditioned = check_segment_ceiling(frames, samples.len())?;
    write_canonical_samples(path, &samples)?;
    check_exposed_endpoints(samples[0], samples[samples.len() - 1])?;
    // The same check reuse applies. Conditioning satisfies it by construction,
    // and asserting that here is what stops a later change to the conditioner
    // publishing entries this build would afterwards refuse to read.
    check_edge_silence(&samples)?;
    Ok(ConditionedStage {
        frames: conditioned,
        conditioning,
    })
}

/// What conditioning left behind, for the record published beside the audio.
///
/// Both halves, because ADR-0001 §13.4 requires the counts to be recorded and
/// the artifact needs the frame count: returning only the length is what let
/// the counts be measured and then discarded.
#[derive(Debug)]
struct ConditionedStage {
    frames: u32,
    conditioning: EdgeConditioning,
}

/// Refuses conditioned audio the ceiling will not let this build publish.
///
/// The ceiling is applied twice because conditioning moves the count:
/// [`validate_wav`] holds the worker to it, and this holds this build's own
/// edit to it. Without the second application, audio arriving at exactly the
/// ceiling is published up to 20 ms over it — and the reload that immediately
/// follows publication refuses the entry this build has just written, for ever,
/// because no quarantine path reaches an entry already renamed into place.
///
/// # Errors
///
/// [`AudioFault::ConditionedTooLong`] naming both counts, so the operator can
/// see that the file they can measure is within the ceiling and the padding is
/// what crossed it, and [`AudioFault::FrameCountOverflow`] when the conditioned
/// length exceeds what the artifact record's `u32` can carry.
fn check_segment_ceiling(frames: u32, conditioned: usize) -> Result<u32, AudioFault> {
    let conditioned_frames =
        u32::try_from(conditioned).map_err(|_| AudioFault::FrameCountOverflow)?;
    let max_frames = max_segment_frames();
    if conditioned_frames > max_frames {
        return Err(AudioFault::ConditionedTooLong {
            frames,
            conditioned_frames,
            max_frames,
            max_milliseconds: MAX_SEGMENT_AUDIO_MS,
        });
    }
    Ok(conditioned_frames)
}

/// Validates one WAV, reporting *which* property failed and leaving the path
/// and the remedy to the caller, which is the only one that knows whether the
/// file is published or staged.
fn validate_wav(path: &Path) -> Result<Vec<f32>, AudioFault> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != CANONICAL_CHANNELS
        || spec.sample_rate != CANONICAL_SAMPLE_RATE
        || spec.bits_per_sample != CANONICAL_BITS_PER_SAMPLE
        || spec.sample_format != hound::SampleFormat::Float
    {
        return Err(AudioFault::NonCanonical {
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            bits_per_sample: spec.bits_per_sample,
            sample_format: match spec.sample_format {
                hound::SampleFormat::Float => "float",
                hound::SampleFormat::Int => "integer",
            },
            required_channels: CANONICAL_CHANNELS,
            required_sample_rate: CANONICAL_SAMPLE_RATE,
            required_bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
        });
    }

    // The artifact and manifest carry a `u32` frame count, so each sample's
    // index is narrowed before use. The segment ceiling is checked before the
    // sample enters the retained buffer, keeping validation memory bounded by
    // the same limit the cache publishes.
    let mut samples = Vec::new();
    let max_frames = max_segment_frames();
    for sample in reader.samples::<f32>() {
        let sample = sample?;
        let index = u32::try_from(samples.len()).map_err(|_| AudioFault::FrameCountOverflow)?;
        if !sample.is_finite() || sample.abs() > 1.0 {
            return Err(AudioFault::OutOfRangeSample {
                index,
                value: sample,
            });
        }
        if index >= max_frames {
            return Err(AudioFault::TooLong {
                frames: index.saturating_add(1),
                max_frames,
                max_milliseconds: MAX_SEGMENT_AUDIO_MS,
            });
        }
        samples.push(sample);
    }
    if samples.is_empty() {
        return Err(AudioFault::Empty);
    }
    Ok(samples)
}

fn validated_frames(samples: &[f32]) -> u32 {
    u32::try_from(samples.len()).unwrap_or(u32::MAX)
}

/// The most frames one segment's canonical audio may carry.
///
/// `MAX_SEGMENT_AUDIO_MS` at the canonical rate, multiplied at `u64` width so
/// the product cannot wrap before it is narrowed. Shared by the two places the
/// ceiling applies — what the worker wrote, and what conditioning left behind —
/// so one rule cannot drift into two.
fn max_segment_frames() -> u32 {
    u32::try_from(u64::from(MAX_SEGMENT_AUDIO_MS) * u64::from(CANONICAL_SAMPLE_RATE) / 1_000)
        .unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    use crate::{BackendError, SynthesisReport, durable::OsDurableFileSystem};

    /// One named change to a reported synthesis input, for the identity gate.
    type ContextDrift = (&'static str, fn(&mut SynthesisContext));

    /// One rejection path: its label, the error it produced, the entry
    /// directory it must name, and a predicate for the fault it must report.
    type RejectionPath = (
        &'static str,
        BuildError,
        PathBuf,
        fn(&CacheEntryFault) -> bool,
    );

    /// A well-formed key that still reads as the label the test chose.
    ///
    /// Right-padded rather than written out, so the shard the entry lands in
    /// stays visible in the label. `CacheKey` accepts nothing shorter, which is
    /// the whole point of the type.
    fn key(label: &str) -> CacheKey {
        format!("{label:0<width$}", width = CacheKey::LENGTH)
            .parse()
            .expect("test label pads to a well-formed key")
    }

    fn planned(cache_key: &str) -> PlannedSegment {
        PlannedSegment {
            id: "seg-0001".to_owned(),
            speaker: "nadia".to_owned(),
            voice_profile: "nadia-v1".to_owned(),
            display_text: "Same speech.".to_owned(),
            spoken_text: "Same speech.".to_owned(),
            style: study_tts_core::DeliveryStyle::Calm,
            pause_after_ms: 75,
            take: study_tts_core::BASE_TAKE,
            cache_key: key(cache_key),
            synthesis_base_key: key(cache_key),
            audio_blake3: None,
        }
    }

    /// The inputs every producer in this module reports having used.
    fn producer_context() -> SynthesisContext {
        SynthesisContext {
            worker_bundle_hash: "1".repeat(64).parse().expect("a digest of ones parses"),
            model_repository: "study-tts/test-backend".to_owned(),
            model_revision: "v1".parse().expect("`v1` is a revision"),
            model_artifacts_hash: "4".repeat(64).parse().expect("a digest of fours parses"),
            tokenizer_revision: "none".parse().expect("`none` is a revision"),
            language: "en".parse().expect("`en` is a well-formed language tag"),
            determinism_class: DeterminismClass::Reproducible,
            seed: 0,
            generation_parameters: BTreeMap::new(),
            // The speaker this module's `planned` segment names, mapped to the
            // artifact `producer_report` reports reading. The two halves of a
            // report must agree about the conditioning artifact, so a helper
            // pair that disagreed would be building a report no worker may
            // produce — see
            // the test named for a report that contradicts itself.
            voice_conditioning_hashes: BTreeMap::from([(
                "nadia".to_owned(),
                producer_conditioning(),
            )]),
        }
    }

    /// The conditioning artifact this module's producer reports reading.
    fn producer_conditioning() -> VoiceConditioningHash {
        blake3::hash(b"voice").into()
    }

    /// What a producer that used [`producer_context`] reports.
    fn producer_report(frames: u32, sample_rate: u32) -> SynthesisReport {
        SynthesisReport {
            sample_rate,
            channels: CANONICAL_CHANNELS,
            frames,
            backend_revision: "test-backend-v1".to_owned(),
            context: producer_context(),
            voice_conditioning_hash: producer_conditioning(),
            voice_profile: "nadia-v1".to_owned(),
        }
    }

    /// A planned segment whose key [`producer_context`] actually derives.
    ///
    /// Derived rather than labelled, because publication recomputes the key
    /// from what the producer reports and refuses a mismatch. A hand-written
    /// key would be refused by that gate — which is what the gate is for, and
    /// what `t1_e1_audio_reported_under_other_identities_is_not_published`
    /// proves separately.
    fn synthesized_segment() -> PlannedSegment {
        let mut segment = planned("abcdef");
        segment.cache_key = producer_context().key_for(&segment);
        segment
    }

    /// A segment of genuinely different speech, and therefore of a genuinely
    /// different identity.
    ///
    /// Used where a test needs two keys that disagree. Deriving the key from
    /// changed speech rather than writing a different one keeps the segment
    /// self-consistent, so the mismatch under test is the one the test names.
    fn other_speech_segment() -> PlannedSegment {
        let mut segment = synthesized_segment();
        segment.spoken_text = "Different speech.".to_owned();
        segment.cache_key = producer_context().key_for(&segment);
        segment
    }

    fn write_tone(path: &Path, frames: u32, sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: CANONICAL_CHANNELS,
            sample_rate,
            bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create test WAV");
        for _ in 0..frames {
            writer.write_sample(0.25_f32).expect("write test sample");
        }
        writer.finalize().expect("finalize test WAV");
    }

    /// Publishes a valid entry, then hands back the pieces needed to corrupt
    /// it.
    ///
    /// `label` names the directory only. The key the entry is filed under is
    /// the one [`producer_context`] derives for [`synthesized_segment`],
    /// because an honest entry is one whose recorded provenance recomputes the
    /// key beside it — a hand-written key would be refused by the check this
    /// helper exists to exercise the other paths of.
    fn published_entry(root: &Path, label: &str) -> (PathBuf, PathBuf, PathBuf) {
        published_entry_for(root, label, &producer_context(), &synthesized_segment())
    }

    /// Publishes an entry whose recorded provenance reports `context`.
    ///
    /// [`published_entry`] is this with the module's own producer context. A
    /// test needing a different recorded input — one generation parameter,
    /// say — derives `segment`'s key from the same `context`, because an
    /// honest entry is one whose provenance recomputes the key beside it.
    fn published_entry_for(
        root: &Path,
        label: &str,
        context: &SynthesisContext,
        segment: &PlannedSegment,
    ) -> (PathBuf, PathBuf, PathBuf) {
        // Any unique directory will do: `load_validated` is handed its paths,
        // so nothing here depends on where the real layout puts an entry.
        let dir = root.join(label);
        fs::create_dir_all(&dir).expect("create entry directory");
        let audio = dir.join("audio.wav");
        let artifact = dir.join("artifact.json");
        write_tone(&audio, 2_400, CANONICAL_SAMPLE_RATE);
        // Conditioned as publication conditions it. A published entry is
        // conditioned audio by definition, so a fixture that skipped this would
        // be a file the cache could never have written — and the edge check on
        // reuse would refuse it.
        let samples = validate_wav(&audio).expect("validate the fixture's audio");
        let staged =
            condition_staged_audio(&audio, samples).expect("condition the fixture's edges");
        let record = CacheArtifact {
            schema_version: CACHE_SCHEMA_VERSION.to_owned(),
            cache_key: segment.cache_key.clone(),
            audio_blake3: hash_file(&audio).expect("hash test audio"),
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: CANONICAL_CHANNELS,
            sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
            frames: staged.frames,
            // What the conditioner reported for this fixture's audio, under
            // the threshold it applied. A hand-written value here would be a
            // record the cache could never have published.
            edge_conditioning: RecordedConditioning::new(
                staged.conditioning,
                SilenceThreshold::provisional(),
            ),
            provenance: ArtifactProvenance {
                worker_bundle_hash: context.worker_bundle_hash.clone(),
                model_repository: context.model_repository.clone(),
                model_revision: context.model_revision.clone(),
                tokenizer_revision: context.tokenizer_revision.clone(),
                model_artifacts_hash: context.model_artifacts_hash.clone(),
                language: context.language.clone(),
                determinism_class: context.determinism_class,
                seed: context.seed,
                generation_parameters: context.generation_parameters.clone(),
                // From the context, as publication builds it: an entry whose
                // recorded provenance omitted an input the context carries
                // would not recompute the key filed beside it.
                voice_conditioning_hash: context.voice_conditioning_for(&segment.speaker).cloned(),
                voice_profile: "nadia-v1".to_owned(),
                backend_revision: "test-backend-v1".to_owned(),
            },
        };
        write_json_atomically(&OsDurableFileSystem, &artifact, &record)
            .expect("write test artifact");
        (dir, audio, artifact)
    }

    /// Rewrites one field of a published artifact, leaving the audio it
    /// describes untouched.
    fn overwrite_field(artifact: &Path, field: &str, value: serde_json::Value) {
        let mut record = read_artifact(artifact);
        record[field] = value;
        write_artifact(artifact, &record);
    }

    /// Rewrites one recorded conditioning count or its calibration.
    ///
    /// The tampering nothing else can detect: the ramp is not recoverable from
    /// the audio, so a record claiming conditioning ADR-0001 never permits is
    /// caught by the declared value or not at all.
    fn overwrite_conditioning(artifact: &Path, field: &str, value: serde_json::Value) {
        let mut record = read_artifact(artifact);
        record["edge_conditioning"][field] = value;
        write_artifact(artifact, &record);
    }

    /// Republishes `audio` as `samples`, re-pinning the record to match.
    ///
    /// Everything a self-consistent entry needs stays true: the frame count and
    /// the digest are recomputed, so every check that exists today still passes
    /// and only the property under test differs.
    fn republish_audio(artifact: &Path, audio: &Path, samples: &[f32]) {
        write_canonical_samples(audio, samples).expect("write replacement audio");
        overwrite_field(
            artifact,
            "frames",
            json!(u32::try_from(samples.len()).expect("a test fixture fits a u32")),
        );
        overwrite_field(
            artifact,
            "audio_blake3",
            json!(hash_file(audio).expect("hash replacement audio")),
        );
    }

    /// Rewrites one recorded provenance input, leaving the key beside it and
    /// the audio it describes untouched.
    ///
    /// The tampering the key alone cannot detect: both copies of the key still
    /// agree, and only recomputing the key from what is recorded here shows
    /// that the record no longer describes this audio.
    fn overwrite_provenance(artifact: &Path, field: &str, value: serde_json::Value) {
        let mut record = read_artifact(artifact);
        record["provenance"][field] = value;
        write_artifact(artifact, &record);
    }

    fn read_artifact(artifact: &Path) -> serde_json::Value {
        serde_json::from_slice(&fs::read(artifact).expect("read artifact")).expect("parse")
    }

    fn write_artifact(artifact: &Path, record: &serde_json::Value) {
        fs::write(
            artifact,
            serde_json::to_vec_pretty(record).expect("serialize"),
        )
        .expect("write artifact");
    }

    /// Yields `payload` in small chunks, reporting `Interrupted` before every
    /// one, which is how a signal arriving mid-read looks to the caller.
    ///
    /// Chunks are deliberately smaller than `HASH_BUFFER_BYTES` so a short read
    /// is exercised too: a stream may hand back less than the buffer holds
    /// without being at its end.
    struct InterruptingReader<'a> {
        payload: &'a [u8],
        chunk: usize,
        interrupt_next: bool,
        interruptions: usize,
    }

    #[derive(Debug, Default)]
    struct CountingProducer {
        count: AtomicUsize,
    }

    impl StagedAudioProducer for CountingProducer {
        fn produce(&mut self, destination: &Path) -> Result<SynthesisReport, BackendError> {
            write_tone(destination, 2_400, CANONICAL_SAMPLE_RATE);
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(producer_report(2_400, CANONICAL_SAMPLE_RATE))
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum RenameFault {
        Before,
        After,
    }

    #[derive(Debug)]
    struct FaultingRenameFileSystem {
        inner: OsDurableFileSystem,
        fault: RenameFault,
        triggered: AtomicBool,
    }

    impl FaultingRenameFileSystem {
        fn new(fault: RenameFault) -> Self {
            Self {
                inner: OsDurableFileSystem,
                fault,
                triggered: AtomicBool::new(false),
            }
        }
    }

    impl DurableFileSystem for FaultingRenameFileSystem {
        fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_file(path)
        }

        fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_directory(path)
        }

        fn rename_noreplace(
            &self,
            staged: &Path,
            destination: &Path,
        ) -> Result<RenameOutcome, BuildError> {
            if self.triggered.swap(true, Ordering::SeqCst) {
                return self.inner.rename_noreplace(staged, destination);
            }
            match self.fault {
                RenameFault::Before => Err(io_error(
                    destination,
                    io::Error::other("injected interruption before cache rename"),
                )),
                RenameFault::After => {
                    self.inner.rename_noreplace(staged, destination)?;
                    Err(io_error(
                        destination,
                        io::Error::other("injected interruption after cache rename"),
                    ))
                }
            }
        }

        fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError> {
            self.inner.replace_file(staged, destination)
        }
    }

    #[derive(Debug, Default)]
    struct NonCanonicalProducer;

    impl StagedAudioProducer for NonCanonicalProducer {
        fn produce(&mut self, destination: &Path) -> Result<SynthesisReport, BackendError> {
            write_tone(destination, 2_400, 48_000);
            Ok(producer_report(2_400, 48_000))
        }
    }

    #[derive(Debug, Default)]
    struct FailingQuarantineFileSystem {
        inner: OsDurableFileSystem,
    }

    impl DurableFileSystem for FailingQuarantineFileSystem {
        fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_file(path)
        }

        fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_directory(path)
        }

        fn rename_noreplace(
            &self,
            staged: &Path,
            destination: &Path,
        ) -> Result<RenameOutcome, BuildError> {
            if destination
                .file_name()
                .is_some_and(|name| name == "cache-entry")
            {
                return Err(io_error(
                    destination,
                    io::Error::other("injected quarantine failure"),
                ));
            }
            self.inner.rename_noreplace(staged, destination)
        }

        fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError> {
            self.inner.replace_file(staged, destination)
        }
    }

    fn crash_test_roots(root: &Path) -> (PathBuf, PathBuf) {
        let cache = root.join("cache");
        let quarantine = root.join("quarantine");
        fs::create_dir(&cache).expect("create cache root");
        fs::create_dir(&quarantine).expect("create quarantine root");
        (
            fs::canonicalize(cache).expect("canonical cache root"),
            fs::canonicalize(quarantine).expect("canonical quarantine root"),
        )
    }

    impl Read for InterruptingReader<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.interrupt_next {
                self.interrupt_next = false;
                self.interruptions += 1;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            self.interrupt_next = true;
            let taken = self.payload.len().min(buffer.len()).min(self.chunk);
            buffer[..taken].copy_from_slice(&self.payload[..taken]);
            self.payload = &self.payload[taken..];
            Ok(taken)
        }
    }

    #[test]
    fn t1_e0_hashing_retries_reads_the_operating_system_interrupts() {
        let payload: Vec<u8> = (0..HASH_BUFFER_BYTES * 2 + 7)
            .map(|index| (index % 251) as u8)
            .collect();
        let mut source = InterruptingReader {
            payload: &payload,
            chunk: 1_000,
            interrupt_next: true,
            interruptions: 0,
        };

        // An interrupted read consumed nothing, so retrying it must produce the
        // same digest as a stream that was never interrupted. Before the retry,
        // this failed outright.
        let digest = hash_stream(&mut source).expect("an interrupted read must be retried");

        assert_eq!(digest, blake3::hash(&payload).to_hex().to_string());
        assert!(
            source.interruptions > 2,
            "the reader interrupted only {} times, so the retry was barely exercised",
            source.interruptions
        );
    }

    #[test]
    fn t1_e0_hashing_still_reports_reads_that_genuinely_fail() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            }
        }

        // The retry must be narrow: a real IO failure still has to surface, or
        // a checksum would be computed over a file never fully read.
        let error = hash_stream(&mut FailingReader).expect_err("a real failure must surface");

        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn t1_e0_hashing_a_file_does_not_depend_on_the_read_buffer() {
        let workspace = TempDir::new().expect("create cache workspace");

        // The sizes that a chunked read gets wrong: nothing to read, a single
        // partial buffer, an exact multiple of the buffer, and a multiple with
        // a short final read. A loop that dropped the last partial read or
        // rehashed a boundary would still produce *a* digest, and every
        // recorded checksum in every cache entry and manifest would silently
        // stop matching its file.
        for length in [
            0,
            1,
            HASH_BUFFER_BYTES - 1,
            HASH_BUFFER_BYTES,
            HASH_BUFFER_BYTES + 1,
            HASH_BUFFER_BYTES * 2,
            HASH_BUFFER_BYTES * 2 + 7,
        ] {
            // Position-dependent bytes, so a buffer reused without truncating
            // to the filled length changes the digest rather than repeating a
            // value that happens to match.
            let contents: Vec<u8> = (0..length).map(|index| (index % 251) as u8).collect();
            let path = workspace.path().join(format!("{length}.bin"));
            fs::write(&path, &contents).expect("write hash fixture");

            assert_eq!(
                hash_file(&path).expect("hash fixture"),
                blake3::hash(&contents).to_hex().to_string(),
                "streaming digest differs from the whole-file digest at {length} bytes"
            );
        }
    }

    #[test]
    fn t1_e0_entry_dir_is_sharded_by_key_prefix() {
        let cache_key = key("abcdef");

        assert_eq!(
            entry_path_elements(&cache_key),
            [SEGMENTS_DIRECTORY, "ab", cache_key.as_str()]
        );
    }

    #[test]
    fn t4_e0_interruption_before_cache_rename_exposes_no_entry() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();
        let mut producer = CountingProducer::default();

        resolve(
            &FaultingRenameFileSystem::new(RenameFault::Before),
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect_err("interruption before rename must fail publication");

        let (_, entry) = resolve_entry_location(&cache_root, &segment.cache_key)
            .expect("resolve cache destination");
        assert!(!entry.exists());
        assert_eq!(producer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t4_e0_interruption_after_cache_rename_reconciles_without_resynthesis() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();
        let mut producer = CountingProducer::default();

        resolve(
            &FaultingRenameFileSystem::new(RenameFault::After),
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect_err("interruption after rename must surface");

        let recovered = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect("published entry must reconcile as a hit");
        assert!(recovered.audio_path.is_file());
        assert_eq!(producer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t4_e0_invalid_audio_error_names_its_quarantine_artifact() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());

        let mut producer = NonCanonicalProducer;
        let error = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &planned("abcdef"),
            &mut producer,
        )
        .expect_err("non-canonical audio must be quarantined");

        let BuildError::Audio(AudioError::UnusableAudio { path, .. }) = error else {
            panic!("invalid audio produced the wrong error: {error}");
        };
        assert!(path.starts_with(&quarantine_root));
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(AUDIO_RECORD)
        );
        assert!(path.is_file());
    }

    #[test]
    fn t4_e0_quarantine_failure_preserves_primary_and_cleanup_errors() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());

        let mut producer = NonCanonicalProducer;
        let error = resolve(
            &FailingQuarantineFileSystem::default(),
            &cache_root,
            &quarantine_root,
            "job",
            &planned("abcdef"),
            &mut producer,
        )
        .expect_err("quarantine failure must preserve both errors");

        let BuildError::DurableState(state) = &error else {
            panic!("quarantine failure produced the wrong error: {error}");
        };
        let crate::DurableStateError::QuarantineFailed {
            primary, cleanup, ..
        } = &**state
        else {
            panic!("quarantine failure produced the wrong state error: {state}");
        };
        assert!(matches!(
            **primary,
            BuildError::Audio(AudioError::UnusableAudio { .. })
        ));
        assert!(cleanup.to_string().contains("injected quarantine failure"));
        assert!(error.to_string().contains("not usable lesson audio"));
        assert!(error.to_string().contains("injected quarantine failure"));
    }

    #[test]
    fn t4_e2_published_entry_quarantine_failure_preserves_the_invalid_entry() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();
        let mut producer = CountingProducer::default();
        let published = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect("publish a valid entry");
        fs::write(&published.audio_path, b"invalid wav").expect("invalidate published audio");

        let error = resolve(
            &FailingQuarantineFileSystem::default(),
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect_err("a failed quarantine must stop before replacement synthesis");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, crate::DurableStateError::QuarantineFailed { .. })
        ));
        assert_eq!(
            fs::read(&published.audio_path).expect("invalid entry remains"),
            b"invalid wav"
        );
        assert_eq!(producer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t1_e1_audio_reported_under_other_identities_is_not_published() {
        // The gate that makes a content-addressed cache honest: a producer that
        // synthesized under different inputs must not have its audio filed
        // under a key describing the inputs that were asked for, because every
        // later reuse of that entry would be silently wrong.
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();

        // One changed input per run, so the gate is shown to depend on the
        // whole identity rather than on whichever field happens to be checked.
        let drifts: [ContextDrift; 4] = [
            ("worker_bundle_hash", |context| {
                context.worker_bundle_hash =
                    "9".repeat(64).parse().expect("a digest of nines parses");
            }),
            ("model_revision", |context| {
                context.model_revision = "v2".parse().expect("`v2` is a revision");
            }),
            ("language", |context| {
                context.language = "de".parse().expect("`de` is a well-formed tag");
            }),
            ("voice_conditioning_hashes", |context| {
                context.voice_conditioning_hashes.insert(
                    "nadia".to_owned(),
                    "7".repeat(64).parse().expect("a digest of sevens parses"),
                );
            }),
        ];

        for (field, drift) in drifts {
            let mut drifted = producer_context();
            drift(&mut drifted);
            let mut producer = |destination: &Path| {
                write_tone(destination, 2_400, CANONICAL_SAMPLE_RATE);
                Ok(SynthesisReport {
                    context: drifted.clone(),
                    // Kept consistent with the drifted context, because a
                    // worker that drifted reports one artifact, not two. A
                    // report contradicting itself is a different failure with
                    // its own gate and its own test, and leaving this half
                    // undrifted would test that one instead of this one.
                    voice_conditioning_hash: drifted
                        .voice_conditioning_for(&segment.speaker)
                        .cloned()
                        .unwrap_or_else(producer_conditioning),
                    ..producer_report(2_400, CANONICAL_SAMPLE_RATE)
                })
            };

            let error = resolve(
                &OsDurableFileSystem,
                &cache_root,
                &quarantine_root,
                "job",
                &segment,
                &mut producer,
            )
            .expect_err("a drifting report must not publish");

            assert!(
                matches!(
                    error,
                    BuildError::Audio(AudioError::SynthesizerIdentityMismatch { .. })
                ),
                "a report drifting in `{field}` must name the identity mismatch, got {error:?}"
            );
            // The audio the worker wrote is retained in quarantine and no entry
            // exists, so a rerun re-synthesizes rather than reusing a lie.
            let (_, entry) = resolve_entry_location(&cache_root, &segment.cache_key)
                .expect("resolve cache destination");
            assert!(
                !entry.exists(),
                "a report drifting in `{field}` must leave no published entry"
            );
        }

        // The undrifted producer still publishes, so the gate refuses drift
        // rather than refusing everything.
        let mut honest = CountingProducer::default();
        resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut honest,
        )
        .expect("a report matching the plan must publish");
    }

    #[test]
    fn t1_e1_a_report_whose_conditioning_identity_contradicts_itself_is_refused() {
        // The worker reports the conditioning artifact twice and only the copy
        // inside `context` reaches the synthesis key. A report that names one
        // artifact in `voice_conditioning_hash` and another in `context` would
        // therefore satisfy the identity gate — both sides of that comparison
        // come from the same half of the report — while publishing provenance
        // built from the half the other half contradicts. Nothing downstream
        // could then say which voice produced the audio.
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();

        let mut producer = |destination: &Path| {
            write_tone(destination, 2_400, CANONICAL_SAMPLE_RATE);
            Ok(SynthesisReport {
                // The context is left honest, so the key still derives and the
                // identity gate below has nothing to catch. Only the worker's
                // own account of what it read is changed.
                voice_conditioning_hash: "7".repeat(64).parse().expect("a digest of sevens parses"),
                ..producer_report(2_400, CANONICAL_SAMPLE_RATE)
            })
        };

        let error = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect_err("a self-contradicting report must not publish");

        assert!(
            matches!(
                error,
                BuildError::Audio(AudioError::ConditioningIdentityContradiction(_))
            ),
            "a report contradicting itself must name that, got {error:?}"
        );
        let (_, entry) = resolve_entry_location(&cache_root, &segment.cache_key)
            .expect("resolve cache destination");
        assert!(
            !entry.exists(),
            "a self-contradicting report must leave no published entry"
        );
    }

    #[test]
    fn t1_e0_valid_entry_loads() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (dir, audio, artifact) = published_entry(workspace.path(), "abcdef");

        let cached = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
            .expect("entry should load");

        // 2,400 written frames plus the 10 ms of zero padding conditioning
        // adds at each exposed edge, which is 240 frames at the canonical rate.
        assert_eq!(cached.frames, 2_400 + 240 * 2);
        assert_eq!(cached.segment_id, "seg-0001");
    }

    #[test]
    fn t1_e0_every_rejection_names_the_entry_directory_and_the_remedy() {
        let workspace = TempDir::new().expect("create cache workspace");

        // Path 0: missing artifact.
        let (dir0, audio0, artifact0) = published_entry(workspace.path(), "aa0000");
        fs::remove_file(&artifact0).expect("remove artifact");
        let unreadable_artifact =
            load_validated(&synthesized_segment(), &dir0, &audio0, &artifact0)
                .expect_err("missing artifact must be rejected");

        // Path 1: unparseable artifact.
        let (dir, audio, artifact) = published_entry(workspace.path(), "aa1111");
        fs::write(&artifact, b"{ not json").expect("corrupt artifact");
        let unparseable = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
            .expect_err("unparseable artifact must be rejected");

        // Path 2: a recorded digest that is not a digest.
        let (dir6, audio6, artifact6) = published_entry(workspace.path(), "aa2222");
        overwrite_field(&artifact6, "audio_blake3", json!("not-a-digest"));
        let malformed_digest = load_validated(&synthesized_segment(), &dir6, &audio6, &artifact6)
            .expect_err("a malformed recorded digest must be rejected");

        // Path 3: incompatible declared metadata.
        let (dir2, audio2, artifact2) = published_entry(workspace.path(), "bb2222");
        overwrite_field(&artifact2, "schema_version", json!("future"));
        let incompatible = load_validated(&synthesized_segment(), &dir2, &audio2, &artifact2)
            .expect_err("incompatible metadata must be rejected");

        // Path 4: cache-key mismatch.
        let (dir3, audio3, artifact3) = published_entry(workspace.path(), "cc3333");
        let mismatched = load_validated(&other_speech_segment(), &dir3, &audio3, &artifact3)
            .expect_err("cache-key mismatch must be rejected");

        // Path 5: audio that no longer matches its record.
        let (dir4, audio4, artifact4) = published_entry(workspace.path(), "ee5555");
        write_tone(&audio4, 1_200, CANONICAL_SAMPLE_RATE);
        // Conditioned like any published audio, so what differs from the record
        // is the frame *count* alone. An unconditioned tone would be refused
        // for its edges first, and this path is about the count.
        let samples = validate_wav(&audio4).expect("validate the replacement's audio");
        condition_staged_audio(&audio4, samples).expect("condition the replacement's edges");
        let audio_mismatch = load_validated(&synthesized_segment(), &dir4, &audio4, &artifact4)
            .expect_err("frame mismatch must be rejected");

        // Path 5b: audio that is no longer readable at all.
        let (dir5, audio5, artifact5) = published_entry(workspace.path(), "ff6666");
        fs::write(&audio5, b"not a wav").expect("corrupt audio");
        let unreadable = load_validated(&synthesized_segment(), &dir5, &audio5, &artifact5)
            .expect_err("unreadable audio must be rejected");

        // Path 6: audio carrying no edge silence, from an older or defective
        // producer. Endpoints are exactly zero, so the check that existed
        // before this one accepted it; ADR-0001 §12.6 makes the *silence* check
        // a condition of using an entry, and nothing performed it.
        let (dir7, audio7, artifact7) = published_entry(workspace.path(), "aa7777");
        let mut unsilenced = vec![0.25_f32; 2_880];
        unsilenced[0] = 0.0;
        let last = unsilenced.len() - 1;
        unsilenced[last] = 0.0;
        republish_audio(&artifact7, &audio7, &unsilenced);
        let no_silence = load_validated(&synthesized_segment(), &dir7, &audio7, &artifact7)
            .expect_err("audio without edge silence must be rejected");

        // Path 7: a record declaring a ramp longer than ADR-0001 §13.4 permits.
        // One sample past 5 ms at the canonical rate.
        let (dir8, audio8, artifact8) = published_entry(workspace.path(), "aa8888");
        overwrite_conditioning(&artifact8, "leading_ramp", json!(121));
        let wide_ramp = load_validated(&synthesized_segment(), &dir8, &audio8, &artifact8)
            .expect_err("a ramp beyond the ratified bound must be rejected");

        // Path 8: conditioning calibrated against a threshold this build does
        // not apply. What makes `ADR-0001-D007` expire cleanly once ADR-0003
        // freezes a value.
        let (dir9, audio9, artifact9) = published_entry(workspace.path(), "aa9999");
        overwrite_conditioning(&artifact9, "calibration_source", json!("frozen"));
        let other_calibration = load_validated(&synthesized_segment(), &dir9, &audio9, &artifact9)
            .expect_err("another calibration must be rejected");

        // Each path carries the fault it is supposed to report, so a rejection
        // that reaches the right variant for the wrong reason still fails here.
        let paths: [RejectionPath; 10] = [
            ("unreadable artifact", unreadable_artifact, dir0, |fault| {
                matches!(fault, CacheEntryFault::UnreadableArtifact { .. })
            }),
            ("unparseable artifact", unparseable, dir, |fault| {
                matches!(fault, CacheEntryFault::UnparseableArtifact { .. })
            }),
            (
                "malformed recorded digest",
                malformed_digest,
                dir6,
                |fault| matches!(fault, CacheEntryFault::MalformedRecordedDigest { .. }),
            ),
            ("incompatible metadata", incompatible, dir2, |fault| {
                matches!(fault, CacheEntryFault::IncompatibleArtifact { .. })
            }),
            ("cache-key mismatch", mismatched, dir3, |fault| {
                matches!(fault, CacheEntryFault::CacheKeyMismatch { .. })
            }),
            ("frame mismatch", audio_mismatch, dir4, |fault| {
                matches!(fault, CacheEntryFault::FrameCountMismatch { .. })
            }),
            ("unreadable audio", unreadable, dir5, |fault| {
                matches!(fault, CacheEntryFault::Audio(AudioFault::Unreadable(_)))
            }),
            ("audio without edge silence", no_silence, dir7, |fault| {
                matches!(
                    fault,
                    CacheEntryFault::Audio(AudioFault::InsufficientEdgeSilence { .. })
                )
            }),
            ("a ramp beyond the bound", wide_ramp, dir8, |fault| {
                matches!(
                    fault,
                    CacheEntryFault::ConditioningOutsideRatifiedGeometry { .. }
                )
            }),
            ("another calibration", other_calibration, dir9, |fault| {
                matches!(
                    fault,
                    CacheEntryFault::ConditionedUnderAnotherCalibration { .. }
                )
            }),
        ];

        for (label, error, dir, reports_expected_fault) in paths {
            let BuildError::Cache(CacheError::UnusableCacheEntry {
                entry_dir,
                segment_id,
                fault,
            }) = &error
            else {
                panic!("{label} produced the wrong variant: {error}");
            };
            assert!(
                reports_expected_fault(fault),
                "{label} reported the wrong fault: {fault}"
            );
            assert_eq!(entry_dir, &dir, "{label} named the wrong entry directory");
            assert_eq!(segment_id, "seg-0001", "{label} named the wrong segment");

            let message = error.to_string();
            assert!(
                message.contains(&dir.display().to_string()),
                "{label} did not name the entry directory: `{message}`"
            );
            assert!(
                message.contains("runtime reconciliation"),
                "{label} did not state the remedy: `{message}`"
            );
        }
    }

    #[test]
    fn t1_e1_an_entry_whose_provenance_does_not_derive_its_key_is_refused() {
        // Comparing the recorded key with the plan's proves only that two
        // copies of one value agree. The provenance beside them is the entry's
        // audit record, and an operator or a tampered backup can edit it
        // without touching either key — leaving the entry reusable while its
        // record names a model, a language, or a worker bundle that never
        // produced this audio.
        //
        // One changed input per run, driven off the ADR-0001 §12.5 input list
        // rather than off `ArtifactProvenance`, so an input this check stopped
        // covering would fail here rather than pass silently.
        let workspace = TempDir::new().expect("create cache workspace");
        let edits: [(&str, serde_json::Value); 7] = [
            ("worker_bundle_hash", json!("9".repeat(64))),
            ("model_repository", json!("study-tts/other-backend")),
            ("model_revision", json!("v2")),
            ("tokenizer_revision", json!("v9")),
            ("language", json!("de")),
            ("determinism_class", json!("seeded_nondeterministic")),
            ("seed", json!(7)),
        ];

        for (input, value) in edits {
            let (dir, audio, artifact) = published_entry(workspace.path(), input);
            overwrite_provenance(&artifact, input, value);

            let error = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
                .expect_err("an entry whose provenance was edited must not be reused");

            let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
                panic!("editing `{input}` produced the wrong variant: {error}");
            };
            assert!(
                matches!(**fault, CacheEntryFault::ProvenanceKeyMismatch { .. }),
                "editing `{input}` was not reported as a provenance mismatch: {fault}"
            );
        }

        // Fields ADR-0001 §12.5 does not make synthesis-key inputs must not
        // move the derived key. A check that folded them in would refuse
        // entries a planner could never have produced a matching key for.
        for diagnostic in ["voice_profile", "backend_revision"] {
            let (dir, audio, artifact) = published_entry(workspace.path(), diagnostic);
            overwrite_provenance(&artifact, diagnostic, json!("0".repeat(64)));

            load_validated(&synthesized_segment(), &dir, &audio, &artifact).unwrap_or_else(
                |error| {
                    panic!(
                        "editing the diagnostic `{diagnostic}` must not refuse the entry: {error}"
                    )
                },
            );
        }
    }

    /// The audio is left intact in every case here, so a rejection that speaks
    /// of a mismatch would be accusing the wrong file. Uppercase is the trap
    /// worth naming: it is a digest of the right audio, in the wrong spelling.
    /// A record naming one generation parameter twice is refused, not reused.
    ///
    /// The map keeps the last binding, and the key recomputes from what it
    /// kept, so the earlier spelling is dropped before any check can see it:
    /// the entry still derives its own key and is handed back as a hit, while
    /// the record on disk no longer says one thing about what produced the
    /// audio. Path 1 is the only place that is visible, which is why the
    /// refusal is the parse and not a later comparison.
    ///
    /// The edit is textual because `serde_json::Value` cannot hold a name
    /// twice — the shape under test is one it has no way to represent. The
    /// second spelling is the published one, so a reader that keeps the last
    /// binding reproduces this entry exactly: with the derived `BTreeMap` back
    /// on the field, `load_validated` returns the entry and this test fails.
    #[test]
    fn t1_e1_a_cache_record_naming_one_generation_parameter_twice_is_not_reused() {
        let root = TempDir::new().expect("create a cache root");
        let mut context = producer_context();
        context.generation_parameters =
            BTreeMap::from([("temperature".to_owned(), "0.7".to_owned())]);
        let mut segment = planned("abcdef");
        segment.cache_key = context.key_for(&segment);
        let (dir, audio, artifact) =
            published_entry_for(root.path(), "repeated-parameter", &context, &segment);
        let published = fs::read_to_string(&artifact).expect("read the published artifact");
        let edited = published.replace(
            r#""temperature": "0.7""#,
            r#""temperature": "0.1",
        "temperature": "0.7""#,
        );
        assert_ne!(
            edited, published,
            "the edit must reach the recorded parameter"
        );
        fs::write(&artifact, &edited).expect("write the edited artifact");

        let error = load_validated(&segment, &dir, &audio, &artifact)
            .expect_err("an entry recording one parameter twice must not be reused");

        let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
            panic!("a repeated parameter name produced the wrong variant: {error}");
        };
        assert!(
            matches!(**fault, CacheEntryFault::UnparseableArtifact { .. }),
            "the refusal must be the parse, not a later comparison: {error}"
        );
    }

    #[test]
    fn t1_e0_malformed_recorded_digest_is_reported_as_malformed() {
        let workspace = TempDir::new().expect("create cache workspace");

        // Every published entry holds the same tone, so one digest describes
        // all of them and the malformations below are spellings of a digest
        // that would otherwise match.
        let reference = workspace.path().join("reference.wav");
        write_tone(&reference, 2_400, CANONICAL_SAMPLE_RATE);
        let digest = hash_file(&reference).expect("hash reference audio");
        let truncated = digest[..digest.len() - 1].to_owned();
        let malformations = [
            ("aa11", digest.to_uppercase()),
            ("bb22", truncated.clone()),
            ("cc33", format!("{truncated}z")),
            ("dd44", String::new()),
        ];

        for (label, malformed) in malformations {
            let (dir, audio, artifact) = published_entry(workspace.path(), label);
            overwrite_field(&artifact, "audio_blake3", json!(malformed));

            let error = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
                .expect_err("a malformed recorded digest must be rejected");

            let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
                panic!("`{malformed}` produced the wrong variant: {error}");
            };
            assert!(
                matches!(**fault, CacheEntryFault::MalformedRecordedDigest { .. }),
                "`{malformed}` was not reported as malformed: {fault}"
            );
            assert!(
                !matches!(**fault, CacheEntryFault::ChecksumMismatch { .. }),
                "`{malformed}` was reported as a mismatch: {fault}"
            );
        }
    }

    /// The distinction the malformed-recorded-digest test draws, applied to
    /// the other recorded field a key is derived from. Told the key does not
    /// match, an operator goes looking at their audio; what is wrong is a
    /// revision beside it, and the record never had to be trusted far enough
    /// to derive a key from at all.
    #[test]
    fn t1_e1_a_malformed_recorded_revision_is_reported_before_any_key_is_derived() {
        let workspace = TempDir::new().expect("create cache workspace");
        let malformations = [("ee55", ""), ("ff66", "main"), ("aa77", "v1 ")];

        for (label, malformed) in malformations {
            let (dir, audio, artifact) = published_entry(workspace.path(), label);
            overwrite_provenance(&artifact, "model_revision", json!(malformed));

            let error = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
                .expect_err("a malformed recorded revision must be rejected");

            let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
                panic!("`{malformed}` produced the wrong variant: {error}");
            };
            assert!(
                matches!(**fault, CacheEntryFault::UnparseableArtifact { .. }),
                "`{malformed}` was not refused when the record was parsed: {fault}"
            );
            assert!(
                !matches!(**fault, CacheEntryFault::ProvenanceKeyMismatch { .. }),
                "`{malformed}` was reported as a key mismatch: {fault}"
            );
        }
    }

    #[test]
    fn t1_e0_fresh_synthesis_failures_carry_no_delete_remedy() {
        let workspace = TempDir::new().expect("create cache workspace");
        let staged = workspace.path().join("staged.wav");
        write_tone(&staged, 2_400, 48_000);

        let fault = validate_wav(&staged).expect_err("non-canonical WAV must be rejected");

        assert!(
            matches!(fault, AudioFault::NonCanonical { .. }),
            "fault was `{fault}`"
        );
        // Nothing is published yet, so advising a deletion would point at a
        // path that does not exist. `AudioFault` carries no remedy at all, and
        // the caller that knows whether the file is published is the one that
        // attaches one — `load_validated` does, `resolve` does not.
        let error = BuildError::from(AudioError::UnusableAudio {
            path: staged.clone(),
            fault,
        });
        assert!(!error.to_string().contains("delete"), "error was `{error}`");
        assert!(
            error.to_string().contains(&staged.display().to_string()),
            "error did not name the staged file: `{error}`"
        );
    }

    /// A published entry records what conditioning did, not merely that it ran.
    ///
    /// ADR-0001 §11.1 and §13.4 both require the padding and ramp sample counts
    /// to be recorded. The expectation is read from the conditioner rather than
    /// written out, because the counts depend on the fixture's audio; what this
    /// pins is that the *recorded* value is the one conditioning reported,
    /// which a stub or a default would not be.
    #[test]
    fn t1_e1_a_published_entry_records_what_conditioning_did() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = synthesized_segment();
        let mut producer = CountingProducer::default();

        // Through `resolve`, not through the test fixture: what is under test
        // is the record *publication* writes, and a fixture that assembled the
        // record itself would agree with itself whatever publication did.
        let published = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &mut producer,
        )
        .expect("publish one entry");

        let recorded: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(published.entry_dir().join(ARTIFACT_RECORD))
                .expect("read the published artifact"),
        )
        .expect("the published artifact parses");
        let conditioning = &recorded["edge_conditioning"];

        // What the conditioner reports for this fixture's audio: a constant
        // 0.25 tone, so neither edge is silent and both take the full padding.
        let mut samples = vec![0.25_f32; 2_400];
        let expected = condition_edges(
            &mut samples,
            CANONICAL_SAMPLE_RATE,
            SilenceThreshold::provisional(),
        );

        assert_eq!(conditioning["leading_padding"], expected.leading_padding);
        assert_eq!(conditioning["trailing_padding"], expected.trailing_padding);
        assert_eq!(conditioning["leading_ramp"], expected.leading_ramp);
        assert_eq!(conditioning["trailing_ramp"], expected.trailing_ramp);
        // The calibration the counts were produced under. Without it an entry
        // conditioned against the provisional threshold cannot be told from one
        // conditioned against the value ADR-0003 will freeze.
        assert_eq!(conditioning["calibration_source"], "provisional");
    }

    #[test]
    fn t4_e1_conditioning_metadata_detached_from_audio_is_refused() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (dir, audio, artifact) = published_entry(workspace.path(), "detached-conditioning");
        overwrite_conditioning(&artifact, "leading_ramp", json!(0));

        let error = load_validated(&synthesized_segment(), &dir, &audio, &artifact)
            .expect_err("a ramp count detached from the audio must be rejected");

        let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = error else {
            panic!("detached conditioning produced the wrong error: {error}");
        };
        assert!(
            matches!(
                *fault,
                CacheEntryFault::ConditioningInconsistentWithAudio {
                    field: "leading_ramp",
                    declared: 0,
                    minimum: 120,
                    maximum: 120,
                }
            ),
            "detached conditioning reported the wrong fault: {fault}"
        );
    }

    #[test]
    fn t4_e1_quiet_nonzero_edges_are_conditioned_into_acceptable_audio() {
        const QUIET: f32 = 4.312_751e-6;
        let workspace = TempDir::new().expect("create cache workspace");
        let audio = workspace.path().join("quiet-edges.wav");
        let mut samples = vec![QUIET; 480];
        samples.push(0.25);
        samples.extend(std::iter::repeat_n(QUIET, 480));
        write_canonical_samples(&audio, &samples).expect("write quiet-edge fixture");

        let decoded = validate_wav(&audio).expect("validate quiet nonzero edges");
        let staged =
            condition_staged_audio(&audio, decoded).expect("condition quiet nonzero edges");
        let samples = validate_wav(&audio).expect("accept conditioned quiet edges");

        assert_eq!(staged.frames, samples.len() as u32);
        assert_eq!(staged.conditioning, EdgeConditioning::default());
        assert_eq!(samples.first(), Some(&0.0));
        assert_eq!(samples.last(), Some(&0.0));
        check_edge_silence(&samples).expect("conditioned quiet edges meet the silence requirement");
    }

    /// The ceiling refuses what conditioning pushed past it, with both counts.
    ///
    /// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
    /// gives one segment's audio ten minutes, which is 14,400,000 frames at the
    /// canonical 24 kHz. Written out rather than recomputed from the constants
    /// `max_segment_frames` multiplies, so this reads against the document
    /// rather than agreeing with the arithmetic it is checking.
    #[test]
    fn t1_e1_conditioning_may_not_carry_a_segment_past_the_audio_ceiling() {
        assert_eq!(max_segment_frames(), 14_400_000);

        // Label, what the worker wrote, what conditioning left, and whether it
        // must be refused. The last case is the defect this exists for: an
        // input at the ceiling, padded 240 frames at each exposed edge.
        const CASES: [(&str, u32, u32, bool); 4] = [
            ("well inside", 1_000, 1_480, false),
            ("padded onto the ceiling", 14_399_760, 14_400_000, false),
            ("one frame past it", 14_400_000, 14_400_001, true),
            ("padded past it", 14_400_000, 14_400_480, true),
        ];

        for (label, frames, conditioned, refused) in CASES {
            let outcome = check_segment_ceiling(
                frames,
                usize::try_from(conditioned).expect("a frame count fits a usize"),
            );

            match outcome {
                Ok(count) => {
                    assert!(!refused, "{label}: published a count past the ceiling");
                    assert_eq!(count, conditioned, "{label}");
                }
                Err(AudioFault::ConditionedTooLong {
                    frames: reported,
                    conditioned_frames,
                    max_frames,
                    max_milliseconds,
                }) => {
                    assert!(refused, "{label}: refused a count within the ceiling");
                    // The refusal names the file the operator can measure, not
                    // only the length this build would have written.
                    assert_eq!(reported, frames, "{label}");
                    assert_eq!(conditioned_frames, conditioned, "{label}");
                    assert_eq!(max_frames, 14_400_000, "{label}");
                    assert_eq!(max_milliseconds, MAX_SEGMENT_AUDIO_MS, "{label}");
                }
                Err(other) => panic!("{label}: fault was `{other}`"),
            }
        }
    }

    /// The ceiling bounds what this build publishes, not only what it reads.
    ///
    /// Conditioning adds edge silence, so audio the worker wrote at exactly
    /// the ceiling crosses it. Published, that entry is refused by the reload
    /// on the next line of `synthesize_transaction` and by every run after it,
    /// because no quarantine path reaches an entry already renamed into place.
    ///
    /// T4 rather than T1: proving the wiring needs a file at the real ceiling,
    /// and the real ceiling is 57.6 MB.
    #[test]
    fn t4_e1_at_limit_audio_is_refused_rather_than_conditioned_over_the_ceiling() {
        let workspace = TempDir::new().expect("create cache workspace");
        let staged = workspace.path().join("at-limit.wav");
        // Ten minutes at 24 kHz: the ceiling §Provisional resource ceilings of
        // `docs/architecture/WALKING-SKELETON.md` records. `write_tone` writes
        // a constant 0.25, so neither edge is silent and both take the full
        // 10 ms of padding, 240 frames apiece.
        write_tone(&staged, 14_400_000, CANONICAL_SAMPLE_RATE);

        let samples = validate_wav(&staged).expect("validate the at-limit audio");
        let fault = condition_staged_audio(&staged, samples)
            .expect_err("conditioning past the ceiling must be refused");

        assert!(
            matches!(
                fault,
                AudioFault::ConditionedTooLong {
                    frames: 14_400_000,
                    conditioned_frames: 14_400_480,
                    max_frames: 14_400_000,
                    max_milliseconds: MAX_SEGMENT_AUDIO_MS,
                }
            ),
            "fault was `{fault}`"
        );
        // The refusal precedes the write-back, so the stage still holds exactly
        // what the worker produced. That is what quarantine retains, what a
        // person measures, and why the fault does not blame the worker for a
        // count only this build would have written.
        assert_eq!(
            validated_frames(
                &validate_wav(&staged).expect("the worker's own file is within the ceiling")
            ),
            14_400_000,
            "conditioning rewrote a file it had already refused"
        );
    }
}
