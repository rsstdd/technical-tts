//! Path-enriched failures from filesystem, WAV, and JSON operations.

use std::{io, path::PathBuf};

use thiserror::Error;

/// Why a path-bound input or output operation failed.
#[derive(Debug, Error)]
pub enum IoError {
    /// An input the build was told to read is not readable.
    #[error("could not read `{path}`: {source}")]
    ReadFile {
        /// The file that could not be read.
        path: PathBuf,
        /// What the filesystem reported.
        source: io::Error,
    },

    /// A filesystem operation the build performs itself failed.
    #[error("filesystem operation failed for `{path}`: {source}")]
    FileSystem {
        /// The path being operated on.
        path: PathBuf,
        /// What the filesystem reported.
        source: io::Error,
    },

    /// An audio read or write failed.
    #[error("audio operation failed for `{path}`: {source}")]
    AudioAt {
        /// The audio file being read or written.
        path: PathBuf,
        /// What the WAV layer reported.
        source: hound::Error,
    },

    /// A record could not be serialized to its destination.
    #[error("could not write JSON to `{path}`: {source}")]
    WriteJson {
        /// The record being written.
        path: PathBuf,
        /// What the serializer reported.
        source: serde_json::Error,
    },
}
