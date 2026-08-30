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
//! `e1.worker.1.1` while that one refused it, and that end accepted a
//! `trace_context` under `e1.worker.1.0` while this one refused it. A rule only
//! one end enforces is a rule the other end can send past.

use std::collections::BTreeMap;
use std::num::NonZeroU32;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use study_tts_core::{MAX_REVISION_BYTES, Revision, VoiceProfileHash, WorkerBundleHash};
use thiserror::Error;

/// Baseline wire version in the provisional contract record.
pub const WORKER_PROTOCOL_VERSION: &str = "e1.worker.1.0";

/// Optional trace-extension version in the provisional contract record.
pub const WORKER_PROTOCOL_EXTENSION_VERSION: &str = "e1.worker.1.1";

/// Mirrors the frame ceiling in the E0-S4 record's wire-compatibility section.
pub const MAX_WORKER_FRAME_BYTES: usize = 1024 * 1024;

/// Longest correlation identity either end of the protocol will accept.
///
/// A refusal has to name the request it refuses, so an identity bounded only by
/// [`MAX_WORKER_FRAME_BYTES`] is one that can make the answer to a frame larger
/// than the ceiling the answer must itself fit inside. Bounded here, at
/// validation, rather than shortened on the way out: an identity the supervisor
/// cannot match is at least reported as refused, while a shortened one comes
/// back looking like a different request that was answered.
///
/// Generous against what this build constructs. The longest is
/// `pipeline.rs`'s `e0-<cache key>-<segment id>`, at 3 + 64 + 1 + 64 bytes.
/// `study_tts_worker.protocol.MAX_REQUEST_ID_BYTES` is the same rule at the
/// other end and names this constant in return.
pub const MAX_WORKER_REQUEST_ID_BYTES: usize = 256;

