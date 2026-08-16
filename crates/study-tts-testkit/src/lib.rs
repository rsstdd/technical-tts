use std::{
    f32::consts::TAU,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use study_tts_core::{CANONICAL_SAMPLE_RATE, PlannedSegment};
use study_tts_runtime::{SegmentSynthesizer, SynthesisError, SynthesisReport};

const TONE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

#[derive(Debug, Default)]
pub struct DeterministicToneWorker {
    synthesis_count: AtomicUsize,
    synthesized_texts: Mutex<Vec<String>>,
}

impl DeterministicToneWorker {
    pub fn synthesis_count(&self) -> usize {
        self.synthesis_count.load(Ordering::SeqCst)
    }

    pub fn synthesized_texts(&self) -> Vec<String> {
        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl SegmentSynthesizer for DeterministicToneWorker {
    fn identity(&self) -> &str {
        "deterministic-tone-worker-v1"
    }

    fn synthesize(
        &self,
        segment: &PlannedSegment,
        destination: &Path,
    ) -> Result<SynthesisReport, SynthesisError> {
        let freq = 300.0
            + f32::from(
                segment
                    .cache_key
                    .bytes()
                    .fold(0_u8, |accumulator, byte| accumulator.wrapping_add(byte)),
            );
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(destination, spec)
            .map_err(|error| SynthesisError::new(error.to_string()))?;

        for frame in 0..TONE_FRAMES {
            let phase = TAU * freq * frame as f32 / CANONICAL_SAMPLE_RATE as f32;
            writer
                .write_sample(phase.sin() * 0.2)
                .map_err(|error| SynthesisError::new(error.to_string()))?;
        }

        writer
            .finalize()
            .map_err(|error| SynthesisError::new(error.to_string()))?;

        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(segment.spoken_text.clone());

        self.synthesis_count.fetch_add(1, Ordering::SeqCst);

        Ok(SynthesisReport {
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: 1,
            frames: TONE_FRAMES,
        })
    }
}

pub fn walking_skeleton_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lessons/e0-s0-two-segment.json")
}

pub fn cache_identity_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lessons/e0-s0-cache-identity.json")
}
