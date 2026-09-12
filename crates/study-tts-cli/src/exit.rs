//! Exit-code classes, and what each refusal is an instance of.
//!
//! ADR-0001 §7.3 fixes the vocabulary and stops there: "Exit-code classes
//! distinguish invalid input, missing dependency, incompatible environment,
//! worker failure, audio-quality failure, cancellation, and internal error."
//! Seven classes, no numbers. The numbers are this crate's, because an exit
//! code is a CLI surface rather than a durable domain decision — the boundary
//! `crates/AGENTS.md` draws between `study-tts-cli` and `study-tts-core`.
//!
//! Six of the seven are declared; §7.3's cancellation class waits for a
//! refusal that can produce it, and the enum records why at the gap.
//!
//! [`ExitClass::of_class`] is the whole class-level mapping and is an
//! exhaustive `match`, so a new [`BuildErrorClass`] is a compile error here
//! rather than a refusal that silently inherits somebody else's number.
//! [`ExitClass::of`] refines the one class that is too coarse to act on —
//! [`BuildErrorClass::Io`] covers both a path the operator mistyped and a
//! filesystem that failed, and those are different things to do next.

use std::process::ExitCode;

use study_tts_runtime::{BuildError, BuildErrorClass, IoError, RemedyOwner};

/// What kind of failure a refusal was, in ADR-0001 §7.3's vocabulary.
///
/// The discriminants are the ADR's listing order, starting at `2`. `0` is
/// success and `1` is left alone: a shell that runs `study-tts` under `set -e`,
/// or a harness that treats any nonzero as failure, already reads `1` as "it
/// failed" without claiming which way, and so does every wrapper that never
/// learned this table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExitClass {
    /// The operator gave something this build cannot accept.
    InvalidInput = 2,
    /// Something this build needs was not installed.
    MissingDependency = 3,
    /// The environment cannot host this build as configured.
    IncompatibleEnvironment = 4,
    /// The synthesis worker failed, was refused, or could not be supervised.
    WorkerFailure = 5,
    /// Audio failed a structural or quality invariant.
    AudioQualityFailure = 6,
    // `7` is ADR-0001 §7.3's cancellation class and is deliberately absent:
    // nothing in this build cancels yet, and a variant no refusal can produce
    // would be a number this crate publishes and never uses. The discriminants
    // are explicit, so declaring it at `7` when a cancellation path lands
    // renumbers nothing.
    /// A defect in this build.
    InternalError = 8,
}

impl ExitClass {
    /// Classifies one refusal, preferring what the repository already says
    /// about who acts on it.
    ///
    /// `BuildError::class` answers "which boundary refused", which is not the
    /// same question an exit code asks. `BuildError::remedy` answers "who acts
    /// on this", which is, and `docs/governance/ROUTING-TABLES.md` §Failure
    /// routing is its other side — mirrored exhaustively and pinned by
    /// `t1_e0_governed_remedy_mappings_are_exhaustive`.
    ///
    /// Deriving from the remedy rather than writing a second table is what
    /// keeps the two from disagreeing. A hand-written arm per error variant
    /// would be 38 for `ToolError` alone and 64 for `DurableStateError`, and
    /// the first version of this got `ToolError` wrong for exactly that
    /// reason: a real render refused at the loudness gate and exited
    /// `MissingDependency`, because one class carries both "FFmpeg is not
    /// installed" and "FFmpeg produced audio that failed an invariant".
    ///
    /// A refusal with no governed remedy falls back to its class. That is the
    /// right default rather than a gap: `ROUTING-TABLES.md` establishes no
    /// owner for those, and `MissingTool` says so in as many words — "its own
    /// message already names the remedy".
    #[must_use]
    pub fn of(error: &BuildError) -> Self {
        if let Some(remedy) = error.remedy() {
            return Self::of_owner(remedy.owner());
        }
        match error {
            BuildError::Io(io) => Self::of_io(io),
            other => Self::of_class(other.class()),
        }
    }

    /// Maps a remedy owner onto the class ADR-0001 §7.3 names for their work.
    ///
    /// Exhaustive, so a seventh owner is a compile error here rather than one
    /// that inherits another's exit code.
    const fn of_owner(owner: RemedyOwner) -> Self {
        match owner {
            // Publication, rights, and consent are the operator's decisions,
            // and a refusal about one is theirs to answer.
            RemedyOwner::ProjectOwner => Self::InvalidInput,
            // Durable state and reconciliation: the governed root cannot host
            // the work as it stands.
            RemedyOwner::Runtime => Self::IncompatibleEnvironment,
            // Both audio owners produce ADR-0001 §7.3's audio-quality class,
            // and they are distinct for a reason that does not change the exit
            // code: an audio-runtime refusal is corrected by changing a
            // setting, a review finding by recording and deciding. Either way
            // what failed was the audio.
            RemedyOwner::AudioRuntime | RemedyOwner::HumanReview => Self::AudioQualityFailure,
            RemedyOwner::WorkerRuntime => Self::WorkerFailure,
            // Corrective release-gate work: the build is sound and the claim
            // is not, which is the shape `publish` already refuses with.
            RemedyOwner::GateOwner => Self::InvalidInput,
        }
    }

