//! Refusals raised while deriving the identity of the executable worker bundle.

use std::{fmt, path::PathBuf};

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};
use crate::worker_bundle::{PythonRuntimeIdentity, StartupModuleName};

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

    /// The configured interpreter answered with a runtime and environment
    /// report this build cannot read.
    ///
    /// Separate from a mismatch because the remedies differ: a mismatch means
    /// the wrong environment is installed, while this means the probe did not
    /// produce a valid report — a missing `packaging`, malformed boundary data,
    /// a truncated answer, or an interpreter that failed before printing one.
    #[error(
        "worker bundle interpreter `{interpreter}` did not report runtime and environment data \
         this build can read ({detail}); the worker/runtime owner must restore the locked \
         environment per docs/operations/WORKER-ENVIRONMENT.md, because a bundle whose runtime \
         cannot be observed has no identity this build may derive"
    )]
    UnreadableRuntimeIdentity {
        /// The interpreter that was probed.
        interpreter: PathBuf,
        /// What the probe reported, sanitized to a single terminal-safe line.
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
        /// Which installed-environment invariant failed.
        mismatch: Box<EnvironmentMismatch>,
    },

    /// `worker/pyproject.toml` declares a requirement the lock does not
    /// resolve.
    ///
    /// The gap between two declared bundle inputs that nothing compared. The
    /// lock is what the environment is restored from; `worker/pyproject.toml`
    /// only *states* what should be resolved, and both reach the identity as
    /// bytes. So a dependency bot that bumped the declaration and left the
    /// lock alone moved every cache key in the project while the resolved
    /// set, the installed environment, and the audio all stayed exactly where
    /// they were.
    #[error(
        "worker requirements `{path}` {fault}; ADR-0001 §12.5 makes both this file and \
         worker/requirements.lock synthesis-key inputs, so the worker/runtime owner must \
         reconcile them per docs/operations/WORKER-ENVIRONMENT.md rather than derive an identity \
         from a declaration the lock does not resolve"
    )]
    RequirementsDisagreeWithLock {
        /// The requirements declaration that could not be reconciled.
        path: PathBuf,
        /// Exact reconciliation invariant that failed.
        fault: Box<WorkerRequirementFault>,
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

    /// `worker/launcher.json` is absent or is not the record this build reads.
    #[error(
        "worker launcher `{path}` cannot be read as the launcher this build starts workers \
         from ({detail}); ADR-0001 §12.5 makes inference-affecting launcher configuration a \
         synthesis-key input, so the worker/runtime owner must repair it rather than start a \
         worker under settings this build only partly understands"
    )]
    UnreadableLauncher {
        /// The launcher that could not be read.
        path: PathBuf,
        /// What the reader reported, without the file's contents.
        detail: String,
    },

    /// `worker/launcher.json` declares a layout this build does not implement.
    ///
    /// Checked before the shape, because a later layout may mean something
    /// different by a field this build would otherwise read under the old
    /// meaning — and reporting one of its new fields as unknown would send an
    /// operator to edit a record this build cannot read anyway.
    #[error(
        "worker launcher declares layout `{found}` but this build reads `{supported}`; align \
         the launcher and the build rather than start a worker under a layout it only partly \
         understands"
    )]
    UnsupportedLauncher {
        /// Layout the launcher declares.
        found: String,
        /// Layout this build implements.
        supported: &'static str,
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
    /// The governed pin lacks one well-formed adjacent source revision.
    #[error("does not keep one full governed-source provenance commit beside its governed pin")]
    InvalidProvenance,
}

