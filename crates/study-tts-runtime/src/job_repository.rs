//! Durable job ownership and the `job.json` repository port.
//!
//! The filesystem adapter keeps Linux ownership locking and atomic JSON
//! replacement, appends an event after each durable write (ADR-0001 §12.3
//! step 5), and refuses to overwrite a record it cannot trust. The document
//! itself and its state machine are `study_tts_core::job`; this module owns
//! only their durability and the checks a loaded record must pass.
//!
//! `docs/architecture/G1-FREEZE-CHARTER.md` freezes `job_state` against
//! [`JobRepository`] and [`JobOwnership`].

use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use study_tts_core::{
    AbandonedAttempt, JOB_SCHEMA_VERSION, JobDocument, JobState, MAX_LESSON_SEGMENTS,
    PLAN_SCHEMA_VERSION, ReleaseError, ReleaseStatus, RenderPlan,
};

use crate::{
    BuildError, DurableStateError,
    durable::{
        DurableFileSystem, OsDurableFileSystem, read_bounded_bytes, write_bytes_atomically,
        write_json_atomically,
    },
    job_events::{JobEvent, JobEventKind, append_event, preflight_append, validate_event_log},
    locking, managed, pipeline, preview,
};

const JOB_DOCUMENT_NAME: &str = "job.json";
const RETAINED_LESSON_NAME: &str = "lesson.json";
const RETAINED_PLAN_NAME: &str = "plan.json";
// Mirrored by `docs/architecture/WALKING-SKELETON.md` §Provisional resource
// ceilings; these bound untrusted durable JSON before Serde sees it.
const MAX_JOB_JSON_BYTES: usize = 16 * 1024 * 1024;
const MAX_RETAINED_PLAN_JSON_BYTES: usize = 32 * 1024 * 1024;

/// Keeps exclusive ownership of one job until the guard is dropped.
pub trait JobOwnership: Debug + Send {}

impl JobOwnership for locking::JobLock {}

/// Durable replacement and ownership boundary for the job document.
pub trait JobRepository: Send + Sync {
    /// Claims exclusive ownership until the returned guard is dropped.
    ///
    /// # Errors
    ///
    /// [`BuildError::ManagedPath`] when the job path is unsafe,
    /// [`BuildError::DurableState`] when a lock is malformed, incompatible,
    /// or live, or [`BuildError::Io`] when lock storage fails.
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError>;

