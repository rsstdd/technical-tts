//! Refusals raised while deriving the identity of the executable worker bundle.

use std::{fmt, path::PathBuf};

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};
use crate::worker_bundle::PythonRuntimeIdentity;

/// Why a worker bundle could not be identified.
///
/// ADR-0001 §12.5 derives the bundle hash "mechanically" from declared inputs,
/// so a declared input that is absent or oversized is a refusal rather than
/// something to hash around: skipping it would produce a hash that matches a
/// different bundle, and every cache entry named by it would be wrong.
#[derive(Debug, Error)]
pub enum WorkerBundleError {
    /// A declared bundle input is not present beneath the bundle root.
    #[error(
        "declared worker bundle input `{path}` is missing beneath `{root}`; ADR-0001 §12.5 \
         derives the bundle hash from every declared input, so the worker/runtime owner must \
         restore the file or amend the declared input list rather than hash a partial bundle"
    )]
    MissingDeclaredInput {
        /// The declared input, relative to the bundle root.
        path: PathBuf,
        /// The bundle root the input was resolved beneath.
        root: PathBuf,
    },

    /// A Python module lives beneath a declared import root that the manifest
    /// does not declare.
    ///
    /// The refusal that makes the declared list a checked fact rather than a
    /// claim. Hashing anyway would produce a bundle identity that ignores a
    /// file the worker can load, so a behavior change would leave every cache
    /// key untouched and every reuse silently wrong.
    #[error(
        "worker bundle module `{module}` lies beneath the declared import root `{import_root}` \
         but `{manifest}` does not declare it; ADR-0001 §12.5 derives the bundle hash from the \
         worker source and its project-owned modules, so the worker/runtime owner must declare \
         the module or remove it rather than leave it outside an identity that would then \
         describe a different bundle"
    )]
    UndeclaredModule {
        /// The module, relative to the bundle root.
        module: PathBuf,
        /// The declared import root it lies beneath.
        import_root: PathBuf,
        /// The manifest that should have declared it.
        manifest: PathBuf,
    },

    /// The manifest omits an input this build requires every bundle to
    /// declare.
    ///
    /// Distinct from a missing file: the file may well be on disk. What is
    /// absent is the declaration, and an undeclared input is one whose bytes
    /// never reach the identity — so the bundle hash stops moving when that
    /// input changes, which is silent cache poisoning rather than a failure.
    #[error(
        "worker bundle manifest `{manifest}` does not declare `{path}`; ADR-0001 §12.5 names it \
         among the bundle inputs, so the worker/runtime owner must declare it rather than derive \
         an identity that stops moving when it changes"
    )]
    UndeclaredRequiredInput {
        /// The required input, relative to the bundle root.
        path: PathBuf,
        /// The manifest that should have declared it.
        manifest: PathBuf,
    },

    /// The manifest omits the import root whose contents this build checks.
    ///
    /// The completeness walk is scoped by `import_roots`, so an empty or
    /// narrowed list is a walk with nothing to find: every module in the
    /// package passes as declared without being declared.
    #[error(
        "worker bundle manifest `{manifest}` does not declare the import root `{import_root}`; it \
         is what scopes the completeness check, so the worker/runtime owner must declare it rather \
         than derive an identity that accepts any module the package holds"
    )]
    UndeclaredImportRoot {
        /// The required import root, relative to the bundle root.
        import_root: PathBuf,
        /// The manifest that should have declared it.
        manifest: PathBuf,
    },

    /// The bundle manifest is not readable as the record this build expects.
    #[error(
        "worker bundle manifest `{path}` could not be read ({source}); the worker/runtime owner \
         must correct the manifest, because a bundle whose declaration cannot be parsed has no \
         identity this build may derive"
    )]
    UnreadableBundleManifest {
        /// The manifest that could not be parsed.
        path: PathBuf,
        /// What the parser reported.
        #[source]
        source: serde_json::Error,
    },

    /// The bundle manifest declares a layout this build does not implement.
    #[error(
        "worker bundle manifest `{path}` declares layout `{declared}` but this build implements \
         `{required}`; the worker/runtime owner must align the manifest and the build rather than \
         hash a declaration this build can only partly read"
    )]
    UnsupportedBundleManifest {
        /// The manifest carrying the unknown version.
        path: PathBuf,
        /// Layout the manifest declares.
        declared: String,
        /// Layout this build implements.
        required: &'static str,
    },

    /// The interpreter that would run the worker is not the one the manifest
    /// declares.
    ///
    /// ADR-0001 §12.5 counts "Python runtime and platform ABI identity" among
    /// the bundle inputs, and until this check the manifest was the only
    /// witness to it. A bundle carried to another interpreter patch version or
    /// platform kept its identity while loading different compiled wheels, so
    /// two different renders shared one cache key.
    #[error(
        "worker bundle {mismatch}; ADR-0001 §12.5 counts the Python runtime and platform ABI \
         among the bundle inputs, so the worker/runtime owner must restore the declared \
         environment or re-lock the bundle for this interpreter rather than record an identity \
         the running interpreter does not have"
    )]
    RuntimeIdentityMismatch {
        /// What the manifest declares and what the interpreter reports.
        mismatch: Box<RuntimeIdentityMismatch>,
    },

    /// The configured interpreter answered with something this build cannot
    /// read as a runtime identity.
    ///
    /// Separate from a mismatch because the remedies differ: a mismatch means
    /// the wrong environment is installed, while this means the probe did not
    /// run — a missing `packaging`, a truncated answer, or an interpreter that
    /// failed before printing one.
    #[error(
        "worker bundle interpreter `{interpreter}` did not report a runtime identity this build \
         can read ({detail}); the worker/runtime owner must restore the locked environment per \
         docs/operations/WORKER-ENVIRONMENT.md, because a bundle whose runtime cannot be observed \
         has no identity this build may derive"
    )]
    UnreadableRuntimeIdentity {
        /// The interpreter that was probed.
        interpreter: PathBuf,
        /// What the probe reported, redacted to a single line.
        detail: String,
    },

    /// The environment beside the interpreter is not the one
    /// `worker/requirements.lock` describes.
    ///
    /// The lockfile reaches the bundle identity as bytes, so until this check
    /// the hash proved what that file *says* and nothing about what is
    /// installed. A distribution upgraded in place, or a governed one the
    /// configured index satisfied at the same version, left every declared
    /// input byte-identical and every cache key where it was while the audio
    /// changed.
    #[error(
        "worker bundle environment {mismatch}; the worker/runtime owner must restore the locked \
         environment per docs/operations/WORKER-ENVIRONMENT.md rather than derive an identity \
         from a lockfile the installed environment does not match"
    )]
    EnvironmentDoesNotMatchLock {
        /// Which distribution disagrees, and how.
        mismatch: Box<EnvironmentMismatch>,
    },

    /// A `worker/requirements.lock` invariant is malformed or absent.
    ///
    /// The locus and not the line: a wrongly regenerated lock is exactly where
    /// a `chatterbox-tts @ file:///...` line appears, and
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a governed model
    /// root out of logs.
    #[error(
        "worker lockfile `{path}` {locus} {reason}; ADR-0001 §12.5 makes this \
         file a synthesis-key input, so the worker/runtime owner must regenerate it per \
         docs/operations/WORKER-ENVIRONMENT.md rather than derive an identity from a resolved set \
         this build cannot read"
    )]
    UnreadableWorkerLockfile {
        /// The lockfile that could not be read.
        path: PathBuf,
        /// Where in the lockfile the invariant failed.
        locus: WorkerLockfileLocus,
        /// Exact lockfile invariant that failed.
        reason: WorkerLockfileErrorReason,
    },

    /// A declared bundle input exceeds the size this boundary will read.
    #[error(
        "declared worker bundle input `{path}` exceeds the {max_bytes}-byte limit; the \
         worker/runtime owner must correct the declared input list rather than raise the limit \
         to admit an artifact the bundle should not contain"
    )]
    DeclaredInputTooLarge {
        /// The declared input, relative to the bundle root.
        path: PathBuf,
        /// The ceiling this boundary enforces.
        max_bytes: usize,
    },
}

