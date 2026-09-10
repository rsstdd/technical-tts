//! The human review a private preview must pass, and the records of it.
//!
//! ADR-0001 §18 requires "a **reviewed** 60-minute technical lesson" and that
//! "no omitted, duplicated, inserted, or materially mispronounced technical
//! content survives **review**". Review is therefore a ratified precondition of
//! an accepted build, not a courtesy. What the ADR does not fix is *which*
//! criteria a reviewer answered and *what* their decision produces, and this
//! module owns both.
//!
//! Three items, in the order a reader meets them: the checklist version that
//! fixes the criteria, the [`ApprovalRecord`] one reviewer writes about one
//! package, and the [`PreviewReleaseRecord`] that declares an approved
//! generation released. Each names the one before it and none names the one
//! after, which is how `DELIVERY-PLAN.md`'s M2 acceptance gets human approval
//! "without a checksum cycle".
//!
//! `docs/operations/PREVIEW-REVIEW-CHECKLIST.md` is the checklist itself. This
//! module deliberately holds no copy of its criteria: a second list would
//! recreate the drift that document exists to stop.

use serde::{Deserialize, Serialize};

use thiserror::Error;

use crate::{
    digest::{blake3_newtype, json_schema_as_string},
    job::ManifestDigest,
    schema::SchemaVersion,
};

/// Version of `docs/operations/PREVIEW-REVIEW-CHECKLIST.md` this build records.
///
/// Two-sided with that document's §Checklist version line, which names this
/// constant in return, and pinned by
/// `t1_e2_checklist_version_matches_the_checklist_document` below.
///
/// A [`SchemaVersion`] rather than a string, because
/// `PREVIEW-REVIEW-CHECKLIST.md` §Checklist version states its rule on the two
/// parts separately — a criterion added moves the minor, one removed or
/// redefined moves the major — and comparing whole strings could only ever
/// answer "same or different".
///
/// An approval is only meaningful beside the version that fixed its criteria.
/// The checklist itself records why: four listening reviews were taken against
/// four different criteria sets, so "no two reviews are comparable, and a
/// criterion can go missing without anyone noticing it left". An approval that
/// did not say which set it answered would reintroduce exactly that.
pub const PREVIEW_REVIEW_CHECKLIST_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// Version of the published approval document this build writes and reads.
///
/// Moves on its own terms, independently of the checklist it names: the
/// document's *shape* and the *criteria* a reviewer answered are different
/// facts, and a new criterion must not force every stored approval to a new
/// major.
pub const APPROVAL_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// File-name stem of the published approval schema.
pub const APPROVAL_SCHEMA_STEM: &str = "approval";

/// What a reviewer decided about a package.
///
/// Closed, and refused at the boundary rather than defaulted: an approval
/// document carrying a disposition this build does not know is not an
/// approval, and treating it as a rejection would be as wrong as treating it
/// as an acceptance.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ApprovalDisposition {
    /// The reviewer accepted this package.
    Accepted,
    /// The reviewer rejected it. Recorded, never deleted: a rejection is the
    /// evidence that a generation was judged and found wanting.
    Rejected,
}

/// One person's decision about one immutable package.
///
/// **The reference runs one way only.** This names the manifest; the manifest
/// has no field able to name this. `DELIVERY-PLAN.md`'s M2 acceptance requires
/// human approval recorded "without a checksum cycle", and the cycle is
/// prevented by construction rather than by assertion — there is no field here
/// a release digest could occupy, and none in the manifest an approval digest
/// could.
///
/// It records the decision, not the findings. The per-segment observations a
/// reviewer made live in the checklist they filled and the evidence record that
/// cites it; duplicating them here would create a second place for them to
/// disagree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct ApprovalRecord {
    /// Layout of this document.
    pub schema_version: SchemaVersion,
    /// Lesson the reviewed package renders.
    pub lesson_id: String,
    /// BLAKE3 of the reviewed `manifest.json`.
    ///
    /// The whole package in one value: the manifest checksums every artifact,
    /// and the package directory is named by this digest. Recording the
    /// artifact digests again would duplicate what this already covers, and
    /// give them somewhere to drift.
    pub manifest_blake3: ManifestDigest,
    /// Checklist version whose criteria the reviewer answered.
    pub checklist_version: SchemaVersion,
    /// Who listened.
    pub reviewer: String,
    /// The role they signed in, per `docs/governance/ROUTING-TABLES.md`.
    pub reviewer_role: String,
    /// Hardware and room, which the checklist requires because "laptop
    /// speakers and monitors do not hear the same faults".
    pub playback_environment: String,
    /// What they decided.
    pub disposition: ApprovalDisposition,
}

/// BLAKE3 digest of a stored approval document.
///
/// A value object for the reason [`ManifestDigest`] is one: a release record
/// that names an approval by a value which is not a digest names nothing, and
/// the mistake is cheaper to refuse at the parse boundary than to chase from a
/// release nobody can verify.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ApprovalDigest(String);

impl ApprovalDigest {
    /// The digest as it is written into a release record.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(ApprovalDigest, MalformedApprovalDigest);

/// Remedy routing: recomputed from the approval document it names, never
/// edited. `docs/governance/ROUTING-TABLES.md` §Failure routing sends checksum
/// corruption to reconciliation rather than to deleting the record.
#[derive(Debug, Error)]
#[error(
    "approval digest `{0}` is not a BLAKE3 digest in lowercase hexadecimal; recompute it from \
     the approval document rather than editing the recorded value, and preserve both for runtime \
     reconciliation"
)]
pub struct MalformedApprovalDigest(String);

json_schema_as_string!(
    ApprovalDigest,
    "ApprovalDigest",
    "BLAKE3 over a stored approval document's bytes, as 64 lowercase \
     hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// Version of the published preview-release document.
pub const PREVIEW_RELEASE_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// File-name stem of the published preview-release schema.
pub const PREVIEW_RELEASE_SCHEMA_STEM: &str = "preview-release";

/// The record that declares one reviewed generation released as a preview.
///
/// **This is the third document, and the reference still runs one way.** It
/// names the manifest and the approval; neither names it, and neither has a
/// field that could. `DELIVERY-PLAN.md`'s M2 acceptance requires the approval
/// recorded "without a checksum cycle", and a cycle here is not merely absent
/// but unrepresentable —
/// `t3_e2_release_record_references_manifest_and_approval_without_cycle`
/// asserts that on the field lists rather than by scanning bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PreviewReleaseRecord {
    /// Layout of this document.
    pub schema_version: SchemaVersion,
    /// Lesson this release covers.
    pub lesson_id: String,
    /// BLAKE3 of the released `manifest.json`.
    pub manifest_blake3: ManifestDigest,
    /// BLAKE3 of the approval document that reviewed that manifest.
    pub approval_blake3: ApprovalDigest,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    /// The constant and the document it names cannot drift apart.
    ///
    /// A one-sided mirror is what `rust-comment` §Coupling comments calls a
    /// finding, and this is the half a compiler cannot check: the constant
    /// would keep claiming a version the checklist had moved past, and every
    /// approval recorded under it would name criteria nobody could
    /// reconstruct.
    #[test]
    fn t1_e2_checklist_version_matches_the_checklist_document() {
        let checklist = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/operations/PREVIEW-REVIEW-CHECKLIST.md");
        let text = std::fs::read_to_string(&checklist)
            .expect("the checklist is committed beside the code that versions it");

        let declared = format!("Checklist version: `{PREVIEW_REVIEW_CHECKLIST_VERSION}`");
        assert!(
            text.contains(&declared),
            "`{}` must declare `{declared}`",
            checklist.display()
        );
    }
}
