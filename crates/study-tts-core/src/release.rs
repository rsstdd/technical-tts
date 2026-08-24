//! What an artifact may claim to be, and the gates a production claim has to
//! satisfy.
//!
//! [`REQUIRED_PRODUCTION_GATES`] is transcribed from ADR-0001 §18 and mirrored
//! in `docs/governance/RELEASE-PROFILES.md` §3. The claim is checked in one
//! place so that nothing which writes a manifest can re-derive a weaker
//! version of it.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Gates a production release must satisfy.
///
/// Transcribed from ADR-0001 §18 and mirrored in
/// `docs/governance/RELEASE-PROFILES.md` §3. The two must agree, and changing
/// either requires an ADR amendment rather than an edit.
///
/// ASR calibration is deliberately absent. ADR-0001 §17.18 and §18 stated
/// different ASR release conditions; version 1.0 adopts the §18 condition,
/// recorded in `docs/adr/deviations/ADR-0001-D001-asr-release-condition.md`.
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
    /// Mirrors the serde representation above so a refusal quotes what the
    /// manifest actually declares. The exhaustive match makes a new variant a
    /// compile error rather than a silent fallback string, and
    /// `t3_e0_release_status_spellings_match_their_serde_representation` proves
    /// the two agree.
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
    /// The artifact claims a profile it did not earn; only `publish` produces a
    /// release.
    #[error("a private preview cannot report production release")]
    PrivateProfileCannotClaimProduction,
    /// A required gate has no evidence record, named so the owner knows which
    /// one to run.
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

    /// The profile this claim asserts.
    pub fn status(&self) -> ReleaseStatus {
        self.status
    }

    /// Accepts the claim only if the profile is production and every required
    /// gate has evidence.
    ///
    /// # Errors
    ///
    /// [`ReleaseError::PrivateProfileCannotClaimProduction`] when the claim is
    /// not a production one, and [`ReleaseError::MissingGateEvidence`] naming
    /// every gate that has no evidence — all of them, not the first, so the
    /// owner learns the whole remaining list in one refusal.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use study_tts_core::{ReleaseClaim, ReleaseError};
    /// use study_tts_core::REQUIRED_PRODUCTION_GATES;
    ///
    /// assert!(matches!(
    ///     ReleaseClaim::private_preview().validate_as_production(),
    ///     Err(ReleaseError::PrivateProfileCannotClaimProduction)
    /// ));
    ///
    /// let every_gate = REQUIRED_PRODUCTION_GATES
    ///     .iter()
    ///     .map(|gate| (*gate).to_owned())
    ///     .collect();
    /// assert!(
    ///     ReleaseClaim::production_release(every_gate)
    ///         .validate_as_production()
    ///         .is_ok()
    /// );
    ///
    /// let no_evidence = ReleaseClaim::production_release(Vec::new());
    /// assert!(matches!(
    ///     no_evidence.validate_as_production(),
    ///     Err(ReleaseError::MissingGateEvidence(_))
    /// ));
    /// ```
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

    /// The gate identifiers this crate is expected to require, transcribed
    /// independently from `docs/governance/RELEASE-PROFILES.md` §3 rather than
    /// read from `REQUIRED_PRODUCTION_GATES`. A test that derives its cases
    /// from the implementation drops a case whenever a gate is dropped, and
    /// stays green while the policy shrinks. Update this table only alongside
    /// an ADR amendment that changes §3.
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
        assert_eq!(claim.status(), ReleaseStatus::PrivatePreview);
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

    /// Every status, so a spelling cannot go untested. The match inside the
    /// test is what makes a new variant a compile error; this array only says
    /// which values to run it over.
    const ALL_RELEASE_STATUSES: [ReleaseStatus; 2] = [
        ReleaseStatus::PrivatePreview,
        ReleaseStatus::ProductionRelease,
    ];

    #[test]
    fn t3_e0_release_status_spellings_match_their_serde_representation() {
        for status in ALL_RELEASE_STATUSES {
            // Transcribed from the serde attribute on the enum rather than read
            // from `as_str`, so this table cannot agree with a wrong spelling.
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
    fn t3_e0_required_gates_match_the_release_profile_document() {
        assert_eq!(
            REQUIRED_PRODUCTION_GATES, EXPECTED_PRODUCTION_GATES,
            "REQUIRED_PRODUCTION_GATES no longer matches RELEASE-PROFILES.md §3"
        );
    }

    #[test]
    fn t3_e0_gate_list_has_no_duplicates() {
        let mut sorted = EXPECTED_PRODUCTION_GATES;
        sorted.sort_unstable();
        let mut unique = sorted.to_vec();
        unique.dedup();

        assert_eq!(
            unique.len(),
            EXPECTED_PRODUCTION_GATES.len(),
            "gate identifiers must be unique"
        );
    }
}