/// Where in `worker/requirements.lock` an invariant failed.
///
/// A locus rather than a line number, because three of the invariants are not
/// a line's to break: the bytes are not UTF-8, a required directive is absent,
/// and the governed pin is missing. A line number said them anyway — `0` for
/// the first and one past the last line for the other two — so a 42-line lock
/// reported "line 43 omits a required resolution directive" and sent the
/// operator to a line that is not there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerLockfileLocus {
    /// The 1-based line carrying the fault.
    Line(usize),
    /// No one line: the invariant is the whole file's to keep.
    WholeFile,
}

impl fmt::Display for WorkerLockfileLocus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Line(line) => write!(formatter, "line {line}"),
            Self::WholeFile => formatter.write_str("as a whole"),
        }
    }
}

/// Why `worker/requirements.lock` cannot describe one reproducible environment.
///
/// `docs/operations/WORKER-ENVIRONMENT.md` §Their startup hooks are not ignored
/// states the lock rules these variants refuse, and names this enum in return.
/// One variant per rule, so a test asserts which invariant failed rather than
/// that something did.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WorkerLockfileErrorReason {
    /// The lockfile bytes are not UTF-8.
    #[error("is not UTF-8")]
    InvalidUtf8,
    /// A package line is not one exact name-and-version pin.
    #[error("is not an exact `name==version` pin")]
    MalformedPin,
    /// An index pin does not bind exactly one artifact digest.
    #[error("does not carry exactly one lowercase SHA-256 artifact hash")]
    InvalidArtifactHash,
    /// One required installer directive is absent.
    #[error("omits a required resolution directive")]
    MissingRequiredDirective,
    /// One required installer directive appears more than once.
    #[error("repeats a required resolution directive")]
    DuplicateRequiredDirective,
    /// A directive outside the governed set is present.
    #[error("contains an unsupported resolution directive")]
    UnsupportedDirective,
    /// The governed pin and its source revision are not adjacent and unique.
    #[error("does not keep one governed-source provenance marker beside its governed pin")]
    InvalidProvenance,
}