/// Optional correlation metadata added by worker protocol 1.1.
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
    /// Optional 1.1 trace extension; absent means no trace correlation.
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
    /// Report readiness and model-resource residency.
    Health {
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
        #[schemars(schema_with = "request_id_json_schema")]
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
            | Self::Health {
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
            | Self::Health { request_id, .. }
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

    fn active_request_id(&self) -> Option<&str> {
        match self {
            Self::Cancel {
                active_request_id, ..
            } => Some(active_request_id),
            _ => None,
        }
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

/// Immutable identities loaded by a successfully initialized worker.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerInitializationIdentities {
    /// Pinned model revision loaded by the worker.
    #[schemars(schema_with = "revision_json_schema")]
    pub model_revision: Revision,
    /// Pinned tokenizer or codec revision loaded by the worker.
    #[schemars(schema_with = "revision_json_schema")]
    pub tokenizer_revision: Revision,
    /// Identity of the executable worker bundle.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Loaded voice profiles keyed by their stable profile identifiers.
    #[serde(deserialize_with = "deserialize_voice_profile_hashes")]
    #[schemars(schema_with = "nonempty_voice_profile_hashes_json_schema")]
    pub voice_profile_hashes: BTreeMap<String, VoiceProfileHash>,
}

/// Reads the loaded voice profiles, refusing an empty set or a repeated name.
///
/// Through [`crate::distinct_map`] rather than a derived [`BTreeMap`], which
/// keeps the last binding silently. The digest it would keep is an ADR-0001
/// §12.5 conditioning identity, so a worker naming one profile under two
/// digests would choose which digest this build records for that name.
/// `worker/study_tts_worker/protocol.py`'s `_distinct_keys` is the same rule at
/// the other end, applied to every object it reads.
///
/// Not in `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` with the
/// other duplicate-name cases: both ends read that file through their request
/// parser, and only this end parses a response at all.
fn deserialize_voice_profile_hashes<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, VoiceProfileHash>, D::Error>
where
    D: Deserializer<'de>,
{
    let hashes = crate::distinct_map::deserialize(deserializer)?;
    if hashes.is_empty() {
        return Err(D::Error::custom(
            "a successful initialization must report at least one voice-profile identity",
        ));
    }
    Ok(hashes)
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
        identities: WorkerInitializationIdentities,
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
    /// Worker readiness and model-resource residency were reported.
    Health {
        /// Worker protocol version.
        #[schemars(schema_with = "protocol_version_json_schema")]
        protocol_version: String,
        /// Request correlation identity.
        #[schemars(schema_with = "request_id_json_schema")]
        request_id: String,
        /// Whether this process can accept a synthesis request.
        ready: bool,
        /// Whether the speech model currently occupies worker resources.
        model_loaded: bool,
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
        #[schemars(schema_with = "request_id_json_schema")]
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
            | Self::Health {
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
            | Self::Health { request_id, .. }
            | Self::Progress { request_id, .. }
            | Self::SynthesisSucceeded { request_id, .. }
            | Self::Cancelled { request_id, .. }
            | Self::Shutdown { request_id, .. }
            | Self::Failure { request_id, .. } => request_id,
        }
    }

    fn active_request_id(&self) -> Option<&str> {
        match self {
            Self::Cancelled {
                active_request_id, ..
            } => Some(active_request_id),
            _ => None,
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
    /// Request identity contains a non-ASCII character.
    #[error("worker frame request ID must contain only ASCII characters")]
    NonAsciiRequestId,
    /// Request identity is longer than either end will correlate.
    #[error("worker frame request ID is {found} bytes but the ceiling is {maximum}")]
    RequestIdTooLong {
        /// Bytes the identity occupies.
        found: usize,
        /// Configured identity ceiling.
        maximum: usize,
    },
    /// A 1.1 field appeared on a 1.0 frame.
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
/// [`WorkerFrameError::EmptyRequestId`],
/// [`WorkerFrameError::NonAsciiRequestId`],
/// [`WorkerFrameError::RequestIdTooLong`], or
/// [`WorkerFrameError::ExtensionRequiresVersion`] when the named boundary
/// invariant fails.
pub fn parse_worker_request(bytes: &[u8]) -> Result<WorkerRequestFrame, WorkerFrameError> {
    validate_frame_bytes(bytes)?;
    let frame: WorkerRequestFrame =
        serde_json::from_slice(bytes).map_err(WorkerFrameError::Malformed)?;
    validate_frame_identity(frame.protocol_version(), frame.request_id())?;
    if let Some(active_request_id) = frame.active_request_id() {
        validate_request_identity(active_request_id)?;
    }
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
/// [`WorkerFrameError::EmptyRequestId`],
/// [`WorkerFrameError::NonAsciiRequestId`],
/// [`WorkerFrameError::RequestIdTooLong`], or
/// [`WorkerFrameError::InvalidProgress`] when the named boundary invariant
/// fails.
pub fn parse_worker_response(bytes: &[u8]) -> Result<WorkerResponseFrame, WorkerFrameError> {
    validate_frame_bytes(bytes)?;
    let frame: WorkerResponseFrame =
        serde_json::from_slice(bytes).map_err(WorkerFrameError::Malformed)?;
    validate_frame_identity(frame.protocol_version(), frame.request_id())?;
    if let Some(active_request_id) = frame.active_request_id() {
        validate_request_identity(active_request_id)?;
    }
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

/// Publishes the three rules this build applies to a correlation identity.
///
/// [`validate_request_identity`] refuses an empty one, because a refusal the
/// supervisor cannot correlate is a refusal it reports as a timeout, and one
/// past [`MAX_WORKER_REQUEST_ID_BYTES`], because an identity that cannot fit in
/// the answer to its own frame cannot be correlated either. ASCII makes JSON
/// Schema's character count and both runtimes' UTF-8 byte counts the same unit.
/// Published rather than left to be discovered: a supervisor reads the ceiling
/// here instead of from the first refusal it cannot match.
fn request_id_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_WORKER_REQUEST_ID_BYTES,
        "pattern": r"^[\x00-\x7F]+$(?![\s\S])",
    })
}

/// Publishes the format constraints [`Revision`] enforces at deserialization.
///
/// Moving branch names are still refused by [`Revision`] at runtime. JSON
/// Schema's portable regular-expression vocabulary cannot express that
/// case-insensitive denylist without a negative lookaround, which this
/// repository deliberately excludes from published patterns.
fn revision_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_REVISION_BYTES,
        "pattern": r"^[\x21-\x7E]+$(?![\s\S])",
    })
}

fn nonempty_voice_profile_hashes_json_schema(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    let value_schema = generator.subschema_for::<VoiceProfileHash>();
    schemars::json_schema!({
        "type": "object",
        "minProperties": 1,
        "additionalProperties": value_schema,
    })
}

fn validate_frame_identity(version: &str, request_id: &str) -> Result<(), WorkerFrameError> {
    validate_version(version)?;
    validate_request_identity(request_id)
}

fn validate_request_identity(request_id: &str) -> Result<(), WorkerFrameError> {
    if request_id.is_empty() {
        return Err(WorkerFrameError::EmptyRequestId);
    }
    if !request_id.is_ascii() {
        return Err(WorkerFrameError::NonAsciiRequestId);
    }
    if request_id.len() > MAX_WORKER_REQUEST_ID_BYTES {
        return Err(WorkerFrameError::RequestIdTooLong {
            found: request_id.len(),
            maximum: MAX_WORKER_REQUEST_ID_BYTES,
        });
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
        MAX_WORKER_REQUEST_ID_BYTES, WORKER_PROTOCOL_EXTENSION_VERSION, WorkerFrameError,
        WorkerResponseFrame, parse_worker_request, parse_worker_response,
    };

    fn complete_initialized_response() -> serde_json::Value {
        serde_json::json!({
            "event": "initialized",
            "protocol_version": "e1.worker.1.0",
            "request_id": "request-1",
            "identities": {
                "model_revision": "model-v1",
                "tokenizer_revision": "tokenizer-v1",
                "worker_bundle_hash": "1".repeat(64),
                "voice_profile_hashes": {
                    "synthetic-test-voice-v1": "2".repeat(64),
                },
            },
        })
    }

    #[test]
    fn t1_e1_complete_initialized_response_parses() {
        let frame = parse_worker_response(
            &serde_json::to_vec(&complete_initialized_response())
                .expect("the initialized response serializes"),
        )
        .expect("complete typed initialization identities must parse");

        assert!(matches!(frame, WorkerResponseFrame::Initialized { .. }));
    }

    #[test]
    fn t1_e1_initialized_response_requires_every_identity_category() {
        for missing in [
            "model_revision",
            "tokenizer_revision",
            "worker_bundle_hash",
            "voice_profile_hashes",
        ] {
            let mut frame = complete_initialized_response();
            frame["identities"]
                .as_object_mut()
                .expect("the fixture identities are an object")
                .remove(missing);

            let error = parse_worker_response(
                &serde_json::to_vec(&frame).expect("the initialized response serializes"),
            )
            .expect_err("an initialized response missing an identity must be refused");

            assert!(
                matches!(error, WorkerFrameError::Malformed(_)),
                "missing `{missing}` must be a malformed frame, got {error}"
            );
        }
    }

    #[test]
    fn t1_e1_initialized_response_refuses_each_malformed_identity_category() {
        let cases = [
            ("model_revision", serde_json::json!("main")),
            (
                "tokenizer_revision",
                serde_json::json!("tokenizer revision"),
            ),
            ("worker_bundle_hash", serde_json::json!("abc")),
            (
                "voice_profile_hashes",
                serde_json::json!({"synthetic-test-voice-v1": "abc"}),
            ),
        ];

        for (malformed, value) in cases {
            let mut frame = complete_initialized_response();
            frame["identities"][malformed] = value;

            let error = parse_worker_response(
                &serde_json::to_vec(&frame).expect("the initialized response serializes"),
            )
            .expect_err("an initialized response naming a malformed identity must be refused");

            assert!(
                matches!(error, WorkerFrameError::Malformed(_)),
                "malformed `{malformed}` must be refused at the typed boundary, got {error}"
            );
        }
    }

    #[test]
    fn t1_e1_initialized_response_refuses_unknown_or_empty_identity_data() {
        let mut unknown = complete_initialized_response();
        unknown["identities"]["backend_revision"] = serde_json::json!("fake-v1");
        let unknown_error = parse_worker_response(
            &serde_json::to_vec(&unknown).expect("the initialized response serializes"),
        )
        .expect_err("unknown initialization identity data must be refused");
        assert!(matches!(unknown_error, WorkerFrameError::Malformed(_)));

        let mut empty = complete_initialized_response();
        empty["identities"]["voice_profile_hashes"] = serde_json::json!({});
        let empty_error = parse_worker_response(
            &serde_json::to_vec(&empty).expect("the initialized response serializes"),
        )
        .expect_err("an empty voice-profile identity set must be refused");
        assert!(matches!(empty_error, WorkerFrameError::Malformed(_)));
    }

    /// A repeated voice-profile name is refused rather than resolved.
    ///
    /// The mutation check for [`crate::distinct_map`] on this field: with a
    /// derived `BTreeMap` back in its place the frame below parses, keeping
    /// whichever digest the sender wrote last. That digest is the ADR-0001
    /// §12.5 conditioning identity recorded for the name, so a worker naming
    /// one profile twice would otherwise choose it. Every other object in a
    /// frame is a struct, where `serde` already refuses a repeated field.
    ///
    /// The message is asserted, not just the variant, for the reason
    /// `t1_e1_a_frame_naming_an_identity_that_is_not_a_digest_is_refused`
    /// gives: `Malformed` is also what a typo in this literal would produce.
    #[test]
    fn t1_e1_a_response_naming_one_voice_profile_twice_is_refused() {
        let frame = format!(
            concat!(
                r#"{{"event":"initialized","protocol_version":"e1.worker.1.0","#,
                r#""request_id":"request-1","identities":{{"#,
                r#""model_revision":"model-v1","tokenizer_revision":"tokenizer-v1","#,
                r#""worker_bundle_hash":"{bundle}","voice_profile_hashes":{{"#,
                r#""synthetic-test-voice-v1":"{first}","#,
                r#""synthetic-test-voice-v1":"{second}"}}}}}}"#,
            ),
            bundle = "1".repeat(64),
            first = "2".repeat(64),
            second = "3".repeat(64),
        );

        let error = parse_worker_response(frame.as_bytes())
            .expect_err("a response naming one voice profile twice must be refused");

        assert!(
            matches!(&error, WorkerFrameError::Malformed(_)) && error.to_string().contains("twice"),
            "the refusal must name the repeated binding, not some other frame fault: {error}"
        );
    }

    #[test]
    fn t1_e0_worker_protocol_1_0_refuses_explicit_null_trace_context() {
        let error = parse_worker_request(
            concat!(
                r#"{"method":"synthesize","protocol_version":"e1.worker.1.0","#,
                r#""request_id":"request-1","parameters":{"text":"reviewed","#,
                r#""voice":"voice-1","style":"calm","seed":7,"take":0,"#,
                r#""output":"request-1.wav","trace_context":null}}"#,
            )
            .as_bytes(),
        )
        .expect_err("an explicitly present 1.1 extension must be refused by protocol 1.0");

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
                r#"{"method":"initialize","protocol_version":"e1.worker.1.0","#,
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
                r#"{"event":"synthesis_succeeded","protocol_version":"e1.worker.1.0","#,
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

    #[test]
    fn t1_e1_cancelled_active_request_id_past_the_ceiling_is_refused() {
        let frame = serde_json::json!({
            "event": "cancelled",
            "protocol_version": "e1.worker.1.0",
            "request_id": "cancel-request",
            "active_request_id": "r".repeat(MAX_WORKER_REQUEST_ID_BYTES + 1),
        });

        let error = parse_worker_response(
            &serde_json::to_vec(&frame).expect("the cancellation response serializes"),
        )
        .expect_err("an oversized echoed cancellation identity must be refused");

        assert!(matches!(error, WorkerFrameError::RequestIdTooLong { .. }));
    }
}
