//! The order a preview build happens in, and the gates that precede it.
//!
//! Lesson and voice gates precede executor validation and tool preflight; all
//! of them precede durable build writes and synthesis. That ordering is the
//! point of this module: a refusal must name the policy that refused rather
//! than the first thing that happened to break. Tests pin the order with
//! observable fakes and missing tools.

use std::{
    collections::BTreeMap,
    fs::{self, File},
    future::Future,
    io::Read,
    path::{Path, PathBuf},
    pin::pin,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    thread,
};

use serde::Deserialize;
use serde_json::Value;
use study_tts_core::{
    AudioDigest, CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, JobDocument,
    JobState, LessonDiagnostic, LessonDigest, MAX_LESSON_JSON_BYTES, MAX_TAKES_JSON_BYTES,
    PlanError, ReleaseClaim, ReleaseStatus, RenderPlan, RightsDecision, SegmentStatus,
    SourceRightsDeclaration, SynthesisContext, TakeSelection, TakeSelectionSource, TakesError,
    ValidatedLesson, ValidatedTakes, VoiceError, VoiceUse, validate_lesson_id,
};

use crate::{
    BackendError, BuildError, CachePublisher, CacheResolveRequest, DurableStateError,
    FileSystemCachePublisher, FileSystemJobRepository, FileSystemPackageWriter, IoError,
    JobOwnership, JobRepository, PackagePreflightRequest, PackagePrepareRequest,
    PackageWriteRequest, PackageWriter, PreparedPackageWriter, PublicationError, RightsError,
    SynthesisRequest, TtsExecutor, durable::read_bounded_bytes, export, io_error, managed, tools,
    voice_gate,
};

/// Everything one preview build needs, named explicitly rather than read from
/// ambient state.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    /// The lesson document to build; its validated `lesson_id` is the job,
    /// lock, and publication identity.
    pub lesson_path: PathBuf,
    /// Root the build owns; outputs, cache, and staging all resolve beneath it.
    pub workspace: PathBuf,
    /// FFmpeg to encode with, resolved and version-probed before any work
    /// begins.
    pub ffmpeg_executable: PathBuf,
    /// ffprobe to validate the encoded output with, on the same terms.
    pub ffprobe_executable: PathBuf,
    /// Root holding one ADR-0001 §12.1 profile directory per voice profile a
    /// lesson may name, gated fail-closed before any tool or synthesis work.
    ///
    /// Required rather than optional since E1-S2: the conditioning artifact
    /// under each profile is a §12.5 synthesis-key input, so a build with
    /// nowhere to resolve one would derive cache keys for voices it never
    /// loaded.
    pub voice_profile_root: PathBuf,
    /// Segments to render an alternate performance of, and at which take.
    ///
    /// The library seam behind ADR-0001 §12.1's `study-tts retake`, whose
    /// command belongs to E2-S5. Empty for an ordinary build, which is the
    /// common case and the reason this is a map rather than an option: a
    /// retake is per segment, and §11.4 requires the segments beside it to keep
    /// their identity.
    ///
    /// A takes document records a selection among takes that *exist*, so it
    /// cannot request one that does not: `audio_blake3` names audio a reviewer
    /// listened to. This is the request path, and
    /// [`TakeSelection::with_retakes`] states what it does to a selection.
    pub retakes: BTreeMap<String, u32>,
}

/// What a resume needs that the job directory does not hold.
///
/// The lesson and plan are read back from `jobs/<job-id>/`; tools and the
/// voice root are environment, and a resumed build is gated on them exactly
/// as a fresh one is.
#[derive(Clone, Debug)]
pub struct ResumeRequest {
    /// The job to resume; the validated `lesson_id` its build recorded.
    pub job_id: String,
    /// Root the job lives beneath.
    pub workspace: PathBuf,
    /// FFmpeg to encode with, resolved and version-probed before any work.
    pub ffmpeg_executable: PathBuf,
    /// ffprobe to validate the encoded output with, on the same terms.
    pub ffprobe_executable: PathBuf,
    /// Root holding the voice profiles the retained lesson names.
    pub voice_profile_root: PathBuf,
}

/// What a successful preview build wrote.
#[derive(Clone, Debug)]
pub struct BuildResult {
    /// Immutable directory holding the selected preview generation.
    pub package_dir: PathBuf,
    /// Atomic record selecting `package_dir` for preview consumers.
    pub publication_record: PathBuf,
    /// The assembled canonical-format master.
    pub master_wav: PathBuf,
    /// The default listening file, encoded from the master.
    pub m4a: PathBuf,
    /// The compatibility output, encoded independently from the master.
    pub mp3: PathBuf,
    /// The readable speaker-labelled transcript.
    pub transcript: PathBuf,
    /// The segment-level WebVTT captions.
    pub captions: PathBuf,
    /// The FFMETADATA chapter source.
    pub chapters: PathBuf,
    /// The manifest recording segments, checksums, and the tools used.
    pub manifest: PathBuf,
}

/// Published provisional services used by one preview orchestration.
#[derive(Clone, Copy)]
pub struct PreviewServiceBundle<'a> {
    /// Asynchronous backend executor.
    pub executor: &'a dyn TtsExecutor,
    /// Validated cache publication port.
    pub cache: &'a dyn CachePublisher,
    /// Master-first package writer.
    pub packages: &'a dyn PackageWriter,
    /// Durable job ownership and document repository.
    pub jobs: &'a dyn JobRepository,
}

impl std::fmt::Debug for PreviewServiceBundle<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreviewServiceBundle")
            .field("executor", &"dyn TtsExecutor")
            .field("cache", &"dyn CachePublisher")
            .field("packages", &"dyn PackageWriter")
            .field("jobs", &"dyn JobRepository")
            .finish()
    }
}

