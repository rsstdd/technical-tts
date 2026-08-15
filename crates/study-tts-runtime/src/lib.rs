mod assembly;
mod cache;
mod export;
mod manifest;
mod pipeline;
mod synthesis;
mod tools;

pub use pipeline::{
    BuildRequest, BuildResult, build_preview, publish, validate_production_manifest,
};
pub use synthesis::{SegmentSynthesizer, SynthesisError, SynthesisReport};

use std::{io, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("could not read `{path}`: {source}")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error(transparent)]
    Lesson(#[from] study_tts_core::LessonError),
    #[error("filesystem operation failed for `{path}`: {source}")]
    FileSystem { path: PathBuf, source: io::Error },
    #[error("audio operation failed: {0}")]
    Audio(#[from] hound::Error),
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
    #[error("manifest serialization failed: {0}")]
    Manifest(#[from] serde_json::Error),
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    BuildError::FileSystem {
        path: path.into(),
        source,
    }
}
