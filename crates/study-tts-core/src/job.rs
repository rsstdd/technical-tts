//! Minimal E0 job state shared by orchestration and durable repositories.
//!
//! This deliberately records ownership progress and selected-package identity
//! only. ADR-0001 §12.4 assigns segment attempts, recovery, and resume
//! semantics to E2-S1 rather than this provisional baseline.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records this module's
//! public representation, consumers, fake, identity effects, and G1 status.

use serde::{Deserialize, Serialize};

/// Mirrors the job-state version in the E0-S4 provisional contract baseline.
pub const PROVISIONAL_JOB_SCHEMA_VERSION: &str = "e0.job-state.0.1";

/// The E0 progress record that does not claim the complete E2 state machine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvisionalJobStage {
    /// A validated lesson has a deterministic render plan.
    Planned,
    /// Cache lookup and staged-audio publication are in progress.
    Caching,
    /// Validated cached artifacts are ready for package writing.
    Packaging,
    /// An immutable package has been selected for preview consumers.
    PackageSelected,
}

/// Immutable package identity retained in the selected job state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SelectedPackageIdentity {
    /// Content identity naming the immutable package directory.
    pub package_id: String,
    /// BLAKE3 digest of the selected package manifest.
    pub manifest_blake3: String,
}

/// Minimal durable snapshot replaced atomically by the runtime repository port.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProvisionalJobSnapshot {
    /// Provisional schema version required at the repository boundary.
    pub schema_version: String,
    /// E0 ownership identity, currently the validated lesson identifier.
    pub job_id: String,
    /// Hash of the deterministic render plan.
    pub plan_hash: String,
    /// Last durable E0 stage.
    pub stage: ProvisionalJobStage,
    /// Selected package identity, present only in `package_selected`.
    pub selected_package: Option<SelectedPackageIdentity>,
}

impl ProvisionalJobSnapshot {
    /// Creates a snapshot at the planned stage.
    pub fn planned(job_id: impl Into<String>, plan_hash: impl Into<String>) -> Self {
        Self {
            schema_version: PROVISIONAL_JOB_SCHEMA_VERSION.to_owned(),
            job_id: job_id.into(),
            plan_hash: plan_hash.into(),
            stage: ProvisionalJobStage::Planned,
            selected_package: None,
        }
    }

    /// Returns a replacement snapshot at `stage` with no selected package.
    pub fn advancing(&self, stage: ProvisionalJobStage) -> Self {
        Self {
            stage,
            selected_package: None,
            ..self.clone()
        }
    }

    /// Returns the terminal E0 snapshot selecting `package`.
    pub fn selecting(&self, package: SelectedPackageIdentity) -> Self {
        Self {
            stage: ProvisionalJobStage::PackageSelected,
            selected_package: Some(package),
            ..self.clone()
        }
    }
}
