//! Builds one lesson into a private preview: gating, planning, synthesis
//! caching, assembly, encoding, and the manifest that records what was
//! produced.
//!
//! Every gate runs before any tool or synthesis work, so a refusal names the
//! policy that refused rather than the first thing that happened to break.

mod assembly;
mod cache;
mod error;
mod export;
mod managed;
mod manifest;
mod pipeline;
mod process;
mod synthesis;
mod tools;
mod voice_gate;

pub use error::{
    AudioError, AudioFault, BuildError, CacheEntryFault, CacheError, IoError, ManagedPathError,
    PublicationError, RemedyAdvice, RemedyOwner, RightsError, ToolError, ToolInvocation,
    ToolOperation, ToolOutputStream, VoiceProfileError,
};
pub use pipeline::{
    BuildRequest, BuildResult, build_preview, publish, validate_encoded_output,
    validate_production_manifest,
};
pub use synthesis::{SegmentSynthesizer, SynthesisError, SynthesisReport};

/// Re-exported at the root so every module keeps constructing these the same
/// way, rather than half of them reaching into `error` directly.
pub(crate) use error::{audio_error, io_error};
