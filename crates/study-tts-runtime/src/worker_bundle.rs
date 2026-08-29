//! The deterministic identity of the executable worker bundle.
//!
//! ADR-0001 §12.5 makes this hash a synthesis-key input and names what goes
//! into it: "production worker source and imported project-owned modules, the
//! production Python lockfile, the worker protocol schema, launcher
//! configuration that affects inference, and Python runtime and platform ABI
//! identity." §22 adds the constraint that shapes the rest of this module:
//! "Derive the worker-bundle hash mechanically; do not depend on a
//! human-managed revision marker."
//!
//! Neither a directory walk nor a hand-written list satisfies that alone. A
//! walk would hash editor backups, `__pycache__`, and virtualenvs, invalidating
//! a cache over files the worker never loads. A list supplied by the caller
//! proves nothing: adding an imported module and forgetting to declare it would
//! leave the hash unchanged while the worker's behavior changed, which is
//! precisely the silent cache poisoning the hash exists to prevent.
//!
//! So the inputs are declared by the **worker itself**, in
//! `worker/bundle-manifest.json`, and the declaration is then checked against
//! the tree: every `.py` file beneath a declared import root must itself be
//! declared. The manifest says what the bundle is; the walk of the directories
//! it claims says whether that claim is complete. The check is over the
//! directory rather than over `import` statements because `importlib`,
//! `__import__`, and a parenthesized multi-line `from ... import (...)` all
//! load a file a line-oriented scan does not see.
//!
//! A declaration that decides its own scope would decide its own identity, so
//! [`REQUIRED_BUNDLE_INPUTS`] and [`REQUIRED_IMPORT_ROOT`] are Rust-owned and a
//! manifest omitting either is refused before a byte is hashed. What keeps the
//! rest honest is that the manifest declares *itself* among its inputs: the
//! check's own scope is a manifest field, and the manifest's bytes are hashed
//! like every other input, so there is no edit that shrinks the check and
//! leaves the identity where it was.
//!
//! The declared runtime ABI is the one input with no file behind it, and
//! [`crate::worker_environment`] is what proves it — along with the environment
//! the lockfile only describes. [`WorkerBundle::verified_hash`] is the way out
//! of this module because it asks first.
//!
//! What the walk cannot see is stated rather than implied: a module the worker
//! loads from outside every declared import root is outside the identity. That
//! is a bounded gap, not a hidden one — `docs/operations/WORKER-ENVIRONMENT.md`
//! records the rule that the worker's own modules live under a declared root,
//! and names this module in return.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use study_tts_core::{CanonicalValue, WorkerBundleHash, canonical_digest};

use crate::error::WorkerBundleError;
use crate::worker_environment::{self, WORKER_INTERPRETER_PATH};
use crate::{BuildError, ManagedPathError, io_error, managed};

/// Version of the bundle-hash definition itself.
///
/// The lever that invalidates every bundle identity when the derivation below
/// changes, matching [`study_tts_core::SYNTHESIS_IDENTITY_VERSION`] in role.
/// Moved to `e1-s1-v3` when a valid environment lock began requiring explicit
/// package sources, artifact kinds, and one SHA-256 artifact hash per
/// index-supplied distribution. Moved to `e1-s1-v4` when the breaking worker
/// protocol replaced its schema input with `worker-protocol-v1`.
pub const WORKER_BUNDLE_IDENTITY_VERSION: &str = "e1-s1-v4";

/// Path of the bundle manifest, relative to the repository root.
///
/// Fixed rather than configurable: a bundle whose manifest could live anywhere
/// is a bundle whose contents are decided by whoever calls this code, which is
/// the property this module exists to remove.
pub const BUNDLE_MANIFEST_PATH: &str = "worker/bundle-manifest.json";

/// Layout version this build publishes for a bundle manifest.
pub const BUNDLE_MANIFEST_SCHEMA_VERSION: &str = "1.2";

/// Manifest layouts this build reads.
///
/// `1.1` adds `startup_modules` as an optional field and `1.2` adds
/// `record_digests` as a required one. An older manifest remains readable and
/// declares neither, but each decoder still rejects the fields a later layout
/// added; accepting a future declaration under an older version would make the
/// version meaningless. A layout outside this list is refused rather than
/// guessed at.
///
/// Reading an older layout is not the same as passing under it. A manifest that
/// declares no `record_digests` has nothing to authenticate an installed
/// `RECORD` against, so `verified_hash` refuses every locked distribution with
/// [`crate::EnvironmentMismatch::UndeclaredDistributionRecord`] — which is the
/// point of the field rather than a gap in it.
const SUPPORTED_BUNDLE_MANIFEST_SCHEMA_VERSIONS: [&str; 3] =
    ["1.0", "1.1", BUNDLE_MANIFEST_SCHEMA_VERSION];

/// Path of the resolved Python lockfile, relative to the repository root.
pub const WORKER_LOCKFILE_PATH: &str = "worker/requirements.lock";

/// Path of the launcher configuration, relative to the repository root.
pub const WORKER_LAUNCHER_PATH: &str = "worker/launcher.json";

/// Path of the published worker protocol schema, relative to the repository
/// root.
///
/// Spelled out rather than composed, because a `const` is what the required
/// list below needs; `t1_e1_the_required_protocol_schema_is_the_published_file`
/// pins it to the name [`crate::PUBLISHED_SCHEMAS`] generates, so publishing a
/// new major cannot leave this naming a file that is no longer written.
pub const WORKER_PROTOCOL_SCHEMA_PATH: &str = "schemas/worker-protocol-v1.schema.json";

/// The project-owned Python package, relative to the repository root.
pub const WORKER_PACKAGE_ROOT: &str = "worker/study_tts_worker";

/// Module the worker process is started from, relative to the repository root.
pub const WORKER_ENTRYPOINT_PATH: &str = "worker/study_tts_worker/worker.py";

/// Inputs the manifest must declare, whatever else it declares.
///
/// ADR-0001 §12.5 names each of these individually — "production worker source
/// and imported project-owned modules, the production Python lockfile, the
/// worker protocol schema, launcher configuration that affects inference" — so
/// which of them belong to the bundle is not the manifest's to decide. Left to
/// the manifest, dropping `worker/launcher.json` from `inputs` produced a
/// bundle that still hashed, under a key that no longer moved when the launcher
/// changed.
///
/// `docs/operations/WORKER-ENVIRONMENT.md` §The floor is not the manifest's to
/// set names this constant in return.
pub const REQUIRED_BUNDLE_INPUTS: [&str; 5] = [
    BUNDLE_MANIFEST_PATH,
    WORKER_LAUNCHER_PATH,
    WORKER_LOCKFILE_PATH,
    WORKER_PROTOCOL_SCHEMA_PATH,
    WORKER_ENTRYPOINT_PATH,
];

/// Import root the manifest must declare.
///
/// The completeness walk's scope, and the reason it cannot be switched off:
/// with `import_roots` empty there was no directory to walk, so every module in
/// the package passed as declared. The entrypoint is required as an input above
/// for the case this cannot see — a package root that is walked but is empty
/// because its modules were deleted.
pub const REQUIRED_IMPORT_ROOT: &str = WORKER_PACKAGE_ROOT;