/// How `worker/pyproject.toml` disagrees with `worker/requirements.lock`.
///
/// `docs/operations/WORKER-ENVIRONMENT.md`
/// §The declaration is reconciled with the lock
/// states the rules these variants refuse, and names this enum in
/// return. One variant per operator action, so the refusal says whether the
/// owner must write an exact pin, regenerate the lock, or decide which of two
/// versions is the intended one.
///
/// **No variant prints the requirement text**, for the reason
/// [`WorkerLockfileErrorReason`] never prints a lockfile line: a wrongly
/// written requirement is exactly where a `chatterbox-tts @ file:///...`
/// direct reference to the governed model root appears, and
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps that path out of
/// logs. The distribution name says everything the operator needs.
///
/// Boxed by its one variant in [`WorkerBundleError`] for the reason
/// [`EnvironmentMismatch`] is: `crate::BuildError` is measured at 80 bytes in
/// `docs/architecture/WALKING-SKELETON.md`.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum WorkerRequirementFault {
    /// A requirement carries a range, a direct reference, or no `==` at all.
    ///
    /// The lock resolves one version per distribution, so a declaration that
    /// admits more than one cannot be reconciled with it even when today's
    /// resolution happens to satisfy both.
    #[error("declares `{distribution}` without an exact `==` version pin")]
    NotAnExactPin {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// A pinned requirement names a distribution the lock does not pin.
    #[error("declares `{distribution}` at `{declared}`, which the lockfile does not pin")]
    NotLocked {
        /// Canonicalized distribution name.
        distribution: String,
        /// Version `worker/pyproject.toml` declares.
        declared: String,
    },

    /// A pinned requirement disagrees with the version the lock resolved.
    ///
    /// The failure that actually occurred: a dependency bot raised the
    /// declaration and could not regenerate the lock, leaving the two inputs
    /// naming different versions of one distribution.
    #[error("declares `{distribution}` at `{declared}` but the lockfile resolves `{locked}`")]
    LockedAtAnotherVersion {
        /// Canonicalized distribution name.
        distribution: String,
        /// Version `worker/pyproject.toml` declares.
        declared: String,
        /// Version `worker/requirements.lock` resolves.
        locked: String,
    },

    /// The file uses a TOML string spelling this reader does not implement.
    ///
    /// A refusal rather than a best effort. A multi-line basic string or an
    /// escaped quote desynchronizes the scan, and a desynchronized scan drops
    /// requirements out of the comparison without saying so — which is the
    /// silence this check exists to end.
    #[error("uses a multi-line or escaped string this build does not read")]
    Unreadable,
}

