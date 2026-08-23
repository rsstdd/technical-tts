use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{CANONICAL_SAMPLE_RATE, CacheKey, PlannedSegment, is_blake3_hex};
use tempfile::Builder;

use crate::{AudioFault, BuildError, CacheEntryFault, SegmentSynthesizer, io_error};

const CACHE_SCHEMA_VERSION: &str = "0.1-skeleton";

/// Sample format every cache entry and every assembled master carries.
const CANONICAL_SAMPLE_FORMAT: &str = "f32le";

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
/// Kept below the key length so the prefix slice in `entry_dir` is in bounds,
/// and asserted here rather than trusted: `CacheKey` guarantees the length, and
/// this is where that guarantee stops being a comment and becomes a compile
/// error.
const CACHE_SHARD_WIDTH: usize = 2;
const _: () = assert!(CACHE_SHARD_WIDTH <= CacheKey::LENGTH);

#[derive(Clone, Debug)]
pub(crate) struct CachedSegment {
    pub segment_id: String,
    pub cache_key: CacheKey,
    pub entry_dir: PathBuf,
    pub audio_path: PathBuf,
    pub audio_blake3: String,
    pub frames: u32,
    pub pause_after_ms: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CacheArtifact {
    schema_version: String,
    cache_key: CacheKey,
    audio_blake3: String,
    sample_rate: u32,
    channels: u16,
    sample_format: String,
    frames: u32,
}

/// Directory holding one cache entry. Shared with tests so the sharding scheme
/// is defined once.
///
/// Total for every `CacheKey`: the type guarantees `CacheKey::LENGTH` ASCII
/// characters, so the shard prefix is in bounds and on a character boundary.
/// Taking a `&str` here is what made this a panic reachable from a deserialized
/// plan.
pub(crate) fn entry_dir(cache_root: &Path, cache_key: &CacheKey) -> PathBuf {
    let key = cache_key.as_str();
    cache_root
        .join("segments")
        .join(&key[..CACHE_SHARD_WIDTH])
        .join(key)
}

/// Names the entry a fault was found in.
///
/// Shared with `assembly`, which detects a truncated entry while reading it, so
/// both report the same violated invariant with the same remedy rather than two
/// messages that happen to agree.
pub(crate) fn rejected(entry_dir: &Path, segment_id: &str, fault: CacheEntryFault) -> BuildError {
    BuildError::UnusableCacheEntry {
        entry_dir: entry_dir.to_path_buf(),
        segment_id: segment_id.to_owned(),
        fault: Box::new(fault),
    }
}

pub(crate) fn resolve(
    cache_root: &Path,
    segment: &PlannedSegment,
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<CachedSegment, BuildError> {
    let entry_dir = entry_dir(cache_root, &segment.cache_key);
    let audio_path = entry_dir.join("audio.wav");
    let artifact_path = entry_dir.join("artifact.json");

    // A partial entry is treated as a miss and re-synthesized. E2-S1 replaces
    // this with explicit reconciliation between job state, cache artifacts, and
    // outputs.
    if audio_path.is_file() && artifact_path.is_file() {
        return load_validated(segment, &entry_dir, &audio_path, &artifact_path);
    }

    fs::create_dir_all(&entry_dir).map_err(|error| io_error(&entry_dir, error))?;
    // The temporary file reserves a unique path inside the entry directory; the
    // synthesizer replaces it with a new file at that path rather than writing
    // through the handle. E1-S3 hardens this with an explicit staging root and
    // containment checks.
    let staged = Builder::new()
        .prefix("audio-")
        .suffix(".wav")
        .tempfile_in(&entry_dir)
        .map_err(|error| io_error(&entry_dir, error))?;
    let staged_path = staged.path().to_path_buf();
    let report = synthesizer.synthesize(segment, &staged_path)?;

    // Freshly synthesized output carries no remedy: the staged file is
    // discarded on drop and there is no published entry for the user to delete.
    let frames = validate_wav(&staged_path).map_err(|fault| BuildError::UnusableAudio {
        path: staged_path.clone(),
        fault,
    })?;
    if report.sample_rate != CANONICAL_SAMPLE_RATE
        || report.channels != 1
        || report.frames != frames
    {
        // The WAV itself passed validation, so its shape is canonical by
        // construction; what disagrees is the worker's account of what it
        // wrote.
        return Err(BuildError::SynthesizerReportMismatch {
            segment_id: segment.id.clone(),
            reported_sample_rate: report.sample_rate,
            reported_channels: report.channels,
            reported_frames: report.frames,
            written_sample_rate: CANONICAL_SAMPLE_RATE,
            written_channels: 1,
            written_frames: frames,
        });
    }
    let audio_blake3 = hash_file(&staged_path)?;
    staged
        .persist(&audio_path)
        .map_err(|error| io_error(&audio_path, error.error))?;

    let artifact = CacheArtifact {
        schema_version: CACHE_SCHEMA_VERSION.to_owned(),
        cache_key: segment.cache_key.clone(),
        audio_blake3: audio_blake3.clone(),
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: 1,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
        frames,
    };
    write_json_atomically(&artifact_path, &artifact)?;

    Ok(CachedSegment {
        segment_id: segment.id.clone(),
        cache_key: segment.cache_key.clone(),
        entry_dir,
        audio_path,
        audio_blake3,
        frames,
        pause_after_ms: segment.pause_after_ms,
    })
}

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
        || artifact.channels != 1
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
                required_channels: 1,
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
    let mut hasher = blake3::Hasher::new();
    // Read straight into the hashing buffer rather than through a `BufReader`,
    // which would hold a second buffer of its own to no purpose.
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];