/// Length of an unpadded URL-safe base64 SHA-256 digest.
const RECORD_SHA256_BASE64_LENGTH: usize = 43;

/// Canonical final sextets for a 32-byte unpadded base64 value.
const RECORD_SHA256_FINAL_BYTES: &[u8] = b"AEIMQUYcgkosw048";

/// Largest declared bundle input this boundary will read, in bytes.
///
/// A provisional security ceiling in the sense of
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings: it
/// bounds what an untrusted-by-default file can make this process allocate. It
/// is generous for source, a lockfile, and a schema, and far below a model
/// weight — a declared input that trips it is a declared input list that has
/// gone wrong, which is why exceeding it is a refusal rather than a truncation.
pub const MAX_BUNDLE_INPUT_BYTES: usize = 8 * 1024 * 1024;

/// Python runtime and platform ABI identity.
///
/// Separate from the hashed files because none of it lives in the bundle: the
/// same source and lockfile on a different ABI can load different compiled
/// wheels and produce different audio, which is exactly the case ADR-0001 §12.5
/// means by "runtime ABI".
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PythonRuntimeIdentity {
    /// Interpreter implementation, such as `cpython`.
    pub implementation: String,
    /// Full interpreter version, such as `3.12.3`.
    pub version: String,
    /// ABI tag the interpreter builds extension modules against, such as
    /// `cp312`.
    pub abi_tag: String,
    /// Platform ABI tag wheels are resolved for, such as
    /// `manylinux_2_39_x86_64`.
    ///
    /// The tag carrying the ABI *version*, never the bare `linux_x86_64` that
    /// `packaging` offers first: that one is the same string on every glibc,
    /// and this field exists to tell two of them apart.
    pub platform_tag: String,
}

/// A module Python's `site` machinery can import during interpreter startup.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StartupModuleName {
    /// The machine-wide startup module.
    Sitecustomize,
    /// The user-site startup module.
    Usercustomize,
}

impl fmt::Display for StartupModuleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sitecustomize => formatter.write_str("sitecustomize"),
            Self::Usercustomize => formatter.write_str("usercustomize"),
        }
    }
}

impl fmt::Display for PythonRuntimeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} ({}, {})",
            self.implementation, self.version, self.abi_tag, self.platform_tag
        )
    }
}

/// `worker/bundle-manifest.json`: what the worker declares itself to be.
///
/// `deny_unknown_fields` because this is a project-owned format, and a field
/// this build cannot honor is a declaration it must refuse rather than one it
/// may ignore — an ignored field here would be an input silently left out of
/// the identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct BundleManifest {
    /// Layout version of this manifest.
    pub schema_version: String,
    /// Package directories, relative to the bundle root, whose modules are
    /// project-owned.
    ///
    /// An import that resolves beneath one of these must be declared. An import
    /// that does not is a third-party dependency, whose identity comes from the
    /// lockfile and the runtime ABI instead.
    pub import_roots: Vec<String>,
    /// Every file that belongs to the bundle, relative to the bundle root.
    pub inputs: Vec<String>,
    /// Interpreter and platform the bundle is resolved for.
    pub python: PythonRuntimeIdentity,
    /// Interpreter startup modules this machine carries that no locked
    /// distribution owns, by `RECORD`-spelled digest.
    ///
    /// Added by manifest layout `1.1`, defaulting to empty so a `1.0` manifest
    /// stays valid and declares nothing.
    ///
    /// The escape hatch a distribution-owned check cannot avoid needing. The
    /// reference Ubuntu environment resolves `/etc/python3.12/sitecustomize.py`
    /// before every site directory on `sys.path`, so it executes in the worker
    /// and belongs to no distribution. Refusing it outright would ask an
    /// operator to edit system Python; ignoring it would leave startup code
    /// outside the identity. Declaring its digest here does neither: the file
    /// is named where a reviewer sees it, and changing it changes this
    /// manifest, which is itself a hashed input.
    #[serde(default)]
    pub startup_modules: Vec<DeclaredStartupModule>,
    /// The `RECORD` each locked distribution must ship, by digest.
    ///
    /// Added by manifest layout `1.2`, and required by it: an omitted
    /// declaration refuses the distribution rather than exempting it.
    ///
    /// `RECORD` is a distribution's own statement of which bytes it installed,
    /// and it sits inside the environment it describes. Comparing installed
    /// files against it alone answers whether the install is self-consistent,
    /// which an edit to a module and its `RECORD` line keeps true. Declaring
    /// the digest here moves the claim outside the environment: this manifest
    /// is a hashed bundle input, so a change to what the lock is allowed to
    /// have installed is a change to every cache key, made where a reviewer
    /// sees it.
    ///
    /// Only the locked distributions are declared. An unlocked one is tolerated
    /// because the worker does not load it, and the one part of it that is not
    /// inert — a `.pth` — is already refused by name.
    pub record_digests: Vec<DeclaredDistributionRecord>,
}

/// One locked distribution's `RECORD`, as the manifest declares it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct DeclaredDistributionRecord {
    /// Distribution name, canonicalized before comparison.
    pub distribution: String,
    /// URL-safe unpadded base64 SHA-256 over the `RECORD` claims this build
    /// verifies, in `RECORD`'s own digest spelling.
    ///
    /// Not the digest of the `RECORD` file. `.dist-info` rows are installer
    /// bookkeeping — `INSTALLER`, `REQUESTED`, `direct_url.json` — that moves
    /// with the command that installed rather than with what the worker
    /// imports, so pinning the file itself would make a correct restore look
    /// like tampering. `runtime_probe.py` states the exact canonical form it
    /// hashes and names this field in return.
    #[serde(deserialize_with = "deserialize_record_digest")]
    pub digest: String,
}

/// One startup module the manifest accounts for, by digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct DeclaredStartupModule {
    /// `sitecustomize` or `usercustomize`.
    pub module: StartupModuleName,
    /// URL-safe unpadded base64 SHA-256 of the file, in `RECORD`'s spelling.
    ///
    /// The same spelling `RECORD` uses, so a declared digest and an owned one
    /// are the same kind of value and a reviewer compares them by eye.
    #[serde(deserialize_with = "deserialize_record_digest")]
    pub digest: String,
}

fn deserialize_record_digest<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    validate_record_digest(String::deserialize(deserializer)?)
}

pub(crate) fn deserialize_optional_record_digest<'de, D>(
    deserializer: D,
) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)?
        .map(validate_record_digest)
        .transpose()
}

fn validate_record_digest<E>(digest: String) -> Result<String, E>
where
    E: serde::de::Error,
{
    // Mirrors the three `record_digest_*` values in `runtime_probe.py`, which
    // names this validator in return.
    let valid = digest.len() == RECORD_SHA256_BASE64_LENGTH
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        && digest
            .bytes()
            .last()
            .is_some_and(|byte| RECORD_SHA256_FINAL_BYTES.contains(&byte));
    if valid {
        Ok(digest)
    } else {
        Err(E::custom(
            "expected a canonical unpadded URL-safe base64 SHA-256 digest",
        ))
    }
}

