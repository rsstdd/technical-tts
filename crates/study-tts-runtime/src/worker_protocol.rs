//! Strict provisional NDJSON frames for the replaceable worker boundary.
//!
//! Every frame carries a protocol version and request ID, and parsing enforces
//! the E0-S4 ceiling before JSON decoding. Unknown fields are rejected because
//! this is a project-owned format, not diagnostic tool output.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records the versions,
//! fixture set, consumers, and stabilization gate mirrored by this module.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Mirrors the baseline wire version in the E0-S4 provisional contract record.
pub const WORKER_PROTOCOL_VERSION: &str = "e0.worker.0.1";

/// Mirrors the optional-extension version in the E0-S4 contract record.
pub const WORKER_PROTOCOL_EXTENSION_VERSION: &str = "e0.worker.0.2";

/// Mirrors the frame ceiling in the E0-S4 record's wire-compatibility section.
pub const MAX_WORKER_FRAME_BYTES: usize = 1024 * 1024;

/// Optional correlation metadata added by worker protocol 0.2.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TraceContext {
    /// Opaque local trace identifier; never source text or a voice path.
    pub trace_id: String,
}

/// Parameters that initialize one persistent worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InitializeParameters {
    /// Immutable worker-bundle identity.
    pub worker_bundle_hash: String,
    /// Maximum native threads assigned to this worker.
    pub threads: usize,
}

/// Parameters for a synthesis request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerSynthesisParameters {
    /// Exact reviewed text to synthesize.
    pub text: String,
    /// Voice-profile identity, never a raw reference path.
    pub voice: String,
    /// Delivery style.
    pub style: String,
    /// Deterministic seed when the backend supports one.
    pub seed: u64,
    /// Take number within the synthesis base identity.
    pub take: u32,
    /// Managed relative output path assigned by Rust.
    pub output: String,
    /// Optional 0.2 trace extension; absent means no trace correlation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<TraceContext>,
}

/// Requests Rust may send to one worker process.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "method")]
pub enum WorkerRequestFrame {
    /// Load immutable model, voice, and device state.
    Initialize {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Initialization parameters.
        parameters: InitializeParameters,
    },
    /// Query the backend's supported request envelope.
    Capabilities {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
    },
    /// Render one approved segment.
    Synthesize {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Backend synthesis parameters.
        parameters: WorkerSynthesisParameters,
    },
    /// Request cancellation of the active synthesis.
    Cancel {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Active synthesis request to cancel.
        active_request_id: String,
    },
    /// Unload the worker and exit cleanly.
    Shutdown {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
    },
}

impl WorkerRequestFrame {
    fn protocol_version(&self) -> &str {
        match self {
            Self::Initialize {
                protocol_version, ..
            }
            | Self::Capabilities {
                protocol_version, ..
            }
            | Self::Synthesize {
                protocol_version, ..
            }
            | Self::Cancel {
                protocol_version, ..
            }
            | Self::Shutdown {
                protocol_version, ..
            } => protocol_version,
        }
    }

    fn request_id(&self) -> &str {
        match self {
            Self::Initialize { request_id, .. }
            | Self::Capabilities { request_id, .. }
            | Self::Synthesize { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::Shutdown { request_id, .. } => request_id,
        }
    }

    fn uses_trace_extension(&self) -> bool {
        matches!(
            self,
            Self::Synthesize {
                parameters: WorkerSynthesisParameters {
                    trace_context: Some(_),
                    ..
                },
                ..
            }
        )
    }
}

/// Backend capabilities returned after initialization.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerCapabilities {
    /// Supported language identifiers.
    pub languages: Vec<String>,
    /// Maximum accepted text bytes per request.
    pub max_text_bytes: usize,
    /// Supported voice profile identifiers.
    pub voices: Vec<String>,
    /// Supported style identifiers.
    pub styles: Vec<String>,
    /// Canonical output sample rate.
    pub sample_rate: u32,
    /// Canonical output channel count.
    pub channels: u16,
    /// Canonical output sample format.
    pub sample_format: String,
    /// Whether a fixed seed is characterized as deterministic.
    pub deterministic_seed: bool,
    /// Selected execution device.
    pub device: String,
}