    /// Splits an I/O refusal by who can act on it.
    ///
    /// Exhaustive, so a new [`IoError`] variant is a compile error here rather
    /// than one that inherits whichever half it was not written for.
    const fn of_io(error: &IoError) -> Self {
        match error {
            // The operator named this path. They can name another.
            // `DestinationExists` says so in its own doc: "the person who chose
            // the path is the only one who can choose another".
            IoError::ReadFile { .. }
            | IoError::LessonNotRegularFile { .. }
            | IoError::DestinationExists { .. } => Self::InvalidInput,
            // The filesystem refused work this build had every right to do.
            IoError::FileSystem { .. } | IoError::AudioAt { .. } | IoError::WriteJson { .. } => {
                Self::IncompatibleEnvironment
            }
        }
    }

    /// Classifies one refusal by class alone.
    ///
    /// Read against ADR-0001 §7.3's seven names rather than against the
    /// variants it maps from: the question each arm answers is "what should an
    /// operator do about this", and that is what an exit code is for.
    #[must_use]
    pub const fn of_class(class: BuildErrorClass) -> Self {
        match class {
            // The operator authored or selected something this build refuses.
            BuildErrorClass::Lesson
            | BuildErrorClass::Takes
            | BuildErrorClass::Voice
            | BuildErrorClass::VoiceProfile
            | BuildErrorClass::Rights
            | BuildErrorClass::Publication => Self::InvalidInput,
            // A tool, a model artifact, or a worker bundle was absent or did
            // not match what this build pinned. Installing or restoring it is
            // the remedy, which is what separates this from a bad environment.
            BuildErrorClass::Tool
            | BuildErrorClass::ModelArtifacts
            | BuildErrorClass::WorkerBundle => Self::MissingDependency,
            // The governed layout cannot host the work: containment refused a
            // path, or durable state is unusable. `Io` reaches here only
            // through `of_class`; `of` splits it first, because half of it is
            // the operator's to fix.
            //
            // `DurableState` needs the same split and does not have it yet.
            // `ApprovedPackageMissing` is the clearest case: its own message
            // says "approve the generation that exists", which is the operator
            // acting, and this reports their environment as incompatible. The
            // fix is not a table — `DurableStateError` has 64 variants and a
            // hand-written one would mis-class more than it helped. It is to
            // derive the class from `BuildError::remedy`'s `RemedyOwner`,
            // which already answers "who can act on this" and is pinned
            // exhaustively by `t1_e0_governed_remedy_mappings_are_exhaustive`.
            // E2-S5 task 3's recovery-command work is where that lands.
            BuildErrorClass::Io | BuildErrorClass::ManagedPath | BuildErrorClass::DurableState => {
                Self::IncompatibleEnvironment
            }
            // The worker failed, or a cache entry it produced did not validate.
            BuildErrorClass::Synthesis | BuildErrorClass::Cache => Self::WorkerFailure,
            // Audio failed an invariant it is supposed to hold.
            BuildErrorClass::Audio => Self::AudioQualityFailure,
            // Planning cannot fail on operator input this build already
            // accepted, so a plan refusal is a defect in this build.
            BuildErrorClass::Plan => Self::InternalError,
        }
    }

