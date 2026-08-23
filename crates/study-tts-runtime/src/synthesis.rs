use std::path::Path;

use study_tts_core::PlannedSegment;
use thiserror::Error;

/// What a synthesizer says it wrote, checked against the file it actually wrote.
#[derive(Clone, Debug)]
pub struct SynthesisReport {
    /// Sample rate the worker claims for the file, in hertz.
    pub sample_rate: u32,
    /// Channel count the worker claims for the file.
    pub channels: u16,
    /// Frame count the worker claims for the file.
    pub frames: u32,
}

/// A synthesizer refused or failed to produce audio for a segment.
#[derive(Debug, Error)]
#[error("synthesis failed: {message}")]
pub struct SynthesisError {
    /// What the synthesizer reported; opaque here because the worker owns the vocabulary.
    pub message: String,
}

impl SynthesisError {
    /// Wraps whatever the synthesizer reported.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// Provisional E0-S0 seam. E0-S4 replaces this with the async worker contract.
pub trait SegmentSynthesizer: Send + Sync {
    /// Stable identity of this synthesizer, which participates in every cache key.
    fn identity(&self) -> &str;

    /// Renders one planned segment to `destination` as canonical mono float WAV.
    fn synthesize(
        &self,
        segment: &PlannedSegment,
        destination: &Path,
    ) -> Result<SynthesisReport, SynthesisError>;
}
