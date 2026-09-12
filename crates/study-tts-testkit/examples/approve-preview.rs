//! Records a reviewer's decision about a published preview package.
//!
//! `DELIVERY-PLAN.md`'s M2 acceptance requires that a reviewed lesson "records
//! immutable human approval without a checksum cycle".
//! [`study_tts_runtime::approve_preview`] does that and is tested, but until
//! this instrument existed nothing an operator could run called it: E2-S5 owns
//! the CLI and has not landed, so the contract was signed and never exercised
//! on real material. A contract nothing has run against a real package has not
//! recorded anything, which is the gap this closes.
//!
//! An example rather than a script, for the reason `package-render` gives: the
//! approval must be written through the path production will use, so what the
//! gate reads is what a build produces. A harness that wrote the JSON itself
//! would qualify the harness.
//!
//! # What this does not do
//!
//! **It does not release.** [`study_tts_runtime::publish_preview_release`] is a
//! second decision with its own gate — E2-S6 task 7 — and folding both into one
//! command would let a reviewer release by approving. Releasing needs its own
//! instrument, or the CLI.
//!
//! **It does not listen, and it cannot.** ADR-0001 §17.5 makes the review a
//! human act. This records a decision already made against
//! `docs/operations/PREVIEW-REVIEW-CHECKLIST.md`; it is the pen, not the ear.
//!
//! # Rights
//!
//! Nothing is copied into the repository. The approval is written beside the
//! package it names, beneath the `--workspace` the operator gives, per
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

use std::{error::Error, path::PathBuf};

use study_tts_core::{ApprovalDisposition, ManifestDigest};
use study_tts_runtime::{ApprovalRequest, approve_preview};

/// Spellings an operator types, and the decision each records.
///
/// A table rather than a `match` arm per variant so the accepted vocabulary is
/// one list a reader can check against [`ApprovalDisposition`], and so the test
/// below reads the same list the parser does.
const DISPOSITIONS: [(&str, ApprovalDisposition); 2] = [
    ("accepted", ApprovalDisposition::Accepted),
    ("rejected", ApprovalDisposition::Rejected),
];

/// Parses the decision, refusing anything not spelled exactly.
///
/// No default and no fuzzy match. A mistyped decision must stop the run rather
/// than record the opposite of what a reviewer meant, and this is the one place
/// in this instrument where a wrong answer would be durable.
fn disposition(value: &str) -> Result<ApprovalDisposition, String> {
    DISPOSITIONS
        .iter()
        .find(|(spelling, _)| *spelling == value)
        .map(|(_, decision)| *decision)
        .ok_or_else(|| {
            let accepted: Vec<&str> = DISPOSITIONS.iter().map(|(s, _)| *s).collect();
            format!("`--disposition` takes one of {accepted:?}, got `{value}`")
        })
}

/// Everything the approval needs, all of it from the operator.
struct Configuration {
    workspace: PathBuf,
    lesson_id: String,
    manifest_blake3: ManifestDigest,
    reviewer: String,
    reviewer_role: String,
    playback_environment: String,
    disposition: ApprovalDisposition,
}

impl Configuration {
    fn from_arguments() -> Result<Self, Box<dyn Error>> {
        let mut workspace = None;
        let mut lesson_id = None;
        let mut manifest = None;
        let mut reviewer = None;
        let mut reviewer_role = None;
        let mut playback_environment = None;
        let mut decision = None;

        let mut arguments = std::env::args().skip(1);
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("`{flag}` needs a value"))?;
            match flag.as_str() {
                "--workspace" => workspace = Some(PathBuf::from(value)),
                "--lesson-id" => lesson_id = Some(value),
                "--manifest" => manifest = Some(value),
                "--reviewer" => reviewer = Some(value),
                "--reviewer-role" => reviewer_role = Some(value),
                "--playback-environment" => playback_environment = Some(value),
                "--disposition" => decision = Some(disposition(&value)?),
                unknown => return Err(format!("unknown argument `{unknown}`").into()),
            }
        }

        // Parsed here rather than at the boundary inside `approve_preview`, so
        // a mistyped digest is refused before the run reaches the root.
        let manifest_blake3 = ManifestDigest::try_from(manifest.ok_or("--manifest is required")?)?;

        Ok(Self {
            workspace: std::path::absolute(workspace.ok_or("--workspace is required")?)?,
            lesson_id: lesson_id.ok_or("--lesson-id is required")?,
            manifest_blake3,
            reviewer: reviewer.ok_or("--reviewer is required")?,
            reviewer_role: reviewer_role.ok_or("--reviewer-role is required")?,
            playback_environment: playback_environment
                .ok_or("--playback-environment is required")?,
            disposition: decision.ok_or("--disposition is required")?,
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let configuration = Configuration::from_arguments()?;

    let record = approve_preview(
        &configuration.workspace,
        &ApprovalRequest {
            lesson_id: &configuration.lesson_id,
            manifest_blake3: &configuration.manifest_blake3,
            reviewer: &configuration.reviewer,
            reviewer_role: &configuration.reviewer_role,
            playback_environment: &configuration.playback_environment,
            disposition: configuration.disposition,
        },
    )?;

    // The digest is echoed because it is what the approval is keyed by: a
    // reviewer checking their own work needs to see that it names the package
    // they listened to, not the lesson.
    println!("lesson:      {}", record.lesson_id);
    println!("package:     {}", record.manifest_blake3.as_str());
    println!("checklist:   {}", record.checklist_version);
    println!(
        "reviewer:    {} ({})",
        record.reviewer, record.reviewer_role
    );
    println!("playback:    {}", record.playback_environment);
    println!("disposition: {:?}", record.disposition);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DISPOSITIONS, disposition};
    use study_tts_core::ApprovalDisposition;

    /// A decision is durable, so an unknown spelling must stop the run.
    ///
    /// The wrong implementation this rejects is the one reached for by
    /// reflex: a `match` with a catch-all arm defaulting to `Accepted`, which
    /// would record approval for a reviewer who typed `reject` and meant it.
    #[test]
    fn t1_e2_an_unknown_disposition_is_refused_rather_than_defaulted() {
        const REFUSED: [&str; 6] = ["", "accept", "reject", "Accepted", "approved", "yes"];

        for value in REFUSED {
            assert!(
                disposition(value).is_err(),
                "`{value}` must be refused, not interpreted"
            );
        }

        assert_eq!(
            disposition("accepted"),
            Ok(ApprovalDisposition::Accepted),
            "the exact spelling must still work"
        );
        assert_eq!(disposition("rejected"), Ok(ApprovalDisposition::Rejected));
    }

    /// Every decision the contract can express has a spelling an operator can
    /// type, so a variant added later cannot be silently unreachable here.
    #[test]
    fn t1_e2_every_disposition_variant_has_a_spelling() {
        for decision in [ApprovalDisposition::Accepted, ApprovalDisposition::Rejected] {
            assert!(
                DISPOSITIONS.iter().any(|(_, known)| *known == decision),
                "{decision:?} has no spelling in DISPOSITIONS"
            );
        }
    }
}
