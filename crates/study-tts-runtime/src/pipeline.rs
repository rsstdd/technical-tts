//! The order a preview build happens in, and the gates that precede it.
//!
//! Every gate — lesson validity, rights classification, voice consent,
//! external-tool preflight — runs before any synthesis or tool work. That
//! ordering is the point of this module: a refusal must name the policy that
//! refused rather than the first thing that happened to break, and the tests
//! prove it by pointing a build at a missing tool and asserting the gate's own
//! error.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use study_tts_core::{
    MAX_LESSON_JSON_BYTES, ReleaseClaim, ReleaseStatus, RenderPlan, RightsDecision,
    SourceRightsDeclaration, ValidatedLesson, VoiceError, VoiceUse, validate_lesson_id,
};

use crate::{
    BuildError, IoError, PublicationError, RightsError, SegmentSynthesizer, assembly, cache,
    durable::OsDurableFileSystem, export, io_error, locking, managed, manifest, preview, tools,
    voice_gate,
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
    /// Voice profile directory in the ADR-0001 §12.1 layout, gated fail-closed
    /// before any tool or synthesis work. `None` is valid only while the
    /// deterministic skeleton worker is the backend; the real-worker story
    /// (E0-S3/E1) makes a profile mandatory.
    pub voice_profile_dir: Option<PathBuf>,
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
    /// The encoded distribution copy.
    pub m4a: PathBuf,
    /// The manifest recording segments, checksums, and the tools used.
    pub manifest: PathBuf,
}

