//! Minimal E0 job state shared by orchestration and durable repositories.
//!
//! This deliberately records ownership progress and selected-package identity
//! only. ADR-0001 §12.4 defines the whole `job.json`, and DELIVERY-PLAN E2-S1
//! owns the segment attempts, recovery, and resume semantics this baseline
//! leaves out.
//!
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records this module's
//! public representation, consumers, fake, identity effects, and G1 status.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::{blake3_newtype, json_schema_as_string};
use crate::plan::PlanHash;

/// Mirrors the `job_state` row of
/// `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`, which records the same
/// `e0.job-state.0.1` against [`ProvisionalJobSnapshot`].
///
/// `study_tts_runtime::schemas` const-asserts this spelling against the version
/// of the published `job-v0.schema.json`, so the label on disk and the schema
/// describing it cannot drift apart unnoticed.
pub const PROVISIONAL_JOB_SCHEMA_VERSION: &str = "e0.job-state.0.1";

/// The E0 progress stage a snapshot records, not the complete E2 state machine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
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

/// BLAKE3 digest of an immutable package manifest.
///
/// A value object for the reason [`crate::CacheKey`] is one, and for a second
/// reason that applies here: the digest *is* the package directory's name, so a
/// value that is not one names a directory the package layout cannot hold. The
/// runtime hashes `manifest.json` and this crate only accepts the result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManifestDigest(String);

impl ManifestDigest {
    /// The digest as it is written into job state and used as a directory name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(ManifestDigest, MalformedManifestDigest);

/// Remedy routing: the digest is recomputed from the package manifest it names,
/// so the message names that recomputation rather than an edit.
/// `docs/governance/ROUTING-TABLES.md` §Failure routing sends state and
/// checksum corruption to "refuse overwrite; run reconciliation", which is why
/// the remedy preserves the immutable package instead of pruning it.
#[derive(Debug, Error)]
#[error(
    "package manifest digest `{0}` is not a BLAKE3 digest in lowercase hexadecimal; recompute it \
     from the package manifest rather than editing the recorded value, and preserve the package \
     for runtime reconciliation"
)]
pub struct MalformedManifestDigest(String);

json_schema_as_string!(
    ManifestDigest,
    "ManifestDigest",
    "BLAKE3 over an immutable package manifest's bytes, as 64 lowercase \
     hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// Immutable package identity retained in the selected job state.
///
/// The two fields carry one value today. `study_tts_runtime::package_port`
/// names a package directory by the BLAKE3 of the `manifest.json` inside it, so
/// the identity that resolves the directory and the identity that verifies the
/// manifest are the same digest, and nothing in this type holds them equal — a
/// reader that needs them to agree must compare them. Whether the duplication
/// survives is a G1 freeze question: collapsing it is a breaking change to the
/// published `job-v0.schema.json` under
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, not an edit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SelectedPackageIdentity {
    /// Content identity naming the immutable package directory.
    pub package_id: ManifestDigest,
    /// BLAKE3 digest of the selected package manifest.
    pub manifest_blake3: ManifestDigest,
}

/// Minimal durable snapshot replaced atomically by the runtime repository port.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ProvisionalJobSnapshot {
    /// Provisional schema version required at the repository boundary.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: String,
    /// E0 ownership identity, currently the validated lesson identifier.
    pub job_id: String,
    /// Hash of the deterministic render plan.
    pub plan_hash: PlanHash,
    /// Last durable E0 stage.
    pub stage: ProvisionalJobStage,
    /// Selected package identity, present only in `package_selected`.
    pub selected_package: Option<SelectedPackageIdentity>,
}

impl ProvisionalJobSnapshot {
    /// Creates a snapshot at the planned stage.
    pub fn planned(job_id: impl Into<String>, plan_hash: PlanHash) -> Self {
        Self {
            schema_version: PROVISIONAL_JOB_SCHEMA_VERSION.to_owned(),
            job_id: job_id.into(),
            plan_hash,
            stage: ProvisionalJobStage::Planned,
            selected_package: None,
        }
    }

    /// Returns a replacement snapshot at `stage` with no selected package.
    ///
    /// Dropping the selection is the invariant rather than a convenience:
    /// `study_tts_runtime::job_repository::validate_snapshot` refuses any
    /// non-terminal stage that still carries one, so a snapshot advanced
    /// without clearing it would be written once and rejected on every later
    /// load. Passing [`ProvisionalJobStage::PackageSelected`] here builds a
    /// snapshot that same validator always refuses, for the mirror-image
    /// reason; [`Self::selecting`] is the only route to that stage.
    pub fn advancing(&self, stage: ProvisionalJobStage) -> Self {
        Self {
            stage,
            selected_package: None,
            ..self.clone()
        }
    }

    /// Returns the terminal E0 snapshot selecting `package`.
    ///
    /// The only constructor of [`ProvisionalJobStage::PackageSelected`], which
    /// `study_tts_runtime::job_repository::validate_snapshot` requires to carry
    /// a package.
    pub fn selecting(&self, package: SelectedPackageIdentity) -> Self {
        Self {
            stage: ProvisionalJobStage::PackageSelected,
            selected_package: Some(package),
            ..self.clone()
        }
    }
}

/// Publishes the one version this record may carry.
///
/// `study_tts_runtime::job_repository` refuses a snapshot declaring anything
/// else, so `const` is the parser's rule rather than a narrowing of it. This
/// record has no compatible-extension history to admit: it is the provisional
/// E0 snapshot, and ADR-0001 §12.4 replaces it wholesale at E2-S1.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "const": PROVISIONAL_JOB_SCHEMA_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_e0_advancing_drops_a_recorded_package_selection() {
        let digest: ManifestDigest = "a".repeat(64).parse().expect("a digest of a parses");
        let plan_hash: PlanHash = "b".repeat(64).parse().expect("a digest of b parses");
        let selected = ProvisionalJobSnapshot::planned("lesson-1", plan_hash).selecting(
            SelectedPackageIdentity {
                package_id: digest.clone(),
                manifest_blake3: digest,
            },
        );

        let advanced = selected.advancing(ProvisionalJobStage::Caching);

        assert_eq!(advanced.stage, ProvisionalJobStage::Caching);
        assert_eq!(
            advanced.selected_package, None,
            "a non-terminal stage carrying a selection is refused on load"
        );
    }
}
