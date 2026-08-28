//! Strict provisional NDJSON frames for the replaceable worker boundary.
//!
//! Every frame carries a protocol version and request ID, and parsing enforces
//! the E0-S4 ceiling before JSON decoding. Unknown fields are rejected because
//! this is a project-owned format, not diagnostic tool output.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records the versions,
//! fixture set, consumers, and stabilization gate mirrored by this module.
//!
//! Identity fields parse as value objects rather than as strings, so a frame
//! naming a bundle or a voice profile that is not a digest is refused here
//! rather than downstream at the cache. `worker/study_tts_worker/protocol.py`
//! applies the same rule, and every other rule this module states: the accepted
//! version set, the trace-extension gate, the non-empty request identity, the
//! duplicate-name refusal, and each field's width.
//!
//! **What holds the two ends together is a file, not a convention.**
//! `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` records the frames
//! both implementations must accept or refuse alike;
//! `t3_e1_both_protocol_ends_decide_the_committed_cases_alike` reads it here
//! and `SharedContractCaseTests` reads it there. Each end used to carry its own
//! cases, and they agreed only by coincidence — this end accepted
//! `e0.worker.0.2` while that one refused it, and that end accepted a
//! `trace_context` under `e0.worker.0.1` while this one refused it. A rule only
//! one end enforces is a rule the other end can send past.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use study_tts_core::{VoiceProfileHash, WorkerBundleHash};
use thiserror::Error;

/// Mirrors the baseline wire version in the E0-S4 provisional contract record.
pub const WORKER_PROTOCOL_VERSION: &str = "e0.worker.0.1";

/// Mirrors the optional-extension version in the E0-S4 contract record.
pub const WORKER_PROTOCOL_EXTENSION_VERSION: &str = "e0.worker.0.2";

/// Mirrors the frame ceiling in the E0-S4 record's wire-compatibility section.
pub const MAX_WORKER_FRAME_BYTES: usize = 1024 * 1024;

/// Optional correlation metadata added by worker protocol 0.2.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TraceContext {
    /// Opaque local trace identifier; never source text or a voice path.
    pub trace_id: String,
}

/// Parameters that initialize one persistent worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct InitializeParameters {
    /// Immutable worker-bundle identity.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Maximum native threads assigned to this worker.
    ///
    /// Fixed-width rather than `usize`, which is the pointer width of whoever
    /// compiled the reader: a 32-bit build and a 64-bit build would accept
    /// different frames under one protocol version. The `maximum` the published
    /// schema carries for it comes from that width, in
    /// [`crate::schemas::PublishedSchema::generate`].
    ///
    /// Non-zero because zero is not a smaller allowance but an unanswerable
    /// instruction: no thread count means the worker cannot run at all, and
    /// both ends and the published schema accepted it while nothing yet reads
    /// the value. Applying it is E1-S3's; refusing a value no application
    /// could honor is this boundary's, and refusing it now costs nothing
    /// because no conforming frame carries one. `NonZeroU32` rather than a
    /// hand-written check, so the refusal is `serde`'s at the parse and the
    /// published `minimum` follows from the type.
    pub threads: NonZeroU32,
}

/// Parameters for a synthesis request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "method")]
pub enum WorkerRequestFrame {
    /// Load immutable model, voice, and device state.
    Initialize {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Initialization parameters.
        parameters: InitializeParameters,
    },
    /// Query the backend's supported request envelope.
    Capabilities {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
    },
    /// Render one approved segment.
    Synthesize {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Backend synthesis parameters.
        parameters: WorkerSynthesisParameters,
    },
    /// Request cancellation of the active synthesis.
    Cancel {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Active synthesis request to cancel.
        active_request_id: String,
    },
    /// Unload the worker and exit cleanly.
    Shutdown {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerCapabilities {
    /// Supported language identifiers.
    pub languages: Vec<String>,
    /// Maximum accepted text bytes per request.
    ///
    /// Fixed-width for the reason [`InitializeParameters::threads`] is.
    pub max_text_bytes: u64,
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
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
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "event")]
pub enum WorkerResponseFrame {
    /// Initialization completed successfully.
    Initialized {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Immutable backend identities loaded by the worker.
        identities: BTreeMap<String, String>,
    },
    /// Capability discovery completed successfully.
    Capabilities {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Supported backend envelope.
        capabilities: WorkerCapabilities,
    },
    /// An active synthesis reported bounded progress.
    Progress {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Completion fraction from zero through one.
        ///
        /// The range is published as well as parsed: [`parse_worker_response`]
        /// refuses anything outside it, and a schema that did not say so would
        /// let an author's editor pass a frame this build drops.
        #[schemars(range(min = 0.0, max = 1.0))]
        progress: f32,
    },
    /// Synthesis completed and staged audio is ready for Rust validation.
    SynthesisSucceeded {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
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
        worker_bundle_hash: WorkerBundleHash,
        /// Voice-profile identity used for synthesis.
        voice_profile_hash: VoiceProfileHash,
    },
    /// Cancellation completed for the active request.
    Cancelled {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Active synthesis request that was cancelled.
        active_request_id: String,
    },
    /// Shutdown completed and no model remains loaded.
    Shutdown {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
    },
    /// The worker refused or failed the correlated request.
    Failure {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
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

/// Either direction of the worker protocol, for the published schema.
///
/// The channel carries requests one way and responses the other, but a schema
/// describes *a frame* — and a reader validating a captured NDJSON stream has
/// lines of both kinds. `untagged` because each frame already carries its own
/// discriminator (`method` or `event`), so a wrapper tag would describe a
/// wire shape neither side writes.
///
/// Deserialization goes through [`parse_worker_request`] and
/// [`parse_worker_response`], which know which direction they are reading and
/// therefore give a far better message than an untagged union can. This type
/// exists to be described, not to be parsed.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum WorkerFrame {
    /// A frame Rust sends to the worker.
    Request(WorkerRequestFrame),
    /// A frame the worker sends back.
    Response(WorkerResponseFrame),
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
    if (frame.uses_trace_extension() || trace_context_is_present(bytes)?)
        && frame.protocol_version() != WORKER_PROTOCOL_EXTENSION_VERSION
    {
        return Err(WorkerFrameError::ExtensionRequiresVersion {
            required: WORKER_PROTOCOL_EXTENSION_VERSION,
        });
    }
    Ok(frame)
}

fn trace_context_is_present(bytes: &[u8]) -> Result<bool, WorkerFrameError> {
    let value: Value = serde_json::from_slice(bytes).map_err(WorkerFrameError::Malformed)?;
    Ok(value
        .get("parameters")
        .and_then(Value::as_object)
        .is_some_and(|parameters| parameters.contains_key("trace_context")))
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

/// Publishes the two protocol versions this build speaks, rather than any
/// string.
///
/// [`validate_version`] accepts exactly these, so the `enum` is that function
/// and not a description of it. Both are listed because
/// [`WORKER_PROTOCOL_EXTENSION_VERSION`] is what a frame carrying a trace
/// context must declare, and a schema naming only the baseline would refuse a
/// frame this build accepts.
fn protocol_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "enum": [WORKER_PROTOCOL_VERSION, WORKER_PROTOCOL_EXTENSION_VERSION],
    })
}