    /// Loads and validates the current document when one exists.
    ///
    /// # Errors
    ///
    /// [`DurableStateError::UnsupportedDurableRecord`] when the record
    /// declares a version this build does not read — including every
    /// provisional `e0.job-state.0.1` snapshot, which is refused rather than
    /// migrated; [`DurableStateError::MalformedJobSnapshot`], including when
    /// a recorded digest is not one and its value object refuses it during
    /// parsing; [`DurableStateError::JobSnapshotIdentityMismatch`],
    /// [`DurableStateError::JobSnapshotAttemptMismatch`],
    /// [`DurableStateError::JobSnapshotLastSuccessfulStateMismatch`],
    /// [`DurableStateError::JobSnapshotSelectionMismatch`], or
    /// [`DurableStateError::JobSnapshotPackageIdentityMismatch`] when durable
    /// state cannot be trusted;
    /// [`ReleaseError::PrivateProfileCannotClaimProduction`] when a private
    /// preview records the production-only `Published` state;
    /// [`DurableStateError::MalformedJobEventLog`]
    /// when the append-only companion has a malformed, foreign, or partial
    /// line; [`DurableStateError::DurableRecordTooLarge`],
    /// [`DurableStateError::JobEventLineTooLarge`], or
    /// [`DurableStateError::JobSnapshotSegmentCountExceeded`] when a resource
    /// ceiling is exceeded;
    /// [`BuildError::ManagedPath`] or [`BuildError::Io`] when the managed
    /// documents cannot be located or read.
    fn load(&self, workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError>;

    /// Atomically replaces the authoritative document while ownership is held.
    ///
    /// # Errors
    ///
    /// The validation variants documented by [`Self::load`],
    /// [`DurableStateError::JobReplacementPredecessorMismatch`] when a new
    /// attempt does not continue the document on disk,
    /// [`DurableStateError::MalformedJobEventLog`] when the event that
    /// records the replacement cannot be appended, [`BuildError::ManagedPath`]
    /// for an unsafe destination, or [`BuildError::Io`] when serialization or
    /// atomic replacement fails.
    fn replace(&self, workspace: &Path, document: &JobDocument) -> Result<(), BuildError>;

    /// Retains the validated lesson bytes and the plan beside the document,
    /// ADR-0001 §12.1, so a job can be resumed from its identity alone.
    ///
    /// # Errors
    ///
    /// [`BuildError::ManagedPath`] for an unsafe destination, or
    /// [`BuildError::Io`] when staging or atomic replacement fails.
    fn retain_inputs(
        &self,
        workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError>;

    /// Reads back the retained lesson bytes, or `None` when nothing was
    /// retained.
    ///
    /// # Errors
    ///
    /// [`crate::IoError::ReadFile`] when the retained lesson cannot be read,
    /// [`crate::IoError::LessonNotRegularFile`] when something else occupies
    /// its name, [`study_tts_core::LessonError::LessonJsonTooLarge`] when it
    /// exceeds the lesson size ceiling, or [`BuildError::ManagedPath`] for an
    /// unsafe job path.
    fn retained_lesson(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError>;

    /// Reads and validates the retained plan, or `None` when none exists.
    ///
    /// # Errors
    ///
    /// [`DurableStateError::MalformedRetainedPlan`],
    /// [`DurableStateError::RetainedPlanIdentityMismatch`],
    /// [`DurableStateError::RetainedPlanHashMismatch`],
    /// [`DurableStateError::RetainedPlanSegmentCountExceeded`],
    /// [`DurableStateError::DurableRecordTooLarge`], or
    /// [`DurableStateError::UnsupportedDurableRecord`] when the plan cannot be
    /// trusted; [`BuildError::ManagedPath`] or [`BuildError::Io`] when its path
    /// cannot be safely located or read.
    fn retained_plan(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError>;

    /// Checks a recorded preview completion against reconciled output state.
    ///
    /// A document with no preview completion accepts a selected output: the
    /// package may have become durable before `job.json` advanced. A recorded
    /// completion, however, must name the selected package exactly, whose
    /// validated manifest must name the recorded plan. This is ADR-0001 §12.7
    /// steps 3–4 at the job-state boundary.
    ///
    /// # Errors
    ///
    /// [`DurableStateError::JobPreviewSelectionMismatch`] when the records
    /// disagree, plus the exact durable-state, containment, and filesystem
    /// errors raised while validating `current.json` and its package.
    fn validate_preview_selection(
        &self,
        workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError>;
}

/// Linux-filesystem job repository.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemJobRepository;

impl JobRepository for FileSystemJobRepository {
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        let filesystem = OsDurableFileSystem;
        let roots = preview::roots(workspace, job_id)?;
        let lock = locking::acquire_job_lock(&filesystem, &roots.job_dir, job_id)?;
        Ok(Box::new(lock))
    }

    fn load(&self, workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError> {
        let path = job_document_path(workspace, job_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let document = load_document(&path, job_id)?;
        validate_event_log(path.parent().unwrap_or(workspace), job_id)?;
        Ok(Some(document))
    }

    fn replace(&self, workspace: &Path, document: &JobDocument) -> Result<(), BuildError> {
        replace_document(&OsDurableFileSystem, workspace, document)
    }

    fn retain_inputs(
        &self,
        workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError> {
        let job_dir = job_directory(workspace, job_id)?;
        write_bytes_atomically(
            &OsDurableFileSystem,
            &managed::leaf(&job_dir, RETAINED_LESSON_NAME)?,
            lesson,
        )?;
        write_json_atomically(
            &OsDurableFileSystem,
            &managed::leaf(&job_dir, RETAINED_PLAN_NAME)?,
            plan,
        )
    }

    fn retained_lesson(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        let path = managed::leaf(&job_directory(workspace, job_id)?, RETAINED_LESSON_NAME)?;
        if !path.exists() {
            return Ok(None);
        }
        // The same bounded reader the build used, so the retained copy is
        // trusted no further than the author's original was.
        pipeline::read_lesson(&path).map(Some)
    }

    fn retained_plan(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError> {
        let path = managed::leaf(&job_directory(workspace, job_id)?, RETAINED_PLAN_NAME)?;
        if !path.exists() {
            return Ok(None);
        }
        load_retained_plan(&path, job_id).map(Some)
    }

    fn validate_preview_selection(
        &self,
        workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError> {
        let Some(recorded) = &document.preview_package else {
            return Ok(());
        };
        let roots = preview::roots(workspace, &document.job_id)?;
        let selected =
            preview::current_manifest_digest(&roots, &document.job_id, &document.plan_hash)?;
        if selected.as_ref() == Some(&recorded.manifest_blake3) {
            return Ok(());
        }
        Err(DurableStateError::JobPreviewSelectionMismatch {
            job_id: document.job_id.clone(),
            recorded: recorded.manifest_blake3.as_str().to_owned(),
            selected: selected.map(|digest| digest.as_str().to_owned()),
        }
        .into())
    }
}

/// Replaces `job.json` atomically, then records that it happened.
///
/// The event append is the last statement and runs only when the durable
/// replacement returned `Ok`: ADR-0001 §12.3 step 5. Because that ordering
/// leaves the append no way to refuse without stranding a state already
/// recorded, the event is built and preflighted first, while `job.json` still
/// holds its prior bytes. The filesystem is a parameter so a test can
/// interrupt the rename and prove no event describes the state that was never
/// reached.
fn replace_document(
    filesystem: &dyn DurableFileSystem,
    workspace: &Path,
    document: &JobDocument,
) -> Result<(), BuildError> {
    let path = job_document_path(workspace, &document.job_id)?;
    validate_document(&path, &document.job_id, document)?;
    if path.exists() {
        let previous = load_document(&path, &document.job_id)?;
        validate_replacement(&path, &previous, document)?;
    } else {
        validate_initial_state(document)?;
    }
    let job_dir = path.parent().unwrap_or(workspace);
    let event = JobEvent::new(
        &document.job_id,
        Some(document.build_attempt.get()),
        JobEventKind::StateDurable {
            state: document.state,
        },
    );
    preflight_append(job_dir, &event)?;
    write_json_atomically(filesystem, &path, document)?;
    append_event(filesystem, job_dir, &event)
}

fn load_retained_plan(path: &Path, job_id: &str) -> Result<RenderPlan, BuildError> {
    let bytes = read_bounded_bytes(path, MAX_RETAINED_PLAN_JSON_BYTES)?;
    let malformed = |source| DurableStateError::MalformedRetainedPlan {
        path: path.to_path_buf(),
        source,
    };
    let declared: DeclaredVersion = serde_json::from_slice(&bytes).map_err(malformed)?;
    let accepted = declared.schema_version.parse().ok().is_some_and(
        |version: study_tts_core::SchemaVersion| version.accepted_by(PLAN_SCHEMA_VERSION).is_ok(),
    );
    if !accepted {
        return Err(DurableStateError::UnsupportedDurableRecord {
            path: path.to_path_buf(),
            schema_version: declared.schema_version,
        }
        .into());
    }
    let plan: RenderPlan = serde_json::from_slice(&bytes).map_err(malformed)?;
    if plan.segments.len() > MAX_LESSON_SEGMENTS {
        return Err(DurableStateError::RetainedPlanSegmentCountExceeded {
            path: path.to_path_buf(),
            found: plan.segments.len(),
            max: MAX_LESSON_SEGMENTS,
        }
        .into());
    }
    if plan.lesson_id != job_id {
        return Err(DurableStateError::RetainedPlanIdentityMismatch {
            path: path.to_path_buf(),
            required: job_id.to_owned(),
            actual: plan.lesson_id,
        }
        .into());
    }
    let actual = plan.derived_hash();
    if plan.plan_hash != actual {
        return Err(DurableStateError::RetainedPlanHashMismatch {
            path: path.to_path_buf(),
            recorded: plan.plan_hash.as_str().to_owned(),
            actual: actual.as_str().to_owned(),
        }
        .into());
    }
    // The hash above covers the identity fields and deliberately not the two
    // selection fields ADR-0001 §12.2 has the plan record for audit. This is
    // the verification those are recorded under instead, and it is why they
    // can stay outside the identity — see
    // `RenderPlan::verify_recorded_selection`.
    plan.verify_recorded_selection()?;
    Ok(plan)
}

/// The version field alone, read before the strict parse so a record from
/// another version is reported as unsupported rather than as malformed.
#[derive(Deserialize)]
struct DeclaredVersion {
    schema_version: String,
}

fn load_document(path: &Path, job_id: &str) -> Result<JobDocument, BuildError> {
    let bytes = read_bounded_bytes(path, MAX_JOB_JSON_BYTES)?;
    let malformed = |source| DurableStateError::MalformedJobSnapshot {
        path: path.to_path_buf(),
        source,
    };
    let declared: DeclaredVersion = serde_json::from_slice(&bytes).map_err(malformed)?;
    let accepted = declared.schema_version.parse().ok().is_some_and(
        |version: study_tts_core::SchemaVersion| version.accepted_by(JOB_SCHEMA_VERSION).is_ok(),
    );
    if !accepted {
        return Err(DurableStateError::UnsupportedDurableRecord {
            path: path.to_path_buf(),
            schema_version: declared.schema_version,
        }
        .into());
    }
    let document: JobDocument = serde_json::from_slice(&bytes).map_err(malformed)?;
    validate_document(path, job_id, &document)?;
    Ok(document)
}

fn job_directory(workspace: &Path, job_id: &str) -> Result<PathBuf, BuildError> {
    let jobs = managed::subdirectory(workspace, "jobs")?;
    managed::subdirectory(&jobs, job_id)
}

fn job_document_path(workspace: &Path, job_id: &str) -> Result<PathBuf, BuildError> {
    managed::leaf(&job_directory(workspace, job_id)?, JOB_DOCUMENT_NAME)
}

fn validate_document(path: &Path, job_id: &str, document: &JobDocument) -> Result<(), BuildError> {
    if document
        .schema_version
        .accepted_by(JOB_SCHEMA_VERSION)
        .is_err()
    {
        return Err(DurableStateError::UnsupportedDurableRecord {
            path: path.to_path_buf(),
            schema_version: document.schema_version.to_string(),
        }
        .into());
    }
    if document.job_id != job_id {
        return Err(DurableStateError::JobSnapshotIdentityMismatch {
            path: path.to_path_buf(),
            recorded: document.job_id.clone(),
            required: job_id.to_owned(),
        }
        .into());
    }
    if document.segments.len() > MAX_LESSON_SEGMENTS {
        return Err(DurableStateError::JobSnapshotSegmentCountExceeded {
            path: path.to_path_buf(),
            found: document.segments.len(),
            max: MAX_LESSON_SEGMENTS,
        }
        .into());
    }
    if document.state == JobState::Published
        && document.release_status != ReleaseStatus::ProductionRelease
    {
        return Err(ReleaseError::PrivateProfileCannotClaimProduction.into());
    }
    if !attempts_are_coherent(document) {
        return Err(DurableStateError::JobSnapshotAttemptMismatch {
            path: path.to_path_buf(),
            build_attempt: document.build_attempt.get(),
            abandoned_attempt: document
                .abandoned_attempt
                .map(|attempt| attempt.build_attempt.get()),
        }
        .into());
    }
    if !states_are_coherent(document.state, document.last_successful_state) {
        return Err(DurableStateError::JobSnapshotLastSuccessfulStateMismatch {
            path: path.to_path_buf(),
            state: format!("{:?}", document.state),
            last_successful_state: format!("{:?}", document.last_successful_state),
        }
        .into());
    }
    if document.preview_package.is_some() && !document.last_successful_state.has_rendered() {
        return Err(DurableStateError::JobSnapshotSelectionMismatch {
            path: path.to_path_buf(),
            state: format!("{:?}", document.state),
        }
        .into());
    }
    if let Some(package) = &document.preview_package
        && package.package_id != package.manifest_blake3
    {
        return Err(DurableStateError::JobSnapshotPackageIdentityMismatch {
            path: path.to_path_buf(),
            package_id: package.package_id.as_str().to_owned(),
            manifest_blake3: package.manifest_blake3.as_str().to_owned(),
        }
        .into());
    }
    Ok(())
}

fn validate_replacement(
    path: &Path,
    current: &JobDocument,
    replacement: &JobDocument,
) -> Result<(), BuildError> {
    if replacement.build_attempt == current.build_attempt {
        if replacement.abandoned_attempt != current.abandoned_attempt {
            return Err(replacement_predecessor_mismatch(path, current, replacement));
        }
        if replacement.state != current.state && !current.state.may_transition_to(replacement.state)
        {
            return Err(DurableStateError::IllegalJobTransition {
                job_id: replacement.job_id.clone(),
                from: current.state,
                to: replacement.state,
            }
            .into());
        }
        return Ok(());
    }

    let expected_attempt = current.build_attempt.checked_add(1);
    let expected_predecessor = Some(AbandonedAttempt {
        build_attempt: current.build_attempt,
        state: current.state,
    });
    if Some(replacement.build_attempt) != expected_attempt
        || replacement.abandoned_attempt != expected_predecessor
    {
        return Err(replacement_predecessor_mismatch(path, current, replacement));
    }

    // The pipeline validates and plans before its first durable write, so a
    // new attempt may first appear at `Planned`; no later state may be skipped.
    validate_initial_state(replacement)
}

fn validate_initial_state(document: &JobDocument) -> Result<(), BuildError> {
    if matches!(
        document.state,
        JobState::Created | JobState::Validated | JobState::Planned
    ) {
        return Ok(());
    }
    Err(DurableStateError::IllegalJobTransition {
        job_id: document.job_id.clone(),
        from: JobState::Created,
        to: document.state,
    }
    .into())
}

fn replacement_predecessor_mismatch(
    path: &Path,
    current: &JobDocument,
    replacement: &JobDocument,
) -> BuildError {
    DurableStateError::JobReplacementPredecessorMismatch {
        path: path.to_path_buf(),
        current_attempt: current.build_attempt.get(),
        current_state: current.state,
        replacement_attempt: replacement.build_attempt.get(),
        abandoned_attempt: replacement
            .abandoned_attempt
            .map(|attempt| attempt.build_attempt.get()),
        abandoned_state: replacement.abandoned_attempt.map(|attempt| attempt.state),
    }
    .into()
}

fn attempts_are_coherent(document: &JobDocument) -> bool {
    match document.abandoned_attempt {
        None => document.build_attempt == std::num::NonZeroU32::MIN,
        Some(abandoned) => abandoned.build_attempt.checked_add(1) == Some(document.build_attempt),
    }
}

fn states_are_coherent(state: JobState, last_successful_state: JobState) -> bool {
    match state {
        JobState::Failed => matches!(
            last_successful_state,
            JobState::Rendering | JobState::Verifying | JobState::Assembling
        ),
        JobState::Cancelled => last_successful_state == JobState::Rendering,
        _ => state == last_successful_state,
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use study_tts_core::{
        CacheKey, DeliveryStyle, JobDocument, JobState, MAX_LESSON_SEGMENTS, ManifestDigest,
        PLAN_SCHEMA_VERSION, PlannedSegment, ReleaseError, RenderPlan, SegmentStatus,
        SelectedPackageIdentity, TakeSelectionSource,
    };
    use tempfile::TempDir;

    use super::{
        FileSystemJobRepository, JobRepository, MAX_JOB_JSON_BYTES, load_document,
        load_retained_plan, replace_document, validate_document,
    };
    use crate::{
        BuildError, DurableStateError, IoError, PublicationError,
        durable::{OsDurableFileSystem, fault::FailingReplacementFileSystem},
        job_events::{self, MAX_JOB_EVENT_LOG_BYTES},
    };

    const JOB_ID: &str = "job-1";
    const DOCUMENT_PATH: &str = "jobs/job-1/job.json";

    fn document(state: JobState) -> JobDocument {
        JobDocument {
            state,
            last_successful_state: state,
            ..JobDocument::open_attempt(
                JOB_ID,
                "a".repeat(64).parse().expect("a digest of a parses"),
                "b".repeat(64).parse().expect("a digest of b parses"),
                None,
            )
            .expect("the first attempt is representable")
        }
    }

    fn selected_package() -> SelectedPackageIdentity {
        let manifest_blake3: ManifestDigest = "c".repeat(64).parse().expect("a digest of c parses");
        SelectedPackageIdentity {
            package_id: manifest_blake3.clone(),
            manifest_blake3,
        }
    }

    fn validation_error(document: &JobDocument, job_id: &str) -> BuildError {
        validate_document(Path::new(DOCUMENT_PATH), job_id, document)
            .expect_err("the incoherent document must be refused")
    }

    #[test]
    fn t4_e2_interrupt_before_rename_preserves_prior_state() {
        let workspace = TempDir::new().expect("create workspace");
        let job_dir = workspace.path().join("jobs").join(JOB_ID);
        replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Planned),
        )
        .expect("the prior state is durable");
        let prior = std::fs::read(job_dir.join("job.json")).expect("read prior record");

        let error = replace_document(
            &FailingReplacementFileSystem::default(),
            workspace.path(),
            &document(JobState::Rendering),
        )
        .expect_err("the injected rename interruption must surface");

        assert!(matches!(error, BuildError::Io(IoError::FileSystem { .. })));
        assert_eq!(
            std::fs::read(job_dir.join("job.json")).expect("read record"),
            prior,
            "the authoritative record keeps its prior bytes"
        );
        let events = job_events::read_events(&job_dir).expect("the event log parses");
        assert_eq!(
            events.len(),
            1,
            "no event may describe the state whose rename never happened"
        );
        assert_eq!(
            events[0].kind,
            job_events::JobEventKind::StateDurable {
                state: JobState::Planned,
            }
        );
    }

    #[test]
    fn t4_e2_illegal_state_replacement_preserves_prior_state() {
        let workspace = TempDir::new().expect("create workspace");
        let prior = document(JobState::Planned);
        replace_document(&OsDurableFileSystem, workspace.path(), &prior)
            .expect("the prior state is durable");
        let path = workspace.path().join(DOCUMENT_PATH);
        let prior_bytes = std::fs::read(&path).expect("read prior record");

        let error = replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Rendered),
        )
        .expect_err("a replacement must not skip the rendering state");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::IllegalJobTransition {
                        from: JobState::Planned,
                        to: JobState::Rendered,
                        ..
                    }
                )
        ));
        assert_eq!(
            std::fs::read(path).expect("job document remains"),
            prior_bytes
        );
    }

    #[test]
    fn t4_e2_initial_job_document_cannot_skip_planning() {
        let workspace = TempDir::new().expect("create workspace");

        let error = replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Rendered),
        )
        .expect_err("the first durable document cannot skip planning and rendering");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::IllegalJobTransition {
                        from: JobState::Created,
                        to: JobState::Rendered,
                        ..
                    }
                )
        ));
        assert!(!workspace.path().join(DOCUMENT_PATH).exists());
    }

    #[test]
    fn t4_e2_replacement_must_name_the_document_on_disk_as_its_predecessor() {
        let workspace = TempDir::new().expect("create workspace");
        let current = document(JobState::Planned);
        replace_document(&OsDurableFileSystem, workspace.path(), &current)
            .expect("the current attempt is durable");
        let path = workspace.path().join(DOCUMENT_PATH);
        let current_bytes = std::fs::read(&path).expect("read current attempt");
        let false_predecessor = document(JobState::Rendered);
        let replacement = JobDocument::open_attempt(
            JOB_ID,
            current.lesson_blake3.clone(),
            current.plan_hash.clone(),
            Some(&false_predecessor),
        )
        .and_then(|document| document.transition(JobState::Validated))
        .and_then(|document| document.transition(JobState::Planned))
        .expect("the replacement is internally coherent");

        let error = replace_document(&OsDurableFileSystem, workspace.path(), &replacement)
            .expect_err("the replacement must name the actual predecessor");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::JobReplacementPredecessorMismatch {
                        current_attempt: 1,
                        current_state: JobState::Planned,
                        replacement_attempt: 2,
                        abandoned_attempt: Some(1),
                        abandoned_state: Some(JobState::Rendered),
                        ..
                    }
                )
        ));
        assert_eq!(
            std::fs::read(path).expect("job document remains"),
            current_bytes
        );
    }

    #[test]
    fn t4_e2_private_preview_cannot_be_persisted_as_published() {
        let workspace = TempDir::new().expect("create workspace");

        let error = replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Published),
        )
        .expect_err("a private preview must not persist a production state");

        assert!(matches!(
            error,
            BuildError::Publication(PublicationError::Release(
                ReleaseError::PrivateProfileCannotClaimProduction
            ))
        ));
        assert!(!workspace.path().join(DOCUMENT_PATH).exists());
    }

    #[test]
    fn t4_e2_a_partial_event_log_refuses_state_replacement() {
        let workspace = TempDir::new().expect("create workspace");
        let job_dir = workspace.path().join("jobs").join(JOB_ID);
        replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Planned),
        )
        .expect("the prior state is durable");
        let path = job_dir.join("job.json");
        let prior = std::fs::read(&path).expect("read prior record");
        std::fs::write(job_dir.join("events.ndjson"), b"{torn").expect("write torn event log");

        let error = replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Rendering),
        )
        .expect_err("a torn event log must refuse the replacement before it becomes durable");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::MalformedJobEventLog { .. })
        ));
        assert_eq!(
            std::fs::read(path).expect("job document remains"),
            prior,
            "event-log corruption must not advance authoritative state"
        );
    }

    #[test]
    fn t4_e2_a_full_event_log_refuses_state_replacement() {
        let workspace = TempDir::new().expect("create workspace");
        let job_dir = workspace.path().join("jobs").join(JOB_ID);
        replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Planned),
        )
        .expect("the prior state is durable");
        let path = job_dir.join("job.json");
        let prior = std::fs::read(&path).expect("read prior record");
        // Valid lines, repeated until one more cannot fit: the log is within
        // its ceiling, so only the pending append exceeds it.
        let line = std::fs::read(job_dir.join("events.ndjson")).expect("the first event line");
        std::fs::write(
            job_dir.join("events.ndjson"),
            line.repeat(MAX_JOB_EVENT_LOG_BYTES / line.len()),
        )
        .expect("fill the event log to its ceiling");

        let error = replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Rendering),
        )
        .expect_err("an append that cannot fit must refuse before the replacement");

        assert!(
            matches!(
                error,
                BuildError::DurableState(ref state)
                    if matches!(
                        **state,
                        DurableStateError::DurableRecordTooLarge {
                            max_bytes: MAX_JOB_EVENT_LOG_BYTES,
                            ..
                        }
                    )
            ),
            "{error}"
        );
        assert_eq!(
            std::fs::read(path).expect("job document remains"),
            prior,
            "a state no event could record must not become authoritative"
        );
    }

    #[test]
    fn t4_e2_a_partial_event_log_refuses_job_load() {
        let workspace = TempDir::new().expect("create workspace");
        let job_dir = workspace.path().join("jobs").join(JOB_ID);
        replace_document(
            &OsDurableFileSystem,
            workspace.path(),
            &document(JobState::Planned),
        )
        .expect("the prior state is durable");
        std::fs::write(job_dir.join("events.ndjson"), b"{torn").expect("write torn event log");

        let error = FileSystemJobRepository
            .load(workspace.path(), JOB_ID)
            .expect_err("a torn authoritative event log must refuse the job load");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::MalformedJobEventLog { .. })
        ));
    }

    #[test]
    fn t4_e2_unsupported_job_record_version_is_refused_without_migration() {
        let workspace = TempDir::new().expect("create workspace");
        let path = workspace.path().join("job.json");
        // The provisional E0 snapshot, byte for byte as E1 wrote it.
        let provisional = concat!(
            "{\"schema_version\":\"e0.job-state.0.1\",\"job_id\":\"job-1\",",
            "\"plan_hash\":\"",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "\",\"stage\":\"planned\",\"selected_package\":null}",
        );
        std::fs::write(&path, provisional).expect("write provisional record");

        let error = load_document(&path, JOB_ID).expect_err("an E0 record is not migrated");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::UnsupportedDurableRecord { ref schema_version, .. }
                        if schema_version == "e0.job-state.0.1"
                )
        ));
        assert_eq!(
            std::fs::read(&path).expect("record remains").as_slice(),
            provisional.as_bytes()
        );
    }

    #[test]
    fn t4_e2_job_document_size_is_bounded_before_decoding() {
        let workspace = TempDir::new().expect("create workspace");
        let path = workspace.path().join("job.json");
        let bytes = vec![b' '; MAX_JOB_JSON_BYTES + 1];
        std::fs::write(&path, &bytes).expect("write oversized job document");

        let error = load_document(&path, JOB_ID).expect_err("an oversized job must be refused");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::DurableRecordTooLarge {
                        max_bytes: MAX_JOB_JSON_BYTES,
                        ..
                    }
                )
        ));
        assert_eq!(std::fs::read(path).expect("job remains"), bytes);
    }

    #[test]
    fn t1_e2_job_document_segment_count_is_bounded() {
        let mut document = document(JobState::Rendered);
        let status = SegmentStatus {
            cache_key: "e".repeat(64).parse().expect("a digest of e parses"),
            audio_blake3: "f".repeat(64).parse().expect("a digest of f parses"),
        };
        for index in 0..=MAX_LESSON_SEGMENTS {
            document
                .segments
                .insert(format!("seg-{index}"), status.clone());
        }

        let error = validation_error(&document, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::JobSnapshotSegmentCountExceeded {
                        found,
                        max: MAX_LESSON_SEGMENTS,
                        ..
                    } if found == MAX_LESSON_SEGMENTS + 1
                )
        ));
    }

    #[test]
    fn t4_e2_retained_plan_segment_count_is_bounded() {
        let root = TempDir::new().expect("create job directory");
        let path = root.path().join("plan.json");
        let segment = PlannedSegment {
            id: "seg-1".to_owned(),
            speaker: "speaker".to_owned(),
            voice_profile: "voice".to_owned(),
            display_text: "text".to_owned(),
            spoken_text: "text".to_owned(),
            style: DeliveryStyle::Calm,
            pause_after_ms: 0,
            take: 0,
            cache_key: "e".repeat(CacheKey::LENGTH).parse().expect("cache key"),
            synthesis_base_key: "e".repeat(CacheKey::LENGTH).parse().expect("base key"),
            audio_blake3: None,
        };
        let plan = RenderPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            lesson_id: JOB_ID.to_owned(),
            plan_hash: "f".repeat(64).parse().expect("plan hash"),
            take_selection_source: TakeSelectionSource::Implicit,
            segments: vec![segment; MAX_LESSON_SEGMENTS + 1],
        };
        std::fs::write(
            &path,
            serde_json::to_vec(&plan).expect("serialize oversized plan"),
        )
        .expect("write oversized plan");

        let error = load_retained_plan(&path, JOB_ID)
            .expect_err("an oversized retained plan must be refused");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::RetainedPlanSegmentCountExceeded {
                        found,
                        max: MAX_LESSON_SEGMENTS,
                        ..
                    } if found == MAX_LESSON_SEGMENTS + 1
                )
        ));
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_unsupported_schema_version() {
        let mut document = document(JobState::Planned);
        document.schema_version = study_tts_core::SchemaVersion::new(9, 0);

        let error = validation_error(&document, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::UnsupportedDurableRecord {
                        path,
                        schema_version,
                    } if path == &PathBuf::from(DOCUMENT_PATH) && schema_version == "9.0"
                )
        ));
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_job_identity_mismatch() {
        let document = document(JobState::Planned);

        let error = validation_error(&document, "job-2");

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::JobSnapshotIdentityMismatch {
                        path,
                        recorded,
                        required,
                    } if path == &PathBuf::from(DOCUMENT_PATH)
                        && recorded == JOB_ID
                        && required == "job-2"
                )
        ));
    }

    #[test]
    fn t1_e2_a_preview_package_before_rendering_is_refused() {
        let premature = document(JobState::Rendering).with_preview_package(selected_package());

        let error = validation_error(&premature, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::JobSnapshotSelectionMismatch { state, .. }
                        if state == "Rendering"
                )
        ));
        validate_document(
            Path::new(DOCUMENT_PATH),
            JOB_ID,
            &document(JobState::Rendered).with_preview_package(selected_package()),
        )
        .expect("a package recorded once rendering is complete is coherent");
    }

    #[test]
    fn t1_e2_a_preview_package_identity_must_agree_with_its_manifest() {
        let mismatch = SelectedPackageIdentity {
            package_id: "c".repeat(64).parse().expect("a digest of c parses"),
            manifest_blake3: "d".repeat(64).parse().expect("a digest of d parses"),
        };
        let document = document(JobState::Rendered).with_preview_package(mismatch);

        let error = validation_error(&document, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::JobSnapshotPackageIdentityMismatch { .. }
                )
        ));
    }

    #[test]
    fn t1_e2_a_failed_post_render_attempt_retains_preview_completion() {
        let verifying = document(JobState::Rendered)
            .with_preview_package(selected_package())
            .transition(JobState::Verifying)
            .expect("rendered audio may enter verification");
        let assembling = verifying
            .clone()
            .transition(JobState::Verified)
            .and_then(|document| document.transition(JobState::Assembling))
            .expect("verified audio may enter assembly");

        for document in [verifying, assembling] {
            let failed = document
                .transition(JobState::Failed)
                .expect("verification and assembly may fail");
            validate_document(Path::new(DOCUMENT_PATH), JOB_ID, &failed)
                .expect("a post-render failure retains its valid preview completion");
        }
    }

    #[test]
    fn t1_e2_last_successful_state_must_explain_the_current_state() {
        let incoherent = JobDocument {
            last_successful_state: JobState::Created,
            ..document(JobState::Rendered)
        };

        let error = validation_error(&incoherent, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::JobSnapshotLastSuccessfulStateMismatch { .. }
                )
        ));
    }

    #[test]
    fn t1_e2_abandoned_attempt_must_immediately_precede_the_current_attempt() {
        let first_with_predecessor = JobDocument {
            abandoned_attempt: Some(study_tts_core::AbandonedAttempt {
                build_attempt: std::num::NonZeroU32::MIN,
                state: JobState::Rendered,
            }),
            ..document(JobState::Planned)
        };
        let later_without_predecessor = JobDocument {
            build_attempt: std::num::NonZeroU32::new(2).expect("two is nonzero"),
            ..document(JobState::Planned)
        };
        let self_predecessor = JobDocument {
            build_attempt: std::num::NonZeroU32::new(2).expect("two is nonzero"),
            abandoned_attempt: Some(study_tts_core::AbandonedAttempt {
                build_attempt: std::num::NonZeroU32::new(2).expect("two is nonzero"),
                state: JobState::Rendered,
            }),
            ..document(JobState::Planned)
        };

        for incoherent in [
            first_with_predecessor,
            later_without_predecessor,
            self_predecessor,
        ] {
            let error = validation_error(&incoherent, JOB_ID);
            assert!(matches!(
                error,
                BuildError::DurableState(ref state)
                    if matches!(
                        **state,
                        DurableStateError::JobSnapshotAttemptMismatch { .. }
                    )
            ));
        }

        let second = JobDocument::open_attempt(
            JOB_ID,
            document(JobState::Rendered).lesson_blake3,
            document(JobState::Rendered).plan_hash,
            Some(&document(JobState::Rendered)),
        )
        .expect("the second attempt is coherent");
        validate_document(Path::new(DOCUMENT_PATH), JOB_ID, &second)
            .expect("the immediately preceding attempt is accepted");
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_a_recorded_digest_that_is_not_one() {
        // Refused at the parse rather than by a check after it, because every
        // recorded digest is a value object. That a document carrying a
        // malformed digest can no longer be *constructed* is the point, so the
        // case has to be expressed as the JSON a repository actually reads
        // back.
        //
        // The offending value is asserted, not just the failure: a mistyped
        // pointer here would also fail to deserialize, and a variant-only
        // check would pass without the digest ever having been exercised.
        let complete = document(JobState::Rendered)
            .with_segment(
                "seg-1",
                SegmentStatus {
                    cache_key: "e".repeat(64).parse().expect("a digest of e parses"),
                    audio_blake3: "f".repeat(64).parse().expect("a digest of f parses"),
                },
            )
            .with_preview_package(selected_package());

        for (pointer, malformed) in [
            ("/plan_hash", "not-a-plan-hash"),
            ("/lesson_blake3", "not-a-lesson-digest"),
            ("/preview_package/package_id", "not-a-package-id"),
            ("/preview_package/manifest_blake3", "not-a-manifest-digest"),
            ("/segments/seg-1/cache_key", "not-a-cache-key"),
            ("/segments/seg-1/audio_blake3", "not-an-audio-digest"),
        ] {
            let mut recorded = serde_json::to_value(&complete).expect("a job document serializes");
            *recorded
                .pointer_mut(pointer)
                .expect("the pointer names a recorded field") = Value::String(malformed.to_owned());

            let error = serde_json::from_value::<JobDocument>(recorded)
                .expect_err("a recorded digest that is not one must not deserialize");

            assert!(
                error.to_string().contains(malformed),
                "refusing `{pointer}` must name the offending value: {error}"
            );
        }
    }
}