/// Manifest layout `1.0`, before startup modules could be declared.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct BundleManifestV1_0 {
    schema_version: String,
    import_roots: Vec<String>,
    inputs: Vec<String>,
    python: PythonRuntimeIdentity,
}

impl From<BundleManifestV1_0> for BundleManifest {
    fn from(manifest: BundleManifestV1_0) -> Self {
        Self {
            schema_version: manifest.schema_version,
            import_roots: manifest.import_roots,
            inputs: manifest.inputs,
            python: manifest.python,
            startup_modules: Vec::new(),
            record_digests: Vec::new(),
        }
    }
}

/// Manifest layout `1.1`, before installed `RECORD`s could be declared.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct BundleManifestV1_1 {
    schema_version: String,
    import_roots: Vec<String>,
    inputs: Vec<String>,
    python: PythonRuntimeIdentity,
    #[serde(default)]
    startup_modules: Vec<DeclaredStartupModule>,
}

impl From<BundleManifestV1_1> for BundleManifest {
    fn from(manifest: BundleManifestV1_1) -> Self {
        Self {
            schema_version: manifest.schema_version,
            import_roots: manifest.import_roots,
            inputs: manifest.inputs,
            python: manifest.python,
            startup_modules: manifest.startup_modules,
            record_digests: Vec::new(),
        }
    }
}

/// Version-only view read before the manifest is held to a specific shape.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct BundleManifestVersion {
    schema_version: String,
    // The selected decoder reparses these fields with unknown-field rejection.
    #[serde(flatten)]
    _remaining: BTreeMap<String, serde::de::IgnoredAny>,
}

/// One worker bundle: a root and the manifest found beneath it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerBundle {
    root: PathBuf,
    manifest: BundleManifest,
}

impl WorkerBundle {
    /// Loads the bundle declared beneath `root`.
    ///
    /// `root` is the repository root, because ADR-0001 §12.5 lists the worker
    /// protocol schema among the inputs and `schemas/` is a sibling of
    /// `worker/`. A bundle rooted at `worker/` could not name it without
    /// escaping its own root, and containment refuses that for good reason.
    ///
    /// # Errors
    ///
    /// [`WorkerBundleError::MissingDeclaredInput`] when the manifest itself is
    /// absent, [`WorkerBundleError::DeclaredInputTooLarge`] when it exceeds
    /// [`MAX_BUNDLE_INPUT_BYTES`],
    /// [`WorkerBundleError::UnreadableBundleManifest`] when it is not the
    /// declared shape, and [`WorkerBundleError::UnsupportedBundleManifest`] for
    /// a layout version this build does not implement.
    /// [`crate::ManagedPathError::ManagedPathEscape`] when a link occupies the
    /// manifest's path.
    pub fn load(root: impl Into<PathBuf>) -> Result<Self, BuildError> {
        let root = root.into();
        let bundle = Self {
            root,
            // A placeholder that declares nothing, so the manifest read below
            // goes through exactly the containment and size checks every other
            // declared input does. Reading it specially would leave one input
            // outside the rules the rest are held to.
            manifest: BundleManifest {
                schema_version: BUNDLE_MANIFEST_SCHEMA_VERSION.to_owned(),
                import_roots: Vec::new(),
                inputs: Vec::new(),
                python: PythonRuntimeIdentity {
                    implementation: String::new(),
                    version: String::new(),
                    abi_tag: String::new(),
                    platform_tag: String::new(),
                },
                startup_modules: Vec::new(),
                record_digests: Vec::new(),
            },
        };

        let resolved = bundle.resolve(BUNDLE_MANIFEST_PATH)?;
        let bytes = bundle.read_bounded(BUNDLE_MANIFEST_PATH, &resolved)?;
        let version: BundleManifestVersion = serde_json::from_slice(&bytes).map_err(|source| {
            WorkerBundleError::UnreadableBundleManifest {
                path: resolved.clone(),
                source,
            }
        })?;
        if !SUPPORTED_BUNDLE_MANIFEST_SCHEMA_VERSIONS.contains(&version.schema_version.as_str()) {
            return Err(WorkerBundleError::UnsupportedBundleManifest {
                path: resolved,
                declared: version.schema_version,
                required: BUNDLE_MANIFEST_SCHEMA_VERSION,
            }
            .into());
        }

        let manifest = match version.schema_version.as_str() {
            "1.0" => serde_json::from_slice::<BundleManifestV1_0>(&bytes).map(BundleManifest::from),
            "1.1" => serde_json::from_slice::<BundleManifestV1_1>(&bytes).map(BundleManifest::from),
            _ => serde_json::from_slice::<BundleManifest>(&bytes),
        }
        .map_err(|source| WorkerBundleError::UnreadableBundleManifest {
            path: resolved.clone(),
            source,
        })?;

        Ok(Self {
            root: bundle.root,
            manifest,
        })
    }

    /// The manifest this bundle declared.
    pub fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    /// Derives the bundle's identity, after proving the interpreter that would
    /// run it is the one the manifest declares.
    ///
    /// The gate ordering is load-bearing and is the reason this, rather than
    /// `WorkerBundle::hash`, is what the crate exposes. The declared runtime
    /// ABI reaches the digest as four strings a human wrote; without the probe
    /// they are the one bundle input nothing checks, so the same bundle carried
    /// to another interpreter patch version or platform would keep its identity
    /// while loading different compiled wheels — two renders under one cache
    /// key. ADR-0001 §22 forbids depending on a human-managed marker, and an
    /// unchecked declaration is one.
    ///
    /// Which interpreter is asked is not a parameter.
    /// [`WORKER_INTERPRETER_PATH`] beneath this bundle's own root is the only
    /// one, because a caller that chooses the interpreter chooses the answer:
    /// pointing this at a system Python that happens to match the manifest
    /// satisfies the check while the environment that actually runs the worker
    /// is never consulted.
    ///
    /// # Errors
    ///
    /// [`WorkerBundleError::RuntimeIdentityMismatch`] when the interpreter
    /// disagrees with the manifest,
    /// [`WorkerBundleError::UnreadableRuntimeIdentity`] when it answers with
    /// something this build cannot read,
    /// [`WorkerBundleError::UnreadableWorkerLockfile`] when the lock cannot be
    /// parsed, [`WorkerBundleError::EnvironmentDoesNotMatchLock`] when the
    /// installed environment disagrees with it, including when an installed
    /// `RECORD` differs from the declaration this manifest authenticates or a
    /// file differs from that authenticated `RECORD`,
    /// [`crate::ToolError::MissingTool`] when nothing executable is at
    /// [`WORKER_INTERPRETER_PATH`], and whatever `WorkerBundle::hash` reports
    /// once the runtime agrees.
    pub fn verified_hash(&self) -> Result<WorkerBundleHash, BuildError> {
        let interpreter = self.root.join(WORKER_INTERPRETER_PATH);
        // Located first, so a bundle missing both interpreter and lockfile
        // still reports the interpreter: the tool gate precedes the work, and
        // `t4_e1_an_absent_interpreter_is_refused_before_the_bundle_is_read`
        // is what proves the ordering rather than assuming it.
        let resolved = worker_environment::resolve_interpreter(&interpreter)?;
        // Then the lock, read here rather than in `worker_environment` because
        // reading a declared input is this type's job: containment and the byte
        // ceiling apply to it exactly as they do to every other one. What the
        // bytes *mean* is the environment module's.
        let lockfile = self.lockfile_bytes()?;
        worker_environment::check(&interpreter, &resolved, &self.manifest, &lockfile)?;
        self.hash()
    }

