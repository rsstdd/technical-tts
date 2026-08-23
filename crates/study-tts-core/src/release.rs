use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Gates a production release must satisfy.
///
/// Transcribed from ADR-0001 §18 and mirrored in `docs/governance/RELEASE-PROFILES.md` §3. The
/// two must agree, and changing either requires an ADR amendment rather than an edit.
///
/// ASR calibration is deliberately absent. ADR-0001 §17.18 and §18 stated different ASR release
/// conditions; version 1.0 adopts the §18 condition, recorded in
/// `docs/adr/deviations/ADR-0001-D001-asr-release-condition.md`.
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
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseStatus {
    PrivatePreview,
    ProductionRelease,
}

#[derive(Debug, Error)]
pub enum ReleaseError {
    #[error("a private preview cannot report production release")]
    PrivateProfileCannotClaimProduction,
    #[error("production release is missing gate evidence: {0}")]
    MissingGateEvidence(String),
}

/// A release profile together with the gate evidence recorded for it.
#[derive(Clone, Debug)]
pub struct ReleaseClaim {
    status: ReleaseStatus,
    satisfied_gates: Vec<String>,
}

impl ReleaseClaim {
    pub fn private_preview() -> Self {
        Self {
            status: ReleaseStatus::PrivatePreview,
            satisfied_gates: Vec::new(),
        }
    }

    pub fn production_release(satisfied_gates: Vec<String>) -> Self {
        Self {
            status: ReleaseStatus::ProductionRelease,
            satisfied_gates,
        }
    }

    pub fn status(&self) -> ReleaseStatus {
        self.status
    }

    /// Accepts the claim only if the profile is production and every required gate has evidence.
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

    fn all_gates() -> Vec<String> {
        REQUIRED_PRODUCTION_GATES
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
        assert_eq!(claim.status(), ReleaseStatus::PrivatePreview);
    }

    #[test]
    fn t3_e0_production_profile_rejects_missing_gate_evidence() {
        for omitted in REQUIRED_PRODUCTION_GATES {
            let gates = all_gates()
                .into_iter()
                .filter(|gate| gate != omitted)
                .collect();

            let error = ReleaseClaim::production_release(gates)
                .validate_as_production()
                .expect_err("a missing gate must be rejected");

            assert!(
                matches!(error, ReleaseError::MissingGateEvidence(ref missing) if missing == omitted),
                "omitting `{omitted}` produced `{error}`"
            );
        }

        ReleaseClaim::production_release(all_gates())
            .validate_as_production()
            .expect("a complete gate set must be accepted");
    }

    #[test]
    fn t3_e0_unknown_release_status_is_rejected() {
        assert_eq!(
            serde_json::from_str::<ReleaseStatus>("\"private_preview\"").expect("known status"),
            ReleaseStatus::PrivatePreview
        );
        assert_eq!(
            serde_json::from_str::<ReleaseStatus>("\"production_release\"").expect("known status"),
            ReleaseStatus::ProductionRelease
        );

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
    fn t3_e0_gate_list_has_no_duplicates() {
        let mut sorted = REQUIRED_PRODUCTION_GATES;
        sorted.sort_unstable();
        let mut unique = sorted.to_vec();
        unique.dedup();

        assert_eq!(
            unique.len(),
            REQUIRED_PRODUCTION_GATES.len(),
            "gate identifiers must be unique"
        );
    }
}