/// Builds one lesson into a private preview.
///
/// Lesson and voice gates run before executor and tool preflight, and every
/// gate completes before durable build writes or synthesis. The result is
/// always a private preview; only [`publish`] can claim more.
///
/// # Errors
///
/// [`IoError::ReadFile`] when the lesson cannot be opened or read and
/// [`IoError::LessonNotRegularFile`] when the opened descriptor is not a
/// regular file. The document's own shape returns
/// [`study_tts_core::LessonError::InvalidJson`], together with
/// [`study_tts_core::LessonError::UnsupportedSchema`] and
/// [`study_tts_core::LessonError::UnexpectedSchemaLink`]. A value outside one
/// of the three closed vocabularies returns that vocabulary's own refusal —
/// [`study_tts_core::LessonError::UnknownSegmentRole`],
/// [`study_tts_core::LessonError::UnknownDeliveryStyle`], or
/// [`study_tts_core::LessonError::UnknownReviewStatus`] — and a segment
/// declaring none at all returns
/// [`study_tts_core::LessonError::MissingSegmentRole`],
/// [`study_tts_core::LessonError::MissingDeliveryStyle`], or
/// [`study_tts_core::LessonError::MissingReviewStatus`], which are separate
/// because an absent field is one to add where an unrecognized value is one to
/// correct. Lesson identity and
/// provenance return [`study_tts_core::LessonError::MissingLessonId`],
/// [`study_tts_core::LessonError::InvalidLessonId`],
/// [`study_tts_core::LessonError::MalformedLanguage`],
/// [`study_tts_core::LessonError::EmptyLearningObjective`],
/// [`study_tts_core::LessonError::EmptyLessonReference`], or
/// [`study_tts_core::LessonError::MalformedSourceContentHash`] for a recorded
/// source digest that is not one, which is its own refusal rather than a shape
/// error because it is recompiled from the source document rather than edited.
/// Speaker declarations
/// return [`study_tts_core::LessonError::MissingVoiceProfile`],
/// [`study_tts_core::LessonError::InvalidVoiceProfile`], or
/// [`study_tts_core::LessonError::DuplicateSpeaker`] when the document binds
/// one speaker twice, which the parsed lesson can no longer show because a
/// `BTreeMap` has kept one of the two. Segment validation
/// returns [`study_tts_core::LessonError::MissingSegments`],
/// [`study_tts_core::LessonError::MissingSegmentId`],
/// [`study_tts_core::LessonError::InvalidSegmentId`],
/// [`study_tts_core::LessonError::DuplicateSegmentId`],
/// [`study_tts_core::LessonError::MissingSpokenText`],
/// [`study_tts_core::LessonError::MissingDisplayText`],
/// [`study_tts_core::LessonError::MissingSourceRefs`],
/// [`study_tts_core::LessonError::EmptySourceRef`],
/// [`study_tts_core::LessonError::UnapprovedSegment`],
/// [`study_tts_core::LessonError::MissingSpeaker`],
/// [`study_tts_core::LessonError::UndeclaredSpeaker`],
/// [`study_tts_core::LessonError::PauseOutOfRange`],
/// [`study_tts_core::LessonError::RecallPromptWithoutResponseInterval`], or
/// [`study_tts_core::LessonError::RecallPromptResponseIntervalTooLong`] — the
/// two ends of ADR-0001 §13.2's recall range, separate because one is answered
/// by lengthening the pause and the other by shortening it.
/// Resource refusal returns
/// [`study_tts_core::LessonError::LessonJsonTooLarge`],
/// [`study_tts_core::LessonError::TooManySegments`],
/// [`study_tts_core::LessonError::SpokenTextTooLong`],
/// [`study_tts_core::LessonError::DisplayTextTooLong`],
/// [`study_tts_core::LessonError::TooManySourceRefs`],
/// [`study_tts_core::LessonError::SourceRefTooLong`],
/// [`study_tts_core::LessonError::TooManyLearningObjectives`],
/// [`study_tts_core::LessonError::LearningObjectiveTooLong`],
/// [`study_tts_core::LessonError::TooManyLessonReferences`],
/// [`study_tts_core::LessonError::LessonReferenceTooLong`], or
/// [`study_tts_core::LessonError::AuthoredTextTooLarge`].
///
/// Voice gating returns
/// [`crate::VoiceProfileError::MissingVoiceProfileDirectory`],
/// [`crate::VoiceProfileError::VoiceProfileNotDirectory`],
/// [`crate::VoiceProfileError::VoiceProfileIdMismatch`],
/// [`crate::VoiceProfileError::MissingVoiceRecord`],
/// [`crate::VoiceProfileError::VoiceRecordNotRegularFile`],
/// [`crate::VoiceProfileError::VoiceChecksumMismatch`],
/// [`study_tts_core::VoiceError::InvalidJson`],
/// [`study_tts_core::VoiceError::UnsupportedSchema`],
/// [`study_tts_core::VoiceError::MissingField`],
/// [`study_tts_core::VoiceError::MalformedChecksum`],
/// [`study_tts_core::VoiceError::ConsentNotGranted`],
/// [`study_tts_core::VoiceError::ProfileNotApproved`],
/// [`study_tts_core::VoiceError::ConsentScopeExcluded`], or
/// [`study_tts_core::VoiceError::ConsentChecksumDisagreement`].
///
/// Planning returns [`study_tts_core::PlanError::UnresolvedSpeaker`] when the
/// voice gate above returned no conditioning artifact for a speaker some
/// segment names. The lesson is valid in that case, which is why the refusal
/// is its own category rather than a lesson diagnostic.
/// [`study_tts_core::PlanError::BaseTakeKeyMismatch`] and
/// [`study_tts_core::PlanError::RetakeUsesBaseKey`] cannot be returned here:
/// both are raised by [`RenderPlan::verify_recorded_selection`] on a plan read
/// back from disk, which only [`resume_preview`] does.
///
/// Take selection returns the refusals of the `<lesson-stem>.takes.json`
/// sibling ADR-0001 §12.1 puts beside the lesson.
/// [`study_tts_core::TakesError::TakesJsonTooLarge`],
/// [`study_tts_core::TakesError::InvalidJson`],
/// [`study_tts_core::TakesError::UnsupportedSchema`],
/// [`study_tts_core::TakesError::UnexpectedSchemaLink`],
/// [`study_tts_core::TakesError::InvalidLessonId`],
/// [`study_tts_core::TakesError::InvalidSegmentId`],
/// [`study_tts_core::TakesError::MissingSelections`],
/// [`study_tts_core::TakesError::TooManySelections`],
/// [`study_tts_core::TakesError::DuplicateSelection`],
/// [`study_tts_core::TakesError::BaseTakeKeyMismatch`], and
/// [`study_tts_core::TakesError::RetakeUsesBaseKey`] refuse the document on its
/// own terms. Against the plan it selects,
/// [`study_tts_core::TakesError::LessonMismatch`] reports selections recorded
/// for another lesson, [`study_tts_core::TakesError::UnselectedSegment`] a
/// segment left unapproved, [`study_tts_core::TakesError::UnplannedSelection`]
/// a selection for a segment the plan does not carry,
/// [`study_tts_core::TakesError::StaleSynthesisBaseKey`] ADR-0001 §12.2's
/// refusal of a selection whose base key no longer matches, and
/// [`study_tts_core::TakesError::SelectedCacheKeyMismatch`] a recorded key that
/// is not the one its take derives. Once a segment resolves,
/// [`study_tts_core::TakesError::ApprovedAudioMismatch`] refuses a cache entry
/// holding audio other than the audio the selection approved.
/// [`study_tts_core::TakesError::PlanIsNotBaseTakes`] cannot be returned here:
/// it reports a caller reconciling against an already-selected plan, and this
/// module reconciles only against the plan it derived at take zero.
///
/// Tool work returns [`crate::ToolError::MissingTool`],
/// [`crate::ToolError::InspectTool`], [`crate::ToolError::ToolProbeFailed`],
/// [`crate::ToolError::MissingEncoder`] when the resolved FFmpeg cannot encode
/// a format the package requires,
/// [`crate::ToolError::StartFfmpeg`], [`crate::ToolError::Ffmpeg`],
/// [`crate::ToolError::Ffprobe`], [`crate::ToolError::ToolTimedOut`],
/// [`crate::ToolError::ToolOutputOverflow`],
/// [`crate::ToolError::ToolPipeUnavailable`],
/// [`crate::ToolError::ToolCaptureConfigurationFailed`],
/// [`crate::ToolError::ToolCaptureStartFailed`],
/// [`crate::ToolError::ToolCaptureReadFailed`],
/// [`crate::ToolError::ToolCaptureChannelClosed`],
/// [`crate::ToolError::ToolCaptureThreadPanicked`],
/// [`crate::ToolError::ToolCaptureShutdownTimedOut`],
/// [`crate::ToolError::ToolCaptureIncomplete`],
/// [`crate::ToolError::ToolCleanupFailed`],
/// [`crate::ToolError::ToolChildInspectionFailed`],
/// [`crate::ToolError::ToolTerminationSignalFailed`],
/// [`crate::ToolError::ToolContainmentInspectionFailed`],
/// [`crate::ToolError::ToolContainmentSignalFailed`],
/// [`crate::ToolError::ToolChildReapFailed`],
/// [`crate::ToolError::ToolTerminationTimedOut`],
/// [`crate::ToolError::ToolReaperStartFailed`],
/// [`crate::ToolError::ToolCaptureReaperStartFailed`],
/// [`crate::ToolError::UnreadableProbeResponse`],
/// [`crate::ToolError::UnexpectedEncodedStreamCount`], or
/// [`crate::ToolError::UnexpectedEncodedStream`].
///
/// Managed state and audio return
/// [`crate::ManagedPathError::ManagedPathEscape`],
/// [`crate::ManagedPathError::UnrootedDestination`],
/// [`crate::CacheError::UnusableCacheEntry`],
/// [`crate::CacheError::UncontainedStagedFile`],
/// [`crate::CacheError::PackageArtifactCountMismatch`],
/// [`crate::CacheError::PackageArtifactPlanMismatch`],
/// [`crate::AudioError::UnusableAudio`],
/// [`crate::AudioError::SynthesizerReportMismatch`],
/// [`crate::AudioError::SynthesizerIdentityMismatch`],
/// [`crate::AudioError::ConditioningIdentityContradiction`],
/// [`crate::AudioError::PauseFrameOverflow`],
/// [`crate::AudioError::PlannedLengthOverflow`],
/// [`crate::AudioError::AssembledLengthOverflow`],
/// [`crate::AudioError::AssembledLengthMismatch`], [`IoError::FileSystem`],
/// [`IoError::AudioAt`], [`IoError::WriteJson`], or
/// [`BuildError::Synthesis`].
///
/// [`crate::CacheError::UnrecognizedCacheEntry`] is not among them. It reports
/// a directory inside the cache tree that no cache key names, which only
/// [`crate::prune_candidates`] can find: a build resolves the one entry its
/// plan derives rather than enumerating the tree, so it never looks at a
/// directory it did not name.
///
/// [`IoError::DestinationExists`] is not among them either. A build claims
/// names inside a workspace it owns, where losing a publication race is
/// reported by the durable-state category that owns the record; refusing to
/// replace a file somebody else authored is [`crate::scaffold_lesson`]'s
/// invariant, and it is the only caller that raises it.
///
/// [`crate::ManagedPathError::InvalidManagedName`] is not among them: every
/// name this function offers a managed helper is either a literal or an
/// identifier the lesson gate already refused, so an unusable spelling is
/// reported as the authoring mistake it is.
///
/// [`crate::VoiceProfileError::VoiceProfileNameNotUtf8`] is not among them
/// either. It belongs to [`crate::admit_voice_root`], which walks a whole
/// governed root before a worker is started; this function is handed an
/// executor that already exists and resolves only the profiles a lesson names,
/// each through a `profile_id` the lesson gate has already accepted as text.
///
/// Durable ownership and publication may return
/// [`crate::DurableStateError::LiveJobLock`],
/// [`crate::DurableStateError::MalformedJobLock`],
/// [`crate::DurableStateError::IncompatibleJobLock`],
/// [`crate::DurableStateError::MalformedJobSnapshot`],
/// [`crate::DurableStateError::MalformedJobEventLog`],
/// [`crate::DurableStateError::DurableRecordTooLarge`],
/// [`crate::DurableStateError::JobEventLineTooLarge`],
/// [`crate::DurableStateError::JobSnapshotSegmentCountExceeded`],
/// [`crate::DurableStateError::JobSnapshotIdentityMismatch`],
/// [`crate::DurableStateError::JobSnapshotAttemptMismatch`],
/// [`crate::DurableStateError::JobReplacementPredecessorMismatch`],
/// [`crate::DurableStateError::JobSnapshotLastSuccessfulStateMismatch`],
/// [`crate::DurableStateError::JobSnapshotSelectionMismatch`],
/// [`crate::DurableStateError::JobSnapshotPackageIdentityMismatch`],
/// [`crate::DurableStateError::JobPreviewSelectionMismatch`],
/// [`crate::DurableStateError::IllegalJobTransition`],
/// [`crate::DurableStateError::JobAttemptOverflow`],
/// [`crate::DurableStateError::CacheLockTimeout`],
/// [`crate::DurableStateError::MalformedPublicationJournal`],
/// [`crate::DurableStateError::MalformedCurrentPreview`],
/// [`crate::DurableStateError::UnsupportedDurableRecord`],
/// [`crate::DurableStateError::CurrentLessonMismatch`],
/// [`crate::DurableStateError::PublicationJournalLessonMismatch`],
/// [`crate::DurableStateError::InvalidCurrentPackageReference`],
/// [`crate::DurableStateError::MissingPackageDirectory`],
/// [`crate::DurableStateError::MalformedPackageManifest`], including when a
/// recorded digest is not one and its value object refuses it during parsing,
/// [`crate::DurableStateError::UnsupportedPackageManifest`],
/// [`crate::DurableStateError::PackageReleaseStatusMismatch`],
/// [`crate::DurableStateError::PackageLessonMismatch`],
/// [`crate::DurableStateError::EmptyPackageSegmentId`],
/// [`crate::DurableStateError::EmptyPackageSegmentAudio`],
/// [`crate::DurableStateError::IncoherentPackageTimeline`] when a recorded
/// timeline's boundaries, pauses, and master length do not agree,
/// [`crate::DurableStateError::UnexpectedPackageArtifactPath`],
/// [`crate::DurableStateError::PackageArtifactChecksumMismatch`],
/// [`crate::DurableStateError::MissingPackageToolArguments`],
/// [`crate::DurableStateError::PackageManifestChecksumMismatch`],
/// [`crate::DurableStateError::MalformedDurableDigest`],
/// [`crate::DurableStateError::MissingCurrentPreview`],
/// [`crate::DurableStateError::JournalSelectionMismatch`],
/// [`crate::DurableStateError::PackagePlanMismatch`],
/// [`crate::DurableStateError::InvalidJobDirectoryName`],
/// [`crate::DurableStateError::PublicationConflict`], or
/// [`crate::DurableStateError::QuarantineFailed`]. A live job lock directs the
/// caller to wait; integrity failures preserve their records for the runtime
/// owner rather than deleting or overwriting them.
/// [`crate::DurableStateError::NoJobToResume`],
/// [`crate::DurableStateError::RetainedLessonMismatch`],
/// [`crate::DurableStateError::RetainedLessonIdentityMismatch`],
/// [`crate::DurableStateError::MalformedRetainedPlan`],
/// [`crate::DurableStateError::RetainedPlanIdentityMismatch`],
/// [`crate::DurableStateError::RetainedPlanHashMismatch`],
/// [`crate::DurableStateError::RetainedPlanSegmentCountExceeded`], and
/// [`crate::DurableStateError::JobPlanHashMismatch`] are
/// [`resume_preview`]'s alone: a build has its lesson and plan in hand rather
/// than reading them back.
pub fn build_preview(
    request: BuildRequest,
    executor: &dyn TtsExecutor,
) -> Result<BuildResult, BuildError> {
    let cache = FileSystemCachePublisher;
    let packages = FileSystemPackageWriter;
    let jobs = FileSystemJobRepository;
    build_preview_with_services(
        request,
        PreviewServiceBundle {
            executor,
            cache: &cache,
            packages: &packages,
            jobs: &jobs,
        },
    )
}

