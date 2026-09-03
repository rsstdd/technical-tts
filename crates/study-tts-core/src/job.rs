//! The job document and the state machine it records.
//!
//! [`JobState`] and [`JobState::may_transition_to`] transcribe ADR-0001 §6.4
//! edge for edge, and that section names this module in return. The table is
//! scoped to **one build attempt**: §6.4 has no edge from a terminal state back
//! to `Planned`, so a rebuild or a resume of a finished job does not transition
//! — it opens the next attempt through [`JobDocument::open_attempt`], which is
//! what §12.4 separates "job **and build** identity" for. The prior attempt is
//! retained as [`AbandonedAttempt`] (§12.7 step 5).
//!
//! [`JobDocument`] is the §12.4 `job.json`, scoped to what this build writes.
//! Deferred, with the owner named: ASR verification keys, diffs, and
//! adjudications (E4); take selection beyond the plan's default (E2-S2);
//! approval-record references (E2-S6); failure classification and retry
//! budgets (E5); worker and model identities, which every recorded cache key
//! already fixes. `SchemaVersion::accepted_by` admits each as a compatible
//! extension later, which is cheaper than publishing fields nothing writes.
//!
//! Private-preview completion is kept *separate* from the state machine, as
//! DELIVERY-PLAN E2-S1 task 4 requires: a preview build renders and then
//! records its selected package in [`JobDocument::preview_package`] beside
//! [`ReleaseStatus::PrivatePreview`], and never claims `Verified`,
//! `QualityChecked`, or `Published`, whose producers are E4, E2-S3, and the
//! production publication path. `Published` stays in the table because the
//! ADR has the edge; [`JobDocument::transition`] refuses it through
//! [`ReleaseError::PrivateProfileCannotClaimProduction`] while the release
//! status is a preview.
//!
//! `docs/architecture/G1-FREEZE-CHARTER.md` freezes this document as `job` /
//! `1.0` and `docs/architecture/E2-S1-INTERFACE-CHANGE-001.md` records the
//! move from the provisional `e0.job-state.0.1` snapshot.

use std::{collections::BTreeMap, num::NonZeroU32};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    MAX_LESSON_SEGMENTS,
    digest::{blake3_newtype, json_schema_as_string},
    plan::{CacheKey, PlanHash},
    release::{ReleaseError, ReleaseStatus},
    schema::SchemaVersion,
    verification::AudioDigest,
};

/// Version of the published job document, `schemas/job-v1.schema.json`.
///
/// `1.0`: the provisional `0.1` snapshot's own doc said E2-S1 would replace
/// it, and this is that replacement. A `0.1` record is refused, never
/// migrated — see `study_tts_runtime::job_repository`.
pub const JOB_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// One state of the ADR-0001 §6.4 job state machine.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// A build attempt has been opened for the job.
    Created,
    /// The lesson document passed validation.
    Validated,
    /// A deterministic render plan exists for the validated lesson.
    Planned,
    /// Segments are being synthesized or reused from the cache.
    Rendering,
    /// Every required segment is a valid cached artifact.
    Rendered,
    /// ASR verification is running over the rendered segments.
    Verifying,
    /// Every selected segment passed verification or was accepted.
    Verified,
    /// Verification found a mismatch, an uncalibrated term, or a quality
    /// finding.
    NeedsReview,
    /// Final outputs are being assembled from verified segments.
    Assembling,
    /// The assembled outputs passed the quality profile.
    QualityChecked,
    /// The outputs were published as a production release.
    Published,
    /// The attempt failed and records why.
    Failed,
    /// The user cancelled the attempt.
    Cancelled,
}

impl JobState {
    /// Whether ADR-0001 §6.4 has an edge from `self` to `next`.
    ///
    /// Transcribed edge for edge from the §6.4 diagram, one arm per source
    /// state so that a state added to the enum is a compile error here rather
    /// than a state with no edges. Self-loops are the diagram's own
    /// (`Rendering → Rendering`, `Verifying → Verifying`).
    pub fn may_transition_to(self, next: Self) -> bool {
        match self {
            Self::Created => next == Self::Validated,
            Self::Validated => next == Self::Planned,
            Self::Planned => next == Self::Rendering,
            Self::Rendering => matches!(
                next,
                Self::Rendering | Self::Rendered | Self::Failed | Self::Cancelled
            ),
            Self::Rendered => next == Self::Verifying,
            Self::Verifying => matches!(
                next,
                Self::Verifying | Self::Verified | Self::NeedsReview | Self::Failed
            ),
            Self::Verified => matches!(next, Self::Assembling | Self::Verifying),
            Self::NeedsReview => matches!(next, Self::Verified | Self::Planned),
            Self::Assembling => matches!(next, Self::QualityChecked | Self::Failed),
            Self::QualityChecked => next == Self::Published,
            Self::Failed => matches!(next, Self::Verifying | Self::Planned),
            Self::Cancelled => next == Self::Planned,
            Self::Published => false,
        }
    }

