//! The content-addressed synthesis cache: where an entry lives, what it must
//! contain, and when it may be reused.
//!
//! An entry is reused only after its recorded metadata has been validated
//! against this build's canonical audio format and its audio re-hashed to the
//! digest the artifact records. Partial sibling transactions are quarantined;
//! an unreadable or corrupt published entry is a refusal naming the entry and
//! its runtime-reconciliation remedy, never a silent repair that could hide
//! tampering.
//!
//! The sharding layout is stated once, in [`entry_path_elements`], and is
//! private to this crate.

use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE,
    CacheKey, PlannedSegment, is_blake3_hex,
};
use tempfile::Builder;

use crate::{
    AudioError, AudioFault, BuildError, CacheEntryFault, CacheError, SegmentSynthesizer,
    durable::{
        DurableFileSystem, RenameOutcome, publish_directory_noreplace, sync_directory_transaction,
        write_json_atomically,
    },
    io_error, locking, managed,
};

/// Layout version this module accepts for a cache artifact.
///
/// Independent of the lesson and manifest schema versions despite sharing a
/// value today: each versions a different document and moves separately. An
/// artifact declaring anything else is refused rather than read on the guess
/// that the layout did not change.
const CACHE_SCHEMA_VERSION: &str = "0.1-skeleton";

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