/// Builds one lesson exclusively through the published service seams.
///
/// # Errors
///
/// Returns exactly the [`BuildError`] variants documented by [`build_preview`];
/// injected ports retain ownership of their validation and durability errors.
pub fn build_preview_with_services(
    request: BuildRequest,
    services: PreviewServiceBundle<'_>,
) -> Result<BuildResult, BuildError> {
    let lesson_bytes = read_lesson(&request.lesson_path)?;
    let lesson =
        ValidatedLesson::from_json(&request.lesson_path.display().to_string(), &lesson_bytes)?;
    let gated = gate(
        lesson,
        lesson_bytes,
        TakesInput::SiblingOf {
            lesson_path: &request.lesson_path,
            retakes: &request.retakes,
        },
        &request.voice_profile_root,
        &request.ffmpeg_executable,
        &request.ffprobe_executable,
        services,
    )?;

    fs::create_dir_all(&request.workspace).map_err(|error| io_error(&request.workspace, error))?;
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let ownership = services.jobs.claim(&workspace, gated.lesson.lesson_id())?;
    let previous = services.jobs.load(&workspace, gated.lesson.lesson_id())?;
    render_attempt(&workspace, ownership, previous, gated, services)
}

/// Resumes a job from its retained inputs and recorded state.
///
/// ADR-0001 §12.7: acquire the job lock, parse and validate every authoritative
/// document, then continue from the first missing or invalid artifact. The
/// continuation is the same attempt a fresh build runs — the cache and package
/// layers revalidate everything they reuse, so nothing here trusts recorded
/// segment status in place of the artifact it names (§12.7 step 6). A resumed
/// build is gated on voices and tools exactly as a fresh one is.
///
/// # Errors
///
/// Everything [`build_preview`] documents, since the same gates and the same
/// attempt run here, plus retained-input refusals:
/// [`crate::DurableStateError::NoJobToResume`] when the job directory holds no
/// document, lesson, or plan;
/// [`crate::DurableStateError::RetainedLessonMismatch`] or
/// [`crate::DurableStateError::RetainedLessonIdentityMismatch`] when the lesson
/// bytes or identity disagree;
/// [`crate::DurableStateError::MalformedRetainedPlan`],
/// [`crate::DurableStateError::RetainedPlanIdentityMismatch`], or
/// [`crate::DurableStateError::RetainedPlanHashMismatch`] when `plan.json`
/// cannot be trusted; and [`crate::DurableStateError::JobPlanHashMismatch`]
/// when the validated plan disagrees with `job.json`.
pub fn resume_preview(
    request: ResumeRequest,
    executor: &dyn TtsExecutor,
) -> Result<BuildResult, BuildError> {
    let cache = FileSystemCachePublisher;
    let packages = FileSystemPackageWriter;
    let jobs = FileSystemJobRepository;
    resume_preview_with_services(
        request,
        PreviewServiceBundle {
            executor,
            cache: &cache,
            packages: &packages,
            jobs: &jobs,
        },
    )
}

