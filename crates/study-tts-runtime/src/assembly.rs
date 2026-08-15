use std::path::Path;

use study_tts_core::CANONICAL_SAMPLE_RATE;
use tempfile::Builder;

use crate::{BuildError, cache::CachedSegment, io_error};

pub(crate) fn assemble(segments: &[CachedSegment], destination: &Path) -> Result<u64, BuildError> {
    let parent = destination.parent().ok_or_else(|| {
        BuildError::InvalidCache(format!(
            "`{}` has no parent directory",
            destination.display()
        ))
    })?;
    let mut staged = Builder::new()
        .prefix("master-")
        .suffix(".wav")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::new(staged.as_file_mut(), spec)?;
    let mut total_frames = 0_u64;

    for segment in segments {
        let mut reader = hound::WavReader::open(&segment.audio_path)?;
        for sample in reader.samples::<f32>() {
            writer.write_sample(sample?)?;
            total_frames = total_frames.checked_add(1).ok_or_else(|| {
                BuildError::InvalidCache("assembled frame count overflow".to_owned())
            })?;
        }

        let pause_frames = u64::from(segment.pause_after_ms)
            .checked_mul(u64::from(CANONICAL_SAMPLE_RATE))
            .ok_or_else(|| BuildError::InvalidCache("pause frame count overflow".to_owned()))?
            / 1_000;
        for _ in 0..pause_frames {
            writer.write_sample(0.0_f32)?;
        }
        total_frames = total_frames
            .checked_add(pause_frames)
            .ok_or_else(|| BuildError::InvalidCache("assembled frame count overflow".to_owned()))?;
    }

    writer.finalize()?;
    staged
        .persist(destination)
        .map_err(|error| io_error(destination, error.error))?;
    Ok(total_frames)
}