/// How the installed environment disagrees with `worker/requirements.lock`.
///
/// One variant per operator action, so the refusal identifies whether the
/// owner must restore a pin, provenance, installed bytes, or startup behavior.
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
    #[error("carries the startup hook {file:?}, which no installed distribution claims")]
    UnownedPathHook {
        /// File name within the site directory.
        file: String,
    },

    /// A `.pth` file owned by a distribution the lock does not pin.
    ///
    /// Extra distributions are tolerated because the qualification virtualenv
    /// shares this repository's pre-commit tooling. That tolerance holds only
    /// while an extra install stays inert, and a startup hook is not inert.
    #[error("carries the startup hook {file:?} from {owner:?}, which the lockfile does not pin")]
    UnlockedPathHook {
        /// File name within the site directory.
        file: String,
        /// Canonicalized name of the distribution whose `RECORD` lists it.
        owner: String,
    },

    /// A locked distribution's file disagrees with its installed `RECORD`.
    ///
    /// The gap the version comparison leaves open. A pin proves which release
    /// was resolved and says nothing about what the files hold now, so a module
    /// edited in place left every version, every provenance record, and every
    /// declared input byte-identical while the code the worker imports changed.
    /// `RECORD` is the distribution's own per-file digest, and this is the
    /// comparison against it. Remedy: reinstall the distribution from the
    /// lockfile per `docs/operations/WORKER-ENVIRONMENT.md` §Restore the
    /// environment; the worker/runtime owner in
    /// `docs/governance/ROUTING-TABLES.md` owns a tree that keeps diverging.
    #[error(
        "installs `{distribution}` whose file {file:?} does not match the digest the \
         distribution recorded for it"
    )]
    ModifiedDistributionFile {
        /// Canonicalized distribution name.
        distribution: String,
        /// Path as `RECORD` spells it, relative to the site directory.
        file: String,
    },

    /// A locked distribution's `RECORD` lists a file that is not there.
    ///
    /// Separate from [`EnvironmentMismatch::ModifiedDistributionFile`] because
    /// a partial uninstall and an edited module are different accidents, and an
    /// operator reading one refusal should not have to guess which happened.
    #[error(
        "installs `{distribution}` whose recorded file {file:?} is absent, so the install is \
         incomplete"
    )]
    MissingDistributionFile {
        /// Canonicalized distribution name.
        distribution: String,
        /// Path as `RECORD` spells it, relative to the site directory.
        file: String,
    },

    /// A locked distribution ships no `RECORD`, so its files state no digests.
    ///
    /// Refused rather than tolerated: a distribution with no `RECORD` is one
    /// the integrity comparison cannot make at all, and passing it would mean
    /// the check reports success for exactly the install it cannot see.
    #[error(
        "installs `{distribution}` without a `RECORD`, so nothing states which bytes it installed"
    )]
    UnrecordedDistribution {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// A locked distribution's `RECORD` carries a malformed SHA-256 digest.
    ///
    /// Separate from [`EnvironmentMismatch::ModifiedDistributionFile`]: an
    /// invalid digest states no expected bytes, so reporting content drift
    /// would misdiagnose corrupt installed metadata as an edited module.
    #[error("installs `{distribution}` whose `RECORD` contains a malformed SHA-256 digest")]
    MalformedDistributionRecord {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// A locked distribution's `RECORD` contains an unsafe file path.
    ///
    /// `RECORD` is installed metadata, not a trusted path source. Refusing the
    /// whole distribution without echoing the entry prevents traversal,
    /// symlink escape, and terminal-control bytes in that entry from reaching
    /// either the filesystem reader or the diagnostic.
    #[error("installs `{distribution}` whose `RECORD` contains an unsafe file path")]
    UnsafeDistributionRecord {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// A locked distribution whose `RECORD` the manifest does not declare.
    ///
    /// `RECORD` sits inside the environment it describes, so on its own it
    /// proves nothing: an edit to a module and an edit to the line claiming
    /// that module's digest are one action, and
    /// [`EnvironmentMismatch::ModifiedDistributionFile`] cannot see a pair of
    /// them. `worker/bundle-manifest.json` declares each locked `RECORD` from
    /// outside the environment, which is what makes the per-file comparison an
    /// authentication rather than a self-consistency check. A locked
    /// distribution with no declaration is refused rather than checked against
    /// itself. Remedy: regenerate the declarations per
    /// `docs/operations/WORKER-ENVIRONMENT.md` §Declaring what the lock
    /// installed; the worker/runtime owner in
    /// `docs/governance/ROUTING-TABLES.md` owns the manifest.
    #[error(
        "installs `{distribution}`, whose `RECORD` `worker/bundle-manifest.json` does not \
         declare, so nothing outside the environment states which files it holds"
    )]
    UndeclaredDistributionRecord {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// A locked distribution's `RECORD` is not the one the manifest declares.
    ///
    /// The pair of edits [`EnvironmentMismatch::ModifiedDistributionFile`]
    /// cannot see. A file changed together with the `RECORD` line that pins it
    /// leaves the distribution self-consistent, so this is the comparison that
    /// still moves. Remedy: restore the environment from the lock per
    /// `docs/operations/WORKER-ENVIRONMENT.md` §Restoring the environment, or
    /// regenerate the declarations if the change was intended.
    #[error(
        "installs `{distribution}` whose `RECORD` is not the one \
         `worker/bundle-manifest.json` declares for it"
    )]
    ModifiedDistributionRecord {
        /// Canonicalized distribution name.
        distribution: String,
    },

    /// An interpreter startup module the lock does not account for.
    ///
    /// `site` imports `sitecustomize` and `usercustomize` by name as the
    /// interpreter starts, before the worker's own declared inputs are read.
    /// Neither is a `.pth`, so [`EnvironmentMismatch::UnownedPathHook`] never
    /// saw them, and both are the same hazard: arbitrary code inside the
    /// process whose identity says nothing about it. A module owned by a locked
    /// distribution is accounted for; one without a locked owner is not, and
    /// is refused unless `worker/bundle-manifest.json` declares its digest.
    #[error(
        "resolves the startup module `{module}` from a file no locked distribution claims and \
         the manifest does not declare"
    )]
    UnaccountedStartupModule {
        /// `sitecustomize` or `usercustomize`.
        module: StartupModuleName,
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
            Self::UnreadableLauncher { .. } | Self::UnsupportedLauncher { .. } => {
                "repair worker/launcher.json to the layout this build reads"
            }
            Self::RequirementsDisagreeWithLock { .. } => {
                "reconcile worker/pyproject.toml with worker/requirements.lock per \
                 docs/operations/WORKER-ENVIRONMENT.md"
            }
        };
        Some(RemedyAdvice::new(
            RemedyOwner::WorkerRuntime,
            action,
            Some("Worker protocol or containment failure"),
        ))
    }
}