    /// Whether every segment has been rendered, which is what a preview
    /// package may be recorded against.
    ///
    /// Exhaustive so a new state decides this explicitly.
    pub fn has_rendered(self) -> bool {
        match self {
            Self::Created
            | Self::Validated
            | Self::Planned
            | Self::Rendering
            | Self::Failed
            | Self::Cancelled => false,
            Self::Rendered
            | Self::Verifying
            | Self::Verified
            | Self::NeedsReview
            | Self::Assembling
            | Self::QualityChecked
            | Self::Published => true,
        }
    }
}

/// Why a job document could not move to the requested state.
#[derive(Debug, Error)]
pub enum JobStateError {
    /// ADR-0001 §6.4 has no such edge.
    #[error(
        "job `{job_id}` cannot move from `{from:?}` to `{to:?}`: ADR-0001 §6.4 has no such \
         transition; preserve the job document for runtime reconciliation"
    )]
    IllegalTransition {
        /// The job whose document was asked to move.
        job_id: String,
        /// The state the document records.
        from: JobState,
        /// The state that was requested.
        to: JobState,
    },
    /// The next attempt cannot be represented without reusing an identity.
    #[error(
        "job `{job_id}` exhausted its build-attempt identity space; preserve the job document for \
         runtime reconciliation rather than reusing an attempt number"
    )]
    AttemptOverflow {
        /// Job whose next attempt cannot be represented.
        job_id: String,
    },
    /// The edge exists but the release status forbids taking it.
    #[error(transparent)]
    Release(#[from] ReleaseError),
}

/// BLAKE3 digest of the lesson bytes retained as `jobs/<job-id>/lesson.json`.
///
/// An artifact checksum over the stored bytes, not an identity over canonical
/// bytes: the lesson's *identity* already reaches every cache key through the
/// plan, and what resume must know is whether the retained copy is the one
/// this document was built from.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct LessonDigest(String);

impl LessonDigest {
    /// The digest as it is written into the job document.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(LessonDigest, MalformedLessonDigest);

/// Remedy routing: the digest is recomputed from the retained lesson bytes, so
/// the message names that rather than an edit.
/// `docs/governance/ROUTING-TABLES.md` §Failure routing sends state and
/// checksum corruption to "refuse overwrite; run reconciliation".
#[derive(Debug, Error)]
#[error(
    "lesson digest `{0}` is not a BLAKE3 digest in lowercase hexadecimal; recompute it from the \
     retained lesson rather than editing the recorded value, and preserve the job directory for \
     runtime reconciliation"
)]
pub struct MalformedLessonDigest(String);

json_schema_as_string!(
    LessonDigest,
    "LessonDigest",
    "BLAKE3 over the retained lesson document's bytes, as 64 lowercase \
     hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

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

/// Immutable package identity retained as the private-preview completion.
///
/// The two fields carry one value today. `study_tts_runtime::package_port`
/// names a package directory by the BLAKE3 of the `manifest.json` inside it, so
/// the identity that resolves the directory and the identity that verifies the
/// manifest are the same digest, and nothing in this type holds them equal — a
/// reader that needs them to agree must compare them. Kept at `1.0` so the
/// `0.1` shape and the `preview::PublishedPackage` it mirrors stay readable
/// side by side; collapsing it is a later compatible move once E2-S3 decides
/// what a package identity is.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SelectedPackageIdentity {
    /// Content identity naming the immutable package directory.
    pub package_id: ManifestDigest,
    /// BLAKE3 digest of the selected package manifest.
    pub manifest_blake3: ManifestDigest,
}

