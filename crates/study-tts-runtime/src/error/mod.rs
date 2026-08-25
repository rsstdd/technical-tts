//! Typed build refusals grouped by the boundary that owns each decision.
//!
//! [`BuildError`] is deliberately shallow: it identifies the failing category,
//! while each category enum identifies the exact violated invariant. Fault
//! enums such as [`AudioFault`] and [`CacheEntryFault`] carry only reusable
//! inner detail; the owning category supplies paths, artifact identity, and
//! operator remedy so context does not drift between call sites.
//!
//! Only [`io_error`] and [`audio_error`] generically enrich pathless library
//! errors. Domain refusals must be constructed through their owning category
//! enum. When adding a refusal, place it in the narrowest category, preserve a
//! distinct leaf variant, document the invariant and defensive reachability,
//! add a governed [`RemedyAdvice`] only when the Failure routing table in
//! `docs/governance/ROUTING-TABLES.md` establishes an owner, and update exact
//! variant, message, conversion, source-chain, and advice tests together.
//! [`BuildError::remedy`] and the category remedy methods mirror that table;
//! `t1_e0_governed_remedy_mappings_are_exhaustive` pins the governed mapping.

use std::{io, path::PathBuf};

use study_tts_core::{LessonError, ReleaseError, VoiceError};
use thiserror::Error;

use crate::SynthesisError;

mod audio;
mod cache;
mod io_error;
mod managed_path;
mod publication;
mod rights;
mod state;
mod tool;
mod voice_profile;

pub use audio::{AudioError, AudioFault};
pub use cache::{CacheEntryFault, CacheError};
pub use io_error::IoError;
pub use managed_path::ManagedPathError;
pub use publication::PublicationError;
pub use rights::RightsError;
pub use state::DurableStateError;
pub use tool::{ToolError, ToolInvocation, ToolOperation, ToolOutputStream};
pub use voice_profile::VoiceProfileError;

