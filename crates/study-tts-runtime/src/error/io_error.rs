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

    /// A lesson path resolved to something other than a regular file.
    #[error("lesson input `{path}` is not a regular file")]
    LessonNotRegularFile {
        /// The refused lesson path.
        path: PathBuf,
    },

    /// A publication that must not replace anything found its destination
    /// taken.
    ///
    /// Deliberately unrouted: `docs/governance/ROUTING-TABLES.md` §Failure
    /// routing establishes no owner for an authoring refusal, and the person
    /// who chose the path is the only one who can choose another. The message
    /// carries that remedy rather than inventing a governed one, which is why
    /// `BuildError::Io` returns no `RemedyAdvice`.
    #[error(
        "refusing to overwrite `{path}`: an authored file is never replaced; write the scaffold \
         to a path nothing owns, or move the existing document aside first"
    )]
    DestinationExists {
        /// The destination that already holds a file.
        path: PathBuf,
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