/// Stable worker failure vocabulary carried on the protocol channel.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerFailureCode {
    /// Request parameters failed backend validation.
    InvalidRequest,
    /// Model or voice initialization failed.
    InitializationFailed,
    /// Synthesis failed after request acceptance.
    SynthesisFailed,
    /// Assigned staging output could not be written.
    OutputFailed,
    /// Active work exceeded its deadline.
    Timeout,
    /// Cancellation could not complete safely.
    CancellationFailed,
    /// Worker resources were exhausted.
    ResourceExhausted,
    /// Worker internal state became unusable.
    Internal,
}

/// Responses and events one worker may emit on protocol-only stdout.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "event")]
pub enum WorkerResponseFrame {
    /// Initialization completed successfully.
    Initialized {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Immutable backend identities loaded by the worker.
        identities: BTreeMap<String, String>,
    },
    /// Capability discovery completed successfully.
    Capabilities {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Supported backend envelope.
        capabilities: WorkerCapabilities,
    },
    /// An active synthesis reported bounded progress.
    Progress {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Completion fraction from zero through one.
        progress: f32,
    },
    /// Synthesis completed and staged audio is ready for Rust validation.
    SynthesisSucceeded {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Reported sample rate.
        sample_rate: u32,
        /// Reported channel count.
        channels: u16,
        /// Reported frame count.
        frames: u32,
        /// Model revision used for synthesis.
        model_revision: String,
        /// Tokenizer or codec revision used for synthesis.
        codec_revision: String,
        /// Worker-bundle identity used for synthesis.
        worker_bundle_hash: String,
        /// Voice-profile identity used for synthesis.
        voice_profile_hash: String,
    },
    /// Cancellation completed for the active request.
    Cancelled {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Active synthesis request that was cancelled.
        active_request_id: String,
    },
    /// Shutdown completed and no model remains loaded.
    Shutdown {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
    },
    /// The worker refused or failed the correlated request.
    Failure {
        /// Worker protocol version.
        protocol_version: String,
        /// Request correlation identity.
        request_id: String,
        /// Stable failure classification.
        code: WorkerFailureCode,
        /// Redacted diagnostic without source text or voice paths.
        message: String,
        /// Whether a bounded retry may be attempted.
        recoverable: bool,
    },
}

impl WorkerResponseFrame {
    fn protocol_version(&self) -> &str {
        match self {
            Self::Initialized {
                protocol_version, ..
            }
            | Self::Capabilities {
                protocol_version, ..
            }
            | Self::Progress {
                protocol_version, ..
            }
            | Self::SynthesisSucceeded {
                protocol_version, ..
            }
            | Self::Cancelled {
                protocol_version, ..
            }
            | Self::Shutdown {
                protocol_version, ..
            }
            | Self::Failure {
                protocol_version, ..
            } => protocol_version,
        }
    }

    fn request_id(&self) -> &str {
        match self {
            Self::Initialized { request_id, .. }
            | Self::Capabilities { request_id, .. }
            | Self::Progress { request_id, .. }
            | Self::SynthesisSucceeded { request_id, .. }
            | Self::Cancelled { request_id, .. }
            | Self::Shutdown { request_id, .. }
            | Self::Failure { request_id, .. } => request_id,
        }
    }
}

