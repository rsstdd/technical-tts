//! Telling an operator what a long build is doing, while it does it.
//!
//! E2-S5 task 5: "Report progress during model loading and synthesis." A
//! five-minute lesson takes twenty-seven minutes to render, and a command that
//! printed nothing for that long is one an operator reasonably kills.
//!
//! # Why there is no new seam
//!
//! The pipeline already announces every boundary it crosses. `BuildStage` names
//! them — `PlanSelected`, `SegmentSynthesized`, `SegmentReused`,
//! `SegmentFailed`, `PackageAssembled` — and every one goes through
//! [`JobRepository::record_stage`] on its way to `events.ndjson`.
//!
//! So this is a decorator over that port rather than an observer beside it: it
//! passes each stage to the real repository and prints it on the way. Nothing
//! in `study-tts-runtime` changes, no call site moves, and the events an
//! operator watches are exactly the ones the job log will hold — two accounts
//! of one build that cannot disagree, because they are the same account.
//!
//! `RecordingTtsExecutor` in `study-tts-testkit` is the same shape, and
//! `synthesis.rs` records why it matters: a wrapper that takes a default
//! reports nothing and no test notices. Every method here delegates.
//!
//! # Where it goes
//!
//! Standard error. ADR-0001 §10.1 makes standard output protocol-only, and
//! §14 asks that "human terminal output remains concise". `--json` writes one
//! record to stdout at the end; progress must not land in the middle of it.

use std::path::Path;

use study_tts_core::{JobDocument, RenderPlan};
use study_tts_runtime::{BuildError, BuildStage, JobOwnership, JobRepository};

/// A job repository that says what it is recording.
///
/// Holds the real one rather than replacing it: durability, validation, and
/// every refusal stay exactly where they were.
pub struct ReportingJobs<'a> {
    inner: &'a dyn JobRepository,
    quiet: bool,
}

impl<'a> ReportingJobs<'a> {
    /// Wraps a repository, printing stages unless `quiet`.
    ///
    /// `quiet` rather than a separate silent type: under `--json` the operator
    /// asked for one machine-readable record and prose beside it would be
    /// noise, but the wrapper must still be in the chain so both modes run the
    /// same code path.
    #[must_use]
    pub fn new(inner: &'a dyn JobRepository, quiet: bool) -> Self {
        Self { inner, quiet }
    }

    /// Describes one stage for a person, or nothing when it says nothing new.
    ///
    /// Only the stages an operator is waiting through are announced. A build
    /// crosses boundaries a reader does not need narrated, and §14's "concise"
    /// is the reason to leave those to `events.ndjson`.
    fn announce(stage: &BuildStage) -> Option<String> {
        match stage {
            BuildStage::PlanSelected { .. } => Some("planned".to_owned()),
            BuildStage::SegmentSynthesized { segment_id, .. } => {
                Some(format!("synthesized {segment_id}"))
            }
            BuildStage::SegmentReused { segment_id, .. } => Some(format!("reused {segment_id}")),
            BuildStage::SegmentFailed { segment_id, .. } => Some(format!("failed {segment_id}")),
            BuildStage::PackageAssembled => Some("assembled".to_owned()),
            _ => None,
        }
    }
}

impl JobRepository for ReportingJobs<'_> {
    fn record_stage(
        &self,
        workspace: &Path,
        job_id: &str,
        build_attempt: u32,
        stage: BuildStage,
    ) -> Result<(), BuildError> {
        if !self.quiet
            && let Some(line) = Self::announce(&stage)
        {
            eprintln!("  {line}");
        }
        self.inner
            .record_stage(workspace, job_id, build_attempt, stage)
    }

    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        self.inner.claim(workspace, job_id)
    }

    fn load(&self, workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError> {
        self.inner.load(workspace, job_id)
    }

    fn replace(&self, workspace: &Path, document: &JobDocument) -> Result<(), BuildError> {
        self.inner.replace(workspace, document)
    }

    fn retain_inputs(
        &self,
        workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError> {
        self.inner.retain_inputs(workspace, job_id, lesson, plan)
    }

    fn retain_run_report(
        &self,
        workspace: &Path,
        job_id: &str,
        report: &study_tts_runtime::RunReport,
    ) -> Result<(), BuildError> {
        self.inner.retain_run_report(workspace, job_id, report)
    }

    fn retained_lesson(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        self.inner.retained_lesson(workspace, job_id)
    }

    fn retained_plan(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError> {
        self.inner.retained_plan(workspace, job_id)
    }

    fn validate_preview_selection(
        &self,
        workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError> {
        self.inner.validate_preview_selection(workspace, document)
    }
}