/// Why a build or publication was refused, grouped by its owning boundary.
#[derive(Debug, Error)]
pub enum BuildError {
    /// A path-bound IO, WAV, or JSON operation failed.
    #[error(transparent)]
    Io(#[from] IoError),
    /// The lesson domain refused an authored lesson or identifier.
    #[error(transparent)]
    Lesson(#[from] LessonError),
    /// The voice-record domain refused consent, approval, or scope.
    #[error(transparent)]
    Voice(#[from] VoiceError),
    /// Runtime voice-profile records were missing or failed integrity checks.
    #[error(transparent)]
    VoiceProfile(#[from] VoiceProfileError),
    /// Rights declarations refused production publication.
    #[error(transparent)]
    Rights(#[from] RightsError),
    /// A release profile or production manifest refused publication.
    #[error(transparent)]
    Publication(#[from] PublicationError),
    /// A published cache entry failed acceptance.
    #[error(transparent)]
    Cache(#[from] CacheError),
    /// Rendered or assembled audio failed an invariant.
    #[error(transparent)]
    Audio(#[from] AudioError),
    /// External-tool or encoded-stream validation failed.
    #[error(transparent)]
    Tool(#[from] ToolError),
    /// A managed name, path, or destination was unsafe or unusable.
    #[error(transparent)]
    ManagedPath(#[from] ManagedPathError),
    /// Durable ownership, journal, or selected-package state was unsafe.
    #[error(transparent)]
    DurableState(Box<DurableStateError>),
    /// The synthesizer refused or failed.
    #[error(transparent)]
    Synthesis(#[from] SynthesisError),
}

impl BuildError {
    /// Returns governed recovery advice when the routing table establishes it.
    pub fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::Io(_) | Self::Lesson(_) | Self::Synthesis(_) => None,
            Self::Voice(error) => voice_remedy(error),
            Self::VoiceProfile(error) => error.remedy(),
            Self::Rights(error) => error.remedy(),
            Self::Publication(error) => error.remedy(),
            Self::Cache(error) => error.remedy(),
            Self::Audio(error) => error.remedy(),
            Self::Tool(error) => error.remedy(),
            Self::ManagedPath(error) => error.remedy(),
            Self::DurableState(error) => error.remedy(),
        }
    }
}

// `From` is not transitive, so callers need this direct bridge through the
// publication category to keep propagating core release errors with `?`.
impl From<ReleaseError> for BuildError {
    fn from(error: ReleaseError) -> Self {
        Self::Publication(PublicationError::Release(error))
    }
}

impl From<DurableStateError> for BuildError {
    fn from(error: DurableStateError) -> Self {
        Self::DurableState(Box::new(error))
    }
}

/// Structured recovery guidance attached only to governed refusals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemedyAdvice {
    owner: RemedyOwner,
    action: &'static str,
    routing: Option<&'static str>,
}

impl RemedyAdvice {
    const fn new(owner: RemedyOwner, action: &'static str, routing: Option<&'static str>) -> Self {
        Self {
            owner,
            action,
            routing,
        }
    }

    /// Returns the governed owner responsible for the recovery action.
    pub const fn owner(self) -> RemedyOwner {
        self.owner
    }

    /// Returns the static recovery action for the refusal.
    pub const fn action(self) -> &'static str {
        self.action
    }

    /// Returns the routing-table row that establishes the advice, when named.
    pub const fn routing(self) -> Option<&'static str> {
        self.routing
    }
}

/// Governed owner responsible for a structured recovery action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemedyOwner {
    /// The project owner controls publication, rights, and consent decisions.
    ProjectOwner,
    /// The runtime owner controls durable state and reconciliation.
    Runtime,
    /// The audio/runtime owner controls audio validation and encoding.
    AudioRuntime,
    /// The worker/runtime owner controls worker and containment boundaries.
    WorkerRuntime,
    /// The named gate owner controls corrective release-gate work.
    GateOwner,
}

fn voice_remedy(error: &VoiceError) -> Option<RemedyAdvice> {
    match error {
        VoiceError::MalformedChecksum { .. }
        | VoiceError::ConsentNotGranted { .. }
        | VoiceError::ProfileNotApproved { .. }
        | VoiceError::ConsentScopeExcluded { .. }
        | VoiceError::ConsentChecksumDisagreement { .. } => Some(RemedyAdvice::new(
            RemedyOwner::ProjectOwner,
            "resolve the voice consent, approval, or checksum record before use",
            Some("Voice consent/checksum mismatch"),
        )),
        VoiceError::InvalidJson(_)
        | VoiceError::UnsupportedSchema(_)
        | VoiceError::MissingField(_) => None,
    }
}

/// Attaches a path to a generic filesystem failure.
pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    IoError::FileSystem {
        path: path.into(),
        source,
    }
    .into()
}

/// Attaches a path to a generic WAV-layer failure.
pub(crate) fn audio_error(path: impl Into<PathBuf>, source: hound::Error) -> BuildError {
    IoError::AudioAt {
        path: path.into(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        io,
        mem::size_of,
        path::{Path, PathBuf},
    };

    use study_tts_core::{LessonError, ReleaseError, ReleaseStatus, VoiceError};

    use super::*;

    // Mirrors the supported-target measurement in
    // `docs/architecture/WALKING-SKELETON.md` §Provisional boundary ownership.
    const PRE_REFACTOR_BUILD_ERROR_SIZE_BYTES: usize = 80;

    fn json_error() -> serde_json::Error {
        serde_json::from_slice::<serde_json::Value>(b"{")
            .expect_err("the fixture must be malformed JSON")
    }

    fn expected_voice_profile_remedy(error: &VoiceProfileError) -> Option<RemedyAdvice> {
        match error {
            VoiceProfileError::MissingVoiceRecord { .. }
            | VoiceProfileError::VoiceRecordNotRegularFile { .. }
            | VoiceProfileError::VoiceChecksumMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "supply or correct the voice profile record before use",
                Some("Voice consent/checksum mismatch"),
            )),
        }
    }

    fn expected_rights_remedy(error: &RightsError) -> Option<RemedyAdvice> {
        match error {
            RightsError::UnresolvedContentRights { .. }
            | RightsError::InvalidRightsDeclaration { .. }
            | RightsError::MissingContentRightsDeclaration
            | RightsError::MissingVoiceProfileDeclaration
            | RightsError::EmptyManifestIdentifier { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "correct or resolve the rights declaration before publication",
                Some("Missing rights classification"),
            )),
        }
    }

    fn expected_cache_remedy(error: &CacheError) -> Option<RemedyAdvice> {
        match error {
            CacheError::UnusableCacheEntry { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the unusable cache entry and run runtime reconciliation",
                Some("State or checksum corruption"),
            )),
        }
    }

    fn expected_voice_remedy(error: &VoiceError) -> Option<RemedyAdvice> {
        match error {
            VoiceError::MalformedChecksum { .. }
            | VoiceError::ConsentNotGranted { .. }
            | VoiceError::ProfileNotApproved { .. }
            | VoiceError::ConsentScopeExcluded { .. }
            | VoiceError::ConsentChecksumDisagreement { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "resolve the voice consent, approval, or checksum record before use",
                Some("Voice consent/checksum mismatch"),
            )),
            VoiceError::InvalidJson(_)
            | VoiceError::UnsupportedSchema(_)
            | VoiceError::MissingField(_) => None,
        }
    }