/// One segment's audio as the cache holds it, validated and ready to assemble.
///
/// Produced only by [`resolve`], so nothing downstream can name a cache entry
/// that has not passed its checks.
#[derive(Clone, Debug)]
pub(crate) struct CachedSegment {
    /// Identity of the segment within its lesson.
    pub segment_id: String,
    /// The synthesis identity that named this entry.
    pub cache_key: CacheKey,
    /// The entry directory, which a refusal routes to runtime reconciliation.
    pub entry_dir: PathBuf,
    /// The validated audio inside that entry.
    pub audio_path: PathBuf,
    /// Digest of that audio, re-verified before assembly reads it.
    pub audio_blake3: String,
    /// Frames the audio holds, agreeing with the artifact record.
    pub frames: u32,
    /// Silence to write after this segment, in milliseconds.
    pub pause_after_ms: u32,
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
/// staging transaction is quarantined and re-synthesized; a corrupt published
/// entry is refused rather than repaired, because repair would hide tampering.
///
/// # Errors
///
/// [`CacheError::UnusableCacheEntry`] naming the violated invariant when a
/// published entry cannot be trusted, [`AudioError::UnusableAudio`] when fresh
/// synthesis produces audio this build cannot use,
/// [`ManagedPathError::ManagedPathEscape`] when a link occupies an entry path,
/// [`crate::DurableStateError::QuarantineFailed`] when a failed attempt cannot
/// be retained, and [`BuildError::Synthesis`] when the worker itself refuses.
pub(crate) fn resolve(
    filesystem: &dyn DurableFileSystem,
    cache_root: &Path,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<CachedSegment, BuildError> {
    let (_shard, entry_dir) = resolve_entry_location(cache_root, &segment.cache_key)?;
    if entry_dir.is_dir() {
        return load_entry(segment, &entry_dir);
    }

    let _key_lock = locking::acquire_cache_key_lock(cache_root, &segment.cache_key)?;
    let (shard, entry_dir) = resolve_entry_location(cache_root, &segment.cache_key)?;
    if entry_dir.is_dir() {
        return load_entry(segment, &entry_dir);
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
        synthesizer,
    )
}

fn load_entry(segment: &PlannedSegment, entry_dir: &Path) -> Result<CachedSegment, BuildError> {
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
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<CachedSegment, BuildError> {
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

    let report = match synthesizer.synthesize(segment, &audio_path) {
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
    let frames = match validate_wav(&audio_path) {
        Ok(frames) => frames,
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
        || report.frames != frames
    {
        let error = AudioError::SynthesizerReportMismatch {
            segment_id: segment.id.clone(),
            reported_sample_rate: report.sample_rate,
            reported_channels: report.channels,
            reported_frames: report.frames,
            written_sample_rate: CANONICAL_SAMPLE_RATE,
            written_channels: CANONICAL_CHANNELS,
            written_frames: frames,
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

    let audio_blake3 = hash_file(&audio_path)?;
    let artifact = CacheArtifact {
        schema_version: CACHE_SCHEMA_VERSION.to_owned(),
        cache_key: segment.cache_key.clone(),
        audio_blake3,
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
        frames,
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

fn quarantine_transaction(
    filesystem: &dyn DurableFileSystem,
    quarantine_root: &Path,
    job_id: &str,
    segment: &PlannedSegment,
    stage: &Path,
) -> Result<PathBuf, BuildError> {
    let job = managed::subdirectory(quarantine_root, job_id)?;
    let cache = managed::subdirectory(&job, "cache")?;
    let segment_dir = managed::subdirectory(&cache, &segment.id)?;
    let attempt = Builder::new()
        .prefix("attempt-")
        .tempdir_in(&segment_dir)
        .map_err(|error| io_error(&segment_dir, error))?
        .keep();
    let destination = attempt.join("cache-entry");
    if publish_directory_noreplace(filesystem, stage, &destination)? != RenameOutcome::Published {
        return Err(crate::DurableStateError::PublicationConflict { path: destination }.into());
    }
    filesystem.sync_directory(&segment_dir)?;
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
/// this build's, and only then is the audio re-hashed against the digest. A
/// malformed digest reported as a mismatch would tell an operator their file
/// was tampered with when it was merely written wrong.
///
/// # Errors
///
/// [`CacheError::UnusableCacheEntry`] carrying the [`CacheEntryFault`] that
/// names the violated invariant, or [`IoError::FileSystem`] if the artifact
/// cannot be read.
fn load_validated(
    segment: &PlannedSegment,
    entry_dir: &Path,
    audio_path: &Path,
    artifact_path: &Path,
) -> Result<CachedSegment, BuildError> {
    let bytes = fs::read(artifact_path).map_err(|error| io_error(artifact_path, error))?;

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

    // Path 5: the audio itself is unreadable, non-canonical, or does not match
    // the artifact.
    let frames =
        validate_wav(audio_path).map_err(|fault| rejected(entry_dir, &segment.id, fault.into()))?;
    let checksum = hash_file(audio_path)?;
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

    Ok(CachedSegment {
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

/// Validates one WAV, reporting *which* property failed and leaving the path
/// and the remedy to the caller, which is the only one that knows whether the
/// file is published or staged.
fn validate_wav(path: &Path) -> Result<u32, AudioFault> {
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

    // `frames` is the `u32` the artifact record and the manifest carry, so it
    // is counted at that width rather than counted wide and narrowed. No file
    // reaches the ceiling: 4.29e9 f32 frames is about 17 GB of sample data,
    // four times what a WAV data chunk can declare in its 32-bit length. The
    // check is what makes that a refusal rather than a wrap should the
    // canonical format ever move to a container without the cap.
    let mut frames = 0_u32;
    for sample in reader.samples::<f32>() {
        let sample = sample?;
        if !sample.is_finite() || sample.abs() > 1.0 {
            return Err(AudioFault::OutOfRangeSample {
                index: frames,
                value: sample,
            });
        }
        frames = frames
            .checked_add(1)
            .ok_or(AudioFault::FrameCountOverflow)?;
    }
    if frames == 0 {
        return Err(AudioFault::Empty);
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    use crate::{SynthesisError, SynthesisReport, durable::OsDurableFileSystem};

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
            spoken_text: "Same speech.".to_owned(),
            style: "calm".to_owned(),
            pause_after_ms: 75,
            cache_key: key(cache_key),
        }
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
    fn published_entry(root: &Path, cache_key: &str) -> (PathBuf, PathBuf, PathBuf) {
        // Any unique directory will do: `load_validated` is handed its paths,
        // so nothing here depends on where the real layout puts an entry.
        let dir = root.join(cache_key);
        fs::create_dir_all(&dir).expect("create entry directory");
        let audio = dir.join("audio.wav");
        let artifact = dir.join("artifact.json");
        write_tone(&audio, 2_400, CANONICAL_SAMPLE_RATE);
        let record = CacheArtifact {
            schema_version: CACHE_SCHEMA_VERSION.to_owned(),
            cache_key: key(cache_key),
            audio_blake3: hash_file(&audio).expect("hash test audio"),
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: CANONICAL_CHANNELS,
            sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
            frames: 2_400,
        };
        write_json_atomically(&OsDurableFileSystem, &artifact, &record)
            .expect("write test artifact");
        (dir, audio, artifact)
    }

    /// Rewrites one field of a published artifact, leaving the audio it
    /// describes untouched.
    fn overwrite_field(artifact: &Path, field: &str, value: serde_json::Value) {
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(artifact).expect("read artifact")).expect("parse");
        record[field] = value;
        fs::write(
            artifact,
            serde_json::to_vec_pretty(&record).expect("serialize"),
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
    struct CountingSynthesizer {
        count: AtomicUsize,
    }

    impl SegmentSynthesizer for CountingSynthesizer {
        fn identity(&self) -> &str {
            "cache-crash-test-v1"
        }

        fn synthesize(
            &self,
            _segment: &PlannedSegment,
            destination: &Path,
        ) -> Result<SynthesisReport, SynthesisError> {
            write_tone(destination, 2_400, CANONICAL_SAMPLE_RATE);
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(SynthesisReport {
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                frames: 2_400,
            })
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
    struct NonCanonicalSynthesizer;

    impl SegmentSynthesizer for NonCanonicalSynthesizer {
        fn identity(&self) -> &str {
            "non-canonical-test-v1"
        }

        fn synthesize(
            &self,
            _segment: &PlannedSegment,
            destination: &Path,
        ) -> Result<SynthesisReport, SynthesisError> {
            write_tone(destination, 2_400, 48_000);
            Ok(SynthesisReport {
                sample_rate: 48_000,
                channels: CANONICAL_CHANNELS,
                frames: 2_400,
            })
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
        let segment = planned("abcdef");
        let synthesizer = CountingSynthesizer::default();

        resolve(
            &FaultingRenameFileSystem::new(RenameFault::Before),
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &synthesizer,
        )
        .expect_err("interruption before rename must fail publication");

        let (_, entry) = resolve_entry_location(&cache_root, &segment.cache_key)
            .expect("resolve cache destination");
        assert!(!entry.exists());
        assert_eq!(synthesizer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t4_e0_interruption_after_cache_rename_reconciles_without_resynthesis() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());
        let segment = planned("abcdef");
        let synthesizer = CountingSynthesizer::default();

        resolve(
            &FaultingRenameFileSystem::new(RenameFault::After),
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &synthesizer,
        )
        .expect_err("interruption after rename must surface");

        let recovered = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &segment,
            &synthesizer,
        )
        .expect("published entry must reconcile as a hit");
        assert!(recovered.audio_path.is_file());
        assert_eq!(synthesizer.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn t4_e0_invalid_audio_error_names_its_quarantine_artifact() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (cache_root, quarantine_root) = crash_test_roots(workspace.path());

        let error = resolve(
            &OsDurableFileSystem,
            &cache_root,
            &quarantine_root,
            "job",
            &planned("abcdef"),
            &NonCanonicalSynthesizer,
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

        let error = resolve(
            &FailingQuarantineFileSystem::default(),
            &cache_root,
            &quarantine_root,
            "job",
            &planned("abcdef"),
            &NonCanonicalSynthesizer,
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
    fn t1_e0_valid_entry_loads() {
        let workspace = TempDir::new().expect("create cache workspace");
        let (dir, audio, artifact) = published_entry(workspace.path(), "abcdef");

        let cached =
            load_validated(&planned("abcdef"), &dir, &audio, &artifact).expect("entry should load");

        assert_eq!(cached.frames, 2_400);
        assert_eq!(cached.segment_id, "seg-0001");
    }

    #[test]
    fn t1_e0_every_rejection_names_the_entry_directory_and_the_remedy() {
        let workspace = TempDir::new().expect("create cache workspace");

        // Path 1: unparseable artifact.
        let (dir, audio, artifact) = published_entry(workspace.path(), "aa1111");
        fs::write(&artifact, b"{ not json").expect("corrupt artifact");
        let unparseable = load_validated(&planned("aa1111"), &dir, &audio, &artifact)
            .expect_err("unparseable artifact must be rejected");

        // Path 2: a recorded digest that is not a digest.
        let (dir6, audio6, artifact6) = published_entry(workspace.path(), "aa2222");
        overwrite_field(&artifact6, "audio_blake3", json!("not-a-digest"));
        let malformed_digest = load_validated(&planned("aa2222"), &dir6, &audio6, &artifact6)
            .expect_err("a malformed recorded digest must be rejected");

        // Path 3: incompatible declared metadata.
        let (dir2, audio2, artifact2) = published_entry(workspace.path(), "bb2222");
        overwrite_field(&artifact2, "schema_version", json!("future"));
        let incompatible = load_validated(&planned("bb2222"), &dir2, &audio2, &artifact2)
            .expect_err("incompatible metadata must be rejected");

        // Path 4: cache-key mismatch.
        let (dir3, audio3, artifact3) = published_entry(workspace.path(), "cc3333");
        let mismatched = load_validated(&planned("dd4444"), &dir3, &audio3, &artifact3)
            .expect_err("cache-key mismatch must be rejected");

        // Path 5: audio that no longer matches its record.
        let (dir4, audio4, artifact4) = published_entry(workspace.path(), "ee5555");
        write_tone(&audio4, 1_200, CANONICAL_SAMPLE_RATE);
        let audio_mismatch = load_validated(&planned("ee5555"), &dir4, &audio4, &artifact4)
            .expect_err("frame mismatch must be rejected");

        // Path 5b: audio that is no longer readable at all.
        let (dir5, audio5, artifact5) = published_entry(workspace.path(), "ff6666");
        fs::write(&audio5, b"not a wav").expect("corrupt audio");
        let unreadable = load_validated(&planned("ff6666"), &dir5, &audio5, &artifact5)
            .expect_err("unreadable audio must be rejected");

        // Each path carries the fault it is supposed to report, so a rejection
        // that reaches the right variant for the wrong reason still fails here.
        let paths: [RejectionPath; 6] = [
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

    /// The audio is left intact in every case here, so a rejection that speaks
    /// of a mismatch would be accusing the wrong file. Uppercase is the trap
    /// worth naming: it is a digest of the right audio, in the wrong spelling.
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

            let error = load_validated(&planned(label), &dir, &audio, &artifact)
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
}
