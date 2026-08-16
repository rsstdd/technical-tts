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

/// Every rejection names the entry directory and the remedy, because E0-S0 has no quarantine and
/// a poisoned entry would otherwise fail every subsequent build with no stated way out.
fn rejected(entry_dir: &Path, segment_id: &str, detail: &str) -> BuildError {
    BuildError::InvalidCache(format!(
        "cache entry for segment `{segment_id}` is unusable: {detail}; delete `{}` to regenerate \
         this segment",
        entry_dir.display()
    ))
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
    let artifact: CacheArtifact = serde_json::from_slice(&bytes).map_err(|error| {
        rejected(
            entry_dir,
            &segment.id,
            &format!("`{}` could not be parsed: {error}", artifact_path.display()),
        )
    })?;

    if artifact.schema_version != CACHE_SCHEMA_VERSION
        || artifact.sample_rate != CANONICAL_SAMPLE_RATE
        || artifact.channels != 1
        || artifact.sample_format != "f32le"
    {
        return Err(rejected(
            entry_dir,
            &segment.id,
            &format!(
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
    if artifact.cache_key != segment.cache_key {
        return Err(rejected(
            entry_dir,
            &segment.id,
            &format!(
                "artifact records cache key `{}` but the plan requires `{}`",
                artifact.cache_key, segment.cache_key
            ),
        ));
    }

    let frames = validate_wav(audio_path)?;
    let checksum = hash_file(audio_path)?;
    if frames != artifact.frames {
        return Err(rejected(
            entry_dir,
            &segment.id,
            &format!(
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
    let mut reader =
        hound::WavReader::open(path).map_err(|error| audio_error(path, error))?;
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

    #[test]
    fn t1_e0_missing_audio_names_the_file() {
        let workspace = TempDir::new().expect("create cache workspace");
        let missing = workspace.path().join("absent.wav");

        let error = validate_wav(&missing).expect_err("missing audio must fail");

        assert!(matches!(error, BuildError::AudioAt { .. }));
        assert!(
            error.to_string().contains(&missing.display().to_string()),
            "error was `{error}`"
        );
    }

    #[test]
    fn t1_e0_entry_dir_is_sharded_by_key_prefix() {
        let root = Path::new("/cache");

        assert_eq!(
            entry_dir(root, "abcdef"),
            Path::new("/cache/segments/ab/abcdef")
        );
    }
}