mod assembly;
mod cache;
mod export;
mod manifest;
mod pipeline;
mod synthesis;
mod tools;
mod voice_gate;

pub use pipeline::{
    BuildRequest, BuildResult, build_preview, publish, validate_encoded_output,
    validate_production_manifest,
};
pub use synthesis::{SegmentSynthesizer, SynthesisError, SynthesisReport};

use std::{
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("could not read `{path}`: {source}")]
    ReadFile { path: PathBuf, source: io::Error },

    #[error(transparent)]
    Lesson(#[from] study_tts_core::LessonError),

    #[error(transparent)]
    Voice(#[from] study_tts_core::VoiceError),

    // Remedy routing per `docs/governance/ROUTING-TABLES.md` ("Voice consent/checksum mismatch
    // → Refuse profile load → Project owner → Blocked"): the owner resolves the record; the
    // profile is never deleted or repaired automatically.
    //
    // `profile.json` and `consent.json` share this variant because they are the same class of
    // refusal — a record the policy requires is absent — and an absent profile record deserves
    // the same remedy-bearing message as an absent consent record, not a bare IO error.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is missing; \
         profile load fails closed and the project owner must supply the record before use"
    )]
    MissingVoiceRecord {
        profile_dir: PathBuf,
        record: &'static str,
    },

    #[error(
        "voice profile at `{profile_dir}` is refused: `{path}` does not match its recorded \
         checksum; do not use this profile until the project owner re-verifies it against its \
         rights record"
    )]
    VoiceChecksumMismatch { profile_dir: PathBuf, path: PathBuf },

    #[error(
        "production release is refused: source `{source_id}` has unresolved rights \
         classification `{classification}`; the project owner must resolve the classification in \
         its rights record before publication"
    )]
    UnresolvedContentRights {
        source_id: String,
        classification: String,
    },

    /// A manifest's rights section does not parse — an unknown classification, a missing field,
    /// or the wrong shape.
    ///
    /// Distinct from the `Json` catch-all below: on the publication path the section is known
    /// and worth naming, and an operator reading "JSON operation failed" would not learn that
    /// their manifest declared a classification outside the recorded vocabulary.
    #[error(
        "production release is refused: the `{section}` manifest section is not a valid rights \
         declaration ({source}); the project owner must correct the manifest before publication"
    )]
    InvalidRightsDeclaration {
        section: &'static str,
        source: serde_json::Error,
    },

    #[error("filesystem operation failed for `{path}`: {source}")]
    FileSystem { path: PathBuf, source: io::Error },

    #[error("audio operation failed for `{path}`: {source}")]
    AudioAt { path: PathBuf, source: hound::Error },

    #[error(transparent)]
    Synthesis(#[from] SynthesisError),

    #[error("cache artifact is invalid: {0}")]
    InvalidCache(String),

    #[error("FFmpeg failed with status {status}: {stderr}")]
    Ffmpeg { status: String, stderr: String },

    #[error("could not start FFmpeg `{executable}`: {source}")]
    StartFfmpeg {
        executable: PathBuf,
        source: io::Error,
    },

    #[error("required tool {tool} was not found or is not executable at `{requested}`")]
    MissingTool { tool: String, requested: PathBuf },

    #[error("could not inspect {tool} at `{executable}`: {source}")]
    InspectTool {
        tool: String,
        executable: PathBuf,
        source: io::Error,
    },

    #[error("{tool} version probe failed with status {status}: {stderr}")]
    ToolProbeFailed {
        tool: String,
        status: String,
        stderr: String,
    },

    #[error("ffprobe failed with status {status}: {stderr}")]
    Ffprobe { status: String, stderr: String },

    #[error("encoded output failed structural validation: {0}")]
    InvalidEncodedOutput(String),

    #[error("managed path `{path}` resolves outside `{root}`")]
    ManagedPathEscape { path: PathBuf, root: PathBuf },

    #[error("production publication is refused: {reason}")]
    PublicationRefused { reason: String },

    #[error("manifest version `{version}` is not a production manifest")]
    UnsupportedProductionManifest { version: String },

    #[error("could not write JSON to `{path}`: {source}")]
    WriteJson {
        path: PathBuf,
        source: serde_json::Error,
    },

    /// Catch-all for `?` on a `serde_json` call with no useful context to add. Prefer a variant
    /// that carries the path or the subsystem; this exists so a future call site cannot silently
    /// inherit an unrelated error message the way the former `Manifest` variant did.
    #[error("JSON operation failed: {0}")]
    Json(#[from] serde_json::Error),
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    BuildError::FileSystem {
        path: path.into(),
        source,
    }
}

/// `hound::Error` wraps `io::Error`, which carries no filename, so every audio failure must be
/// given its path here or the message names nothing.
pub(crate) fn audio_error(path: impl Into<PathBuf>, source: hound::Error) -> BuildError {
    BuildError::AudioAt {
        path: path.into(),
        source,
    }
}

/// Directory holding one cache entry.
///
/// Exposed so integration tests can corrupt a specific entry without duplicating the sharding
/// scheme. Changing the shard width in `cache::entry_dir` updates the tests automatically.
#[doc(hidden)]
pub fn cache_entry_dir(cache_root: &Path, cache_key: &str) -> PathBuf {
    cache::entry_dir(cache_root, cache_key)
}
