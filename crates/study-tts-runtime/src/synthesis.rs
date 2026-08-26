//! The provisional asynchronous port a speech backend implements.
//!
//! The representation mirrors ADR-0001 §10.4 and the E0-S4 baseline in
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`. Backend fields live
//! here instead of in lesson or planning types, and `&self` permits the E1
//! executor to dispatch concurrent requests without changing this boundary.

use std::{future::Future, path::Path, pin::Pin};

use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, CacheKey,
};
use thiserror::Error;

/// Mirrors the executor version in the E0-S4 provisional contract baseline.
pub const TTS_EXECUTOR_CONTRACT_VERSION: &str = "e0.tts-executor.0.1";

/// Stable identity and supported request envelope of one backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDescriptor {
    /// Executor contract version implemented by this backend.
    pub contract_version: String,
    /// Speech-affecting backend identity included in every synthesis key.
    pub synthesis_identity: String,
    /// Maximum UTF-8 bytes accepted as spoken text in one request.
    pub max_text_bytes: usize,
    /// Whether a fixed seed is expected to be deterministic for this backend.
    pub deterministic_seed: bool,
}

/// Backend-owned synthesis inputs derived from one planned segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisRequest {
    /// Request identity unique within a worker lifetime.
    pub request_id: String,
    /// Segment identity used for diagnostics and staging ownership.
    pub segment_id: String,
    /// Exact reviewed text to speak.
    pub spoken_text: String,
    /// Voice profile selected by the plan.
    pub voice: String,
    /// Delivery style selected by the plan.
    pub style: String,
    /// Synthesis identity of the requested take.
    pub cache_key: CacheKey,
    /// Required output sample rate in hertz.
    pub sample_rate: u32,
    /// Required output channel count.
    pub channels: u16,
    /// Required output sample format.
    pub sample_format: String,
}

/// What an executor says it wrote, checked against the staged file itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisReport {
    /// Sample rate the executor claims for the file, in hertz.
    pub sample_rate: u32,
    /// Channel count the executor claims for the file.
    pub channels: u16,
    /// Frame count the executor claims for the file.
    pub frames: u32,
    /// Backend revision that produced the audio.
    pub backend_revision: String,
    /// Worker-bundle identity that produced the audio.
    pub worker_bundle_hash: String,
    /// Voice-profile identity that produced the audio.
    pub voice_profile_hash: String,
}

/// A request invariant the executor can reject before synthesis begins.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BackendValidationError {
    /// The executor implements a different contract version.
    #[error("executor contract `{found}` is incompatible with required `{required}`")]
    IncompatibleContract {
        /// Contract version the executor reports.
        found: String,
        /// Contract version this build requires.
        required: &'static str,
    },
    /// Capacity zero cannot accept any request.
    #[error("executor capacity must be at least one")]
    ZeroCapacity,
    /// Request IDs are required for correlation and cancellation.
    #[error("synthesis request ID is empty")]
    EmptyRequestId,
    /// Segment IDs are required for diagnostics and containment.
    #[error("synthesis segment ID is empty")]
    EmptySegmentId,
    /// Spoken text exceeded the backend-declared request envelope.
    #[error("spoken text is {found} bytes but the executor accepts at most {maximum}")]
    TextTooLarge {
        /// Bytes in the requested text.
        found: usize,
        /// Backend-declared maximum.
        maximum: usize,
    },
    /// The requested intermediate format differs from the canonical format.
    #[error(
        "requested audio format is {sample_rate} Hz, {channels} channels, `{sample_format}`; the \
         provisional executor requires 24000 Hz, one channel, `f32le`"
    )]
    NonCanonicalFormat {
        /// Requested sample rate.
        sample_rate: u32,
        /// Requested channel count.
        channels: u16,
        /// Requested sample format.
        sample_format: String,
    },
}

/// Why an executor could not produce one staged audio artifact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BackendError {
    /// Request validation failed before backend work began.
    #[error("synthesis request `{request_id}` is invalid: {source}")]
    InvalidRequest {
        /// Request the refusal belongs to.
        request_id: String,
        /// Exact violated request invariant.
        source: BackendValidationError,
    },
    /// The assigned destination was not writable by the backend.
    #[error("executor could not write request `{request_id}` to `{destination}`: {message}")]
    Destination {
        /// Request the failure belongs to.
        request_id: String,
        /// Assigned staging destination.
        destination: std::path::PathBuf,
        /// Backend diagnostic without source text.
        message: String,
    },
    /// Backend inference failed after accepting the request.
    #[error("executor failed request `{request_id}` with `{code}`: {message}")]
    Execution {
        /// Request the failure belongs to.
        request_id: String,
        /// Stable backend failure code.
        code: String,
        /// Redacted backend diagnostic.
        message: String,
    },
    /// Backend work exceeded its bounded deadline.
    #[error("executor timed out request `{request_id}` after {timeout_ms} ms")]
    Timeout {
        /// Request the timeout belongs to.
        request_id: String,
        /// Enforced deadline in milliseconds.
        timeout_ms: u64,
    },
    /// Executor or orchestration protocol invariants failed.
    #[error("executor protocol failed request `{request_id}`: {message}")]
    Protocol {
        /// Request the protocol failure belongs to.
        request_id: String,
        /// Redacted protocol diagnostic.
        message: String,
    },
}

/// Object-safe asynchronous speech executor from ADR-0001 §10.4.
pub trait TtsExecutor: Send + Sync {
    /// Returns stable backend identity and validation limits.
    fn descriptor(&self) -> BackendDescriptor;

    /// Returns the number of requests this executor may run concurrently.
    fn capacity(&self) -> usize;

    /// Refuses a request that this executor cannot honor without starting work.
    ///
    /// # Errors
    ///
    /// [`BackendError::InvalidRequest`] for the exact request invariant that
    /// failed.
    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError>;

    /// Renders one request to the assigned managed staging destination.
    ///
    /// # Errors
    ///
    /// [`BackendError::InvalidRequest`] when validation fails,
    /// [`BackendError::Destination`] for a destination write failure,
    /// [`BackendError::Execution`] for backend inference failure,
    /// [`BackendError::Timeout`] for a bounded deadline, or
    /// [`BackendError::Protocol`] when protocol interaction cannot be trusted.
    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
        destination: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>>;
}

/// Applies the request invariants shared by every provisional executor.
///
/// # Errors
///
/// [`BackendValidationError::IncompatibleContract`],
/// [`BackendValidationError::ZeroCapacity`],
/// [`BackendValidationError::EmptyRequestId`],
/// [`BackendValidationError::EmptySegmentId`],
/// [`BackendValidationError::TextTooLarge`], or
/// [`BackendValidationError::NonCanonicalFormat`] when the named invariant
/// does not hold.
pub fn validate_executor_request(
    descriptor: &BackendDescriptor,
    capacity: usize,
    request: &SynthesisRequest,
) -> Result<(), BackendValidationError> {
    if descriptor.contract_version != TTS_EXECUTOR_CONTRACT_VERSION {
        return Err(BackendValidationError::IncompatibleContract {
            found: descriptor.contract_version.clone(),
            required: TTS_EXECUTOR_CONTRACT_VERSION,
        });
    }
    if capacity == 0 {
        return Err(BackendValidationError::ZeroCapacity);
    }
    if request.request_id.is_empty() {
        return Err(BackendValidationError::EmptyRequestId);
    }
    if request.segment_id.is_empty() {
        return Err(BackendValidationError::EmptySegmentId);
    }
    if request.spoken_text.len() > descriptor.max_text_bytes {
        return Err(BackendValidationError::TextTooLarge {
            found: request.spoken_text.len(),
            maximum: descriptor.max_text_bytes,
        });
    }
    if request.sample_rate != CANONICAL_SAMPLE_RATE
        || request.channels != CANONICAL_CHANNELS
        || request.sample_format != CANONICAL_SAMPLE_FORMAT
    {
        return Err(BackendValidationError::NonCanonicalFormat {
            sample_rate: request.sample_rate,
            channels: request.channels,
            sample_format: request.sample_format.clone(),
        });
    }
    Ok(())
}
