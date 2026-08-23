use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{CANONICAL_SAMPLE_RATE, CacheKey, PlannedSegment, is_blake3_hex};
use tempfile::Builder;

use crate::{BuildError, SegmentSynthesizer, audio_error, io_error};

const CACHE_SCHEMA_VERSION: &str = "0.1-skeleton";

/// Characters of the cache key that name the shard directory grouping entries under `segments/`.
///
/// Kept below the key length so the prefix slice in `entry_dir` is in bounds, and asserted here
/// rather than trusted: `CacheKey` guarantees the length, and this is where that guarantee stops
/// being a comment and becomes a compile error.
const CACHE_SHARD_WIDTH: usize = 2;
const _: () = assert!(CACHE_SHARD_WIDTH <= CacheKey::LENGTH);

#[derive(Clone, Debug)]
pub(crate) struct CachedSegment {
    pub segment_id: String,
    pub cache_key: CacheKey,
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

/// Directory holding one cache entry. Shared with tests so the sharding scheme is defined once.
///
/// Total for every `CacheKey`: the type guarantees `CacheKey::LENGTH` ASCII characters, so the
/// shard prefix is in bounds and on a character boundary. Taking a `&str` here is what made this
/// a panic reachable from a deserialized plan.
pub(crate) fn entry_dir(cache_root: &Path, cache_key: &CacheKey) -> PathBuf {
    let key = cache_key.as_str();
    cache_root
        .join("segments")
        .join(&key[..CACHE_SHARD_WIDTH])
        .join(key)
}

/// Builds a rejection that names the entry directory and the remedy.
///
/// E0-S0 has no quarantine, so a poisoned entry would otherwise fail every subsequent build with
/// no stated way out. ADR-0001 §4.2 requires a failure to explain what remains valid and what the
/// safe recovery action is; deleting the entry is that action, because the segment regenerates
/// from the plan on the next build.
fn rejected(entry_dir: &Path, segment_id: &str, detail: impl std::fmt::Display) -> BuildError {
    BuildError::InvalidCache(format!(
        "cache entry for segment `{segment_id}` is unusable: {detail}; delete `{}` to regenerate \
         this segment",
        entry_dir.display()
    ))
}

/// Re-frames an error raised while validating a *cached* artifact so it carries the remedy.
///
/// `validate_wav` is also called on freshly synthesized output, where no cache entry exists yet
/// and "delete the entry" would be wrong advice. The remedy is therefore attached by the caller
/// that knows the entry is on disk, not by the validator.
fn rejected_from(entry_dir: &Path, segment_id: &str, error: BuildError) -> BuildError {
    rejected(entry_dir, segment_id, error)
}

pub(crate) fn resolve(
    cache_root: &Path,
    segment: &PlannedSegment,
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<CachedSegment, BuildError> {
    let entry_dir = entry_dir(cache_root, &segment.cache_key);
    let audio_path = entry_dir.join("audio.wav");
    let artifact_path = entry_dir.join("artifact.json");

    // A partial entry is treated as a miss and re-synthesized. E2-S1 replaces this with explicit
    // reconciliation between job state, cache artifacts, and outputs.
    if audio_path.is_file() && artifact_path.is_file() {
        return load_validated(segment, &entry_dir, &audio_path, &artifact_path);
    }

    fs::create_dir_all(&entry_dir).map_err(|error| io_error(&entry_dir, error))?;
    // The temporary file reserves a unique path inside the entry directory; the synthesizer
    // replaces it with a new file at that path rather than writing through the handle. E1-S3
    // hardens this with an explicit staging root and containment checks.
    let staged = Builder::new()
        .prefix("audio-")
        .suffix(".wav")
        .tempfile_in(&entry_dir)
        .map_err(|error| io_error(&entry_dir, error))?;
    let staged_path = staged.path().to_path_buf();
    let report = synthesizer.synthesize(segment, &staged_path)?;

    // Freshly synthesized output carries no remedy: the staged file is discarded on drop and
    // there is no published entry for the user to delete.
    let frames = validate_wav(&staged_path)?;
    if report.sample_rate != CANONICAL_SAMPLE_RATE
        || report.channels != 1
        || report.frames != frames
    {
        return Err(BuildError::InvalidCache(format!(
            "synthesizer reported {} Hz, {} channels, and {} frames for segment `{}` but wrote a \
             WAV with {CANONICAL_SAMPLE_RATE} Hz, 1 channel, and {frames} frames",
            report.sample_rate, report.channels, report.frames, segment.id
        )));
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
        sample_format: "f32le".to_owned(),
        frames,
    };
    write_json_atomically(&artifact_path, &artifact)?;

    Ok(CachedSegment {
        segment_id: segment.id.clone(),
        cache_key: segment.cache_key.clone(),
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
            format!(
                "`{}` could not be parsed ({error})",
                artifact_path.display()
            ),
        )
    })?;

    // Path 2: the recorded digest is not a digest at all.
    //
    // Checked here rather than at the comparison below, because a malformed record that reaches
    // the comparison is reported as a checksum *mismatch* — telling the operator their audio was
    // tampered with when the artifact was what broke. `VoiceError::MalformedChecksum` draws the
    // same distinction for voice records.
    if !is_blake3_hex(&artifact.audio_blake3) {
        return Err(rejected(
            entry_dir,
            &segment.id,
            format!(
                "artifact records `{}` as the audio digest, which is not a lowercase BLAKE3 hex \
                 digest and so could never match the audio",
                artifact.audio_blake3
            ),
        ));
    }

    // Path 3: the artifact parses but describes audio this build cannot consume.
    if artifact.schema_version != CACHE_SCHEMA_VERSION
        || artifact.sample_rate != CANONICAL_SAMPLE_RATE
        || artifact.channels != 1
        || artifact.sample_format != "f32le"
    {
        return Err(rejected(
            entry_dir,
            &segment.id,
            format!(
                "artifact declares schema `{}`, {} Hz, {} channels, and format `{}` but this build \
                 requires schema `{CACHE_SCHEMA_VERSION}`, {CANONICAL_SAMPLE_RATE} Hz, 1 channel, \
                 and `f32le`",
                artifact.schema_version,
                artifact.sample_rate,
                artifact.channels,
                artifact.sample_format
            ),
        ));
    }

    // Path 4: the entry belongs to a different synthesis identity.
    if artifact.cache_key != segment.cache_key {
        return Err(rejected(
            entry_dir,
            &segment.id,
            format!(
                "artifact records cache key `{}` but the plan requires `{}`",
                artifact.cache_key, segment.cache_key
            ),
        ));
    }

    // Path 5: the audio itself is unreadable, non-canonical, or does not match the artifact.
    let frames =
        validate_wav(audio_path).map_err(|error| rejected_from(entry_dir, &segment.id, error))?;
    let checksum = hash_file(audio_path)?;
    if frames != artifact.frames {
        return Err(rejected(
            entry_dir,
            &segment.id,
            format!(
                "audio holds {frames} frames but the artifact declares {}",
                artifact.frames
            ),
        ));
    }
    if checksum != artifact.audio_blake3 {
        return Err(rejected(
            entry_dir,
            &segment.id,
            "audio checksum does not match the artifact record",
        ));
    }

    Ok(CachedSegment {
        segment_id: segment.id.clone(),
        cache_key: segment.cache_key.clone(),
        audio_path: audio_path.to_path_buf(),
        audio_blake3: checksum,
        frames,
        pause_after_ms: segment.pause_after_ms,
    })
}

pub(crate) fn hash_file(path: &Path) -> Result<String, BuildError> {
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

fn validate_wav(path: &Path) -> Result<u32, BuildError> {
    let mut reader = hound::WavReader::open(path).map_err(|error| audio_error(path, error))?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != CANONICAL_SAMPLE_RATE
        || spec.bits_per_sample != 32
        || spec.sample_format != hound::SampleFormat::Float
    {
        return Err(BuildError::InvalidCache(format!(
            "`{}` is not canonical 24 kHz mono float WAV",
            path.display()
        )));
    }

    let mut frames = 0_u32;
    for sample in reader.samples::<f32>() {
        let sample = sample.map_err(|error| audio_error(path, error))?;
        if !sample.is_finite() || sample.abs() > 1.0 {
            return Err(BuildError::InvalidCache(format!(
                "`{}` contains invalid float PCM",
                path.display()
            )));
        }
        frames = frames
            .checked_add(1)
            .ok_or_else(|| BuildError::InvalidCache("WAV frame count overflow".to_owned()))?;
    }
    if frames == 0 {
        return Err(BuildError::InvalidCache(format!(
            "`{}` contains no audio frames",
            path.display()
        )));
    }
    Ok(frames)
}

pub(crate) fn write_json_atomically<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), BuildError> {
    let parent = path.parent().ok_or_else(|| {
        BuildError::InvalidCache(format!("`{}` has no parent directory", path.display()))
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

    /// A well-formed key that still reads as the label the test chose.
    ///
    /// Right-padded rather than written out, so the shard the entry lands in stays visible in the
    /// label. `CacheKey` accepts nothing shorter, which is the whole point of the type.
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

    /// Publishes a valid entry, then hands back the pieces needed to corrupt it.
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

    /// Rewrites one field of a published artifact, leaving the audio it describes untouched.
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

        for (label, error, dir) in [
            ("unparseable artifact", unparseable, dir),
            ("malformed recorded digest", malformed_digest, dir6),
            ("incompatible metadata", incompatible, dir2),
            ("cache-key mismatch", mismatched, dir3),
            ("frame mismatch", audio_mismatch, dir4),
            ("unreadable audio", unreadable, dir5),
        ] {
            assert!(
                matches!(error, BuildError::InvalidCache(_)),
                "{label} produced the wrong variant: {error}"
            );
            let message = error.to_string();
            assert!(
                message.contains(&dir.display().to_string()),
                "{label} did not name the entry directory: `{message}`"
            );
            assert!(
                message.contains("delete"),
                "{label} did not state the remedy: `{message}`"
            );
            assert!(
                message.contains("seg-0001"),
                "{label} did not name the segment: `{message}`"
            );
        }
    }

    /// The audio is left intact in every case here, so a rejection that speaks of a mismatch
    /// would be accusing the wrong file. Uppercase is the trap worth naming: it is a digest of
    /// the right audio, in the wrong spelling.
    #[test]
    fn t1_e0_malformed_recorded_digest_is_reported_as_malformed() {
        let workspace = TempDir::new().expect("create cache workspace");

        // Every published entry holds the same tone, so one digest describes all of them and the
        // malformations below are spellings of a digest that would otherwise match.
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

            let message = error.to_string();
            assert!(
                message.contains("not a lowercase BLAKE3 hex digest"),
                "`{malformed}` was not reported as malformed: `{message}`"
            );
            assert!(
                !message.contains("does not match"),
                "`{malformed}` was reported as a mismatch: `{message}`"
            );
        }
    }

    #[test]
    fn t1_e0_fresh_synthesis_failures_carry_no_delete_remedy() {
        let workspace = TempDir::new().expect("create cache workspace");
        let staged = workspace.path().join("staged.wav");
        write_tone(&staged, 2_400, 48_000);

        let error = validate_wav(&staged).expect_err("non-canonical WAV must be rejected");

        // Nothing is published yet, so advising a deletion would point at a path that does not
        // exist. The remedy is attached by `load_validated`, not by the validator.
        assert!(!error.to_string().contains("delete"), "error was `{error}`");
    }
}