/// What a rendered segment resolved to.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SegmentStatus {
    /// The synthesis identity the segment was rendered or reused under.
    pub cache_key: CacheKey,
    /// Digest of the validated audio in that cache entry.
    pub audio_blake3: AudioDigest,
}

/// The attempt a resume or rebuild left behind, ADR-0001 §12.7 step 5.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AbandonedAttempt {
    /// The attempt number that was abandoned.
    pub build_attempt: NonZeroU32,
    /// The state that attempt had reached.
    pub state: JobState,
}

/// The authoritative `job.json`, ADR-0001 §12.4, replaced atomically by
/// `study_tts_runtime::job_repository`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct JobDocument {
    /// Layout version, read before anything else is trusted.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: SchemaVersion,
    /// Job identity: the validated lesson identifier.
    pub job_id: String,
    /// Build identity within the job; starts at one and grows by one per
    /// rebuild or resume.
    pub build_attempt: NonZeroU32,
    /// Where this attempt is in the ADR-0001 §6.4 machine.
    pub state: JobState,
    /// Most recent successful state reached before a failure or cancellation.
    pub last_successful_state: JobState,
    /// The attempt this one superseded, if any.
    pub abandoned_attempt: Option<AbandonedAttempt>,
    /// Checksum of the retained `lesson.json`.
    pub lesson_blake3: LessonDigest,
    /// Identity of the retained `plan.json`.
    pub plan_hash: PlanHash,
    /// Every rendered segment by identifier; ordered so the document is
    /// byte-stable across rebuilds.
    #[schemars(schema_with = "segment_statuses_json_schema")]
    pub segments: BTreeMap<String, SegmentStatus>,
    /// The private-preview completion, recorded only once rendering is done.
    pub preview_package: Option<SelectedPackageIdentity>,
    /// What the outputs of this attempt may claim to be.
    pub release_status: ReleaseStatus,
    /// The `study-tts-core` version that wrote this document.
    pub application_version: String,
}

impl JobDocument {
    /// Opens the next build attempt for a job, at [`JobState::Created`].
    ///
    /// Not a transition: §6.4 has no edge out of a finished attempt, so the
    /// previous document is retained as [`AbandonedAttempt`] and the attempt
    /// counter moves instead. A job with no prior document starts at attempt
    /// one.
    ///
    /// # Errors
    ///
    /// [`JobStateError::AttemptOverflow`] when the previous attempt is already
    /// [`u32::MAX`] and incrementing it would reuse the same durable identity.
    pub fn open_attempt(
        job_id: impl Into<String>,
        lesson_blake3: LessonDigest,
        plan_hash: PlanHash,
        previous: Option<&JobDocument>,
    ) -> Result<Self, JobStateError> {
        let job_id = job_id.into();
        let (build_attempt, abandoned_attempt) = match previous {
            Some(previous) => (
                previous.build_attempt.checked_add(1).ok_or_else(|| {
                    JobStateError::AttemptOverflow {
                        job_id: job_id.clone(),
                    }
                })?,
                Some(AbandonedAttempt {
                    build_attempt: previous.build_attempt,
                    state: previous.state,
                }),
            ),
            None => (NonZeroU32::MIN, None),
        };
        Ok(Self {
            schema_version: JOB_SCHEMA_VERSION,
            job_id,
            build_attempt,
            state: JobState::Created,
            last_successful_state: JobState::Created,
            abandoned_attempt,
            lesson_blake3,
            plan_hash,
            segments: BTreeMap::new(),
            preview_package: None,
            release_status: ReleaseStatus::PrivatePreview,
            application_version: env!("CARGO_PKG_VERSION").to_owned(),
        })
    }

    /// Moves the attempt along one §6.4 edge.
    ///
    /// Returning to [`JobState::Planned`] clears the preview completion: the
    /// ADR labels those edges as an input, rendering, or selected-take change,
    /// so the prior package no longer represents the plan being prepared.
    ///
    /// # Errors
    ///
    /// [`JobStateError::IllegalTransition`] when §6.4 has no edge from the
    /// recorded state to `next`, and [`JobStateError::Release`] carrying
    /// [`ReleaseError::PrivateProfileCannotClaimProduction`] when `next` is
    /// [`JobState::Published`] and the document is a private preview — the
    /// edge exists, and the release status is what forbids taking it.
    pub fn transition(mut self, next: JobState) -> Result<Self, JobStateError> {
        if !self.state.may_transition_to(next) {
            return Err(JobStateError::IllegalTransition {
                job_id: self.job_id,
                from: self.state,
                to: next,
            });
        }
        if next == JobState::Published && self.release_status != ReleaseStatus::ProductionRelease {
            return Err(ReleaseError::PrivateProfileCannotClaimProduction.into());
        }
        self.last_successful_state = match next {
            JobState::Failed | JobState::Cancelled => self.last_successful_state,
            _ => next,
        };
        self.state = next;
        if next == JobState::Planned {
            self.preview_package = None;
        }
        Ok(self)
    }