    fn expected_publication_remedy(error: &PublicationError) -> Option<RemedyAdvice> {
        match error {
            PublicationError::Release(ReleaseError::MissingGateEvidence(_))
            | PublicationError::ProductionGatesUnavailable => Some(RemedyAdvice::new(
                RemedyOwner::GateOwner,
                "preserve the candidate and create a corrective gate issue",
                Some("Failed release gate"),
            )),
            PublicationError::Release(ReleaseError::PrivateProfileCannotClaimProduction)
            | PublicationError::MalformedProductionManifest { .. }
            | PublicationError::ManifestNotProductionRelease { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "publish a corrected manifest from a build that earned production status",
                Some("Production publication"),
            )),
            PublicationError::UnsupportedProductionManifest { .. } => None,
        }
    }

    fn expected_audio_remedy(error: &AudioError) -> Option<RemedyAdvice> {
        match error {
            AudioError::UnusableAudio { .. } => Some(RemedyAdvice::new(
                RemedyOwner::AudioRuntime,
                "quarantine the attempt and retry within the bounded budget",
                Some("Invalid or over-range audio"),
            )),
            AudioError::SynthesizerReportMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "correct the worker report before rerunning the build",
                Some("Worker protocol or containment failure"),
            )),
            AudioError::AssembledLengthMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "reconcile the cache before rebuilding the lesson",
                Some("State or checksum corruption"),
            )),
            AudioError::PauseFrameOverflow { .. }
            | AudioError::PlannedLengthOverflow
            | AudioError::AssembledLengthOverflow { .. } => None,
        }
    }

    fn expected_tool_remedy(error: &ToolError) -> Option<RemedyAdvice> {
        match error {
            ToolError::UnreadableProbeResponse { .. }
            | ToolError::UnexpectedEncodedStreamCount { .. }
            | ToolError::UnexpectedEncodedStream { .. } => Some(RemedyAdvice::new(
                RemedyOwner::AudioRuntime,
                "reconcile the encode settings with output verification",
                Some("Invalid or over-range audio"),
            )),
            ToolError::ToolCleanupFailed { .. }
            | ToolError::ToolChildInspectionFailed { .. }
            | ToolError::ToolTerminationSignalFailed { .. }
            | ToolError::ToolContainmentInspectionFailed { .. }
            | ToolError::ToolContainmentSignalFailed { .. }
            | ToolError::ToolChildReapFailed { .. }
            | ToolError::ToolTerminationTimedOut { .. }
            | ToolError::ToolReaperStartFailed { .. }
            | ToolError::ToolCaptureReaperStartFailed { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "preserve diagnostics and correct the external-tool containment lifecycle",
                Some("Worker protocol or containment failure"),
            )),
            ToolError::Ffmpeg { .. }
            | ToolError::ToolTimedOut { .. }
            | ToolError::ToolOutputOverflow { .. }
            | ToolError::ToolPipeUnavailable { .. }
            | ToolError::ToolCaptureConfigurationFailed { .. }
            | ToolError::ToolCaptureStartFailed { .. }
            | ToolError::ToolCaptureReadFailed { .. }
            | ToolError::ToolCaptureChannelClosed { .. }
            | ToolError::ToolCaptureThreadPanicked { .. }
            | ToolError::ToolCaptureShutdownTimedOut { .. }
            | ToolError::ToolCaptureIncomplete { .. }
            | ToolError::StartFfmpeg { .. }
            | ToolError::MissingTool { .. }
            | ToolError::InspectTool { .. }
            | ToolError::ToolProbeFailed { .. }
            | ToolError::Ffprobe { .. } => None,
        }
    }

    fn expected_managed_path_remedy(error: &ManagedPathError) -> Option<RemedyAdvice> {
        match error {
            ManagedPathError::InvalidManagedName { .. }
            | ManagedPathError::ManagedPathEscape { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "correct the managed-path caller before rerunning the build",
                Some("Worker protocol or containment failure"),
            )),
            ManagedPathError::UnrootedDestination { .. } => None,
        }
    }

    fn expected_durable_state_remedy(error: &DurableStateError) -> Option<RemedyAdvice> {
        match error {
            DurableStateError::LiveJobLock { .. } => None,
            DurableStateError::CacheLockTimeout { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve attempts and inspect the cache-key owner before retrying",
                None,
            )),
            DurableStateError::QuarantineFailed { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the staging attempt and repair quarantine before retrying",
                None,
            )),
            DurableStateError::MalformedJobLock { .. }
            | DurableStateError::IncompatibleJobLock { .. }
            | DurableStateError::MalformedPublicationJournal { .. }
            | DurableStateError::MalformedCurrentPreview { .. }
            | DurableStateError::UnsupportedDurableRecord { .. }
            | DurableStateError::CurrentLessonMismatch { .. }
            | DurableStateError::PublicationJournalLessonMismatch { .. }
            | DurableStateError::InvalidCurrentPackageReference { .. }
            | DurableStateError::MissingPackageDirectory { .. }
            | DurableStateError::MalformedPackageManifest { .. }
            | DurableStateError::UnsupportedPackageManifest { .. }
            | DurableStateError::PackageReleaseStatusMismatch { .. }
            | DurableStateError::PackageLessonMismatch { .. }
            | DurableStateError::MalformedPackagePlanHash { .. }
            | DurableStateError::EmptyPackageSegmentId { .. }
            | DurableStateError::MalformedPackageSegmentChecksum { .. }
            | DurableStateError::EmptyPackageSegmentAudio { .. }
            | DurableStateError::UnexpectedPackageArtifactPath { .. }
            | DurableStateError::MalformedPackageArtifactChecksum { .. }
            | DurableStateError::PackageArtifactChecksumMismatch { .. }
            | DurableStateError::MissingPackageToolArguments { .. }
            | DurableStateError::MalformedPackageToolProfile { .. }
            | DurableStateError::PackageManifestChecksumMismatch { .. }
            | DurableStateError::MalformedDurableDigest { .. }
            | DurableStateError::MissingCurrentPreview { .. }
            | DurableStateError::JournalSelectionMismatch { .. }
            | DurableStateError::PackagePlanMismatch { .. }
            | DurableStateError::InvalidJobDirectoryName { .. }
            | DurableStateError::PublicationConflict { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                concat!(
                    "preserve the artifacts and run runtime reconciliation without overwrite ",
                    "or deletion",
                ),
                Some("State or checksum corruption"),
            )),
        }
    }

    fn assert_expected_remedy(error: BuildError, expected: Option<RemedyAdvice>) {
        assert_eq!(error.remedy(), expected, "`{error}` has the wrong remedy");
    }

    fn assert_durable_state_remedy(error: DurableStateError, expected: Option<RemedyAdvice>) {
        assert_eq!(expected_durable_state_remedy(&error), expected);
        assert_expected_remedy(error.into(), expected);
    }

    fn tool_invocation() -> ToolInvocation {
        ToolInvocation::new("FFmpeg", ToolOperation::M4aEncode, Path::new("lesson.m4a"))
    }

    #[test]
    fn t1_e0_tool_invocation_preserves_typed_operation_context() {
        for (operation, expected_label) in [
            (ToolOperation::VersionProbe, "version probe"),
            (ToolOperation::M4aEncode, "M4A encode"),
            (ToolOperation::M4aValidation, "M4A validation"),
        ] {
            let invocation = ToolInvocation::new("tool", operation, Path::new("subject"));

            assert_eq!(invocation.tool(), "tool");
            assert_eq!(invocation.operation(), operation);
            assert_eq!(invocation.subject(), Path::new("subject"));
            assert_eq!(
                invocation.to_string(),
                format!("tool {expected_label} for `subject`")
            );
        }
    }

    #[test]
    fn t1_e0_every_error_category_converts_to_build_error() {
        let cases = [
            BuildError::from(IoError::FileSystem {
                path: PathBuf::from("workspace"),
                source: io::Error::other("filesystem failure"),
            }),
            BuildError::from(VoiceProfileError::MissingVoiceRecord {
                profile_dir: PathBuf::from("voice"),
                record: "profile.json",
            }),
            BuildError::from(RightsError::MissingContentRightsDeclaration),
            BuildError::from(PublicationError::ProductionGatesUnavailable),
            BuildError::from(CacheError::UnusableCacheEntry {
                entry_dir: PathBuf::from("cache-entry"),
                segment_id: "seg-1".to_owned(),
                fault: Box::new(CacheEntryFault::MalformedRecordedDigest {
                    recorded: "wrong".to_owned(),
                }),
            }),
            BuildError::from(AudioError::PlannedLengthOverflow),
            BuildError::from(ToolError::MissingTool {
                tool: "FFmpeg".to_owned(),
                requested: PathBuf::from("ffmpeg"),
            }),
            BuildError::from(ManagedPathError::InvalidManagedName {
                name: "../escape".to_owned(),
                root: PathBuf::from("workspace"),
            }),
            BuildError::from(DurableStateError::PublicationConflict {
                path: PathBuf::from("package"),
            }),
        ];

        assert!(matches!(
            cases[0],
            BuildError::Io(IoError::FileSystem { .. })
        ));
        assert!(matches!(
            cases[1],
            BuildError::VoiceProfile(VoiceProfileError::MissingVoiceRecord { .. })
        ));
        assert!(matches!(
            cases[2],
            BuildError::Rights(RightsError::MissingContentRightsDeclaration)
        ));
        assert!(matches!(
            cases[3],
            BuildError::Publication(PublicationError::ProductionGatesUnavailable)
        ));
        assert!(matches!(
            cases[4],
            BuildError::Cache(CacheError::UnusableCacheEntry { .. })
        ));
        assert!(matches!(
            cases[5],
            BuildError::Audio(AudioError::PlannedLengthOverflow)
        ));
        assert!(matches!(
            cases[6],
            BuildError::Tool(ToolError::MissingTool { .. })
        ));
        assert!(matches!(
            cases[7],
            BuildError::ManagedPath(ManagedPathError::InvalidManagedName { .. })
        ));
        assert!(matches!(cases[8], BuildError::DurableState(_)));
    }

    #[test]
    fn t1_e0_foreign_domain_errors_convert_directly_to_build_error() {
        assert!(matches!(
            BuildError::from(LessonError::MissingLessonId),
            BuildError::Lesson(LessonError::MissingLessonId)
        ));
        assert!(matches!(
            BuildError::from(VoiceError::UnsupportedSchema("future".to_owned())),
            BuildError::Voice(VoiceError::UnsupportedSchema(_))
        ));
        assert!(matches!(
            BuildError::from(ReleaseError::PrivateProfileCannotClaimProduction),
            BuildError::Publication(PublicationError::Release(
                ReleaseError::PrivateProfileCannotClaimProduction
            ))
        ));
        assert!(matches!(
            BuildError::from(SynthesisError::new("worker failed")),
            BuildError::Synthesis(_)
        ));
    }

    #[test]
    fn t1_e0_transparent_categories_preserve_error_source_chains() {
        let error = BuildError::from(IoError::FileSystem {
            path: PathBuf::from("workspace"),
            source: io::Error::other("filesystem failure"),
        });
        assert!(
            error
                .source()
                .expect("transparent categories must expose the filesystem source")
                .downcast_ref::<io::Error>()
                .is_some()
        );

        let error = BuildError::from(LessonError::InvalidJson(json_error()));
        assert!(
            error
                .source()
                .expect("transparent foreign composition must expose the JSON source")
                .downcast_ref::<serde_json::Error>()
                .is_some()
        );
    }

    #[test]
    fn t1_e0_governed_remedy_mappings_are_exhaustive() {
        for error in [
            VoiceProfileError::MissingVoiceRecord {
                profile_dir: PathBuf::from("voice"),
                record: "profile.json",
            },
            VoiceProfileError::VoiceRecordNotRegularFile {
                profile_dir: PathBuf::from("voice"),
                record: "consent.json",
            },
            VoiceProfileError::VoiceChecksumMismatch {
                profile_dir: PathBuf::from("voice"),
                path: PathBuf::from("voice/reference.wav"),
            },
        ] {
            let expected = expected_voice_profile_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        for error in [
            RightsError::UnresolvedContentRights {
                source_id: "source".to_owned(),
                classification: "rights_review_required".to_owned(),
            },
            RightsError::InvalidRightsDeclaration {
                section: "content_rights",
                source: json_error(),
            },
            RightsError::MissingContentRightsDeclaration,
            RightsError::MissingVoiceProfileDeclaration,
            RightsError::EmptyManifestIdentifier {
                section: "content_rights",
                field: "source_id",
            },
        ] {
            let expected = expected_rights_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        for error in [
            PublicationError::Release(ReleaseError::PrivateProfileCannotClaimProduction),
            PublicationError::MalformedProductionManifest {
                source: json_error(),
            },
            PublicationError::ManifestNotProductionRelease {
                declared: ReleaseStatus::PrivatePreview,
            },
        ] {
            let expected = expected_publication_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        for error in [
            VoiceError::MalformedChecksum {
                field: "reference_wav_blake3",
                value: "wrong".to_owned(),
            },
            VoiceError::ConsentNotGranted {
                profile_id: "voice".to_owned(),
                status: "pending".to_owned(),
            },
            VoiceError::ProfileNotApproved {
                profile_id: "voice".to_owned(),
                decision: "restricted".to_owned(),
            },
            VoiceError::ConsentScopeExcluded {
                profile_id: "voice".to_owned(),
                requested: "private_synthesis",
                permitted: "voice_qualification".to_owned(),
            },
            VoiceError::ConsentChecksumDisagreement {
                profile_id: "voice".to_owned(),
            },
        ] {
            let expected = expected_voice_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        for error in [
            PublicationError::Release(ReleaseError::MissingGateEvidence("gate".to_owned())),
            PublicationError::ProductionGatesUnavailable,
        ] {
            let expected = expected_publication_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        let error = CacheError::UnusableCacheEntry {
            entry_dir: PathBuf::from("cache-entry"),
            segment_id: "seg-1".to_owned(),
            fault: Box::new(CacheEntryFault::MalformedRecordedDigest {
                recorded: "wrong".to_owned(),
            }),
        };
        let expected = expected_cache_remedy(&error);
        assert_expected_remedy(error.into(), expected);

        let error = AudioError::AssembledLengthMismatch {
            destination: PathBuf::from("master.wav"),
            assembled: 1,
            expected: 2,
        };
        let expected = expected_audio_remedy(&error);
        assert_expected_remedy(error.into(), expected);

        let error = AudioError::UnusableAudio {
            path: PathBuf::from("staged.wav"),
            fault: AudioFault::Empty,
        };
        let expected = expected_audio_remedy(&error);
        assert_expected_remedy(error.into(), expected);
        for error in [
            ToolError::UnreadableProbeResponse {
                path: PathBuf::from("lesson.m4a"),
                source: json_error(),
            },
            ToolError::UnexpectedEncodedStreamCount {
                path: PathBuf::from("lesson.m4a"),
                found: 2,
                required: 1,
            },
            ToolError::UnexpectedEncodedStream {
                path: PathBuf::from("lesson.m4a"),
                codec: Some("pcm_f32le".to_owned()),
                channels: Some(1),
                required_codec: "aac",
                required_channels: 1,
            },
            ToolError::ToolTerminationSignalFailed {
                invocation: tool_invocation(),
                source: io::Error::other("termination failure"),
            },
            ToolError::ToolChildInspectionFailed {
                invocation: tool_invocation(),
                source: io::Error::other("child inspection failure"),
            },
            ToolError::ToolContainmentInspectionFailed {
                invocation: tool_invocation(),
                source: io::Error::other("containment inspection failure"),
            },
            ToolError::ToolContainmentSignalFailed {
                invocation: tool_invocation(),
                pid: 7,
                source: io::Error::other("containment signal failure"),
            },
            ToolError::ToolChildReapFailed {
                invocation: tool_invocation(),
                source: io::Error::other("child reap failure"),
            },
            ToolError::ToolTerminationTimedOut {
                invocation: tool_invocation(),
                timeout_ms: 1,
            },
            ToolError::ToolReaperStartFailed {
                invocation: tool_invocation(),
                source: io::Error::other("child reaper failure"),
            },
            ToolError::ToolCaptureReaperStartFailed {
                invocation: tool_invocation(),
                source: io::Error::other("capture reaper failure"),
            },
            ToolError::ToolCleanupFailed {
                primary: Box::new(ToolError::ToolTimedOut {
                    invocation: tool_invocation(),
                    timeout_ms: 1,
                }),
                cleanup: Box::new(ToolError::ToolTerminationTimedOut {
                    invocation: tool_invocation(),
                    timeout_ms: 1,
                }),
            },
        ] {
            let expected = expected_tool_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        let error = AudioError::SynthesizerReportMismatch {
            segment_id: "seg-1".to_owned(),
            reported_sample_rate: 24_000,
            reported_channels: 1,
            reported_frames: 1,
            written_sample_rate: 24_000,
            written_channels: 1,
            written_frames: 2,
        };
        let expected = expected_audio_remedy(&error);
        assert_expected_remedy(error.into(), expected);
        for error in [
            ManagedPathError::InvalidManagedName {
                name: "../escape".to_owned(),
                root: PathBuf::from("workspace"),
            },
            ManagedPathError::ManagedPathEscape {
                path: PathBuf::from("outside"),
                root: PathBuf::from("workspace"),
            },
        ] {
            let expected = expected_managed_path_remedy(&error);
            assert_expected_remedy(error.into(), expected);
        }

        let live_lock = DurableStateError::LiveJobLock {
            path: PathBuf::from("build.lock"),
            pid: 7,
            process_start: 11,
        };
        assert_durable_state_remedy(live_lock, None);

        let cache_lock = DurableStateError::CacheLockTimeout {
            path: PathBuf::from("cache.lock"),
            cache_key: "a"
                .repeat(study_tts_core::CacheKey::LENGTH)
                .parse()
                .expect("valid cache key"),
            timeout_ms: 1,
        };
        assert_durable_state_remedy(
            cache_lock,
            Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve attempts and inspect the cache-key owner before retrying",
                None,
            )),
        );

        let quarantine = DurableStateError::QuarantineFailed {
            staging_path: PathBuf::from("staging"),
            primary: Box::new(BuildError::from(SynthesisError::new("worker failed"))),
            cleanup: Box::new(BuildError::from(IoError::FileSystem {
                path: PathBuf::from("quarantine"),
                source: io::Error::other("quarantine failure"),
            })),
        };
        assert_durable_state_remedy(
            quarantine,
            Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the staging attempt and repair quarantine before retrying",
                None,
            )),
        );

        for shared in [
            DurableStateError::PublicationConflict {
                path: PathBuf::from("package"),
            },
            DurableStateError::MissingCurrentPreview {
                path: PathBuf::from("current.json"),
            },
        ] {
            assert_durable_state_remedy(
                shared,
                Some(RemedyAdvice::new(
                    RemedyOwner::Runtime,
                    concat!(
                        "preserve the artifacts and run runtime reconciliation without overwrite ",
                        "or deletion",
                    ),
                    Some("State or checksum corruption"),
                )),
            );
        }
    }

    #[test]
    fn t1_e0_unrouted_failures_have_no_structured_remedy() {
        let cases = [
            BuildError::from(IoError::ReadFile {
                path: PathBuf::from("lesson.json"),
                source: io::Error::other("read failure"),
            }),
            BuildError::from(IoError::LessonNotRegularFile {
                path: PathBuf::from("lesson.json"),
            }),
            BuildError::from(LessonError::MissingLessonId),
            BuildError::from(VoiceError::UnsupportedSchema("future".to_owned())),
            BuildError::from(PublicationError::UnsupportedProductionManifest {
                version: "future".to_owned(),
            }),
            BuildError::from(AudioError::PauseFrameOverflow {
                segment_id: "seg-1".to_owned(),
                pause_after_ms: u32::MAX,
            }),
            BuildError::from(AudioError::PlannedLengthOverflow),
            BuildError::from(AudioError::AssembledLengthOverflow {
                destination: PathBuf::from("master.wav"),
            }),
            BuildError::from(ToolError::MissingTool {
                tool: "FFmpeg".to_owned(),
                requested: PathBuf::from("ffmpeg"),
            }),
            BuildError::from(ToolError::ToolTimedOut {
                invocation: tool_invocation(),
                timeout_ms: 1,
            }),
            BuildError::from(ToolError::ToolOutputOverflow {
                invocation: tool_invocation(),
                stream: ToolOutputStream::Stderr,
                limit_bytes: 1,
            }),
            BuildError::from(ToolError::ToolCaptureReadFailed {
                invocation: tool_invocation(),
                stream: ToolOutputStream::Stderr,
                source: io::Error::other("capture failure"),
            }),
            BuildError::from(ManagedPathError::UnrootedDestination {
                path: PathBuf::new(),
            }),
            BuildError::from(SynthesisError::new("worker failed")),
        ];

        for error in cases {
            assert!(error.remedy().is_none(), "`{error}` must remain unrouted");
        }
    }

    #[test]
    fn t1_e0_path_enrichment_helpers_preserve_the_supplied_path() {
        let error = io_error("workspace/file", io::Error::other("failure"));
        assert!(matches!(
            error,
            BuildError::Io(IoError::FileSystem { ref path, .. })
                if path == Path::new("workspace/file")
        ));

        let error = audio_error("workspace/audio.wav", hound::Error::FormatError("bad WAV"));
        assert!(matches!(
            error,
            BuildError::Io(IoError::AudioAt { ref path, .. })
                if path == Path::new("workspace/audio.wav")
        ));
    }

    #[test]
    fn t1_e0_cache_and_fresh_audio_retain_distinct_remedies() {
        let cache = BuildError::from(CacheError::UnusableCacheEntry {
            entry_dir: PathBuf::from("cache-entry"),
            segment_id: "seg-1".to_owned(),
            fault: Box::new(CacheEntryFault::Audio(AudioFault::Empty)),
        });
        let fresh = BuildError::from(AudioError::UnusableAudio {
            path: PathBuf::from("staged.wav"),
            fault: AudioFault::Empty,
        });

        assert!(
            cache
                .to_string()
                .contains("preserve it for runtime reconciliation")
        );
        assert_eq!(
            cache.remedy().map(RemedyAdvice::action),
            Some("preserve the unusable cache entry and run runtime reconciliation")
        );
        assert!(!fresh.to_string().contains("delete"));
        assert_eq!(
            fresh.remedy().map(RemedyAdvice::action),
            Some("quarantine the attempt and retry within the bounded budget")
        );
    }

    #[test]
    fn t1_e0_defensive_overflow_variants_remain_constructible_and_matchable() {
        let pause = AudioError::PauseFrameOverflow {
            segment_id: "seg-1".to_owned(),
            pause_after_ms: u32::MAX,
        };
        let planned = AudioError::PlannedLengthOverflow;
        let assembled = AudioError::AssembledLengthOverflow {
            destination: PathBuf::from("master.wav"),
        };
        let frames = AudioFault::FrameCountOverflow;

        assert!(matches!(pause, AudioError::PauseFrameOverflow { .. }));
        assert!(matches!(planned, AudioError::PlannedLengthOverflow));
        assert!(matches!(
            assembled,
            AudioError::AssembledLengthOverflow { .. }
        ));
        assert!(matches!(frames, AudioFault::FrameCountOverflow));
    }

    #[test]
    fn t1_e0_build_error_does_not_grow_during_category_refactor() {
        assert!(
            size_of::<BuildError>() <= PRE_REFACTOR_BUILD_ERROR_SIZE_BYTES,
            "BuildError grew beyond its {}-byte pre-refactor baseline",
            PRE_REFACTOR_BUILD_ERROR_SIZE_BYTES,
        );
    }
}