/// Resumes one job exclusively through the published service seams.
///
/// # Errors
///
/// As [`resume_preview`].
pub fn resume_preview_with_services(
    request: ResumeRequest,
    services: PreviewServiceBundle<'_>,
) -> Result<BuildResult, BuildError> {
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let no_job = || DurableStateError::NoJobToResume {
        path: workspace.join("jobs").join(&request.job_id),
    };
    let ownership = services.jobs.claim(&workspace, &request.job_id)?;
    let previous = services
        .jobs
        .load(&workspace, &request.job_id)?
        .ok_or_else(no_job)?;
    let lesson_bytes = services
        .jobs
        .retained_lesson(&workspace, &request.job_id)?
        .ok_or_else(no_job)?;
    let actual = LessonDigest::from(blake3::hash(&lesson_bytes));
    if actual != previous.lesson_blake3 {
        return Err(DurableStateError::RetainedLessonMismatch {
            path: workspace
                .join("jobs")
                .join(&request.job_id)
                .join("lesson.json"),
            recorded: previous.lesson_blake3.as_str().to_owned(),
            actual: actual.as_str().to_owned(),
        }
        .into());
    }
    let lesson = ValidatedLesson::from_json(
        &format!("jobs/{}/lesson.json", request.job_id),
        &lesson_bytes,
    )?;
    if lesson.lesson_id() != request.job_id {
        return Err(DurableStateError::RetainedLessonIdentityMismatch {
            path: workspace
                .join("jobs")
                .join(&request.job_id)
                .join("lesson.json"),
            required: request.job_id,
            actual: lesson.lesson_id().to_owned(),
        }
        .into());
    }
    let retained_plan = services
        .jobs
        .retained_plan(&workspace, lesson.lesson_id())?
        .ok_or_else(no_job)?;
    if retained_plan.plan_hash != previous.plan_hash {
        return Err(DurableStateError::JobPlanHashMismatch {
            path: workspace
                .join("jobs")
                .join(lesson.lesson_id())
                .join("plan.json"),
            job_recorded: previous.plan_hash.as_str().to_owned(),
            plan_recorded: retained_plan.plan_hash.as_str().to_owned(),
        }
        .into());
    }
    // Invariant A-1: the retained plan is this job's authoritative statement of
    // what it renders, so resume recovers its selection from there and performs
    // no sibling-takes discovery. `JobDocument::open_attempt` compares no plan
    // hashes, so a rediscovering resume would degrade a retake to take zero
    // with nothing reporting it.
    let gated = gate(
        lesson,
        lesson_bytes,
        TakesInput::Retained(&retained_plan),
        &request.voice_profile_root,
        &request.ffmpeg_executable,
        &request.ffprobe_executable,
        services,
    )?;
    render_attempt(&workspace, ownership, Some(previous), gated, services)
}

/// What the gates produced and one attempt renders.
struct GatedBuild {
    lesson: ValidatedLesson,
    /// The exact bytes that were validated, retained beside `job.json`.
    lesson_bytes: Vec<u8>,
    plan: RenderPlan,
    synthesis_requests: Vec<SynthesisRequest>,
    packages: Box<dyn PreparedPackageWriter>,
}

/// Runs every gate that must precede durable work, in the order the module
/// docs commit to: rights, then planning, then executor validation, then tool
/// preflight.
fn gate(
    lesson: ValidatedLesson,
    lesson_bytes: Vec<u8>,
    takes: TakesInput<'_>,
    voice_profile_root: &Path,
    ffmpeg_executable: &Path,
    ffprobe_executable: &Path,
    services: PreviewServiceBundle<'_>,
) -> Result<GatedBuild, BuildError> {
    // Rights precede work, and precede planning too: the conditioning
    // artifact each profile carries is an ADR-0001 §12.5 synthesis-key input,
    // so a plan derived before this gate would name cache entries for voices
    // nobody resolved. A refused voice still performs no observable work.
    let voice_conditioning_hashes =
        voice_gate::resolve_speakers(voice_profile_root, &lesson, VoiceUse::PrivateSynthesis)?;

    let descriptor = services.executor.descriptor();
    let context =
        descriptor.synthesis_context(lesson.language().clone(), voice_conditioning_hashes);
    // Selection precedes executor validation and tool preflight, so a stale
    // takes file is refused before anything observable happens — the same
    // reason the voice gate precedes planning.
    let plan = select_plan(&lesson, &context, takes)?;
    let synthesis_requests = synthesis_requests(&plan, &context)?;
    for synthesis_request in &synthesis_requests {
        services.executor.validate(synthesis_request)?;
    }

    let packages = services.packages.preflight(&PackagePreflightRequest {
        ffmpeg_executable,
        ffprobe_executable,
    })?;
    Ok(GatedBuild {
        lesson,
        lesson_bytes,
        plan,
        synthesis_requests,
        packages,
    })
}

/// Renders one build attempt under held ownership.
///
/// Opens attempt *N+1* over `previous` rather than transitioning from it:
/// ADR-0001 §6.4 has no edge out of a finished attempt, and `job.json` records
/// what the abandoned one had reached (§12.7 step 5). Every state change is a
/// durable replacement through the repository, which appends the event after
/// the write; the cache and package layers keep their own reconciliation and
/// are what decide what is reused.
fn render_attempt(
    workspace: &Path,
    ownership: Box<dyn JobOwnership>,
    previous: Option<JobDocument>,
    gated: GatedBuild,
    services: PreviewServiceBundle<'_>,
) -> Result<BuildResult, BuildError> {
    let _job_ownership = ownership;
    let job_id = gated.lesson.lesson_id();
    gated.packages.prepare(&PackagePrepareRequest {
        workspace,
        job_id,
        plan: &gated.plan,
    })?;
    if let Some(previous) = &previous {
        services
            .jobs
            .validate_preview_selection(workspace, previous)?;
    }
    services
        .jobs
        .retain_inputs(workspace, job_id, &gated.lesson_bytes, &gated.plan)?;
    // Created, Validated, and Planned are walked in memory: the lesson was
    // validated and the plan derived before ownership was claimed, and
    // ADR-0001 §12.3 governs durable state *changes*, of which the first is
    // the plan this attempt will render.
    let mut document = JobDocument::open_attempt(
        job_id,
        LessonDigest::from(blake3::hash(&gated.lesson_bytes)),
        gated.plan.plan_hash.clone(),
        previous.as_ref(),
    )?
    .transition(JobState::Validated)?
    .transition(JobState::Planned)?;
    services.jobs.replace(workspace, &document)?;

    document = document.transition(JobState::Rendering)?;
    services.jobs.replace(workspace, &document)?;
    let mut cached_segments = Vec::with_capacity(gated.plan.segments.len());
    for (segment, synthesis_request) in gated.plan.segments.iter().zip(gated.synthesis_requests) {
        let mut pending_request = Some(synthesis_request);
        let mut producer = |destination: &Path| {
            let request = pending_request
                .take()
                .ok_or_else(|| BackendError::Protocol {
                    request_id: segment.id.clone(),
                    message: "cache requested staged synthesis more than once".to_owned(),
                })?;
            block_on(services.executor.synthesize(request, destination))
        };
        let cached = services.cache.resolve(
            &CacheResolveRequest {
                workspace: workspace.to_path_buf(),
                job_id: job_id.to_owned(),
                segment: segment.clone(),
            },
            &mut producer,
        )?;
        let audio_blake3: AudioDigest = cached.audio_blake3().parse().map_err(|_| {
            DurableStateError::MalformedDurableDigest {
                path: cached.entry_dir().to_path_buf(),
                value: cached.audio_blake3().to_owned(),
            }
        })?;
        // Invariant I-3: the plan records the approved checksum for audit and
        // keeps it out of `plan_hash`, so this comparison against the resolved
        // artifact is the verification it is recorded under. A segment planned
        // without a recorded selection has no approval to contradict.
        if let Some(approved) = &segment.audio_blake3
            && approved != &audio_blake3
        {
            return Err(boxed_takes(TakesError::ApprovedAudioMismatch {
                segment_id: segment.id.clone(),
                cache_key: cached.cache_key().clone(),
                recorded: approved.clone(),
                actual: audio_blake3,
            }));
        }
        // The §6.4 self-loop: one segment completed, and the document says so
        // durably before the next begins.
        document = document.transition(JobState::Rendering)?.with_segment(
            &segment.id,
            SegmentStatus {
                cache_key: cached.cache_key().clone(),
                audio_blake3,
            },
        );
        services.jobs.replace(workspace, &document)?;
        cached_segments.push(cached);
    }

    document = document.transition(JobState::Rendered)?;
    services.jobs.replace(workspace, &document)?;
    let package = gated.packages.write(&PackageWriteRequest {
        workspace,
        job_id,
        plan: &gated.plan,
        cached_artifacts: &cached_segments,
    })?;
    // The private-preview completion is recorded beside the state, not as
    // one: `Rendered` is as far as the ADR-0001 §6.4 machine can honestly go
    // without a verifier, and the selected package is what E2-S1 task 4 keeps
    // separate from it.
    services.jobs.replace(
        workspace,
        &document.with_preview_package(package.identity.clone()),
    )?;

    Ok(BuildResult {
        package_dir: package.package_dir,
        publication_record: package.publication_record,
        master_wav: package.master_wav,
        m4a: package.m4a,
        mp3: package.mp3,
        transcript: package.transcript,
        captions: package.captions,
        chapters: package.chapters,
        manifest: package.manifest,
    })
}

