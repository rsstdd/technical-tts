use std::path::Path;

use study_tts_core::PlannedSegment;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct SynthesisReport {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u32,
}

#[derive(Debug, Error)]
#[error("synthesis failed: {message}")]
pub struct SynthesisError {
    pub message: String,
}

impl SynthesisError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Provisional E0-S0 seam. E0-S4 replaces this with the async worker contract.
pub trait SegmentSynthesizer: Send + Sync {
    fn identity(&self) -> &str;

    fn synthesize(
        &self,
        segment: &PlannedSegment,
        destination: &Path,
    ) -> Result<SynthesisReport, SynthesisError>;
}
