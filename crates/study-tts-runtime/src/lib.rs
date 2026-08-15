mod assembly;
mod cache;
mod export;
mod manifest;
mod pipeline;
mod synthesis;

pub use pipeline::{BuildRequest, BuildResult, build_preview};
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
    #[error("manifest serialization failed: {0}")]
    Manifest(#[from] serde_json::Error),
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    BuildError::FileSystem {
        path: path.into(),
        source,
    }
}