/// Where the takes one build plans at come from.
///
/// The two variants are not interchangeable, and that is the point. Discovery
/// is legitimate only on the path that *establishes* a plan; a resume
/// continues one, and recovers its selection from the plan it already retained
/// rather than from a file that may have moved since. See
/// [`TakeSelection::Recovered`], which states the invariant.
#[derive(Clone, Copy, Debug)]
enum TakesInput<'a> {
    /// The `<lesson-stem>.takes.json` sibling ADR-0001 §12.1 puts beside an
    /// authored lesson, if one is there, plus any alternate performance the
    /// request asked for.
    SiblingOf {
        /// The lesson being built, whose stem names the sibling.
        lesson_path: &'a Path,
        /// The §11.4 alternate performances this build requests.
        retakes: &'a BTreeMap<String, u32>,
    },
    /// The selection a retained `plan.json` already established.
    Retained(&'a RenderPlan),
}

/// Derives the plan this build renders, at the takes `takes` names.
///
/// Applies a selection to a derived plan rather than deriving the two
/// together, because ADR-0001 §12.2's `synthesis_base_key` only exists once a
/// base plan has been derived: the base plan is what a recorded base key is
/// compared against, and every planned segment's cache key *is* that segment's
/// base key exactly while nothing has been selected.
///
/// # Errors
///
/// [`PlanError::UnresolvedSpeaker`]; the [`TakesError`] refusals
/// [`ValidatedTakes::from_json`] and [`ValidatedTakes::reconcile_with_plan`]
/// raise; [`crate::DurableStateError::DurableRecordTooLarge`] for an oversized
/// takes file; [`crate::ManagedPathError`] when the sibling is a symlink or
/// not a regular file; and [`IoError::FileSystem`] when it cannot be read.
fn select_plan(
    lesson: &ValidatedLesson,
    context: &SynthesisContext,
    takes: TakesInput<'_>,
) -> Result<RenderPlan, BuildError> {
    let (lesson_path, retakes) = match takes {
        TakesInput::Retained(retained) => {
            return Ok(RenderPlan::for_lesson_with_takes(
                lesson,
                context,
                &TakeSelection::recovered(retained),
            )?);
        }
        TakesInput::SiblingOf {
            lesson_path,
            retakes,
        } => (lesson_path, retakes),
    };

    let base = RenderPlan::for_lesson(lesson, context)?;
    let Some(document) = read_sibling_takes(lesson_path)? else {
        return Ok(RenderPlan::for_lesson_with_takes(
            lesson,
            context,
            &TakeSelection::implicit().with_retakes(retakes),
        )?);
    };

    let applied = document.reconcile_with_plan(&base).map_err(boxed_takes)?;
    // Verified against the plan the *document alone* derives, before any
    // requested performance is layered on: a retake deliberately moves a
    // segment off its recorded selection, so checking the recorded keys
    // against the retaken plan would report the request as a mismatch.
    let selected =
        RenderPlan::for_lesson_with_takes(lesson, context, &TakeSelection::explicit(&applied))?;
    applied
        .verify_selected_keys(&selected)
        .map_err(boxed_takes)?;
    if retakes.is_empty() {
        return Ok(selected);
    }
    Ok(RenderPlan::for_lesson_with_takes(
        lesson,
        context,
        &TakeSelection::explicit(&applied).with_retakes(retakes),
    )?)
}

/// Reads the takes document beside a lesson, when the author recorded one.
///
/// Resolved through [`managed::leaf`] rather than joined lexically: a
/// validated file name still follows a symlink, and a build that read its
/// selection through one would apply approvals from a document outside the
/// directory the operator named.
///
/// # Errors
///
/// As [`select_plan`], excluding the planning and reconciliation refusals.
fn read_sibling_takes(lesson_path: &Path) -> Result<Option<ValidatedTakes>, BuildError> {
    let (Some(directory), Some(stem)) = (lesson_path.parent(), lesson_path.file_stem()) else {
        return Ok(None);
    };
    let Some(stem) = stem.to_str() else {
        return Ok(None);
    };

    let sibling = managed::leaf(directory, &format!("{stem}.takes.json"))?;
    if !sibling.exists() {
        return Ok(None);
    }

    let bytes = read_bounded_bytes(&sibling, MAX_TAKES_JSON_BYTES)?;
    ValidatedTakes::from_json(&bytes)
        .map(Some)
        .map_err(boxed_takes)
}

/// Boxes a takes refusal, for the reason [`BuildError::Takes`] records.
fn boxed_takes(error: TakesError) -> BuildError {
    BuildError::Takes(Box::new(error))
}

/// Maps each planned segment onto the backend request that must reproduce its
/// cache key.
///
/// Takes the whole [`SynthesisContext`] rather than only the language because
/// the request carries the resolved conditioning artifact too, and both come
/// from the same context the plan was keyed with — two sources could disagree.
///
/// # Errors
///
/// [`study_tts_core::PlanError::UnresolvedSpeaker`], the same refusal
/// [`RenderPlan::for_lesson`] makes. Reachable only if a caller pairs a plan
/// with a context that did not derive it; expressing it costs one `?` and is
/// what keeps a panic out of a path that reaches the worker.
fn synthesis_requests(
    plan: &RenderPlan,
    context: &SynthesisContext,
) -> Result<Vec<SynthesisRequest>, PlanError> {
    plan.segments
        .iter()
        .map(|segment| {
            let voice_conditioning_hash = context
                .voice_conditioning_for(&segment.speaker)
                .ok_or_else(|| PlanError::UnresolvedSpeaker {
                    segment_id: segment.id.clone(),
                    speaker: segment.speaker.clone(),
                })?
                .clone();
            Ok(SynthesisRequest {
                request_id: segment.request_id(),
                segment_id: segment.id.clone(),
                spoken_text: segment.spoken_text.clone(),
                voice: segment.speaker.clone(),
                voice_profile: segment.voice_profile.clone(),
                voice_conditioning_hash,
                style: segment.style.as_str().to_owned(),
                language: context.language.clone(),
                take: segment.take,
                cache_key: segment.cache_key.clone(),
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
            })
        })
        .collect()
}