/// Why one NDJSON worker frame cannot be accepted.
#[derive(Debug, Error)]
pub enum WorkerFrameError {
    /// A frame exceeded the byte ceiling before JSON parsing.
    #[error("worker frame is {found} bytes but the ceiling is {maximum}")]
    TooLarge {
        /// Bytes received.
        found: usize,
        /// Configured frame ceiling.
        maximum: usize,
    },
    /// Input held a newline and therefore more than one NDJSON frame.
    #[error("worker input must contain exactly one JSON object without a newline")]
    NotSingleFrame,
    /// JSON was malformed or did not match the strict typed frame.
    #[error("worker frame is malformed: {0}")]
    Malformed(#[source] serde_json::Error),
    /// The frame declared a version this parser does not implement.
    #[error("worker protocol version `{found}` is unsupported")]
    UnsupportedVersion {
        /// Version the frame declared.
        found: String,
    },
    /// Request identity is required in every frame.
    #[error("worker frame request ID is empty")]
    EmptyRequestId,
    /// A 0.2 field appeared on a 0.1 frame.
    #[error("worker trace context requires protocol `{required}`")]
    ExtensionRequiresVersion {
        /// Minor protocol version that introduced the extension.
        required: &'static str,
    },
    /// A progress event was not a finite fraction from zero through one.
    #[error("worker progress `{found}` is outside the inclusive zero-to-one range")]
    InvalidProgress {
        /// Progress value the worker reported.
        found: f32,
    },
}

/// Parses and validates exactly one request frame without a trailing newline.
///
/// # Errors
///
/// [`WorkerFrameError::TooLarge`], [`WorkerFrameError::NotSingleFrame`],
/// [`WorkerFrameError::Malformed`], [`WorkerFrameError::UnsupportedVersion`],
/// [`WorkerFrameError::EmptyRequestId`], or
/// [`WorkerFrameError::ExtensionRequiresVersion`] when the named boundary
/// invariant fails.
pub fn parse_worker_request(bytes: &[u8]) -> Result<WorkerRequestFrame, WorkerFrameError> {
    validate_frame_bytes(bytes)?;
    let frame: WorkerRequestFrame =
        serde_json::from_slice(bytes).map_err(WorkerFrameError::Malformed)?;
    validate_frame_identity(frame.protocol_version(), frame.request_id())?;
    if frame.uses_trace_extension() && frame.protocol_version() != WORKER_PROTOCOL_EXTENSION_VERSION
    {
        return Err(WorkerFrameError::ExtensionRequiresVersion {
            required: WORKER_PROTOCOL_EXTENSION_VERSION,
        });
    }
    Ok(frame)
}

/// Parses and validates exactly one response frame without a trailing newline.
///
/// # Errors
///
/// [`WorkerFrameError::TooLarge`], [`WorkerFrameError::NotSingleFrame`],
/// [`WorkerFrameError::Malformed`], [`WorkerFrameError::UnsupportedVersion`],
/// [`WorkerFrameError::EmptyRequestId`], or
/// [`WorkerFrameError::InvalidProgress`] when the named boundary invariant
/// fails.
pub fn parse_worker_response(bytes: &[u8]) -> Result<WorkerResponseFrame, WorkerFrameError> {
    validate_frame_bytes(bytes)?;
    let frame: WorkerResponseFrame =
        serde_json::from_slice(bytes).map_err(WorkerFrameError::Malformed)?;
    validate_frame_identity(frame.protocol_version(), frame.request_id())?;
    if let WorkerResponseFrame::Progress { progress, .. } = frame
        && (!progress.is_finite() || !(0.0..=1.0).contains(&progress))
    {
        return Err(WorkerFrameError::InvalidProgress { found: progress });
    }
    Ok(frame)
}

fn validate_frame_bytes(bytes: &[u8]) -> Result<(), WorkerFrameError> {
    if bytes.len() > MAX_WORKER_FRAME_BYTES {
        return Err(WorkerFrameError::TooLarge {
            found: bytes.len(),
            maximum: MAX_WORKER_FRAME_BYTES,
        });
    }
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        return Err(WorkerFrameError::NotSingleFrame);
    }
    Ok(())
}

fn validate_frame_identity(version: &str, request_id: &str) -> Result<(), WorkerFrameError> {
    validate_version(version)?;
    if request_id.is_empty() {
        return Err(WorkerFrameError::EmptyRequestId);
    }
    Ok(())
}

fn validate_version(version: &str) -> Result<(), WorkerFrameError> {
    if matches!(
        version,
        WORKER_PROTOCOL_VERSION | WORKER_PROTOCOL_EXTENSION_VERSION
    ) {
        return Ok(());
    }
    Err(WorkerFrameError::UnsupportedVersion {
        found: version.to_owned(),
    })
}