    loop {
        let filled = file
            .read(&mut buffer)
            .map_err(|error| io_error(path, error))?;
        if filled == 0 {
            break;
        }
        hasher.update(&buffer[..filled]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}

/// Validates one WAV, reporting *which* property failed and leaving the path
/// and the remedy to the caller, which is the only one that knows whether the
/// file is published or staged.
fn validate_wav(path: &Path) -> Result<u32, AudioFault> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != CANONICAL_SAMPLE_RATE
        || spec.bits_per_sample != 32
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
            required_sample_rate: CANONICAL_SAMPLE_RATE,
        });
    }

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

pub(crate) fn write_json_atomically<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), BuildError> {
    let parent = path
        .parent()
        .ok_or_else(|| BuildError::UnrootedDestination {
            path: path.to_path_buf(),
        })?;
    let mut staged = Builder::new()
        .prefix("json-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), value).map_err(|error| {
        BuildError::WriteJson {
            path: path.to_path_buf(),
            source: error,
        }
    })?;
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|error| io_error(staged.path(), error))?;
    staged
        .persist(path)
        .map_err(|error| io_error(path, error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

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
            channels: 1,
            sample_rate,
            bits_per_sample: 32,
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
        let dir = entry_dir(root, &key(cache_key));
        fs::create_dir_all(&dir).expect("create entry directory");
        let audio = dir.join("audio.wav");
        let artifact = dir.join("artifact.json");
        write_tone(&audio, 2_400, CANONICAL_SAMPLE_RATE);
        let record = CacheArtifact {
            schema_version: CACHE_SCHEMA_VERSION.to_owned(),
            cache_key: key(cache_key),
            audio_blake3: hash_file(&audio).expect("hash test audio"),
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: 1,
            sample_format: "f32le".to_owned(),
            frames: 2_400,
        };
        write_json_atomically(&artifact, &record).expect("write test artifact");
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
            entry_dir(Path::new("/cache"), &cache_key),
            Path::new("/cache/segments/ab").join(cache_key.as_str())
        );
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
            let BuildError::UnusableCacheEntry {
                entry_dir,
                segment_id,
                fault,
            } = &error
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
                message.contains("delete"),
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

            let BuildError::UnusableCacheEntry { fault, .. } = &error else {
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
        let error = BuildError::UnusableAudio {
            path: staged.clone(),
            fault,
        };
        assert!(!error.to_string().contains("delete"), "error was `{error}`");
        assert!(
            error.to_string().contains(&staged.display().to_string()),
            "error did not name the staged file: `{error}`"
        );
    }
}