    /// Records what one segment resolved to.
    pub fn with_segment(mut self, segment_id: impl Into<String>, status: SegmentStatus) -> Self {
        self.segments.insert(segment_id.into(), status);
        self
    }

    /// Records the selected private-preview package.
    ///
    /// The separate completion status, not a state: the attempt stays where
    /// §6.4 left it. `study_tts_runtime::job_repository` refuses a document
    /// carrying one before [`JobState::has_rendered`].
    pub fn with_preview_package(self, package: SelectedPackageIdentity) -> Self {
        Self {
            preview_package: Some(package),
            ..self
        }
    }
}

/// Publishes the versions of this document a build reads.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::accepted_versions_json_schema(JOB_SCHEMA_VERSION)
}

fn segment_statuses_json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    let mut schema = generator.subschema_for::<BTreeMap<String, SegmentStatus>>();
    schema.insert("maxProperties".to_owned(), MAX_LESSON_SEGMENTS.into());
    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_has_rendered(state: JobState) -> bool {
        match state {
            JobState::Created
            | JobState::Validated
            | JobState::Planned
            | JobState::Rendering
            | JobState::Failed
            | JobState::Cancelled => false,
            JobState::Rendered
            | JobState::Verifying
            | JobState::Verified
            | JobState::NeedsReview
            | JobState::Assembling
            | JobState::QualityChecked
            | JobState::Published => true,
        }
    }

    fn all_states() -> Vec<JobState> {
        let schema = serde_json::Value::from(schemars::schema_for!(JobState));
        schema["oneOf"]
            .as_array()
            .expect("the derived enum schema lists every state")
            .iter()
            .map(|variant| {
                serde_json::from_value(variant["const"].clone())
                    .expect("every schema spelling parses as a job state")
            })
            .collect()
    }

    /// The 22 edges of ADR-0001 §6.4, read off the diagram top to bottom —
    /// an independent copy a reviewer checks against the document, not a
    /// derivation from `may_transition_to`.
    const EDGES: [(JobState, JobState); 22] = [
        (JobState::Created, JobState::Validated),
        (JobState::Validated, JobState::Planned),
        (JobState::Planned, JobState::Rendering),
        (JobState::Rendering, JobState::Rendering),
        (JobState::Rendering, JobState::Rendered),
        (JobState::Rendering, JobState::Failed),
        (JobState::Rendering, JobState::Cancelled),
        (JobState::Rendered, JobState::Verifying),
        (JobState::Verifying, JobState::Verifying),
        (JobState::Verifying, JobState::Verified),
        (JobState::Verifying, JobState::NeedsReview),
        (JobState::Verifying, JobState::Failed),
        (JobState::Verified, JobState::Assembling),
        (JobState::Verified, JobState::Verifying),
        (JobState::Assembling, JobState::QualityChecked),
        (JobState::Assembling, JobState::Failed),
        (JobState::QualityChecked, JobState::Published),
        (JobState::NeedsReview, JobState::Verified),
        (JobState::NeedsReview, JobState::Planned),
        (JobState::Failed, JobState::Verifying),
        (JobState::Failed, JobState::Planned),
        (JobState::Cancelled, JobState::Planned),
    ];

    fn document() -> JobDocument {
        JobDocument::open_attempt(
            "lesson-1",
            "a".repeat(64).parse().expect("a digest of a parses"),
            "b".repeat(64).parse().expect("a digest of b parses"),
            None,
        )
        .expect("the first attempt is representable")
    }

    fn at(state: JobState) -> JobDocument {
        JobDocument {
            state,
            ..document()
        }
    }

    #[test]
    fn t1_e2_illegal_state_transition_is_refused() {
        let states = all_states();
        for &from in &states {
            for &to in &states {
                let expected = EDGES.contains(&(from, to));
                assert_eq!(
                    from.may_transition_to(to),
                    expected,
                    "{from:?} -> {to:?} should be {}",
                    if expected { "an edge" } else { "refused" }
                );
                let outcome = at(from).transition(to);
                match (expected, to) {
                    (false, _) => assert!(
                        matches!(
                            outcome,
                            Err(JobStateError::IllegalTransition { from: f, to: t, .. })
                                if f == from && t == to
                        ),
                        "{from:?} -> {to:?}"
                    ),
                    (true, JobState::Published) => {
                        // Covered by the test below; the edge is legal and the
                        // release status is what refuses it.
                    }
                    (true, _) => {
                        assert_eq!(
                            outcome.expect("a legal edge").state,
                            to,
                            "{from:?} -> {to:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn t1_e2_private_preview_cannot_transition_to_published() {
        assert!(
            JobState::QualityChecked.may_transition_to(JobState::Published),
            "the ADR-0001 §6.4 edge exists; the release guard is what refuses it"
        );

        let refused = at(JobState::QualityChecked).transition(JobState::Published);
        assert!(matches!(
            refused,
            Err(JobStateError::Release(
                ReleaseError::PrivateProfileCannotClaimProduction
            ))
        ));

        let production = JobDocument {
            release_status: ReleaseStatus::ProductionRelease,
            ..at(JobState::QualityChecked)
        };
        assert_eq!(
            production
                .transition(JobState::Published)
                .expect("a production release may publish")
                .state,
            JobState::Published
        );
    }

    #[test]
    fn t1_e2_a_new_build_attempt_is_not_a_transition() {
        let finished = at(JobState::Rendered).with_preview_package(SelectedPackageIdentity {
            package_id: "c".repeat(64).parse().expect("a digest of c parses"),
            manifest_blake3: "c".repeat(64).parse().expect("a digest of c parses"),
        });
        assert!(
            !finished.state.may_transition_to(JobState::Created),
            "§6.4 has no edge back from a finished attempt"
        );

        let next = JobDocument::open_attempt(
            &finished.job_id,
            finished.lesson_blake3.clone(),
            finished.plan_hash.clone(),
            Some(&finished),
        )
        .expect("the second attempt is representable");

        assert_eq!(next.state, JobState::Created);
        assert_eq!(next.last_successful_state, JobState::Created);
        assert_eq!(next.build_attempt.get(), 2);
        assert_eq!(
            next.abandoned_attempt,
            Some(AbandonedAttempt {
                build_attempt: NonZeroU32::MIN,
                state: JobState::Rendered,
            })
        );
        assert_eq!(next.preview_package, None);
        assert!(next.segments.is_empty());
    }

    #[test]
    fn t1_e2_returning_to_planned_clears_preview_completion() {
        let verifying = at(JobState::Rendered)
            .with_preview_package(SelectedPackageIdentity {
                package_id: "c".repeat(64).parse().expect("a digest of c parses"),
                manifest_blake3: "c".repeat(64).parse().expect("a digest of c parses"),
            })
            .transition(JobState::Verifying)
            .expect("rendered audio may enter verification");
        let needs_review = verifying
            .clone()
            .transition(JobState::NeedsReview)
            .expect("verification findings may need review");
        let failed = verifying
            .transition(JobState::Failed)
            .expect("verification may fail");

        for document in [needs_review, failed] {
            let planned = document
                .transition(JobState::Planned)
                .expect("review and failure may return to planning");
            assert_eq!(
                planned.preview_package, None,
                "a plan-changing transition invalidates its prior package"
            );
        }
    }

    #[test]
    fn t1_e2_attempt_overflow_is_refused() {
        let previous = JobDocument {
            build_attempt: NonZeroU32::MAX,
            ..document()
        };

        let error = JobDocument::open_attempt(
            &previous.job_id,
            previous.lesson_blake3.clone(),
            previous.plan_hash.clone(),
            Some(&previous),
        )
        .expect_err("attempt identity must never be reused after overflow");

        assert!(matches!(error, JobStateError::AttemptOverflow { .. }));
    }

    #[test]
    fn t1_e2_rendering_is_complete_only_from_rendered_onward() {
        for state in all_states() {
            assert_eq!(
                state.has_rendered(),
                expected_has_rendered(state),
                "{state:?}"
            );
        }
    }
}
