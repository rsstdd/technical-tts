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

    /// The manifest records a take selection nobody reviewed.
    ///
    /// ADR-0001 §12.2 requires an explicit versioned takes file for a
    /// production release "even when every selection remains zero", and this is
    /// the published half of that rule: `study_tts_core::TakeSelectionSource`
    /// carries the distinction through a build, and this refuses a manifest
    /// that records the generated one. An absent field is read as generated,
    /// because a manifest that says nothing about how it was selected has not
    /// shown that anyone selected it.
    #[error(
        "production release is refused: the manifest records take selection `{declared}`, and \
         ADR-0001 §12.2 requires an explicit versioned takes file even when every selection \
         remains zero; the project owner must accept a takes file for this build and publish a \
         manifest from the build that read it"
    )]
    ImplicitTakeSelection {
        /// What the manifest recorded, or `implicit` where it recorded nothing.
        declared: &'static str,
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
            // No §Failure routing row names a manifest that claims a status it
            // did not earn, so this advice names its owner and no row. It read
            // "Production publication" until the routing rows were mechanized:
            // that is a §Decision routing row, which names who decides a
            // publication rather than who repairs a refused one.
            // `ImplicitTakeSelection` joins them for the same reason. The
            // nearest §Failure routing row, "Human review finding", names a
            // human-review owner this enum has no variant for, and inventing
            // one to carry a row that does not describe this refusal would be
            // worse than naming the owner in the message.
            Self::Release(ReleaseError::PrivateProfileCannotClaimProduction)
            | Self::MalformedProductionManifest { .. }
            | Self::ImplicitTakeSelection { .. }
            | Self::ManifestNotProductionRelease { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "publish a corrected manifest from a build that earned production status",
                None,
            )),
            Self::UnsupportedProductionManifest { .. } => None,
        }
    }
}