    /// Reads `worker/requirements.lock` as a declared bundle input.
    ///
    /// # Errors
    ///
    /// The containment and size errors [`WorkerBundle::hash`] documents for any
    /// declared input.
    fn lockfile_bytes(&self) -> Result<Vec<u8>, BuildError> {
        let resolved = self.resolve(WORKER_LOCKFILE_PATH)?;
        self.read_bounded(WORKER_LOCKFILE_PATH, &resolved)
    }

    /// Derives the bundle's identity from what the manifest declares.
    ///
    /// Hashes every declared input's bytes together with the declared runtime
    /// ABI, but only after proving the declaration is complete: see
    /// [`WorkerBundle::check_import_roots_declared`].
    ///
    /// Crate-private because the declared runtime ABI is a claim until an
    /// interpreter is asked. [`WorkerBundle::verified_hash`] asks first, and is
    /// the only way out of this crate.
    ///
    /// # Errors
    ///
    /// [`WorkerBundleError::UndeclaredRequiredInput`] when the manifest omits
    /// one of [`REQUIRED_BUNDLE_INPUTS`] and
    /// [`WorkerBundleError::UndeclaredImportRoot`] when it omits
    /// [`REQUIRED_IMPORT_ROOT`], both before any input is read.
    /// [`WorkerBundleError::UndeclaredModule`] when a `.py` file beneath a
    /// declared import root is not itself declared,
    /// [`WorkerBundleError::MissingDeclaredInput`] when a declared input is not
    /// present, and [`WorkerBundleError::DeclaredInputTooLarge`] when one
    /// exceeds [`MAX_BUNDLE_INPUT_BYTES`]; each refuses rather than hash a
    /// bundle that is not the one declared.
    /// [`crate::ManagedPathError::InvalidManagedName`] and
    /// [`crate::ManagedPathError::ManagedPathEscape`] when a declared input is
    /// spelled unsafely or is occupied by a link, because a link would fold
    /// bytes from outside the bundle into the bundle's own identity.
    /// Otherwise [`crate::IoError::FileSystem`] carries what the filesystem
    /// reported.
    pub(crate) fn hash(&self) -> Result<WorkerBundleHash, BuildError> {
        // First, because every check after this one is scoped by the manifest.
        // A manifest declaring no import root passes the completeness walk by
        // giving it nothing to walk, and one omitting the launcher passes every
        // check there is while leaving an inference-affecting file outside the
        // identity.
        self.check_required_declarations()?;

        // A `BTreeMap` rather than the declared order, so the hash depends on
        // which inputs were declared and not on how they were listed.
        let mut digests = BTreeMap::new();
        for declared in &self.manifest.inputs {
            let resolved = self.resolve(declared)?;
            let bytes = self.read_bounded(declared, &resolved)?;
            digests.insert(
                declared.clone(),
                CanonicalValue::from(blake3::hash(&bytes).to_hex().to_string()),
            );
        }

        // Before the digest, not after: a hash produced from an incomplete
        // declaration is a hash that names the wrong bundle, and returning it
        // alongside a warning would let a caller publish under it anyway.
        self.check_import_roots_declared()?;

        Ok(canonical_digest(&CanonicalValue::object([
            ("identity_version", WORKER_BUNDLE_IDENTITY_VERSION.into()),
            ("declared_inputs", CanonicalValue::Object(digests)),
            (
                "python_implementation",
                self.manifest.python.implementation.as_str().into(),
            ),
            (
                "python_version",
                self.manifest.python.version.as_str().into(),
            ),
            (
                "python_abi_tag",
                self.manifest.python.abi_tag.as_str().into(),
            ),
            (
                "platform_tag",
                self.manifest.python.platform_tag.as_str().into(),
            ),
        ]))
        .into())
    }

    /// Refuses a manifest that leaves out an input this build owns the
    /// requirement for.
    ///
    /// Runs before the declared inputs are read, so a manifest that declares
    /// nothing is refused by what it omitted rather than by whichever of its
    /// remaining inputs happens to be missing from disk.
    fn check_required_declarations(&self) -> Result<(), BuildError> {
        for required in REQUIRED_BUNDLE_INPUTS {
            if !self.manifest.inputs.iter().any(|input| input == required) {
                return Err(WorkerBundleError::UndeclaredRequiredInput {
                    path: PathBuf::from(required),
                    manifest: PathBuf::from(BUNDLE_MANIFEST_PATH),
                }
                .into());
            }
        }

        if !self
            .manifest
            .import_roots
            .iter()
            .any(|root| root == REQUIRED_IMPORT_ROOT)
        {
            return Err(WorkerBundleError::UndeclaredImportRoot {
                import_root: PathBuf::from(REQUIRED_IMPORT_ROOT),
                manifest: PathBuf::from(BUNDLE_MANIFEST_PATH),
            }
            .into());
        }

        Ok(())
    }

    /// Refuses a manifest that omits a module in a directory it claims.
    ///
    /// This is what turns the declared list from a claim into a checked fact.
    /// Every `.py` file beneath a declared import root must appear in the
    /// manifest, so adding a module without declaring it cannot leave the
    /// bundle hash unchanged — however that module is later reached.
    ///
    /// Third-party code falls outside this by construction rather than by a
    /// filter: it does not live beneath an import root, and `torch` has its
    /// identity from `worker/requirements.lock` and the runtime ABI already.
    fn check_import_roots_declared(&self) -> Result<(), BuildError> {
        let declared: BTreeSet<&str> = self.manifest.inputs.iter().map(String::as_str).collect();

        for import_root in &self.manifest.import_roots {
            for module in self.modules_under(import_root)? {
                if !declared.contains(module.as_str()) {
                    return Err(WorkerBundleError::UndeclaredModule {
                        module: PathBuf::from(module),
                        import_root: PathBuf::from(import_root),
                        manifest: PathBuf::from(BUNDLE_MANIFEST_PATH),
                    }
                    .into());
                }
            }
        }
        Ok(())
    }

