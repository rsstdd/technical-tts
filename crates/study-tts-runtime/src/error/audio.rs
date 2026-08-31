//! Canonical-audio, synthesis-report, and assembly refusals.

use std::path::PathBuf;

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// The two conditioning identities one worker report carried.
///
/// A named payload rather than three inline fields, so
/// [`AudioError::ConditioningIdentityContradiction`] costs one pointer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditioningContradiction {
    /// The segment whose report disagreed with itself.
    pub segment_id: String,
    /// The artifact the worker reported it read.
    pub reported: String,
    /// The artifact named by the identity inputs the same report returned.
    pub in_context: String,
}

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

    /// The identities the worker reports do not name the audio's cache key.
    ///
    /// A separate refusal from [`AudioError::SynthesizerReportMismatch`]
    /// because the two are different failures with different owners: that one
    /// says the worker miscounted its own frames, this one says the worker
    /// synthesized under inputs the plan did not ask for. Publishing anyway
    /// would put audio under a key describing a different model, bundle, voice,
    /// or language — the one thing a content-addressed cache must never do,
    /// because every later reuse would be silently wrong.
    #[error(
        "segment `{segment_id}` was planned under synthesis key `{planned}` but the worker \
         reports inputs whose key is `{reported}`; the worker is synthesizing under different \
         model, bundle, voice, or language identities than it was asked for, and the \
         worker/runtime owner must correct it before this build is rerun"
    )]
    SynthesizerIdentityMismatch {
        /// The segment whose provenance disagreed.
        segment_id: String,
        /// Synthesis key the plan derived and the entry would be published as.
        planned: study_tts_core::CacheKey,
        /// Synthesis key recomputed from what the worker reports it used.
        reported: study_tts_core::CacheKey,
    },

    /// A report's two conditioning identities name different artifacts.
    ///
    /// The worker reports the conditioning artifact it read twice: once as
    /// [`crate::SynthesisReport::voice_conditioning_hash`], and once inside the
    /// context the cache recomputes the synthesis key from. Only the second
    /// reaches the key, so a report whose two values disagree passes the
    /// identity gate while its published provenance names an artifact the
    /// worker did not say it used. Distinct from
    /// [`AudioError::SynthesizerIdentityMismatch`], which is a worker
    /// disagreeing with the *plan*: this is a worker disagreeing with itself,
    /// and the plan cannot see it.
    ///
    /// Boxed for the reason [`crate::BuildError::Lesson`] is: three owned
    /// strings inline would push `BuildError` past the 80-byte baseline that
    /// `t1_e0_build_error_does_not_grow_during_category_refactor` holds it to,
    /// and that baseline mirrors a measurement in
    /// `docs/architecture/WALKING-SKELETON.md` §Provisional boundary ownership.
    #[error(
        "segment `{}` was rendered by a worker reporting conditioning artifact `{}` while the \
         identity inputs it returned name `{}`; a report that contradicts itself cannot say \
         which voice produced this audio, and the worker/runtime owner must correct it before \
         this build is rerun",
        .0.segment_id,
        .0.reported,
        .0.in_context
    )]
    ConditioningIdentityContradiction(Box<ConditioningContradiction>),

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
            Self::ConditioningIdentityContradiction { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "correct the worker report before rerunning the build",
                Some("Worker protocol or containment failure"),
            )),
            Self::SynthesizerIdentityMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "correct the worker's synthesis identities before rerunning the build",
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

    /// The stream is longer than one segment may be.
    ///
    /// A security ceiling rather than an editorial one: the samples are held in
    /// memory to be conditioned, so an unbounded file is the process.
    /// `crate::MAX_SEGMENT_AUDIO_MS` is the value and
    /// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
    /// records it.
    #[error(
        "it carries {frames} frames, beyond the provisional {max_frames}-frame \
         ({max_milliseconds} ms) ceiling for one segment"
    )]
    TooLong {
        /// Frames the stream carries.
        frames: u32,
        /// The most this build reads or conditions.
        max_frames: u32,
        /// The same ceiling as the duration the document records.
        max_milliseconds: u32,
    },

    /// An exposed edge does not begin or end at exactly zero.
    ///
    /// ADR-0001 §13.4 requires exposed endpoints to be exactly zero so assembly
    /// can concatenate segments without a step at the join. Exactly, not
    /// nearly: a value that is merely small is still a discontinuity when the
    /// previous segment ended at zero.
    #[error("its {edge} sample is `{value}` rather than exactly zero")]
    ExposedEndpointNotZero {
        /// Which end of the stream, for a reader repairing it.
        edge: &'static str,
        /// The value found there.
        value: f32,
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
