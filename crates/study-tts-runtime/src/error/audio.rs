//! Canonical-audio, synthesis-report, and assembly refusals.

use std::path::PathBuf;

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why rendered or assembled audio cannot proceed.
#[derive(Debug, Error)]
pub enum AudioError {
    /// Freshly synthesized audio or a staged master failed validation.
    ///
    /// This carries no deletion instruction because the staged cache attempt
    /// is moved to collision-free quarantine; authoritative content is never
    /// an operator-directed deletion remedy.
    #[error("`{path}` is not usable lesson audio: {fault}")]
    UnusableAudio {
        /// The audio file that failed validation.
        path: PathBuf,
        /// Which audio property failed.
        fault: AudioFault,
    },

    /// The worker's report disagrees with the file it wrote.
    #[error(
        "synthesizer reported {reported_sample_rate} Hz, {reported_channels} channels, and \
         {reported_frames} frames for segment `{segment_id}` but wrote a WAV with \
         {written_sample_rate} Hz, {written_channels} channels, and {written_frames} frames; the \
         worker is misreporting its own output and must be corrected before this build is rerun"
    )]
    SynthesizerReportMismatch {
        /// The segment whose synthesis was misreported.
        segment_id: String,
        /// Sample rate the worker claimed.
        reported_sample_rate: u32,
        /// Channel count the worker claimed.
        reported_channels: u16,
        /// Frame count the worker claimed.
        reported_frames: u32,
        /// Sample rate the file actually carries.
        written_sample_rate: u32,
        /// Channel count the file actually carries.
        written_channels: u16,
        /// Frame count the file actually carries.
        written_frames: u32,
    },

    /// A segment's trailing pause is too long to express as a frame count.
    ///
    /// This defensive variant protects the frame-counter field width even
    /// though the current canonical rate and lesson pause cap cannot reach it.
    #[error(
        "the pause of {pause_after_ms} ms after segment `{segment_id}` overflows the frame count \
         this build can assemble; shorten the pause in the lesson"
    )]
    PauseFrameOverflow {
        /// The segment carrying the pause.
        segment_id: String,
        /// The pause as the lesson declares it, in milliseconds.
        pause_after_ms: u32,
    },

    /// The planned lesson is longer than the build can represent.
    ///
    /// This defensive variant protects checked total-frame arithmetic even
    /// though current lesson and WAV field widths make it unreachable.
    #[error(
        "the planned lesson exceeds the frame count this build can assemble; split the lesson \
         into shorter lessons"
    )]
    PlannedLengthOverflow,

    /// The master grew past what the build can count while being written.
    ///
    /// This defensive variant keeps the write loop independently checked even
    /// though its pre-pass currently proves the same bound.
    #[error(
        "assembling `{destination}` exceeded the frame count this build can track; split the \
         lesson into shorter lessons"
    )]
    AssembledLengthOverflow {
        /// The master being assembled.
        destination: PathBuf,
    },

    /// The master's length disagrees with validated cache metadata.
    #[error(
        "assembled master `{destination}` contains {assembled} frames but the plan requires \
         {expected}; the runtime owner must reconcile the cache before this lesson is rebuilt"
    )]
    AssembledLengthMismatch {
        /// The master whose length disagrees with the plan.
        destination: PathBuf,
        /// Frames actually written.
        assembled: u64,
        /// Frames the validated cache metadata implies.
        expected: u64,
    },
}

impl AudioError {
    /// Returns governed recovery advice when this audio refusal has an owner.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::UnusableAudio { .. } => Some(RemedyAdvice::new(
                RemedyOwner::AudioRuntime,
                "quarantine the attempt and retry within the bounded budget",
                Some("Invalid or over-range audio"),
            )),
            Self::SynthesizerReportMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "correct the worker report before rerunning the build",
                Some("Worker protocol or containment failure"),
            )),
            Self::AssembledLengthMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "reconcile the cache before rebuilding the lesson",
                Some("State or checksum corruption"),
            )),
            Self::PauseFrameOverflow { .. }
            | Self::PlannedLengthOverflow
            | Self::AssembledLengthOverflow { .. } => None,
        }
    }
}

/// Why a WAV cannot serve as canonical lesson audio.
///
/// This inner fault has neither path nor remedy because callers apply the same
/// validation to disposable staged audio and published cache entries. The
/// owning outer category supplies the context appropriate to each use.
#[derive(Debug, Error)]
pub enum AudioFault {
    /// The file is not readable as WAV at all.
    #[error("it could not be read as WAV ({0})")]
    Unreadable(#[from] hound::Error),

    /// The stream is readable but is not the canonical format.
    #[error(
        "the stream is {channels}-channel {sample_rate} Hz {bits_per_sample}-bit \
         {sample_format}, not canonical {required_channels}-channel \
         {required_sample_rate} Hz {required_bits_per_sample}-bit float"
    )]
    NonCanonical {
        /// Channel count the stream carries.
        channels: u16,
        /// Sample rate the stream carries, in hertz.
        sample_rate: u32,
        /// Bit depth the stream carries.
        bits_per_sample: u16,
        /// Whether the stream is integer or float.
        sample_format: &'static str,
        /// The one channel count this project accepts.
        required_channels: u16,
        /// The one sample rate this project accepts, in hertz.
        required_sample_rate: u32,
        /// The one bit depth this project accepts.
        required_bits_per_sample: u16,
    },

    /// A sample is non-finite or beyond full scale.
    #[error("sample {index} is `{value}`, outside the finite range -1.0 to 1.0")]
    OutOfRangeSample {
        /// Zero-based frame the bad sample sits at.
        index: u32,
        /// The offending value.
        value: f32,
    },

    /// The file is a valid WAV holding no audio.
    #[error("it contains no audio frames")]
    Empty,

    /// The file holds more frames than the frame counter can represent.
    ///
    /// This defensive variant protects the artifact record's `u32` field even
    /// though a WAV data chunk cannot currently reach the limit.
    #[error("it holds more frames than this build can count")]
    FrameCountOverflow,
}