    /// The process status this class leaves behind.
    #[must_use]
    pub fn code(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::{BuildError, BuildErrorClass, ExitClass, IoError};

    /// Every class an operator can provoke maps to the class ADR-0001 §7.3
    /// would have a reader act on.
    ///
    /// A table rather than a re-derivation: the expected column is what a
    /// reviewer reads against the ADR's seven names, and a `match` copied from
    /// `ExitClass::of` would agree with any mapping, including a wrong one.
    #[test]
    fn t1_e2_every_refusal_class_maps_to_an_adr_exit_class() {
        const CASES: [(BuildErrorClass, ExitClass); 16] = [
            (BuildErrorClass::Lesson, ExitClass::InvalidInput),
            (BuildErrorClass::Takes, ExitClass::InvalidInput),
            (BuildErrorClass::Voice, ExitClass::InvalidInput),
            (BuildErrorClass::VoiceProfile, ExitClass::InvalidInput),
            (BuildErrorClass::Rights, ExitClass::InvalidInput),
            (BuildErrorClass::Publication, ExitClass::InvalidInput),
            (BuildErrorClass::Tool, ExitClass::MissingDependency),
            (
                BuildErrorClass::ModelArtifacts,
                ExitClass::MissingDependency,
            ),
            (BuildErrorClass::WorkerBundle, ExitClass::MissingDependency),
            (BuildErrorClass::Io, ExitClass::IncompatibleEnvironment),
            (
                BuildErrorClass::ManagedPath,
                ExitClass::IncompatibleEnvironment,
            ),
            (
                BuildErrorClass::DurableState,
                ExitClass::IncompatibleEnvironment,
            ),
            (BuildErrorClass::Synthesis, ExitClass::WorkerFailure),
            (BuildErrorClass::Cache, ExitClass::WorkerFailure),
            (BuildErrorClass::Audio, ExitClass::AudioQualityFailure),
            (BuildErrorClass::Plan, ExitClass::InternalError),
        ];

        for (class, expected) in CASES {
            assert_eq!(ExitClass::of_class(class), expected, "{class:?}");
        }
    }

    /// Which half of an I/O refusal the operator can act on.
    ///
    /// The table is the judgment, stated so a reviewer can disagree with a row
    /// rather than having to reconstruct it from a `match`. The question each
    /// row answers: can the person who typed the command fix this by typing a
    /// different one?
    #[test]
    fn t1_e2_an_io_refusal_splits_by_who_can_act_on_it() {
        use std::path::PathBuf;

        let operators: [IoError; 3] = [
            IoError::ReadFile {
                path: PathBuf::from("absent.json"),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            },
            IoError::LessonNotRegularFile {
                path: PathBuf::from("a-directory"),
            },
            IoError::DestinationExists {
                path: PathBuf::from("taken.json"),
            },
        ];

        for error in operators {
            assert_eq!(
                ExitClass::of(&BuildError::Io(error)),
                ExitClass::InvalidInput,
                "a path the operator chose is theirs to choose again"
            );
        }

        let environment = IoError::FileSystem {
            path: PathBuf::from("workspace"),
            source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        };
        assert_eq!(
            ExitClass::of(&BuildError::Io(environment)),
            ExitClass::IncompatibleEnvironment,
            "a filesystem that refused work this build may do is not an input error"
        );
    }

    /// A refusal routes by who acts on it, not by which module raised it.
    ///
    /// The case that produced this test: a real `study-tts render` refused at
    /// E2-S3's loudness gate and exited `MissingDependency`, because
    /// `LoudnessNotLinear` is a `ToolError` and the class-level map sent every
    /// tool refusal to "something is not installed". ADR-0001 §7.3 has an
    /// audio-quality class and that is what a master whose gain moves under
    /// the audio belongs to.
    ///
    /// The wrong implementation this rejects is the one that shipped: classify
    /// by `BuildError::class` alone. It is wrong for every `ToolError` that is
    /// a judgment about output rather than about a binary.
    #[test]
    fn t1_e2_a_refusal_routes_by_who_acts_on_it_not_by_which_module_raised_it() {
        let loudness = BuildError::from(study_tts_runtime::ToolError::LoudnessNotLinear {
            normalization_type: "dynamic".to_owned(),
        });

        assert_eq!(
            loudness.class(),
            BuildErrorClass::Tool,
            "the class still says which boundary refused"
        );
        assert_eq!(
            ExitClass::of(&loudness),
            ExitClass::AudioQualityFailure,
            "but the exit code says what failed: the audio, not the toolchain"
        );
        assert_ne!(
            ExitClass::of(&loudness),
            ExitClass::of_class(BuildErrorClass::Tool),
            "which is the whole point of preferring the remedy over the class"
        );
    }

    /// No two classes share a number, and none collides with success or with
    /// the generic failure a wrapper may already produce.
    #[test]
    fn t1_e2_exit_codes_are_distinct_and_avoid_zero_and_one() {
        const CLASSES: [ExitClass; 6] = [
            ExitClass::InvalidInput,
            ExitClass::MissingDependency,
            ExitClass::IncompatibleEnvironment,
            ExitClass::WorkerFailure,
            ExitClass::AudioQualityFailure,
            ExitClass::InternalError,
        ];

        let mut seen = Vec::new();
        for class in CLASSES {
            let code = class as u8;
            assert!(
                code > 1,
                "{class:?} claims {code}, which already means something"
            );
            assert!(!seen.contains(&code), "{class:?} reuses exit code {code}");
            seen.push(code);
        }
    }
}
