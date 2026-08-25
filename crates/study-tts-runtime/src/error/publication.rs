//! Release-profile and production-manifest refusals outside rights
//! declarations.

use study_tts_core::{ReleaseError, ReleaseStatus};
use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why the requested production publication could not proceed.
#[derive(Debug, Error)]
pub enum PublicationError {
    /// A release profile refused the claim made on it.
    #[error(transparent)]
    Release(#[from] ReleaseError),

    /// A manifest uses a version this build cannot evaluate for production.
    #[error("manifest version `{version}` is not a production manifest")]
    UnsupportedProductionManifest {
        /// The version the manifest declares.
        version: String,
    },

    /// The manifest is not JSON or not the shape its version requires.
    #[error(
        "production release is refused: the manifest is not a valid production manifest \
         ({source}); the project owner must correct the manifest before publication"
    )]
    MalformedProductionManifest {
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// The manifest does not declare production-release status.
    #[error(
        "production release is refused: the manifest declares release status `{}`, not \
         `production_release`; the project owner must publish the manifest of a build that \
         earned the production profile rather than restate the status",
        declared.as_str()
    )]
    ManifestNotProductionRelease {
        /// The status the manifest declares.
        declared: ReleaseStatus,
    },

    /// Production gates are not available in this walking-skeleton phase.
    #[error(
        "production release is refused: manifest acceptance is unavailable before the \
         production gates of `docs/governance/RELEASE-PROFILES.md` §3 are implemented"
    )]
    ProductionGatesUnavailable,
}

impl PublicationError {
    /// Returns governed recovery advice when this publication refusal has an
    /// owner.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::Release(ReleaseError::MissingGateEvidence(_))
            | Self::ProductionGatesUnavailable => Some(RemedyAdvice::new(
                RemedyOwner::GateOwner,
                "preserve the candidate and create a corrective gate issue",
                Some("Failed release gate"),
            )),
            Self::Release(ReleaseError::PrivateProfileCannotClaimProduction)
            | Self::MalformedProductionManifest { .. }
            | Self::ManifestNotProductionRelease { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "publish a corrected manifest from a build that earned production status",
                Some("Production publication"),
            )),
            Self::UnsupportedProductionManifest { .. } => None,
        }
    }
}
