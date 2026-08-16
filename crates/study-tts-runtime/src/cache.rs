use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{CANONICAL_SAMPLE_RATE, PlannedSegment};
use tempfile::Builder;

use crate::{BuildError, SegmentSynthesizer, audio_error, io_error};

const CACHE_SCHEMA_VERSION: &str = "0.1-skeleton";

#[derive(Clone, Debug)]
pub(crate) struct CachedSegment {
    pub segment_id: String,
    pub cache_key: String,
    pub audio_path: PathBuf,
    pub audio_blake3: String,
    pub frames: u32,
    pub pause_after_ms: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CacheArtifact {
    schema_version: String,
    cache_key: String,
    audio_blake3: String,
    sample_rate: u32,
    channels: u16,
    sample_format: String,
    frames: u32,
}

/// Directory holding one cache entry. Shared with tests so the sharding scheme is defined once.
pub(crate) fn entry_dir(cache_root: &Path, cache_key: &str) -> PathBuf {
    cache_root
        .join("segments")
        .join(&cache_key[..2])
        .join(cache_key)
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

    // Path 2: the artifact parses but describes audio this build cannot consume.
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

    // Path 3: the entry belongs to a different synthesis identity.
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

    // Path 4: the audio itself is unreadable, non-canonical, or does not match the artifact.
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
    use tempfile::TempDir;

    fn planned(cache_key: &str) -> PlannedSegment {
        PlannedSegment {
            id: "seg-0001".to_owned(),
            speaker: "nadia".to_owned(),
            spoken_text: "Same speech.".to_owned(),
            style: "calm".to_owned(),
            pause_after_ms: 75,
            cache_key: cache_key.to_owned(),
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
        let dir = entry_dir(root, cache_key);
        fs::create_dir_all(&dir).expect("create entry directory");
        let audio = dir.join("audio.wav");
        let artifact = dir.join("artifact.json");
        write_tone(&audio, 2_400, CANONICAL_SAMPLE_RATE);
        let record = CacheArtifact {
            schema_version: CACHE_SCHEMA_VERSION.to_owned(),
            cache_key: cache_key.to_owned(),
            audio_blake3: hash_file(&audio).expect("hash test audio"),
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: 1,
            sample_format: "f32le".to_owned(),
            frames: 2_400,
        };
        write_json_atomically(&artifact, &record).expect("write test artifact");
        (dir, audio, artifact)
    }

    #[test]
    fn t1_e0_entry_dir_is_sharded_by_key_prefix() {
        assert_eq!(
            entry_dir(Path::new("/cache"), "abcdef"),
            Path::new("/cache/segments/ab/abcdef")
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

        // Path 2: incompatible declared metadata.
        let (dir2, audio2, artifact2) = published_entry(workspace.path(), "bb2222");
        let mut record: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact2).expect("read artifact")).expect("parse");
        record["schema_version"] = serde_json::Value::String("future".to_owned());
        fs::write(
            &artifact2,
            serde_json::to_vec_pretty(&record).expect("serialize"),
        )
        .expect("write artifact");
        let incompatible = load_validated(&planned("bb2222"), &dir2, &audio2, &artifact2)
            .expect_err("incompatible metadata must be rejected");

        // Path 3: cache-key mismatch.
        let (dir3, audio3, artifact3) = published_entry(workspace.path(), "cc3333");
        let mismatched = load_validated(&planned("dd4444"), &dir3, &audio3, &artifact3)
            .expect_err("cache-key mismatch must be rejected");

        // Path 4: audio that no longer matches its record.
        let (dir4, audio4, artifact4) = published_entry(workspace.path(), "ee5555");
        write_tone(&audio4, 1_200, CANONICAL_SAMPLE_RATE);
        let audio_mismatch = load_validated(&planned("ee5555"), &dir4, &audio4, &artifact4)
            .expect_err("frame mismatch must be rejected");

        // Path 4b: audio that is no longer readable at all.
        let (dir5, audio5, artifact5) = published_entry(workspace.path(), "ff6666");
        fs::write(&audio5, b"not a wav").expect("corrupt audio");
        let unreadable = load_validated(&planned("ff6666"), &dir5, &audio5, &artifact5)
            .expect_err("unreadable audio must be rejected");

        for (label, error, dir) in [
            ("unparseable artifact", unparseable, dir),
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
