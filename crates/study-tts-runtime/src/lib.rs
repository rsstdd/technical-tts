//! Builds one lesson into a private preview: gating, planning, synthesis
//! caching, assembly, encoding, and the manifest that records what was
//! produced.
//!
//! Every gate runs before any tool or synthesis work, so a refusal names the
//! policy that refused rather than the first thing that happened to break.

mod assembly;
mod cache;
mod cache_port;
mod durable;
mod error;
mod export;
mod job_repository;
mod locking;
mod managed;
mod manifest;
mod package_port;
mod pipeline;
mod preview;
mod process;
mod synthesis;
mod tools;
mod voice_gate;
mod worker_protocol;

pub use cache::ValidatedCachedArtifact;
pub use cache_port::{
    CACHE_PUBLICATION_CONTRACT_VERSION, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, StagedAudioProducer,
};
pub use error::{
    AudioError, AudioFault, BuildError, CacheEntryFault, CacheError, DurableStateError, IoError,
    ManagedPathError, PackageArtifactMismatch, PublicationError, RemedyAdvice, RemedyOwner,
    RightsError, ToolError, ToolInvocation, ToolOperation, ToolOutputStream, VoiceProfileError,
};
pub use job_repository::{
    FileSystemJobRepository, JOB_STATE_CONTRACT_VERSION, JobOwnership, JobRepository,
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
pub use synthesis::{
    BackendDescriptor, BackendError, BackendValidationError, SynthesisReport, SynthesisRequest,
    TTS_EXECUTOR_CONTRACT_VERSION, TtsExecutor, validate_executor_request,
};
pub use worker_protocol::{
    InitializeParameters, MAX_WORKER_FRAME_BYTES, TraceContext, WORKER_PROTOCOL_EXTENSION_VERSION,
    WORKER_PROTOCOL_VERSION, WorkerCapabilities, WorkerFailureCode, WorkerFrameError,
    WorkerRequestFrame, WorkerResponseFrame, WorkerSynthesisParameters, parse_worker_request,
    parse_worker_response,
};

/// Re-exported at the root so every module keeps constructing these the same
/// way, rather than half of them reaching into `error` directly.
pub(crate) use error::{audio_error, io_error};