/// How one distribution disagrees with `worker/requirements.lock`.
///
/// One variant per operator action, because each of these is a different
/// mistake with a different fix: reinstall the missing pin, undo an in-place
/// upgrade, redo the install that the index silently satisfied, or point the
/// governed install at the revision the lock records.
///
/// **No variant prints the recorded URL**, because the probe never reports one.
/// PEP 610 records where a local install came from, and for `chatterbox-tts`
/// that is the governed model root
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps out of Git, CI,
/// fixtures, and logs. Naming the commit to reinstall from tells the operator
/// everything the URL would and carries none of the path.
///
/// Boxed by its one variant in [`WorkerBundleError`] for the reason
/// [`RuntimeIdentityMismatch`] is: `crate::BuildError` is measured at 80 bytes
/// in `docs/architecture/WALKING-SKELETON.md`.
#[derive(Debug, Error)]
pub enum EnvironmentMismatch {
    /// A locked distribution is not installed at all.
    #[error("pins `{distribution}` at `{required}` but it is not installed")]
    Absent {
        /// Canonicalized distribution name.
        distribution: String,
        /// Version the lock pins.
        required: String,
    },

    /// A locked distribution is installed at another version.
    #[error("pins `{distribution}` at `{required}` but `{installed}` is installed")]
    Version {
        /// Canonicalized distribution name.
        distribution: String,
        /// Version the lock pins.
        required: String,
        /// Version the environment reports.
        installed: String,
    },