    /// Every `.py` file beneath one declared import root, bundle-relative.
    ///
    /// A missing import root yields nothing rather than refusing: a manifest
    /// that claims a directory it does not ship is a manifest whose declared
    /// inputs are missing, and [`WorkerBundle::hash`] reports that against the
    /// input the author actually wrote.
    ///
    /// Every descent goes through the same containment helper the declared
    /// inputs do, so a link — to a directory or to a `.py` file — is refused
    /// rather than followed or, worse, silently skipped. Skipping it is exactly
    /// the omission this walk exists to remove, and following it would fold
    /// bytes from outside the bundle into the bundle's own identity.
    fn modules_under(&self, import_root: &str) -> Result<Vec<String>, BuildError> {
        let mut found = Vec::new();
        let mut directories = vec![(import_root.to_owned(), self.resolve_directory(import_root)?)];

        while let Some((relative_directory, resolved)) = directories.pop() {
            let entries = match fs::read_dir(&resolved) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(io_error(&resolved, error)),
            };

            for entry in entries {
                let entry = entry.map_err(|error| io_error(&resolved, error))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                let relative = format!("{relative_directory}/{name}");
                // `DirEntry::file_type` reports the link itself rather than its
                // target, so a link reaches `directory_candidate` — which
                // refuses it — instead of passing as whatever it points at.
                let kind = entry
                    .file_type()
                    .map_err(|error| io_error(entry.path(), error))?;

                if kind.is_dir() || kind.is_symlink() {
                    let child = managed::directory_candidate(&resolved, &name)?;
                    directories.push((relative, child));
                } else if name.ends_with(".py") {
                    found.push(relative);
                }
            }
        }