/// Builds one lesson into a private preview.
///
/// Every gate runs before any tool or synthesis work, so a refusal names the
/// gate rather than a missing binary. The result is always a private preview;
/// only `publish` can claim more.
///
/// # Errors
///
/// [`IoError::ReadFile`] when the lesson cannot be opened or read and
/// [`IoError::LessonNotRegularFile`] when the opened descriptor is not a
/// regular file. Lesson parsing and validation return
/// [`study_tts_core::LessonError::InvalidJson`],
/// [`study_tts_core::LessonError::UnsupportedSchema`],
/// [`study_tts_core::LessonError::MissingLessonId`],
/// [`study_tts_core::LessonError::InvalidLessonId`],
/// [`study_tts_core::LessonError::MissingSegments`],
/// [`study_tts_core::LessonError::MissingSegmentId`],
/// [`study_tts_core::LessonError::InvalidSegmentId`],
/// [`study_tts_core::LessonError::DuplicateSegmentId`],
/// [`study_tts_core::LessonError::MissingSpokenText`],
/// [`study_tts_core::LessonError::MissingDisplayText`],
/// [`study_tts_core::LessonError::MissingRole`],
/// [`study_tts_core::LessonError::MissingSourceRefs`],
/// [`study_tts_core::LessonError::EmptySourceRef`],
/// [`study_tts_core::LessonError::UnapprovedSegment`],
/// [`study_tts_core::LessonError::MissingSpeaker`],
/// [`study_tts_core::LessonError::MissingStyle`], or
/// [`study_tts_core::LessonError::PauseOutOfRange`]. Resource refusal returns
/// [`study_tts_core::LessonError::LessonJsonTooLarge`],
/// [`study_tts_core::LessonError::TooManySegments`],
/// [`study_tts_core::LessonError::SpokenTextTooLong`],
/// [`study_tts_core::LessonError::DisplayTextTooLong`],
/// [`study_tts_core::LessonError::TooManySourceRefs`],
/// [`study_tts_core::LessonError::SourceRefTooLong`], or
/// [`study_tts_core::LessonError::AuthoredTextTooLarge`].
///
/// Voice gating returns [`crate::VoiceProfileError::MissingVoiceRecord`],
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
/// Tool work returns [`crate::ToolError::MissingTool`],
/// [`crate::ToolError::InspectTool`], [`crate::ToolError::ToolProbeFailed`],
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
/// [`crate::AudioError::UnusableAudio`],
/// [`crate::AudioError::SynthesizerReportMismatch`],
/// [`crate::AudioError::PauseFrameOverflow`],
/// [`crate::AudioError::PlannedLengthOverflow`],
/// [`crate::AudioError::AssembledLengthOverflow`],
/// [`crate::AudioError::AssembledLengthMismatch`], [`IoError::FileSystem`],
/// [`IoError::AudioAt`], [`IoError::WriteJson`], or
/// [`BuildError::Synthesis`].
///
/// [`crate::ManagedPathError::InvalidManagedName`] is not among them: every
/// name this function offers a managed helper is either a literal or an
/// identifier the lesson gate already refused, so an unusable spelling is
/// reported as the authoring mistake it is.
///
/// Durable ownership and publication may return
/// [`crate::DurableStateError::LiveJobLock`],
/// [`crate::DurableStateError::MalformedJobLock`],
/// [`crate::DurableStateError::IncompatibleJobLock`],
/// [`crate::DurableStateError::CacheLockTimeout`],
/// [`crate::DurableStateError::MalformedPublicationJournal`],
/// [`crate::DurableStateError::MalformedCurrentPreview`],
/// [`crate::DurableStateError::UnsupportedDurableRecord`],
/// [`crate::DurableStateError::CurrentLessonMismatch`],
/// [`crate::DurableStateError::PublicationJournalLessonMismatch`],
/// [`crate::DurableStateError::InvalidCurrentPackageReference`],
/// [`crate::DurableStateError::MissingPackageDirectory`],
/// [`crate::DurableStateError::MalformedPackageManifest`],
/// [`crate::DurableStateError::UnsupportedPackageManifest`],
/// [`crate::DurableStateError::PackageReleaseStatusMismatch`],
/// [`crate::DurableStateError::PackageLessonMismatch`],
/// [`crate::DurableStateError::MalformedPackagePlanHash`],
/// [`crate::DurableStateError::EmptyPackageSegmentId`],
/// [`crate::DurableStateError::MalformedPackageSegmentChecksum`],
/// [`crate::DurableStateError::EmptyPackageSegmentAudio`],
/// [`crate::DurableStateError::UnexpectedPackageArtifactPath`],
/// [`crate::DurableStateError::MalformedPackageArtifactChecksum`],
/// [`crate::DurableStateError::PackageArtifactChecksumMismatch`],
/// [`crate::DurableStateError::MissingPackageToolArguments`],
/// [`crate::DurableStateError::MalformedPackageToolProfile`],
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
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<BuildResult, BuildError> {
    let lesson_bytes = read_lesson(&request.lesson_path)?;
    let lesson = ValidatedLesson::from_json(&lesson_bytes)?;
    let plan = RenderPlan::for_lesson(&lesson, synthesizer.identity());

    // Rights precede work: the profile gate runs before tool preflight and
    // synthesis, so a refused voice performs no observable work. The loaded
    // identity is unused by the skeleton worker; the real-worker story consumes
    // it and records the ADR-0001 §15.3 per-build audit event.
    if let Some(dir) = &request.voice_profile_dir {
        let _profile = voice_gate::load_profile(dir, VoiceUse::PrivateSynthesis)?;
    }

    let ffmpeg = tools::inspect("FFmpeg", &request.ffmpeg_executable)?;
    let ffprobe = tools::inspect("ffprobe", &request.ffprobe_executable)?;
    let export_profiles = export::export_profiles();

    fs::create_dir_all(&request.workspace).map_err(|error| io_error(&request.workspace, error))?;
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let filesystem = OsDurableFileSystem;
    let cache_root = managed::subdirectory(&workspace, "cache")?;
    let roots = preview::roots(&workspace, lesson.lesson_id())?;
    let _job_lock = locking::acquire_job_lock(&filesystem, &roots.job_dir, lesson.lesson_id())?;
    preview::reconcile(&filesystem, &roots, lesson.lesson_id())?;

    let cached_segments = plan
        .segments
        .iter()
        .map(|segment| {
            cache::resolve(
                &filesystem,
                &cache_root,
                &roots.quarantine_root,
                lesson.lesson_id(),
                segment,
                synthesizer,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(package) = preview::current_for_build(
        &roots,
        lesson.lesson_id(),
        &plan.plan_hash,
        &ffmpeg,
        &ffprobe,
        &export_profiles,
    )? {
        return Ok(BuildResult {
            package_dir: package.package_dir,
            publication_record: package.publication_record,
            master_wav: package.master_wav,
            m4a: package.m4a,
            manifest: package.manifest,
        });
    }

    let transaction = preview::start_transaction(
        &filesystem,
        &roots,
        lesson.lesson_id(),
        &plan.plan_hash,
        &ffmpeg,
        &ffprobe,
        &export_profiles,
    )?;

    let master_wav = managed::leaf(&transaction.stage_dir, manifest::MASTER_WAV_NAME)?;
    assembly::assemble(&cached_segments, &master_wav)?;
    let m4a = managed::leaf(&transaction.stage_dir, manifest::M4A_NAME)?;
    let ffmpeg_execution = export::export_m4a(&ffmpeg, &export_profiles.ffmpeg, &master_wav, &m4a)?;
    let ffprobe_execution = export::probe_m4a(&ffprobe, &export_profiles.ffprobe, &m4a)?;
    let manifest_path = managed::leaf(&transaction.stage_dir, manifest::MANIFEST_NAME)?;
    manifest::write(
        &filesystem,
        &manifest_path,
        manifest::ManifestRecords {
            lesson_id: lesson.lesson_id(),
            plan_hash: &plan.plan_hash,
            segments: &cached_segments,
            master_wav: &master_wav,
            m4a: &m4a,
            tools: manifest::ToolRecords {
                ffmpeg: &ffmpeg,
                ffmpeg_execution: &ffmpeg_execution,
                ffprobe: &ffprobe,
                ffprobe_execution: &ffprobe_execution,
            },
        },
    )?;
    let package = preview::publish_transaction(&filesystem, &roots, &transaction)?;

    Ok(BuildResult {
        package_dir: package.package_dir,
        publication_record: package.publication_record,
        master_wav: package.master_wav,
        m4a: package.m4a,
        manifest: package.manifest,
    })
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
        return Err(study_tts_core::LessonError::LessonJsonTooLarge {
            max_bytes: MAX_LESSON_JSON_BYTES,
        }
        .into());
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
        return Err(study_tts_core::LessonError::LessonJsonTooLarge {
            max_bytes: MAX_LESSON_JSON_BYTES,
        }
        .into());
    }
    Ok(bytes)
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
pub fn validate_encoded_output(
    ffprobe_executable: &Path,
    encoded: &Path,
) -> Result<(), BuildError> {
    let ffprobe = tools::inspect("ffprobe", ffprobe_executable)?;
    let profiles = export::export_profiles();
    export::probe_m4a(&ffprobe, &profiles.ffprobe, encoded).map(|_| ())
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

/// The provisional production-manifest shape, pending the E1-S1 versioned JSON
/// Schemas.
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
/// Provisional shape pending the E1-S1 versioned JSON Schemas, like
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
/// `voice_profiles` manifest sections that the E1-S1 schema story will version.
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
    validate_lesson_id(&manifest.lesson_id)?;

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
        io::{self, Read},
        path::Path,
    };

    use study_tts_core::{LessonError, MAX_LESSON_JSON_BYTES};

    use super::read_lesson_from_reader;
    use crate::BuildError;

    #[test]
    fn t1_e0_bounded_lesson_reader_refuses_growth_after_metadata_preflight() {
        let reader = io::repeat(b'{').take((MAX_LESSON_JSON_BYTES + 1) as u64);

        let error = read_lesson_from_reader(Path::new("lesson.json"), reader, 1)
            .expect_err("a stream that grows beyond its advertised size must be refused");

        assert!(matches!(
            error,
            BuildError::Lesson(LessonError::LessonJsonTooLarge { max_bytes })
                if max_bytes == MAX_LESSON_JSON_BYTES
        ));
    }
}