    /// A governed distribution carries no PEP 610 record, so an index supplied
    /// it.
    ///
    /// The failure `docs/operations/WORKER-ENVIRONMENT.md` warns about by name:
    /// installing the lockfile whole lets the index satisfy the pin, and the
    /// governed install that follows then finds the requirement already
    /// satisfied at the same version and does nothing.
    #[error(
        "requires `{distribution}` from the governed source tree at commit `{commit}`, but it \
         carries no PEP 610 record and so came from an index"
    )]
    FromIndex {
        /// Canonicalized distribution name.
        distribution: String,
        /// Commit the lock records beside the pin.
        commit: String,
    },

    /// A governed distribution was installed from a path, which records no
    /// revision.
    ///
    /// The failure the previous check could not see. PEP 610 writes `dir_info`
    /// for a directory install and no `commit_id` at all, so the only evidence
    /// was the directory's *name* — and `code-<commit>-backup` beside the real
    /// tree is a name an operator really creates, while the tree at
    /// `code-<commit>` can hold any bytes at all. The remedy is a different
    /// command rather than a different directory, which is why this is not
    /// [`EnvironmentMismatch::FromAnotherRevision`].
    #[error(
        "requires `{distribution}` from the governed source tree at commit `{commit}`, but it was \
         installed from a path, which records no revision"
    )]
    WithoutRecordedRevision {
        /// Canonicalized distribution name.
        distribution: String,
        /// Commit the lock records beside the pin.
        commit: String,
    },

    /// A governed distribution records a revision other than the locked one.
    #[error(
        "requires `{distribution}` from the governed source tree at commit `{commit}`, but it was \
         installed from a different revision of that tree"
    )]
    FromAnotherRevision {
        /// Canonicalized distribution name.
        distribution: String,
        /// Commit the lock records beside the pin.
        commit: String,
    },

    /// A `.pth` file no installed distribution claims.
    ///
    /// A `.pth` runs at interpreter startup — every line beginning `import`
    /// executes, and every other line joins `sys.path` ahead of the search that
    /// resolves the locked distributions — so it is behavior inside the process
    /// the bundle identity describes. One that no `RECORD` lists was dropped in
    /// by hand or left behind by an uninstall, and either way nothing accounts
    /// for it. An ambiguous hook, claimed by two distributions, is reported
    /// here too: it cannot be attributed, so it is not accounted for either.
    #[error("carries the startup hook `{file}`, which no installed distribution claims")]
    UnownedPathHook {
        /// File name within the site directory.
        file: String,
    },

    /// A `.pth` file owned by a distribution the lock does not pin.
    ///
    /// Extra distributions are tolerated because the qualification virtualenv
    /// shares this repository's pre-commit tooling. That tolerance holds only
    /// while an extra install stays inert, and a startup hook is not inert.
    #[error("carries the startup hook `{file}` from `{owner}`, which the lockfile does not pin")]
    UnlockedPathHook {
        /// File name within the site directory.
        file: String,
        /// Canonicalized name of the distribution whose `RECORD` lists it.
        owner: String,
    },

    /// Two installs share one canonicalized name, so which is loaded is
    /// unknowable.
    ///
    /// PEP 503 canonicalization is many-to-one — `zope.interface` and
    /// `zope-interface` are one name and two distributions — so a comparison
    /// keyed by it would answer for whichever the probe walked last and report
    /// the other as absent.
    #[error("reports `{distribution}` more than once, so which install is loaded is unknowable")]
    AmbiguousDistribution {
        /// The canonicalized name two installs share.
        distribution: String,
    },
}

/// The declared and observed runtime identities at the field they disagree on.
///
/// Boxed by its one variant because [`crate::BuildError`] is measured at 80
/// bytes in `docs/architecture/WALKING-SKELETON.md`, and two runtime identities
/// inline would be most of that on every successful result.
#[derive(Debug, Error)]
#[error(
    "manifest `{manifest}` declares Python {declared} but interpreter `{interpreter}` reports \
     {observed}"
)]
pub struct RuntimeIdentityMismatch {
    /// The manifest carrying the declaration.
    pub manifest: PathBuf,
    /// The interpreter that was probed.
    pub interpreter: PathBuf,
    /// Runtime identity the manifest declares.
    pub declared: PythonRuntimeIdentity,
    /// Runtime identity the interpreter reports.
    pub observed: PythonRuntimeIdentity,
}

impl WorkerBundleError {
    /// Returns governed recovery advice for the worker/runtime owner.
    ///
    /// `docs/governance/ROUTING-TABLES.md` §Failure routing routes worker
    /// boundary failures to worker/runtime, which owns the bundle contents, the
    /// declared input list, and the environment the bundle is identified
    /// against. One owner and one routing row, four actions: restoring a
    /// deleted input, restoring an environment that drifted from the lock,
    /// regenerating the lock, and aligning the manifest with this build are
    /// different repairs, and an operator acts on the one they are handed.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        let action = match self {
            Self::MissingDeclaredInput { .. }
            | Self::DeclaredInputTooLarge { .. }
            | Self::UndeclaredModule { .. }
            | Self::UndeclaredRequiredInput { .. }
            | Self::UndeclaredImportRoot { .. }
            | Self::UnreadableBundleManifest { .. } => {
                "restore the declared worker bundle input or amend the bundle manifest"
            }
            Self::UnsupportedBundleManifest { .. } => {
                "align the bundle manifest layout with the one this build implements"
            }
            Self::RuntimeIdentityMismatch { .. }
            | Self::UnreadableRuntimeIdentity { .. }
            | Self::EnvironmentDoesNotMatchLock { .. } => {
                "restore the locked worker environment per docs/operations/WORKER-ENVIRONMENT.md"
            }
            Self::UnreadableWorkerLockfile { .. } => {
                "regenerate worker/requirements.lock per docs/operations/WORKER-ENVIRONMENT.md"
            }
        };
        Some(RemedyAdvice::new(
            RemedyOwner::WorkerRuntime,
            action,
            Some("Worker protocol or containment failure"),
        ))
    }
}
