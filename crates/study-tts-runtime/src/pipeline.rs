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
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, LessonDiagnostic,
    MAX_LESSON_JSON_BYTES, PlanError, ProvisionalJobSnapshot, ProvisionalJobStage, ReleaseClaim,
    ReleaseStatus, RenderPlan, RightsDecision, SourceRightsDeclaration, SynthesisContext,
    ValidatedLesson, VoiceConditioningHash, VoiceError, VoiceUse, validate_lesson_id,
};

use crate::{
    BackendDescriptor, BackendError, BuildError, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, FileSystemJobRepository, FileSystemPackageWriter, IoError,
    JobRepository, PackagePreflightRequest, PackagePrepareRequest, PackageWriteRequest,
    PackageWriter, PublicationError, RightsError, SynthesisRequest, TtsExecutor, export, io_error,
    tools, voice_gate,
};

/// Everything one preview build needs, named explicitly rather than read from
/// ambient state.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    /// The lesson document to build; its validated `lesson_id` is the
    /// provisional E0 lock and publication identity until versioned job IDs
    /// land in E2.
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
    /// Durable job ownership and snapshot repository.
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
/// [`crate::DurableStateError::JobSnapshotIdentityMismatch`],
/// [`crate::DurableStateError::JobSnapshotSelectionMismatch`],
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

/// Builds one lesson exclusively through the published E0-S4 service seams.
///
/// # Errors
///
/// Returns exactly the [`BuildError`] variants documented by [`build_preview`];
/// injected ports retain ownership of their validation and durability errors.
pub fn build_preview_with_services(
    request: BuildRequest,
    services: PreviewServiceBundle<'_>,
) -> Result<BuildResult, BuildError> {
    let lesson = load_lesson(&request.lesson_path)?;

    // Rights precede work, and now precede planning too: the conditioning
    // artifact each profile carries is an ADR-0001 §12.5 synthesis-key input,
    // so a plan derived before this gate would name cache entries for voices
    // nobody resolved. A refused voice still performs no observable work.
    let voice_conditioning_hashes = voice_gate::resolve_speakers(
        &request.voice_profile_root,
        &lesson,
        VoiceUse::PrivateSynthesis,
    )?;

    let descriptor = services.executor.descriptor();
    let (plan, synthesis_requests) =
        plan_requests(&lesson, &descriptor, voice_conditioning_hashes)?;
    for synthesis_request in &synthesis_requests {
        services.executor.validate(synthesis_request)?;
    }

    let packages = services.packages.preflight(&PackagePreflightRequest {
        ffmpeg_executable: &request.ffmpeg_executable,
        ffprobe_executable: &request.ffprobe_executable,
    })?;

    fs::create_dir_all(&request.workspace).map_err(|error| io_error(&request.workspace, error))?;
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let _job_ownership = services.jobs.claim(&workspace, lesson.lesson_id())?;
    let planned = ProvisionalJobSnapshot::planned(lesson.lesson_id(), plan.plan_hash.clone());
    services.jobs.replace(&workspace, &planned)?;
    packages.prepare(&PackagePrepareRequest {
        workspace: &workspace,
        job_id: lesson.lesson_id(),
        plan: &plan,
    })?;

    services
        .jobs
        .replace(&workspace, &planned.advancing(ProvisionalJobStage::Caching))?;
    let mut cached_segments = Vec::with_capacity(plan.segments.len());
    for (segment, synthesis_request) in plan.segments.iter().zip(synthesis_requests) {
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
                workspace: workspace.clone(),
                job_id: lesson.lesson_id().to_owned(),
                segment: segment.clone(),
            },
            &mut producer,
        )?;
        cached_segments.push(cached);
    }

    services.jobs.replace(
        &workspace,
        &planned.advancing(ProvisionalJobStage::Packaging),
    )?;
    let package = packages.write(&PackageWriteRequest {
        workspace: &workspace,
        job_id: lesson.lesson_id(),
        plan: &plan,
        cached_artifacts: &cached_segments,
    })?;
    services
        .jobs
        .replace(&workspace, &planned.selecting(package.identity.clone()))?;

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

/// Derives the plan and the backend requests for one validated lesson.
///
/// Pure: no filesystem, no process, no clock. That is what lets the ordering
/// guarantees above be asserted without a workspace — an unreviewed lesson
/// never reaches here, and nothing display-only can leave here, because
/// [`RenderPlan`] is the only thing this reads.
fn plan_requests(
    lesson: &ValidatedLesson,
    descriptor: &BackendDescriptor,
    voice_conditioning_hashes: BTreeMap<String, VoiceConditioningHash>,
) -> Result<(RenderPlan, Vec<SynthesisRequest>), BuildError> {
    let context =
        descriptor.synthesis_context(lesson.language().clone(), voice_conditioning_hashes);
    let plan = RenderPlan::for_lesson(lesson, &context)?;
    let requests = synthesis_requests(&plan, &context)?;
    Ok((plan, requests))
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
fn read_lesson(path: &Path) -> Result<Vec<u8>, BuildError> {
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
/// its sources and voices. A manifest that clears all of those is refused with
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

    use super::{block_on, plan_requests, read_lesson_from_reader, synthesis_requests};
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
    /// Pure by construction: [`plan_requests`] reaches no filesystem and starts
    /// no process, which is what lets the two ordering tests below be T1.
    fn requests_for(document: &[u8]) -> Result<Vec<crate::SynthesisRequest>, BuildError> {
        let lesson = ValidatedLesson::from_json(DOCUMENT, document)?;
        let descriptor = crate::synthesis::sample_descriptor();
        Ok(plan_requests(&lesson, &descriptor, sample_conditioning())?.1)
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
        // to send one: `plan_requests` accepts only a `ValidatedLesson`, and
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