/// Publishes the one rule this build applies to a correlation identity.
///
/// [`validate_frame_identity`] refuses an empty one, because a refusal the
/// supervisor cannot correlate is a refusal it reports as a timeout.
fn request_id_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "minLength": 1,
    })
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

#[cfg(test)]
mod tests {
    use super::{
        WORKER_PROTOCOL_EXTENSION_VERSION, WorkerFrameError, parse_worker_request,
        parse_worker_response,
    };

    #[test]
    fn t1_e0_worker_protocol_0_1_refuses_explicit_null_trace_context() {
        let error = parse_worker_request(
            concat!(
                r#"{"method":"synthesize","protocol_version":"e0.worker.0.1","#,
                r#""request_id":"request-1","parameters":{"text":"reviewed","#,
                r#""voice":"voice-1","style":"calm","seed":7,"take":0,"#,
                r#""output":"request-1.wav","trace_context":null}}"#,
            )
            .as_bytes(),
        )
        .expect_err("an explicitly present 0.2 extension must be refused by protocol 0.1");

        assert!(matches!(
            error,
            WorkerFrameError::ExtensionRequiresVersion { required }
                if required == WORKER_PROTOCOL_EXTENSION_VERSION
        ));
    }

    /// A digest field refuses a well-formed JSON string that is not a digest.
    ///
    /// The mutation check for typing these fields: with `worker_bundle_hash`
    /// and `voice_profile_hash` back as `String`, both frames below parse and
    /// this test fails. Both directions are covered because a rule enforced on
    /// only one of them lets the other end send past it.
    ///
    /// A truncated digest is the case that matters rather than obvious junk: it
    /// is what a hand-edited or half-copied identity looks like, and it is the
    /// one a downstream comparison would report as a *mismatch* — telling an
    /// operator their bundle changed when the frame was simply wrong.
    ///
    /// The message is asserted, not just the variant. `Malformed` is also what
    /// a mistyped field name in these literals would produce, so a variant-only
    /// check would keep passing over a frame that never exercised the digest.
    #[test]
    fn t1_e1_a_frame_naming_an_identity_that_is_not_a_digest_is_refused() {
        let request = parse_worker_request(
            concat!(
                r#"{"method":"initialize","protocol_version":"e0.worker.0.1","#,
                r#""request_id":"request-1","parameters":{"#,
                r#""worker_bundle_hash":"abc","threads":1}}"#,
            )
            .as_bytes(),
        )
        .expect_err("an initialize frame naming a truncated bundle hash must be refused");
        assert!(
            matches!(&request, WorkerFrameError::Malformed(_))
                && request.to_string().contains("`abc`"),
            "the refusal must name the offending digest, not some other frame fault: {request}"
        );

        let response = parse_worker_response(
            concat!(
                r#"{"event":"synthesis_succeeded","protocol_version":"e0.worker.0.1","#,
                r#""request_id":"request-1","sample_rate":24000,"channels":1,"frames":10,"#,
                r#""model_revision":"m","codec_revision":"c","#,
                r#""worker_bundle_hash":"11111111111111111111111111111111"#,
                r#"11111111111111111111111111111111","voice_profile_hash":"abc"}"#,
            )
            .as_bytes(),
        )
        .expect_err("a synthesis response naming a truncated voice profile hash must be refused");
        assert!(
            matches!(&response, WorkerFrameError::Malformed(_))
                && response.to_string().contains("`abc`"),
            "the refusal must name the offending digest, not some other frame fault: {response}"
        );
    }
}
