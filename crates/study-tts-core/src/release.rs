//! Release-status claims and production-gate validation.
//!
//! [`REQUIRED_PRODUCTION_GATES`] is transcribed from ADR-0001 §18 and mirrored
//! in `docs/governance/RELEASE-PROFILES.md` §3.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Gates a production release must satisfy.
///
/// Transcribed from ADR-0001 §18 and mirrored in
/// `docs/governance/RELEASE-PROFILES.md` §3, which names this constant in
/// return. Changing either requires an ADR amendment.
///
/// ASR calibration is absent under ADR-0001-D001, which adopts ADR-0001 §18's
/// triage requirement.
pub const REQUIRED_PRODUCTION_GATES: [&str; 12] = [
    "long_form_soak",
    "content_integrity_review",
    "asr_triage_recorded",
    "worker_unloaded_before_verification",
    "explicit_take_selection",
    "frozen_loudness_references",
    "voice_identity_and_format",
    "automated_audio_checks",
    "package_provenance",
    "offline_render_verified",
    "rights_and_licensing",
    "clean_machine_operations",
];

/// What an artifact claims to be.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    /// Rendered and structurally valid, but not verified, approved, or
    /// releasable.
    PrivatePreview,
    /// Every gate in `docs/governance/RELEASE-PROFILES.md` §3 has a passing
    /// evidence record.
    ProductionRelease,
}

impl ReleaseStatus {
    /// The `snake_case` spelling this status carries in a manifest.
    ///
    /// Kept in sync with the serde representation by an exhaustive test.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrivatePreview => "private_preview",
            Self::ProductionRelease => "production_release",
        }
    }
}

/// Why a claim was refused as a production release.
#[derive(Debug, Error)]
pub enum ReleaseError {
    /// A private-preview claim entered the production publication path.
    #[error(
        "release claim is `private_preview`, not `production_release`; the project owner must \
         preserve the preview and use the production publication workflow with complete gate \
         evidence"
    )]
    PrivateProfileCannotClaimProduction,
    /// One or more required gates lack evidence records.
    #[error(
        "production release is missing gate evidence: {0}; the gate owners must record passing \
         evidence before the project owner retries publication"
    )]
    MissingGateEvidence(String),
}

/// A release profile together with the gate evidence recorded for it.
#[derive(Clone, Debug)]
pub struct ReleaseClaim {
    status: ReleaseStatus,
    satisfied_gates: Vec<String>,
}

impl ReleaseClaim {
    /// The claim every build starts as: rendered, with no gate evidence.
    pub fn private_preview() -> Self {
        Self {
            status: ReleaseStatus::PrivatePreview,
            satisfied_gates: Vec::new(),
        }
    }

    /// A production-release claim backed by the given gate evidence.
    pub fn production_release(satisfied_gates: Vec<String>) -> Self {
        Self {
            status: ReleaseStatus::ProductionRelease,
            satisfied_gates,
        }
    }

    /// Accepts the claim only if the profile is production and every required
    /// gate has evidence.
    ///
    /// # Errors
    ///
    /// [`ReleaseError::PrivateProfileCannotClaimProduction`] tells the project
    /// owner to preserve a private preview, while
    /// [`ReleaseError::MissingGateEvidence`] gives the project owner every gate
    /// whose owner must record passing evidence.
    pub fn validate_as_production(&self) -> Result<(), ReleaseError> {
        if self.status != ReleaseStatus::ProductionRelease {
            return Err(ReleaseError::PrivateProfileCannotClaimProduction);
        }

        let missing: Vec<&str> = REQUIRED_PRODUCTION_GATES
            .iter()
            .copied()
            .filter(|gate| !self.satisfied_gates.iter().any(|held| held == gate))
            .collect();
        if !missing.is_empty() {
            return Err(ReleaseError::MissingGateEvidence(missing.join(", ")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent policy copy: deriving this from the implementation would
    // let a deleted gate leave the test green.
    const EXPECTED_PRODUCTION_GATES: [&str; 12] = [
        "long_form_soak",
        "content_integrity_review",
        "asr_triage_recorded",
        "worker_unloaded_before_verification",
        "explicit_take_selection",
        "frozen_loudness_references",
        "voice_identity_and_format",
        "automated_audio_checks",
        "package_provenance",
        "offline_render_verified",
        "rights_and_licensing",
        "clean_machine_operations",
    ];

    fn all_gates() -> Vec<String> {
        EXPECTED_PRODUCTION_GATES
            .iter()
            .map(|gate| (*gate).to_owned())
            .collect()
    }

    #[test]
    fn t3_e0_private_profile_cannot_report_production_release() {
        let claim = ReleaseClaim {
            status: ReleaseStatus::PrivatePreview,
            satisfied_gates: all_gates(),
        };

        assert!(matches!(
            claim.validate_as_production(),
            Err(ReleaseError::PrivateProfileCannotClaimProduction)
        ));
    }

    #[test]
    fn t3_e0_production_profile_rejects_missing_gate_evidence() {
        for omitted in EXPECTED_PRODUCTION_GATES {
            let gates = all_gates()
                .into_iter()
                .filter(|gate| gate != omitted)
                .collect();

            let error = ReleaseClaim::production_release(gates)
                .validate_as_production()
                .expect_err("a missing gate must be rejected");

            assert!(
                matches!(
                    error,
                    ReleaseError::MissingGateEvidence(ref missing) if missing == omitted
                ),
                "omitting `{omitted}` produced `{error}`"
            );
        }

        ReleaseClaim::production_release(all_gates())
            .validate_as_production()
            .expect("a complete gate set must be accepted");
    }

    #[test]
    fn t1_e0_release_refusals_name_the_remedy_owner() {
        let private = ReleaseClaim::private_preview()
            .validate_as_production()
            .expect_err("a private preview must be refused");
        let missing = ReleaseClaim::production_release(Vec::new())
            .validate_as_production()
            .expect_err("missing gate evidence must be refused");

        assert!(private.to_string().contains("project owner"));
        assert!(missing.to_string().contains("gate owners"));
    }

    #[test]
    fn t3_e0_release_status_spellings_match_their_serde_representation() {
        for status in [
            ReleaseStatus::PrivatePreview,
            ReleaseStatus::ProductionRelease,
        ] {
            let spelling = match status {
                ReleaseStatus::PrivatePreview => "private_preview",
                ReleaseStatus::ProductionRelease => "production_release",
            };

            assert_eq!(
                serde_json::to_value(status).expect("unit variant serializes"),
                serde_json::Value::String(spelling.to_owned()),
                "`{status:?}` serde representation drifted"
            );
            assert_eq!(
                status.as_str(),
                spelling,
                "`{status:?}` spelling drifted from its serde representation"
            );
        }
    }

    #[test]
    fn t3_e0_unknown_release_status_is_rejected() {
        for unknown in [
            "\"public\"",
            "\"released\"",
            "\"preview\"",
            "\"PrivatePreview\"",
            "\"private-preview\"",
            "\"\"",
            "null",
            "0",
        ] {
            assert!(
                serde_json::from_str::<ReleaseStatus>(unknown).is_err(),
                "`{unknown}` must be rejected"
            );
        }
    }

    #[test]
    fn t3_e0_required_gates_match_the_release_profile_document() {
        assert_eq!(
            REQUIRED_PRODUCTION_GATES, EXPECTED_PRODUCTION_GATES,
            "REQUIRED_PRODUCTION_GATES no longer matches RELEASE-PROFILES.md §3"
        );
    }
}