// E0 keeps `build_preview` synchronous while the published executor stays
// compatible with concurrent async dispatch planned for E1.
fn block_on<F: Future>(future: F) -> F::Output {
    let parker = Arc::new(ThreadParker {
        thread: thread::current(),
    });
    let waker = Waker::from(parker);
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[derive(Debug)]
struct ThreadParker {
    thread: thread::Thread,
}

impl Wake for ThreadParker {
    fn wake(self: Arc<Self>) {
        self.thread.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.thread.unpark();
    }
}

/// Reads and validates one authored lesson document.
///
/// The load-and-validate step [`build_preview`] performs, published so
/// `study-tts lesson validate` checks a document the same way the build that
/// will render it does. Two implementations of "is this lesson usable" would
/// be free to disagree, and the one an author consulted would be the one that
/// did not matter.
///
/// # Errors
///
/// [`IoError::ReadFile`] when the document cannot be opened or read, and
/// [`IoError::LessonNotRegularFile`] when the opened descriptor is not a
/// regular file. The document's own refusals are located by
/// [`study_tts_core::LessonDiagnostic`] and delegated whole:
/// [`study_tts_core::LessonError::LessonJsonTooLarge`] is raised here rather
/// than by the parser, because the bounded reader is what stops oversized
/// bytes reaching one, and every other variant is the one
/// [`study_tts_core::ValidatedLesson::from_json`] documents.
///
/// No other [`IoError`] variant is reachable, because this function writes
/// nothing: [`IoError::DestinationExists`], [`IoError::FileSystem`],
/// [`IoError::AudioAt`], and [`IoError::WriteJson`] belong to the boundaries
/// that do.
pub fn load_lesson(path: &Path) -> Result<ValidatedLesson, BuildError> {
    let bytes = read_lesson(path)?;
    Ok(ValidatedLesson::from_json(
        &path.display().to_string(),
        &bytes,
    )?)
}

/// Reads at most one byte beyond the core lesson envelope.
///
/// Metadata avoids reading a file already known to be oversized; the bounded
/// read remains authoritative because the file may grow after that preflight.
pub(crate) fn read_lesson(path: &Path) -> Result<Vec<u8>, BuildError> {
    let file = open_lesson(path)?;

    let metadata = file.metadata().map_err(|source| IoError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_file() {
        return Err(IoError::LessonNotRegularFile {
            path: path.to_path_buf(),
        }
        .into());
    }

    read_lesson_from_reader(path, file, metadata.len())
}

#[cfg(unix)]
fn open_lesson(path: &Path) -> Result<File, BuildError> {
    use rustix::fs::{Mode, OFlags, open};

    let descriptor = open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|source| IoError::ReadFile {
        path: path.to_path_buf(),
        source: source.into(),
    })?;
    Ok(File::from(descriptor))
}

#[cfg(not(unix))]
fn open_lesson(path: &Path) -> Result<File, BuildError> {
    File::open(path).map_err(|source| {
        IoError::ReadFile {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

fn read_lesson_from_reader(
    path: &Path,
    reader: impl Read,
    advertised_bytes: u64,
) -> Result<Vec<u8>, BuildError> {
    if advertised_bytes > MAX_LESSON_JSON_BYTES as u64 {
        return Err(lesson_too_large(path));
    }

    let initial_capacity = usize::try_from(advertised_bytes)
        .unwrap_or(MAX_LESSON_JSON_BYTES)
        .min(MAX_LESSON_JSON_BYTES)
        .saturating_add(1);

    let mut bytes = Vec::with_capacity(initial_capacity);

    reader
        .take((MAX_LESSON_JSON_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| IoError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;

    if bytes.len() > MAX_LESSON_JSON_BYTES {
        return Err(lesson_too_large(path));
    }
    Ok(bytes)
}

/// The size refusal this reader raises, located in the file it was reading.
///
/// Raised here rather than by the parser because the whole point of the
/// bounded reader is that oversized bytes never reach one.
fn lesson_too_large(path: &Path) -> BuildError {
    LessonDiagnostic::about(
        &path.display().to_string(),
        study_tts_core::LessonError::LessonJsonTooLarge {
            max_bytes: MAX_LESSON_JSON_BYTES,
        },
    )
    .into()
}

/// Preflights ffprobe and requires the encoded artifact to be a single mono AAC
/// stream.
///
/// `build_preview` performs this check internally; the entry point exists so
/// the rejection path can be exercised from the integration suite, which is
/// where a test needing a real ffprobe belongs.
///
/// # Errors
///
/// [`crate::ToolError::MissingTool`] or [`crate::ToolError::InspectTool`] when
/// ffprobe cannot be resolved or launched,
/// [`crate::ToolError::ToolProbeFailed`] when its version probe fails,
/// [`crate::ToolError::ToolTimedOut`] when either ffprobe operation exceeds its
/// deadline, [`crate::ToolError::ToolOutputOverflow`] when either captured
/// stream exceeds its ceiling, and
/// [`crate::ToolError::ToolPipeUnavailable`],
/// [`crate::ToolError::ToolCaptureConfigurationFailed`],
/// [`crate::ToolError::ToolCaptureStartFailed`],
/// [`crate::ToolError::ToolCaptureReadFailed`],
/// [`crate::ToolError::ToolCaptureChannelClosed`],
/// [`crate::ToolError::ToolCaptureThreadPanicked`],
/// [`crate::ToolError::ToolCaptureShutdownTimedOut`],
/// [`crate::ToolError::ToolCaptureIncomplete`],
/// [`crate::ToolError::ToolCleanupFailed`],
/// [`crate::ToolError::ToolChildInspectionFailed`],
/// [`crate::ToolError::ToolTerminationSignalFailed`],
/// [`crate::ToolError::ToolContainmentInspectionFailed`],
/// [`crate::ToolError::ToolContainmentSignalFailed`],
/// [`crate::ToolError::ToolChildReapFailed`],
/// [`crate::ToolError::ToolTerminationTimedOut`],
/// [`crate::ToolError::ToolReaperStartFailed`], or
/// [`crate::ToolError::ToolCaptureReaperStartFailed`] when the named
/// supervision invariant fails,
/// [`crate::ToolError::Ffprobe`] when output inspection fails,
/// [`crate::ToolError::UnreadableProbeResponse`] when its output cannot be
/// parsed, and [`crate::ToolError::UnexpectedEncodedStreamCount`] or
/// [`crate::ToolError::UnexpectedEncodedStream`] when the artifact is not a
/// single mono AAC stream.
///
/// Named for the M4A rather than for encoded output generally, because it
/// accepts only that one: the package now holds a second encoded artifact, and
/// handing this the MP3 would refuse a correct file for carrying the codec it
/// is supposed to carry.
pub fn validate_m4a_output(ffprobe_executable: &Path, m4a: &Path) -> Result<(), BuildError> {
    let ffprobe = tools::inspect("ffprobe", ffprobe_executable)?;
    let profiles = export::export_profiles();
    export::probe(&ffprobe, &profiles.ffprobe, export::PackagedAudio::M4a, m4a).map(|_| ())
}

/// Refuses publication for the E0-S0 skeleton.
///
/// Asked of the release profile rather than answered with a sentence: every
/// `build_preview` output is a private preview holding no gate evidence, and
/// `ReleaseClaim` already owns what such a claim may become. The refusal
/// therefore stays correct once the production gates of
/// `docs/governance/RELEASE-PROFILES.md` §3 exist — a preview will still not be
/// publishable, because it is not the artifact that earned them.
///
/// # Errors
///
/// Always [`PublicationError::Release`] carrying
/// [`study_tts_core::ReleaseError::PrivateProfileCannotClaimProduction`].
pub fn publish(_preview: &BuildResult) -> Result<(), BuildError> {
    Ok(ReleaseClaim::private_preview().validate_as_production()?)
}

/// The one manifest version this build knows how to evaluate.
const PRODUCTION_MANIFEST_VERSION: &str = "1.0";

/// Just enough of any manifest to learn which shape to expect.
///
/// Deliberately not strict: the version is what says which fields are legal, so
/// a document cannot be held to a shape before it has been read.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct ManifestVersion {
    schema_version: Option<String>,
}

/// The provisional production-release claim shape, pending E1-S4's complete
/// package manifest.
///
/// `deny_unknown_fields` because a top-level field this build does not know
/// is a field it cannot gate on, and publication must refuse what it cannot
/// evaluate rather than ignore it.
///
/// The rights sections stay as `Value` and are deserialized one at a time by
/// `declare_section`, so a malformed entry names the section it is in.
/// `serde_json` errors carry no field path, so typing them here would tell an
/// operator that something failed to parse without saying where.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
struct ProductionManifest {
    schema_version: String,
    /// Typed, so a status this build does not know is a parse error here rather
    /// than a string carried past every gate that would have consulted it.
    release_status: ReleaseStatus,
    lesson_id: String,
    /// Optional so an older manifest is refused for what it claims rather than
    /// for its shape: `deny_unknown_fields` above means a required field would
    /// make every manifest written before this one fail to parse, and
    /// `MalformedProductionManifest` is not the refusal such a document has
    /// earned. Absent is read as [`TakeSelectionSource::Implicit`], which is
    /// the fail-closed reading.
    take_selection_source: Option<TakeSelectionSource>,
    content_rights: Option<Value>,
    voice_profiles: Option<Value>,
}

/// A voice profile a production manifest declares it used.
///
/// Provisional shape pending E1-S4's complete package manifest, like
/// `content_rights` below.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
struct DeclaredVoiceProfile {
    profile_id: String,
    approval: RightsDecision,
    rights_record_id: String,
}

/// Rejects an identifier that parses but names nothing.
fn require_identifier(
    section: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), BuildError> {
    if value.trim().is_empty() {
        return Err(RightsError::EmptyManifestIdentifier { section, field }.into());
    }
    Ok(())
}

/// Parses one rights section of a production manifest, refusing a section that
/// declares nothing.
///
/// An absent section and an empty one are one refusal, reported as
/// `undeclared`: both name no record, and a gate reading either as "no
/// obligations here" would let a manifest omit its way past a check the
/// sections beside it have to satisfy. Malformed content stays separate and
/// names the section it is in, because `serde_json` errors carry no field path
/// and an operator told only that parsing failed would not know which
/// declaration to correct.
///
/// Borrows the subtree rather than cloning it: `&Value` is itself a
/// deserializer.
///
/// # Errors
///
/// [`RightsError::InvalidRightsDeclaration`] when the section is present and
/// does not parse; otherwise `undeclared` when it declares nothing.
fn require_declarations<'de, T: Deserialize<'de>>(
    section: &'static str,
    value: Option<&'de Value>,
    undeclared: BuildError,
) -> Result<Vec<T>, BuildError> {
    let declarations = value
        .map(|section_value| {
            Vec::<T>::deserialize(section_value)
                .map_err(|source| RightsError::InvalidRightsDeclaration { section, source })
        })
        .transpose()?
        .unwrap_or_default();
    if declarations.is_empty() {
        return Err(undeclared);
    }
    Ok(declarations)
}

/// Always refuses publication until the production manifest and release gates
/// exist.
///
/// Every precondition this build can check runs before that refusal, so each is
/// reported as itself rather than as the generic gate refusal. They run
/// outward in: what the document claims to be, then what it claims about the
/// sources and voices it was made from. The rights checks enforce
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` — "Unresolved external
/// distribution blocks publish", and the source *and* voice record identifiers
/// its "Generated release" row requires — over provisional `content_rights` and
/// `voice_profiles` manifest sections that E1-S4's complete package manifest
/// will replace.
///
/// # Errors
///
/// [`PublicationError::MalformedProductionManifest`] or
/// [`PublicationError::UnsupportedProductionManifest`] for what the document
/// is; [`PublicationError::ManifestNotProductionRelease`] for what it claims;
/// [`study_tts_core::LessonError::MissingLessonId`] or
/// [`study_tts_core::LessonError::InvalidLessonId`] for an identifier a lesson
/// could not name;
/// [`RightsError::InvalidRightsDeclaration`],
/// [`RightsError::MissingContentRightsDeclaration`],
/// [`RightsError::UnresolvedContentRights`],
/// [`RightsError::MissingVoiceProfileDeclaration`],
/// [`RightsError::EmptyManifestIdentifier`], or
/// [`study_tts_core::VoiceError::ProfileNotApproved`] for what it claims about
/// its sources and voices. A manifest that clears those is then refused with
/// [`PublicationError::ImplicitTakeSelection`] when it records a generated
/// take selection, or records nothing about one, per ADR-0001 §12.2. A
/// manifest that clears all of them is refused with
/// [`PublicationError::ProductionGatesUnavailable`], because the gates it
/// would have to satisfy do not exist yet.
pub fn validate_production_manifest(bytes: &[u8]) -> Result<(), BuildError> {
    // Two stages, because the version is what says which shape is legal: a
    // document of an unknown version must be reported as an unknown version,
    // not as a violation of a shape it never claimed.
    let declared_version: ManifestVersion = serde_json::from_slice(bytes)
        .map_err(|source| PublicationError::MalformedProductionManifest { source })?;
    let version = declared_version
        .schema_version
        .unwrap_or_else(|| "missing".to_owned());
    if version != PRODUCTION_MANIFEST_VERSION {
        return Err(PublicationError::UnsupportedProductionManifest { version }.into());
    }

    let manifest: ProductionManifest = serde_json::from_slice(bytes)
        .map_err(|source| PublicationError::MalformedProductionManifest { source })?;
    debug_assert_eq!(manifest.schema_version, PRODUCTION_MANIFEST_VERSION);

    // What the document claims to be, before what it claims about its sources.
    // Adjudicating the rights of a manifest that never asked to be published
    // would hand its author corrections for a release they did not request.
    if manifest.release_status != ReleaseStatus::ProductionRelease {
        return Err(PublicationError::ManifestNotProductionRelease {
            declared: manifest.release_status,
        }
        .into());
    }

    // Through the lesson rule rather than a blank check: this identifier names
    // the same output directory a lesson's does, so a manifest must not name
    // what a lesson could not.
    validate_lesson_id(&manifest.lesson_id)
        .map_err(|error| LessonDiagnostic::about("manifest.json", error))?;

    // An absent section and an empty one are the same claim: nothing was
    // declared. Both sections are held to it, because a production lesson
    // always has at least one source and is always spoken by at least one
    // voice, and `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Generated
    // release") requires a release to record the identifiers of both.
    let sources: Vec<SourceRightsDeclaration> = require_declarations(
        "content_rights",
        manifest.content_rights.as_ref(),
        RightsError::MissingContentRightsDeclaration.into(),
    )?;
    for source in &sources {
        require_identifier("content_rights", "source_id", &source.source_id)?;
        require_identifier(
            "content_rights",
            "rights_record_id",
            &source.rights_record_id,
        )?;
        if !source.classification.permits_production_release() {
            return Err(RightsError::UnresolvedContentRights {
                source_id: source.source_id.clone(),
                classification: source.classification.as_str().to_owned(),
            }
            .into());
        }
    }

    let profiles: Vec<DeclaredVoiceProfile> = require_declarations(
        "voice_profiles",
        manifest.voice_profiles.as_ref(),
        RightsError::MissingVoiceProfileDeclaration.into(),
    )?;
    for profile in profiles {
        require_identifier("voice_profiles", "profile_id", &profile.profile_id)?;
        require_identifier(
            "voice_profiles",
            "rights_record_id",
            &profile.rights_record_id,
        )?;
        if profile.approval != RightsDecision::Approved {
            return Err(BuildError::Voice(VoiceError::ProfileNotApproved {
                profile_id: profile.profile_id,
                decision: profile.approval.as_str().to_owned(),
            }));
        }
    }

    // After what the manifest claims about its sources and voices, and before
    // the terminal refusal, so this reports selection rather than shadowing a
    // rights finding or being shadowed by the catch-all. The order matters
    // both ways: ahead of the rights checks it would answer every rights
    // question with a selection complaint.
    let selection = manifest
        .take_selection_source
        .unwrap_or(TakeSelectionSource::Implicit);
    selection
        .production()
        .map_err(|_| PublicationError::ImplicitTakeSelection {
            declared: selection.name(),
        })?;

    Err(PublicationError::ProductionGatesUnavailable.into())
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        io::{self, Read},
        path::Path,
        pin::Pin,
        sync::mpsc,
        task::{Context, Poll, Waker},
        thread,
    };

    use study_tts_core::{
        LessonError, MAX_LESSON_JSON_BYTES, RenderPlan, ReviewStatus, SynthesisContext,
        ValidatedLesson, VoiceConditioningHash,
    };

    use super::{block_on, read_lesson_from_reader, synthesis_requests};
    use crate::BuildError;

    struct PendingThenReady {
        waker_sender: Option<mpsc::Sender<Waker>>,
    }

    impl Future for PendingThenReady {
        type Output = &'static str;

        fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
            if let Some(sender) = self.waker_sender.take() {
                sender
                    .send(context.waker().clone())
                    .expect("send the local block_on waker to the waking thread");
                return Poll::Pending;
            }
            Poll::Ready("resumed")
        }
    }

    #[test]
    fn t1_e0_bounded_lesson_reader_refuses_growth_after_metadata_preflight() {
        let reader = io::repeat(b'{').take((MAX_LESSON_JSON_BYTES + 1) as u64);

        let error = read_lesson_from_reader(Path::new("lesson.json"), reader, 1)
            .expect_err("a stream that grows beyond its advertised size must be refused");

        assert!(matches!(
            &error,
            BuildError::Lesson(diagnostic)
                if matches!(
                    diagnostic.error(),
                    LessonError::LessonJsonTooLarge { max_bytes }
                        if *max_bytes == MAX_LESSON_JSON_BYTES
                )
        ));
    }

    /// A two-segment lesson document, with one field replaced.
    ///
    /// The tests below all need a document that differs from a valid one in
    /// exactly one way, so each says only what it is about.
    fn lesson_document(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let mut document: serde_json::Value = serde_json::from_slice(
            br#"{
                "schema_version":"3.1",
                "lesson_id":"request-id-test",
                "title":"Request IDs",
                "language":"en",
                "speakers":{"voice-a":{"voice_profile":"synthetic-test-voice-v1"}},
                "segments":[
                    {
                        "id":"segment-a",
                        "speaker":"voice-a",
                        "role":"explanation",
                        "source_refs":["source-a"],
                        "display_text":"Same speech.",
                        "spoken_text":"Same speech.",
                        "style":"calm",
                        "pause_after_ms":0,
                        "review_status":"approved"
                    },
                    {
                        "id":"segment-b",
                        "speaker":"voice-a",
                        "role":"recap",
                        "source_refs":["source-b"],
                        "display_text":"Same speech.",
                        "spoken_text":"Same speech.",
                        "style":"calm",
                        "pause_after_ms":10,
                        "review_status":"approved"
                    }
                ]
            }"#,
        )
        .expect("the request-mapping lesson parses as JSON");
        edit(&mut document);
        serde_json::to_vec(&document).expect("the edited lesson serializes")
    }

    /// What a refusal in these tests names as the document it came from.
    const DOCUMENT: &str = "<pipeline test lesson>";

    /// The conditioning artifact the tests' single speaker resolves to.
    ///
    /// A stand-in for what the voice gate loads: planning needs a hash for
    /// every speaker, and any well-formed digest proves the mapping without
    /// touching a profile directory.
    fn sample_conditioning() -> std::collections::BTreeMap<String, VoiceConditioningHash> {
        std::collections::BTreeMap::from([(
            "voice-a".to_owned(),
            blake3::hash(b"pipeline-test-conditioning").into(),
        )])
    }

    /// A two-segment lesson, the context it is keyed with, and its plan.
    ///
    /// Shared by the request-mapping tests so each states only what it
    /// asserts; both need a plan with more than one segment and neither cares
    /// how it was authored.
    fn planned_lesson() -> (SynthesisContext, RenderPlan) {
        let lesson = ValidatedLesson::from_json(DOCUMENT, &lesson_document(|_| {}))
            .expect("validate the request-mapping lesson");
        let context = crate::synthesis::sample_descriptor()
            .synthesis_context(lesson.language().clone(), sample_conditioning());
        let plan = RenderPlan::for_lesson(&lesson, &context)
            .expect("the sample context resolves the test lesson's speaker");
        (context, plan)
    }

    /// The plan and requests a document produces once it is accepted.
    ///
    /// Pure by construction: planning and [`synthesis_requests`] reach no
    /// filesystem and start no process, which is what lets the two ordering
    /// tests below be T1. Selection is deliberately not exercised here — it is
    /// the one part of the gate that reads a file, and
    /// `t4_e2_a_stale_takes_file_is_refused_before_any_synthesis` observes it.
    fn requests_for(document: &[u8]) -> Result<Vec<crate::SynthesisRequest>, BuildError> {
        let lesson = ValidatedLesson::from_json(DOCUMENT, document)?;
        let descriptor = crate::synthesis::sample_descriptor();
        let context =
            descriptor.synthesis_context(lesson.language().clone(), sample_conditioning());
        let plan = RenderPlan::for_lesson(&lesson, &context)?;
        Ok(synthesis_requests(&plan, &context)?)
    }

    #[test]
    fn t1_e1_unreviewed_lesson_fails_before_worker_start() {
        let unreviewed = lesson_document(|document| {
            document["segments"][1]["review_status"] = serde_json::json!("draft");
        });

        let error = requests_for(&unreviewed).expect_err("an unreviewed lesson must be refused");

        assert!(
            matches!(
                &error,
                BuildError::Lesson(diagnostic)
                    if matches!(
                        diagnostic.error(),
                        LessonError::UnapprovedSegment(id) if id == "segment-b"
                    )
            ),
            "expected an unapproved-segment refusal, got {error}"
        );

        // The refusal precedes the worker because no `SynthesisRequest` exists
        // to send one: planning accepts only a `ValidatedLesson`, and
        // validation is what did not return. Correcting the one field the
        // document is wrong about is what proves it — a lesson refused for
        // some unrelated reason would still be refused here.
        //
        // By construction, which is all a T1 can give: this test observes no
        // executor, and `build_preview` receives one already constructed, so
        // an executor that started a worker in its own constructor would
        // satisfy every assertion here. The observed half is
        // `t4_e0_unapproved_content_fails_before_tools_and_synthesis`, which
        // drives the real orchestration and asserts the backend is never
        // reached — `touch_count() == 0`, not merely unsynthesized.
        let reviewed = lesson_document(|document| {
            document["segments"][1]["review_status"] = serde_json::json!(ReviewStatus::Approved);
        });
        assert_eq!(
            requests_for(&reviewed)
                .expect("only the review state may refuse this document")
                .len(),
            2
        );
    }

    #[test]
    fn t1_e1_display_text_never_enters_synthesis_request() {
        // ADR-0001 §8.3 keeps the two texts apart so a pronunciation edit
        // cannot hide a semantic one, and `AGENTS.md` §Architectural
        // invariants keeps display-only metadata out of the backend entirely.
        // A marker no other field carries is what makes "never entered"
        // checkable rather than assumed.
        const MARKER: &str = "DISPLAY-ONLY-MARKER";
        let document = lesson_document(|document| {
            for segment in document["segments"]
                .as_array_mut()
                .expect("the fixture has segments")
            {
                segment["display_text"] = serde_json::json!(format!("{MARKER} reads differently."));
            }
        });

        let requests = requests_for(&document).expect("the marked lesson is otherwise valid");

        for request in &requests {
            let sent = format!("{request:?}");
            assert!(
                !sent.contains(MARKER),
                "display text reached the synthesis request for `{}`: {sent}",
                request.segment_id
            );
            assert_eq!(request.spoken_text, "Same speech.");
        }
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn t1_e0_synthesis_request_ids_include_segment_identity() {
        let (context, plan) = planned_lesson();

        let requests = synthesis_requests(&plan, &context)
            .expect("the sample context resolves every planned speaker");

        assert_eq!(plan.segments[0].cache_key, plan.segments[1].cache_key);
        assert_eq!(
            requests[0].request_id,
            format!("e0-{}-segment-a", plan.segments[0].cache_key)
        );
        assert_eq!(
            requests[1].request_id,
            format!("e0-{}-segment-b", plan.segments[1].cache_key)
        );
        assert_ne!(requests[0].request_id, requests[1].request_id);
    }

    #[test]
    fn t1_e1_a_synthesis_request_carries_the_take_its_cache_key_names() {
        // The take is a term of the cache key and a required `synthesize`
        // frame parameter, so dropping it here would ask an executor to
        // reproduce a distinction it cannot see. `for_lesson` selects
        // `BASE_TAKE` for every segment today, which is exactly why the plan
        // is edited: a test reading only what the planner writes would pass
        // for a mapping that hard-coded zero.
        let (context, mut plan) = planned_lesson();
        plan.segments[1].take = study_tts_core::BASE_TAKE + 7;

        let requests = synthesis_requests(&plan, &context)
            .expect("the sample context resolves every planned speaker");

        assert_eq!(requests[0].take, plan.segments[0].take);
        assert_eq!(requests[1].take, plan.segments[1].take);
    }

    #[test]
    fn t1_e0_block_on_resumes_after_cross_thread_wake() {
        let (waker_sender, waker_receiver) = mpsc::channel::<Waker>();
        let waking_thread = thread::spawn(move || {
            waker_receiver
                .recv()
                .expect("receive the local block_on waker")
                .wake();
        });

        let output = block_on(PendingThenReady {
            waker_sender: Some(waker_sender),
        });
        waking_thread.join().expect("join the waking thread");

        assert_eq!(output, "resumed");
    }
}
