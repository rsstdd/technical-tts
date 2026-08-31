//! Builds one lesson into a private preview: gating, planning, synthesis
//! caching, assembly, encoding, and the manifest that records what was
//! produced.
//!
//! Every gate runs before any tool or synthesis work, so a refusal names the
//! policy that refused rather than the first thing that happened to break.

mod assembly;
mod audio_edges;
mod cache;
mod cache_port;
mod distinct_map;
mod durable;
mod error;
mod export;
mod job_repository;
mod locking;
mod managed;
mod manifest;
mod model_gate;
mod package_port;
mod pipeline;
mod preview;
mod process;
mod schemas;
mod synthesis;
mod tools;
mod voice_gate;
mod worker_bundle;
mod worker_client;
mod worker_environment;
mod worker_executor;
mod worker_launcher;
mod worker_protocol;

pub use audio_edges::{
    CalibrationSource, EdgeConditioning, MAX_SEGMENT_AUDIO_MS, MAX_TRANSITION_RAMP_MS,
    ProvisionalCalibration, REQUIRED_EDGE_SILENCE_MS, SilenceThreshold, condition_edges,
    measure_edge_silence, samples_for,
};
pub use cache::ValidatedCachedArtifact;
pub use cache_port::{
    CACHE_PUBLICATION_CONTRACT_VERSION, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, StagedAudioProducer,
};
pub use error::{
    AudioError, AudioFault, BuildError, CacheEntryFault, CacheError, ConditioningContradiction,
    DurableStateError, EnvironmentMismatch, IoError, ManagedPathError, ModelArtifactError,
    PackageArtifactMismatch, PublicationError, RemedyAdvice, RemedyOwner, RightsError,
    RuntimeIdentityMismatch, ToolError, ToolInvocation, ToolOperation, ToolOutputStream,
    VoiceProfileError, WorkerBundleError, WorkerLockfileErrorReason, WorkerLockfileLocus,
    WorkerRequirementFault,
};
pub use job_repository::{
    FileSystemJobRepository, JOB_STATE_CONTRACT_VERSION, JobOwnership, JobRepository,
};
pub use model_gate::{
    DECLARED_MODEL_ARTIFACTS, DeclaredArtifact, PINNED_MODEL_REVISION, verify_model_artifacts,
};
pub use package_port::{
    FileSystemPackageWriter, PACKAGE_WRITER_CONTRACT_VERSION, PackagePreflightRequest,
    PackagePrepareRequest, PackagePublication, PackageWriteRequest, PackageWriter,
    PreparedPackageWriter,
};
pub use pipeline::{
    BuildRequest, BuildResult, PreviewServiceBundle, build_preview, build_preview_with_services,
    publish, validate_encoded_output, validate_production_manifest,
};
pub use schemas::{
    JOB_SCHEMA_VERSION, MANIFEST_SCHEMA_VERSION, PUBLISHED_SCHEMAS, PublishedSchema,
    SCHEMA_DIRECTORY, WORKER_PROTOCOL_SCHEMA_VERSION,
};
pub use synthesis::{
    BackendDescriptor, BackendError, BackendValidationError, DriftedIdentity, SynthesisReport,
    SynthesisRequest, TTS_EXECUTOR_CONTRACT_VERSION, TtsExecutor, validate_executor_request,
};
pub use voice_gate::resolve_voice_conditioning;
pub use worker_bundle::{
    BUNDLE_MANIFEST_PATH, BUNDLE_MANIFEST_SCHEMA_VERSION, BundleManifest, DeclaredStartupModule,
    MAX_BUNDLE_INPUT_BYTES, PythonRuntimeIdentity, REQUIRED_BUNDLE_INPUTS, REQUIRED_IMPORT_ROOT,
    StartupModuleName, WORKER_BUNDLE_IDENTITY_VERSION, WORKER_ENTRY_MODULE, WORKER_ENTRYPOINT_PATH,
    WORKER_LAUNCHER_PATH, WORKER_LOCKFILE_PATH, WORKER_PACKAGE_ROOT, WORKER_PROTOCOL_SCHEMA_PATH,
    WORKER_REQUIREMENTS_PATH, WorkerBundle,
};
pub use worker_environment::WORKER_INTERPRETER_PATH;
pub use worker_executor::{
    PROTOCOL_FAKE_BUNDLE_HASH, WORKER_INITIALIZE_DEADLINE, WORKER_REQUEST_DEADLINE,
    WorkerConfiguration, WorkerTtsExecutor,
};
pub use worker_launcher::{LAUNCHER_SCHEMA_VERSION, THREAD_ENVIRONMENT, WorkerLauncher};
pub use worker_protocol::{
    InitializeParameters, MAX_WORKER_FRAME_BYTES, MAX_WORKER_REQUEST_ID_BYTES, TraceContext,
    WORKER_PROTOCOL_EXTENSION_VERSION, WORKER_PROTOCOL_VERSION, WorkerCapabilities,
    WorkerFailureCode, WorkerFrame, WorkerFrameError, WorkerInitializationIdentities,
    WorkerRequestFrame, WorkerResponseFrame, WorkerSynthesisParameters, parse_worker_request,
    parse_worker_response,
};

/// Re-exported at the root so every module keeps constructing these the same
/// way, rather than half of them reaching into `error` directly.
pub(crate) use error::{audio_error, io_error};
