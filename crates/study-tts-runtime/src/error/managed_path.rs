//! Refusals at managed-name and managed-path containment boundaries.

use std::path::PathBuf;

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why a managed name or path was refused before access.
#[derive(Debug, Error)]
pub enum ManagedPathError {
    /// A managed name is not exactly one ordinary path element.
    #[error(
        "managed name `{name}` beneath `{root}` is not one ordinary path element; the build \
         created nothing, and the runtime owner must correct the caller that supplied it"
    )]
    InvalidManagedName {
        /// The name that was refused.
        name: String,
        /// The managed directory the name would have been joined beneath.
        root: PathBuf,
    },

    /// A derived or resolved path leaves its managed root.
    #[error("managed path `{path}` resolves outside `{root}`")]
    ManagedPathEscape {
        /// The path that resolved outside its root.
        path: PathBuf,
        /// The root the build is confined to.
        root: PathBuf,
    },

    /// An atomic destination has no parent in which to stage a file.
    #[error(
        "cannot write `{path}` atomically because it has no parent directory; supply an output \
         path with a directory component"
    )]
    UnrootedDestination {
        /// The destination that has no parent directory to stage into.
        path: PathBuf,
    },
}

impl ManagedPathError {
    /// Returns governed recovery advice when runtime containment owns the
    /// refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::InvalidManagedName { .. } | Self::ManagedPathEscape { .. } => {
                Some(RemedyAdvice::new(
                    RemedyOwner::WorkerRuntime,
                    "correct the managed-path caller before rerunning the build",
                    Some("Worker protocol or containment failure"),
                ))
            }
            Self::UnrootedDestination { .. } => None,
        }
    }
}