        // Sorted so a refusal names the same module on every filesystem;
        // `read_dir` order is not defined.
        found.sort();
        Ok(found)
    }

    /// Resolves a declared directory beneath the bundle root.
    ///
    /// The directory counterpart of [`WorkerBundle::resolve`], contained the
    /// same way: one component at a time, so an intermediate symlinked
    /// directory is refused rather than followed. A manifest is a file this
    /// build reads, so an import root spelled `../..` has to be refused on its
    /// spelling before anything is walked.
    fn resolve_directory(&self, declared: &str) -> Result<PathBuf, BuildError> {
        let mut directory = self.root.clone();

        for component in Path::new(declared).components() {
            let Component::Normal(name) = component else {
                return Err(ManagedPathError::InvalidManagedName {
                    name: declared.to_owned(),
                    root: self.root.clone(),
                }
                .into());
            };
            directory = managed::directory_candidate(&directory, &name.to_string_lossy())?;
        }

        Ok(directory)
    }

    /// Resolves one declared input beneath the bundle root.
    ///
    /// Walks the path one component at a time through the same containment
    /// helpers the cache and package paths use, so an intermediate symlinked
    /// directory is refused rather than followed. Resolving the whole path in
    /// one join would only inspect the final element.
    fn resolve(&self, declared: &str) -> Result<PathBuf, BuildError> {
        let relative = Path::new(declared);
        let mut components = relative.components().peekable();
        let mut directory = self.root.clone();

        while let Some(component) = components.next() {
            let Component::Normal(name) = component else {
                // `..`, `.`, a root, or a prefix. Refused on its spelling
                // before any path exists to contain, which is the same rule
                // and the same message `managed` applies to a single name.
                return Err(ManagedPathError::InvalidManagedName {
                    name: declared.to_owned(),
                    root: self.root.clone(),
                }
                .into());
            };
            let name = name.to_string_lossy();

            if components.peek().is_some() {
                directory = managed::directory_candidate(&directory, &name)?;
            } else {
                return managed::leaf(&directory, &name);
            }
        }

        // An empty declared input names the root itself, which is a directory.
        Err(self.missing(declared))
    }

    /// Reads one declared input under the size ceiling.
    fn read_bounded(&self, declared: &str, resolved: &Path) -> Result<Vec<u8>, BuildError> {
        let mut file = match File::open(resolved) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(self.missing(declared));
            }
            Err(error) => return Err(io_error(resolved, error)),
        };

        // One byte past the ceiling distinguishes "exactly at the limit" from
        // "over it" without reading the whole oversized file.
        let mut bytes = Vec::new();
        file.by_ref()
            .take(MAX_BUNDLE_INPUT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| io_error(resolved, error))?;
        if bytes.len() > MAX_BUNDLE_INPUT_BYTES {
            return Err(WorkerBundleError::DeclaredInputTooLarge {
                path: PathBuf::from(declared),
                max_bytes: MAX_BUNDLE_INPUT_BYTES,
            }
            .into());
        }

        Ok(bytes)
    }

    /// Builds the refusal for a declared input that is not there.
    fn missing(&self, declared: &str) -> BuildError {
        WorkerBundleError::MissingDeclaredInput {
            path: PathBuf::from(declared),
            root: self.root.clone(),
        }
        .into()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// One named change to the runtime ABI, for the sensitivity property.
    pub(crate) type RuntimeMutation = (&'static str, fn(&mut PythonRuntimeIdentity));

    /// The repository this test suite is compiled inside.
    ///
    /// The bundle hashed by these tests is the **real** `worker/` package, not
    /// a shaped-like-it fixture: a fixture would agree with any manifest,
    /// including one that omits half the worker.
    pub(crate) fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A writable copy of the repository's real bundle inputs.
    ///
    /// Copied because these tests mutate inputs to observe the hash move, and a
    /// test that edited the checked-in worker would leave the working tree
    /// dirty when it failed part way through.
    pub(crate) fn bundle_copy() -> TempDir {
        let source = repository_root();
        let copy = TempDir::new().expect("a temporary bundle root");
        let manifest_bytes =
            fs::read(source.join(BUNDLE_MANIFEST_PATH)).expect("the real manifest is readable");
        let manifest: BundleManifest =
            serde_json::from_slice(&manifest_bytes).expect("the real manifest parses");

        for declared in &manifest.inputs {
            let destination = copy.path().join(declared);
            fs::create_dir_all(destination.parent().expect("a declared input has a parent"))
                .expect("declared input directories are creatable");
            fs::copy(source.join(declared), &destination).expect("a declared input is copyable");
        }
        copy
    }

    /// Loads the bundle from disk, so a manifest edit written there takes
    /// effect.
    pub(crate) fn bundle(root: &TempDir) -> WorkerBundle {
        WorkerBundle::load(root.path()).expect("the copied manifest loads")
    }

    #[test]
    fn t1_e1_the_checked_in_worker_bundle_hashes() {
        // The manifest in the repository must describe the repository. If a
        // declared input is renamed or deleted without the manifest being
        // updated, this fails here rather than at the first real render.
        let bundle = WorkerBundle::load(repository_root())
            .expect("the checked-in worker bundle manifest loads");

        bundle
            .hash()
            .expect("the checked-in worker bundle is complete and hashable");
    }

    #[test]
    fn t1_e1_worker_bundle_hash_changes_on_owned_runtime_input() {
        let root = bundle_copy();
        // Loaded once and reused across the loop below. `hash` re-reads every
        // declared input from disk, so the digest still moves — but the
        // manifest is itself a declared input, and reloading it after its bytes
        // had been disturbed would report a parse failure where the test is
        // asking about a hash.
        let loaded = bundle(&root);
        let baseline = loaded.hash().expect("a complete bundle hashes");

        // Every declared input, one at a time, driven from the manifest rather
        // than from a list written here — so an input added to the worker is
        // covered by this test without anybody remembering to add it.
        for declared in &loaded.manifest().inputs {
            let path = root.path().join(declared);
            let original = fs::read(&path).expect("a declared input is readable");
            // Appended rather than replaced, so a JSON input stays parseable:
            // this test asserts that a changed input moves the hash, and an
            // input the build refuses to read never reaches the digest.
            let mut changed = original.clone();
            changed.extend_from_slice(b"\n# changed\n");
            fs::write(&path, &changed).expect("a declared input is writable");

            assert_ne!(
                loaded.hash().expect("the bundle still hashes"),
                baseline,
                "changing `{declared}` must change the worker bundle hash"
            );

            fs::write(&path, original).expect("a declared input is restorable");
        }
        assert_eq!(
            loaded.hash().expect("the bundle still hashes"),
            baseline,
            "restoring every input must restore the hash"
        );

        // The runtime ABI is a bundle input that is not a file: the same source
        // on a different interpreter can load different compiled wheels.
        let abi_changes: [RuntimeMutation; 4] = [
            ("implementation", |runtime| {
                runtime.implementation = "pypy".to_owned();
            }),
            ("version", |runtime| {
                runtime.version = "3.13.1".to_owned();
            }),
            ("abi_tag", |runtime| {
                runtime.abi_tag = "cp313".to_owned();
            }),
            ("platform_tag", |runtime| {
                runtime.platform_tag = "musllinux_1_2_x86_64".to_owned();
            }),
        ];
        for (field, mutate) in abi_changes {
            let mut manifest = loaded.manifest().clone();
            mutate(&mut manifest.python);
            write_manifest(&root, &manifest);

            assert_ne!(
                bundle(&root).hash().expect("the bundle still hashes"),
                baseline,
                "changing the runtime `{field}` must change the worker bundle hash"
            );
        }
    }

    #[test]
    fn t1_e1_worker_bundle_hash_ignores_unrelated_repository_files() {
        let root = bundle_copy();
        let baseline = bundle(&root).hash().expect("a complete bundle hashes");

        // Files a repository really carries beside the bundle. None of them is
        // executable worker input, so none may invalidate a cache entry.
        let unrelated = [
            "README.md",
            "docs/adr/ADR-0001-production-rust-study-guide-tts.md",
            "crates/study-tts-core/src/lib.rs",
            "fixtures/lessons/e0-s0-two-segment.json",
            // Undeclared files inside the bundle directory itself, which is
            // where a directory walk would go wrong.
            "worker/README.md",
            "worker/study_tts_worker/protocol.py.orig",
            "worker/study_tts_worker/__pycache__/protocol.cpython-312.pyc",
            "worker/.venv/pyvenv.cfg",
        ];
        for path in unrelated {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().expect("an unrelated file has a parent"))
                .expect("unrelated directories are creatable");
            fs::write(&path, b"first").expect("an unrelated file is writable");

            assert_eq!(
                bundle(&root).hash().expect("the bundle still hashes"),
                baseline,
                "creating `{}` must not change the worker bundle hash",
                path.display()
            );

            fs::write(&path, b"second").expect("an unrelated file is rewritable");

            assert_eq!(
                bundle(&root).hash().expect("the bundle still hashes"),
                baseline,
                "changing `{}` must not change the worker bundle hash",
                path.display()
            );
        }
    }

    #[test]
    fn t1_e1_a_module_under_an_import_root_the_manifest_omits_is_refused() {
        // The hole a caller-supplied list leaves open: add a module to the
        // package, forget the manifest, and the hash does not move even though
        // the worker can load it.
        let root = bundle_copy();
        let baseline = bundle(&root).hash().expect("the copied bundle hashes");

        fs::write(
            root.path().join("worker/study_tts_worker/pronunciation.py"),
            b"RULES = {}\n",
        )
        .expect("the new module is writable");

        let error = bundle(&root)
            .hash()
            .expect_err("an undeclared module in the package must not hash");

        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::UndeclaredModule { .. })
            ),
            "expected an undeclared module, got {error:?}"
        );

        // Declaring it resolves the refusal *and* moves the hash. Both, because
        // a declaration that changed no identity would be a formality.
        let mut manifest = bundle(&root).manifest().clone();
        manifest
            .inputs
            .push("worker/study_tts_worker/pronunciation.py".to_owned());
        write_manifest(&root, &manifest);

        assert_ne!(
            bundle(&root)
                .hash()
                .expect("declaring the module completes the bundle"),
            baseline,
            "declaring a module must move the bundle hash"
        );
    }

    #[test]
    fn t1_e1_a_module_reached_only_dynamically_is_still_declared() {
        // Why the check walks the directory instead of reading the source. This
        // module is loaded by name at runtime, so no `import` statement names
        // it and no scan of the source could see it — yet it decides what the
        // worker says. A hash that ignored it would name the wrong bundle.
        let root = bundle_copy();
        fs::write(
            root.path().join("worker/study_tts_worker/pronunciation.py"),
            b"RULES = {}\n",
        )
        .expect("the new module is writable");

        let worker = root.path().join("worker/study_tts_worker/worker.py");
        let mut source = fs::read(&worker).expect("the worker source is readable");
        source.extend_from_slice(
            b"\nimport importlib\nRULES = importlib.import_module('.pronunciation', __package__)\n",
        );
        fs::write(&worker, source).expect("the worker source is writable");

        let error = bundle(&root)
            .hash()
            .expect_err("a dynamically loaded module must not escape the identity");

        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::UndeclaredModule { .. })
            ),
            "expected an undeclared module, got {error:?}"
        );
    }

    #[test]
    fn t1_e1_a_module_in_a_subpackage_of_an_import_root_is_declared() {
        // The walk descends; a check that only read the root directory would
        // pass a whole subpackage the worker can import. Python reaches this
        // one as `study_tts_worker.voices.presets`, which is an ordinary
        // import and not an exotic case.
        let root = bundle_copy();
        let subpackage = root.path().join("worker/study_tts_worker/voices");
        fs::create_dir_all(&subpackage).expect("the subpackage directory is made");
        fs::write(subpackage.join("__init__.py"), b"").expect("the subpackage init is writable");

        let error = bundle(&root)
            .hash()
            .expect_err("a module one level down must not escape the identity");

        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::UndeclaredModule { ref module, .. })
                    if module == Path::new("worker/study_tts_worker/voices/__init__.py")
            ),
            "expected the nested module to be named, got {error:?}"
        );
    }

    #[test]
    fn t1_e1_a_symlink_beneath_an_import_root_is_refused_rather_than_skipped() {
        // Both directions are failures the walk exists to prevent. Following a
        // link would fold bytes from outside the bundle into its identity;
        // skipping one would leave a loadable module out of it. Containment
        // refuses instead, so neither is reachable.
        let root = bundle_copy();
        let outside = TempDir::new().expect("a directory outside the bundle");
        fs::write(outside.path().join("pronunciation.py"), b"RULES = {}\n")
            .expect("the outside module is writable");

        for (link, target, what) in [
            (
                "worker/study_tts_worker/pronunciation.py",
                outside.path().join("pronunciation.py"),
                "a linked module",
            ),
            (
                "worker/study_tts_worker/voices",
                outside.path().to_path_buf(),
                "a linked package directory",
            ),
        ] {
            let path = root.path().join(link);
            std::os::unix::fs::symlink(&target, &path).expect("the link is creatable");

            let error = bundle(&root).hash().expect_err("a link must not be walked");
            assert!(
                matches!(
                    error,
                    BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
                ),
                "{what} must be refused as an escape, got {error:?}"
            );

            fs::remove_file(&path).expect("the link is removable");
        }
    }

    #[test]
    fn t1_e1_a_declared_bundle_input_past_the_byte_ceiling_is_refused() {
        // The ceiling `docs/architecture/WALKING-SKELETON.md` records. It
        // bounds what a declared input can make this process allocate, so the
        // boundary itself has to be accepted or the ceiling is really one byte
        // lower than the document says.
        let root = bundle_copy();
        let path = root.path().join("worker/requirements.lock");

        fs::write(&path, vec![b'#'; MAX_BUNDLE_INPUT_BYTES])
            .expect("the boundary input is written");
        bundle(&root)
            .hash()
            .expect("the byte boundary must be accepted");

        fs::write(&path, vec![b'#'; MAX_BUNDLE_INPUT_BYTES + 1])
            .expect("the oversized input is written");
        assert!(matches!(
            bundle(&root).hash(),
            Err(BuildError::WorkerBundle(
                WorkerBundleError::DeclaredInputTooLarge { max_bytes, .. }
            )) if max_bytes == MAX_BUNDLE_INPUT_BYTES
        ));
    }

    #[test]
    fn t1_e1_python_outside_every_import_root_needs_no_declaration() {
        // The other side of the rule, and what keeps it satisfiable. The
        // worker's own test suite is Python beside the package rather than
        // inside it, and third-party code is not in the tree at all: both have
        // their identity from `worker/requirements.lock` and the runtime ABI,
        // so requiring either in the input list would ask the manifest to
        // declare something the worker does not own.
        let root = bundle_copy();
        fs::create_dir_all(root.path().join("worker/tests")).expect("the test directory is made");
        fs::write(
            root.path().join("worker/tests/test_protocol.py"),
            b"import torch\n",
        )
        .expect("the test module is writable");

        bundle(&root)
            .hash()
            .expect("Python outside every import root leaves the bundle complete");
    }

    #[test]
    fn t1_e1_a_missing_declared_bundle_input_is_refused() {
        let root = bundle_copy();
        fs::remove_file(root.path().join("worker/requirements.lock"))
            .expect("the lockfile is removable");

        let error = bundle(&root)
            .hash()
            .expect_err("a bundle missing a declared input must not hash");

        // Naming the exact variant is what proves the refusal is the bundle's
        // and not a generic filesystem error that happened to surface.
        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::MissingDeclaredInput { .. })
            ),
            "expected a missing declared input, got {error:?}"
        );
    }

    #[test]
    fn t1_e1_a_bundle_manifest_this_build_cannot_read_is_refused() {
        let root = bundle_copy();
        let manifest = root.path().join(BUNDLE_MANIFEST_PATH);

        let mut unknown_version = bundle(&root).manifest().clone();
        unknown_version.schema_version = "9.9".to_owned();
        write_manifest(&root, &unknown_version);
        assert!(
            matches!(
                WorkerBundle::load(root.path()),
                Err(BuildError::WorkerBundle(
                    WorkerBundleError::UnsupportedBundleManifest { .. }
                ))
            ),
            "an unknown manifest version must be refused"
        );

        // An unknown field is refused rather than ignored: an ignored field
        // here would be an input silently left out of the identity.
        let mut with_unknown_field: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest).expect("the manifest is readable"))
                .expect("the manifest is JSON");
        with_unknown_field["extra"] = serde_json::Value::Bool(true);
        fs::write(
            &manifest,
            serde_json::to_vec_pretty(&with_unknown_field).expect("the manifest serializes"),
        )
        .expect("the manifest is writable");
        assert!(
            matches!(
                WorkerBundle::load(root.path()),
                Err(BuildError::WorkerBundle(
                    WorkerBundleError::UnsupportedBundleManifest { .. }
                ))
            ),
            "an unknown version must be refused before its future fields are parsed"
        );

        with_unknown_field["schema_version"] =
            serde_json::Value::String(BUNDLE_MANIFEST_SCHEMA_VERSION.to_owned());
        fs::write(
            &manifest,
            serde_json::to_vec_pretty(&with_unknown_field).expect("the manifest serializes"),
        )
        .expect("the manifest is writable");
        assert!(
            matches!(
                WorkerBundle::load(root.path()),
                Err(BuildError::WorkerBundle(
                    WorkerBundleError::UnreadableBundleManifest { .. }
                ))
            ),
            "an unknown manifest field must be refused"
        );

        with_unknown_field
            .as_object_mut()
            .expect("the manifest is an object")
            .remove("extra");
        // Each layout must refuse the field the layout after it added, or
        // declaring an older version buys a reader nothing. Walked down one
        // step at a time, dropping exactly one field per step, so a refusal is
        // attributable to the field the step is about.
        let publish = |manifest_value: &serde_json::Value| {
            fs::write(
                &manifest,
                serde_json::to_vec_pretty(manifest_value).expect("the manifest serializes"),
            )
            .expect("the manifest is writable");
        };
        let drop_field = |manifest_value: &mut serde_json::Value, field: &str| {
            manifest_value
                .as_object_mut()
                .expect("the manifest is an object")
                .remove(field);
        };

        with_unknown_field["schema_version"] = serde_json::Value::String("1.1".to_owned());
        publish(&with_unknown_field);
        assert!(
            matches!(
                WorkerBundle::load(root.path()),
                Err(BuildError::WorkerBundle(
                    WorkerBundleError::UnreadableBundleManifest { .. }
                ))
            ),
            "a 1.1 manifest must not accept the 1.2 record-digest field"
        );

        drop_field(&mut with_unknown_field, "record_digests");
        publish(&with_unknown_field);
        let without_records =
            WorkerBundle::load(root.path()).expect("a strict 1.1 manifest remains readable");
        assert!(
            without_records.manifest().record_digests.is_empty(),
            "layout 1.1 declares no record digests"
        );

        with_unknown_field["schema_version"] = serde_json::Value::String("1.0".to_owned());
        publish(&with_unknown_field);
        assert!(
            matches!(
                WorkerBundle::load(root.path()),
                Err(BuildError::WorkerBundle(
                    WorkerBundleError::UnreadableBundleManifest { .. }
                ))
            ),
            "a 1.0 manifest must not accept the 1.1 startup-module field"
        );

        drop_field(&mut with_unknown_field, "startup_modules");
        publish(&with_unknown_field);
        let legacy =
            WorkerBundle::load(root.path()).expect("a strict 1.0 manifest remains readable");
        assert!(
            legacy.manifest().startup_modules.is_empty(),
            "layout 1.0 declares no startup modules"
        );
        assert!(
            legacy.manifest().record_digests.is_empty(),
            "layout 1.0 declares no record digests"
        );

        with_unknown_field["record_digests"] = serde_json::json!([]);
        for startup_module in [
            serde_json::json!({
                "module": "arbitrary_startup_code",
                "digest": "Q9gRJdkjdrGmnVOnESagQcyaGNgIDpLeoKKuI74Tix4",
            }),
            serde_json::json!({
                "module": "sitecustomize",
                "digest": "Q9gRJdkjdrGmnVOnESagQcyaGNgIDpLeoKKuI74Tix5",
            }),
        ] {
            with_unknown_field["schema_version"] =
                serde_json::Value::String(BUNDLE_MANIFEST_SCHEMA_VERSION.to_owned());
            with_unknown_field["startup_modules"] = serde_json::json!([startup_module]);
            publish(&with_unknown_field);

            assert!(
                matches!(
                    WorkerBundle::load(root.path()),
                    Err(BuildError::WorkerBundle(
                        WorkerBundleError::UnreadableBundleManifest { .. }
                    ))
                ),
                "a malformed startup-module declaration must be refused while parsing"
            );
        }
    }

    #[test]
    fn t1_e1_a_declared_bundle_input_that_escapes_the_root_is_refused() {
        let root = bundle_copy();

        // Refused on spelling, before any path is built. A declared input that
        // reaches outside the bundle would fold bytes this bundle does not
        // contain into the identity that names its cache entries.
        for escaping in [
            "../outside.txt",
            "worker/../../outside.txt",
            "/etc/hostname",
        ] {
            let mut manifest = bundle(&root).manifest().clone();
            manifest.inputs.push(escaping.to_owned());
            write_manifest(&root, &manifest);

            let error = bundle(&root)
                .hash()
                .expect_err("an escaping declared input must not hash");

            assert!(
                matches!(
                    error,
                    BuildError::ManagedPath(ManagedPathError::InvalidManagedName { .. })
                ),
                "expected `{escaping}` to be refused on its spelling, got {error:?}"
            );
        }
    }

    #[test]
    fn t1_e1_a_symlinked_declared_bundle_input_is_refused() {
        let root = bundle_copy();
        let outside = TempDir::new().expect("a temporary directory outside the bundle");
        let planted = outside.path().join("planted.lock");
        fs::write(&planted, b"bytes from outside the bundle").expect("the plant is writable");

        let lock = root.path().join("worker/requirements.lock");
        fs::remove_file(&lock).expect("the lockfile is removable");
        std::os::unix::fs::symlink(&planted, &lock).expect("the symlink is creatable");

        let error = bundle(&root)
            .hash()
            .expect_err("a symlinked declared input must not hash");

        // Following the link would fold bytes from outside the bundle into the
        // bundle's own identity, which is how a planted file passes as a match.
        assert!(
            matches!(error, BuildError::ManagedPath(_)),
            "expected a containment refusal, got {error:?}"
        );
    }
    #[test]
    fn t1_e1_startup_module_names_display_as_they_serialize() {
        // `EnvironmentMismatch::UnaccountedStartupModule` renders this name
        // through `Display`, so the refusal an operator acts on is correct only
        // while that hand-written spelling matches the serde representation the
        // probe answers in. Nothing else compares the two.
        //
        // The expectation is an exhaustive `match` rather than a derived value:
        // a third startup module is a compile error here instead of an untested
        // one, and both representations are held to a spelling a reviewer reads
        // against `docs/operations/WORKER-ENVIRONMENT.md` rather than to each
        // other, which two matching mistakes would satisfy.
        for name in [
            StartupModuleName::Sitecustomize,
            StartupModuleName::Usercustomize,
        ] {
            let spelled = match name {
                StartupModuleName::Sitecustomize => "sitecustomize",
                StartupModuleName::Usercustomize => "usercustomize",
            };
            assert_eq!(
                serde_json::to_value(name).expect("a startup module name serializes"),
                serde_json::Value::String(spelled.to_owned()),
                "the serde spelling drifted from `{spelled}`"
            );
            assert_eq!(
                name.to_string(),
                spelled,
                "the `Display` spelling drifted from `{spelled}`"
            );
        }
    }
    #[test]
    fn t1_e1_a_manifest_omitting_a_required_input_is_refused() {
        // The manifest decided which files were bundle inputs, so it decided
        // which changes moved the identity. Dropping `worker/launcher.json`
        // left a bundle that still hashed under a key that stopped moving when
        // the launcher changed, which is a wrong cache rather than a failure.
        //
        // Driven from the constant rather than from a list written here: an
        // input added to `REQUIRED_BUNDLE_INPUTS` is covered without anybody
        // remembering to extend this.
        for required in REQUIRED_BUNDLE_INPUTS {
            let root = bundle_copy();
            let mut manifest = bundle(&root).manifest().clone();
            manifest.inputs.retain(|input| input != required);
            write_manifest(&root, &manifest);

            let error = bundle(&root)
                .hash()
                .expect_err("a manifest short of a required input must not hash");

            assert!(
                matches!(
                    error,
                    BuildError::WorkerBundle(WorkerBundleError::UndeclaredRequiredInput {
                        ref path,
                        ..
                    }) if path == Path::new(required)
                ),
                "dropping `{required}` produced the wrong error: {error:?}"
            );
        }
    }

    #[test]
    fn t1_e1_a_manifest_declaring_no_import_root_is_refused() {
        // The completeness walk is scoped by `import_roots`, so emptying it
        // switched the check off rather than failing it: with no directory to
        // walk, every module in the package passed as declared. The module
        // planted here is what a passing walk would have had to find.
        let root = bundle_copy();
        let mut manifest = bundle(&root).manifest().clone();
        manifest.import_roots.clear();
        write_manifest(&root, &manifest);
        fs::write(
            root.path().join("worker/study_tts_worker/undeclared.py"),
            b"RULES = {}\n",
        )
        .expect("the undeclared module is writable");

        let error = bundle(&root)
            .hash()
            .expect_err("a manifest that scopes the check to nothing must not hash");

        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::UndeclaredImportRoot {
                    ref import_root,
                    ..
                }) if import_root == Path::new(REQUIRED_IMPORT_ROOT)
            ),
            "expected the missing import root to be named, got {error:?}"
        );
    }

    #[test]
    fn t1_e1_the_required_protocol_schema_is_the_published_file() {
        // The path is a literal because `REQUIRED_BUNDLE_INPUTS` is a `const`.
        // Publishing a new protocol major renames the file it points at, and a
        // required input naming a file nothing writes would refuse every bundle
        // in the project — so the two spellings are pinned to each other here
        // rather than left to be noticed on the day the major moves.
        let published = crate::PUBLISHED_SCHEMAS
            .iter()
            .find(|schema| schema.stem == "worker-protocol")
            .expect("the worker protocol schema is published");

        assert_eq!(
            WORKER_PROTOCOL_SCHEMA_PATH,
            format!("{}/{}", crate::SCHEMA_DIRECTORY, published.file_name())
        );
    }

    pub(crate) fn write_manifest(root: &TempDir, manifest: &BundleManifest) {
        fs::write(
            root.path().join(BUNDLE_MANIFEST_PATH),
            serde_json::to_vec_pretty(manifest).expect("a manifest serializes"),
        )
        .expect("the manifest is writable");
    }
}
