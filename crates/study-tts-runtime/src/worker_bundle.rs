//! The deterministic identity of the executable worker bundle.
//!
//! ADR-0001 §12.5 makes this hash a synthesis-key input and is specific about
//! how it must be produced: "computed deterministically from production worker
//! source and imported project-owned modules, the production Python lockfile,
//! the worker protocol schema, launcher configuration that affects inference,
//! and Python runtime and platform ABI identity." §22 adds the constraint that
//! matters most: "Derive the worker-bundle hash mechanically; do not depend on
//! a human-managed revision marker."
//!
//! Neither a directory walk nor a hand-written list satisfies that on its own,
//! and this module uses neither alone:
//!
//! - **A walk would hash the wrong things.** Editor backups, `__pycache__`,
//!   virtualenvs, and untracked scratch files all live beside the worker, and
//!   every one of them would invalidate a cache that has nothing to do with
//!   them.
//! - **A list supplied by the caller proves nothing.** A caller that names five
//!   files gets the hash of five files, whether or not those are the files the
//!   worker loads. Adding an imported module and forgetting to declare it would
//!   leave the hash unchanged while the worker's behavior changed — which is
//!   precisely the silent cache poisoning the hash exists to prevent.
//!
//! So the inputs are declared by the **worker itself**, in
//! `worker/bundle-manifest.json`, and the declaration is then *checked against
//! the tree*: every `.py` file beneath a declared import root must itself be
//! declared, or [`WorkerBundle::verified_hash`] refuses. The manifest says what
//! the bundle is; the walk of the directories it claims says whether that claim
//! is complete.
//!
//! A declaration that decides its own scope decides its own identity, so two
//! things are taken out of the manifest's hands and put in this file's. The
//! floor is Rust-owned: [`REQUIRED_BUNDLE_INPUTS`] and [`REQUIRED_IMPORT_ROOT`]
//! are the inputs ADR-0001 §12.5 names one by one — the manifest, the lockfile,
//! the launcher, the protocol schema, the package root, and the entrypoint —
//! and a manifest that omits any of them is refused before a byte is hashed.
//! Without that floor an empty `import_roots` switched the completeness check
//! off and a dropped `worker/launcher.json` left an inference-affecting file
//! outside a hash that still looked well formed. The interpreter is the other:
//! [`WorkerBundle::verified_hash`] takes no interpreter argument and resolves
//! [`WORKER_INTERPRETER_PATH`] beneath the bundle root, because an interpreter
//! chosen by the caller can satisfy the ABI check without ever running the
//! worker.
//!
//! That the manifest declares *itself* among its inputs is what keeps the
//! second half honest. The check's own scope — `import_roots` — is a manifest
//! field, so narrowing it to stop the walk reaching a new module rewrites the
//! manifest, and the manifest's bytes are hashed like every other input. There
//! is no edit that shrinks the check and leaves the identity where it was.
//!
//! The check is over the directory rather than over the `import` statements
//! because the identity must not rest on *how* a module is reached.
//! `importlib`, `__import__`, and a parenthesized multi-line
//! `from ... import (...)` all load a file that a line-oriented scan of the
//! source does not see, and each would leave the hash sitting still while the
//! worker's behavior changed. What a caller controls is which directories the
//! manifest claims, so that is what is checked. A module the worker never
//! loads has to be declared too: an undeclared file inside the package is
//! either dead code or a dynamic import,
//! and hashing it costs a rebuilt cache while missing it costs a wrong one.
//!
//! The declared runtime ABI gets the same treatment, for the same reason. It is
//! the one bundle input that is not a file, so nothing in the tree can witness
//! it: [`WorkerBundle::verified_hash`] asks the interpreter that would run the
//! worker what it is, and refuses when it disagrees with the manifest. Without
//! that, the same bundle carried to another interpreter patch version or
//! platform would keep its identity while loading different compiled wheels.
//!
//! Third-party code is carried by `worker/requirements.lock`, and that file is
//! checked the same way rather than trusted. Its bytes reach the digest like
//! every other declared input, which proves what the lock *says* and nothing
//! about the environment it describes: a `torch` upgraded in place, or a
//! `chatterbox-tts` the configured index satisfied at the same version, left
//! every declared input byte-identical and every cache key where it was while
//! the audio changed. So [`WorkerBundle::verified_hash`] asks the same
//! interpreter what is installed beside it and compares that against the lock,
//! including the PEP 610 provenance the lock records for the one distribution
//! that must come from the governed source tree.
//!
//! What the walk cannot see is stated rather than implied: a module the worker
//! loads from outside every declared import root is outside the identity. That
//! is a bounded gap, not a hidden one — `docs/operations/WORKER-ENVIRONMENT.md`
//! records the rule that the worker's own modules live under a declared root.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use study_tts_core::{CanonicalValue, WorkerBundleHash, canonical_digest};

use crate::error::{
    EnvironmentMismatch, RuntimeIdentityMismatch, WorkerBundleError, WorkerLockfileErrorReason,
    WorkerLockfileLocus,
};
use crate::process::{self, CommandRunError, VERSION_PROBE_POLICY};
use crate::{
    BuildError, ManagedPathError, ToolInvocation, ToolOperation, io_error, managed, tools,
};

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

/// Layout version this build accepts for a bundle manifest.
pub const BUNDLE_MANIFEST_SCHEMA_VERSION: &str = "1.0";

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

/// Path of the interpreter that runs the worker, relative to the repository
/// root.
///
/// Fixed rather than configurable, for the reason [`BUNDLE_MANIFEST_PATH`] is:
/// an interpreter chosen by whoever calls this code would let the identity
/// check be satisfied by an interpreter that never runs the worker.
/// `docs/operations/WORKER-ENVIRONMENT.md` restores the environment to exactly
/// this path and names this constant in return.
pub const WORKER_INTERPRETER_PATH: &str = "worker/.venv/bin/python";

/// The two questions this build asks the interpreter about its environment.
///
/// **What it is.** The wheel tags come from `packaging`, which
/// `worker/requirements.lock` pins, rather than from tag rules restated here: a
/// restatement would have to reimplement glibc detection and would disagree
/// with the environment it is meant to describe on exactly the platforms that
/// are hardest to test.
///
/// The *platform* tag is not simply the first one `packaging` yields, and that
/// distinction is load-bearing. `platform_tags()` is ordered by pip's
/// preference, and on Linux it opens with the bare `linux_<arch>` — the tag of
/// a wheel built for this machine and portable nowhere. That string is
/// identical on glibc 2.31 and glibc 2.39, so recording it would leave two
/// environments that load different compiled wheels sharing one bundle
/// identity, which is the failure this probe exists to prevent. ADR-0001 §12.5
/// hashes "platform ABI identity" and
/// `docs/operations/REFERENCE-ENVIRONMENT.md` records the reference machine's
/// glibc as part of what is qualified, so what is recorded is the first tag
/// carrying a platform-ABI version: `manylinux_*` or `musllinux_*` on Linux,
/// and the leading `macosx_*` elsewhere. A Linux environment whose ABI cannot
/// be detected yields only the bare tag, is recorded as such, and therefore
/// fails to match a manifest declaring a real one — which is the correct
/// direction to fail in.
///
/// **What is installed in it.** The ABI answers which wheels *could* load, not
/// which are there. `worker/requirements.lock` is hashed into the bundle
/// identity as bytes, so before this the hash proved the lockfile's contents
/// and nothing about the environment those contents describe — a bundle whose
/// `torch` had been upgraded in place kept its identity while producing
/// different audio. Names are canonicalized by `packaging.utils`, which is the
/// rule `pip` matches requirements with, rather than by a lowercase here that
/// would disagree with it on `HF-Xet` and `hf_xet`.
///
/// PEP 610 answers the second half, and it answers it with a *commit* rather
/// than a path. `pip` writes `direct_url.json` into the `.dist-info` of
/// anything installed from a local path or a URL and writes no such file for an
/// index install; when the install came from a VCS it records `vcs_info`
/// carrying the `commit_id` it actually checked out. A directory install
/// records only `dir_info`, and a directory name is not a revision: the tree at
/// `code-<commit>` can hold any bytes at all, and `code-<commit>-backup` beside
/// it is a name an operator really creates. So the governed distribution is
/// installed from the governed tree's git URL at its commit, and the recorded
/// `commit_id` is what is compared. The URL is never reported at all — the
/// rights and data policy keeps a governed model root out of logs, so it does
/// not leave the interpreter, and the refusal names the commit to reinstall
/// from instead.
///
/// **What is installed is not only distributions.** A `.pth` file in a site
/// directory runs at interpreter startup — every line beginning `import`
/// executes, and every other line joins `sys.path` ahead of the search that
/// resolves `torch`. Extra distributions are tolerated here (see
/// [`WorkerBundle::check_environment_matches_lock`]), so without this the
/// tolerated ones could bring arbitrary startup code into the process the
/// bundle identity claims to describe. Each `.pth` is therefore reported with
/// the distribution that owns it, from that distribution's own `RECORD`, and a
/// hook the lock does not account for is a refusal.
///
/// Run under `-I`: isolated mode ignores `PYTHONPATH` and the user site
/// directory, so a shadowing `packaging` on the environment cannot dictate the
/// identity this build records — and so the site directories reported below are
/// the ones the worker itself will process.
const RUNTIME_PROBE_SCRIPT: &str = concat!(
    // `concat!` rather than a `\`-continued string literal, and that is not a
    // style choice: `\` at the end of a line skips the newline *and every
    // leading space on the next one*, so an indented Python block silently
    // arrives unindented. This script grew a `for` body and shipped an
    // `IndentationError` that only a restored environment could see.
    "import json, os, site, sys\n",
    "from importlib.metadata import distributions\n",
    "from packaging.tags import platform_tags, sys_tags\n",
    "from packaging.utils import canonicalize_name\n",
    "tag = next(iter(sys_tags()))\n",
    // The bare `linux_<arch>` carries no ABI version, so it is skipped in
    // favour of the `manylinux`/`musllinux` tag behind it. Kept as the
    // fallback rather than refused here: an environment with no detectable
    // platform ABI is one the manifest comparison should reject by name.
    "platforms = list(platform_tags())\n",
    "abi_platform = next(\n",
    "    (name for name in platforms if not name.startswith('linux_')),\n",
    "    platforms[0],\n",
    ")\n",
    "installed = []\n",
    "owners = {}\n",
    "for dist in distributions():\n",
    "    name = dist.metadata['Name']\n",
    "    if not name:\n",
    "        continue\n",
    "    canonical = canonicalize_name(name)\n",
    "    record = dist.read_text('direct_url.json')\n",
    "    source = json.loads(record) if record else {}\n",
    // A list rather than a map keyed by the canonical name: two distributions
    // that canonicalize alike are a broken environment, and a map would drop
    // one of them silently. Rust builds the map and refuses the collision.
    "    installed.append({\n",
    "        'name': canonical,\n",
    "        'version': dist.version,\n",
    "        'recorded_source': record is not None,\n",
    "        'commit': source.get('vcs_info', {}).get('commit_id'),\n",
    "    })\n",
    "    for entry in dist.files or ():\n",
    "        hook = os.path.basename(str(entry))\n",
    "        if not hook.endswith('.pth'):\n",
    "            continue\n",
    // Two distributions claiming one hook file leave it unowned rather than
    // attributed to whichever was walked last, so the ambiguity is refused.
    "        owners[hook] = None if hook in owners else canonical\n",
    "hooks = []\n",
    "for directory in site.getsitepackages():\n",
    "    if not os.path.isdir(directory):\n",
    "        continue\n",
    "    for entry in sorted(os.listdir(directory)):\n",
    "        if entry.endswith('.pth'):\n",
    "            hooks.append({'file': entry, 'owner': owners.get(entry)})\n",
    "json.dump({\n",
    "    'runtime': {\n",
    "        'implementation': sys.implementation.name,\n",
    "        'version': '.'.join(str(part) for part in sys.version_info[:3]),\n",
    "        'abi_tag': tag.abi,\n",
    "        'platform_tag': abi_platform,\n",
    "    },\n",
    "    'distributions': installed,\n",
    "    'path_hooks': hooks,\n",
    "}, sys.stdout)\n",
);

/// The comment `worker/requirements.lock` binds a governed source tree with.
///
/// The lock records the resolved *set*, not where each distribution came from,
/// so a pin alone cannot say that `chatterbox-tts` must not be satisfied by an
/// index. This comment is where the lock says it, and
/// `docs/operations/WORKER-ENVIRONMENT.md` §Verify the provenance names this
/// constant in return. The commit follows it on the same line, and the pin it
/// governs is the line after.
const GOVERNED_SOURCE_MARKER: &str = "# installed from a governed local source tree at commit ";

/// Resolution directives `worker/requirements.lock` must state for itself.
///
/// A pin says which version, never which index served it or whether a wheel or
/// a source tree was built. Left to installer configuration, the same lock
/// resolves differently on two machines while every pin and every artifact hash
/// still reads as satisfied. Requiring the four here moves that decision into
/// the bytes the bundle identity hashes.
///
/// A directive outside this list is refused rather than ignored: it would
/// change resolution in a way this build has not reasoned about.
/// `docs/operations/WORKER-ENVIRONMENT.md` §Regenerating the lock names this
/// constant in return.
const REQUIRED_LOCK_DIRECTIVES: [&str; 4] = [
    "--index-url https://pypi.org/simple",
    "--extra-index-url https://download.pytorch.org/whl/cpu",
    "--only-binary=:all:",
    "--no-binary=s3tokenizer",
];

/// Spelling `pip` gives the digest that binds a pin to its artifact bytes.
const ARTIFACT_HASH_PREFIX: &str = "--hash=sha256:";

/// Length of the SHA-256 an artifact hash carries, as lowercase hexadecimal.
const ARTIFACT_HASH_HEX_LENGTH: usize = 64;

/// The one distribution this lock may satisfy from outside an index.
///
/// Naming it here is what makes the marker above load-bearing rather than
/// advisory. A claim nothing requires is a claim that can be dropped, and
/// dropping this one leaves `governed_commit` at `None`, which
/// [`WorkerBundle::check_environment_matches_lock`] reads as "no provenance to
/// check" — the index install the marker exists to refuse. So the pairing is
/// required in both directions: this pin without a marker is a lock that has
/// lost its provenance, and a marker on any other pin is a claim about
/// something it was not about.
const GOVERNED_DISTRIBUTION_NAME: &str = "chatterbox-tts";

/// Name this build reports the worker interpreter under in a tool refusal.
const INTERPRETER_TOOL: &str = "worker interpreter";

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

/// What one interpreter reports about itself and about what is installed
/// beside it.
///
/// Nested rather than flat so [`PythonRuntimeIdentity`] stays exactly the shape
/// `worker/bundle-manifest.json` declares: the manifest's `python` field is
/// compared against `runtime` field for field, and a probe answer that had
/// grown a fifth key would either break that comparison or have to be trusted
/// to ignore it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct RuntimeProbe {
    /// Interpreter and platform ABI, as the interpreter reports them.
    runtime: PythonRuntimeIdentity,
    /// Every distribution installed in that interpreter's environment.
    ///
    /// A list rather than a map keyed by the canonicalized name, because the
    /// canonicalization is many-to-one: `zope.interface` and `zope-interface`
    /// are one key and two installs. A map would keep whichever the probe
    /// walked last and report the other as absent, so the collision is
    /// [`EnvironmentMismatch::AmbiguousDistribution`] rather than a silent
    /// choice.
    distributions: Vec<InstalledDistribution>,
    /// Every `.pth` file in the interpreter's site directories.
    path_hooks: Vec<PathHook>,
}

/// One installed distribution, as `importlib.metadata` reports it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct InstalledDistribution {
    /// Canonicalized name, as `packaging.utils.canonicalize_name` produces it.
    name: String,
    /// Version recorded in the distribution's metadata.
    version: String,
    /// Whether a PEP 610 `direct_url.json` exists at all.
    ///
    /// False is the index's signature: `pip` writes no such file for an index
    /// install. The record's `url` is deliberately not reported — it can name
    /// the governed model root, which
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps out of logs.
    recorded_source: bool,
    /// PEP 610 `vcs_info.commit_id`: the revision `pip` actually checked out.
    ///
    /// Absent for an index install and for a directory install, which records
    /// only `dir_info`. The second is the case worth naming: a directory
    /// install proves a path and a path is not a revision, so a governed
    /// distribution without this is refused as surely as one from an index.
    commit: Option<String>,
}

/// One `.pth` file found in a site directory, and the distribution owning it.
///
/// A `.pth` is executable configuration: Python runs every line beginning
/// `import` as the interpreter starts, and puts every other line on `sys.path`
/// ahead of the search that resolves the locked distributions. It is therefore
/// behavior inside the process the bundle identity describes, arriving through
/// a file no declared input covers.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct PathHook {
    /// File name within the site directory, never the directory itself.
    file: String,
    /// Canonicalized name of the distribution whose `RECORD` lists this file.
    ///
    /// `None` when no installed distribution claims it, and when more than one
    /// does — an ownerless hook is a hand-dropped file and an ambiguous one
    /// cannot be attributed, and neither is accounted for by the lock.
    owner: Option<String>,
}

/// One distribution `worker/requirements.lock` pins.
#[derive(Debug, Eq, PartialEq)]
struct LockedDistribution {
    /// Canonicalized name, as `packaging.utils.canonicalize_name` produces it.
    name: String,
    /// The exact version the lock pins.
    version: String,
    /// Commit of the governed source tree the lock says this came from.
    ///
    /// `None` for everything the index supplies, which is everything but
    /// `chatterbox-tts`. Provenance is checked only where the lock declares
    /// it: an operator restoring from a local wheelhouse writes a
    /// `direct_url.json` for every distribution, and refusing that would be a
    /// stricter rule than the lock states.
    governed_commit: Option<String>,
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
        if version.schema_version != BUNDLE_MANIFEST_SCHEMA_VERSION {
            return Err(WorkerBundleError::UnsupportedBundleManifest {
                path: resolved,
                declared: version.schema_version,
                required: BUNDLE_MANIFEST_SCHEMA_VERSION,
            }
            .into());
        }

        let manifest: BundleManifest = serde_json::from_slice(&bytes).map_err(|source| {
            WorkerBundleError::UnreadableBundleManifest {
                path: resolved.clone(),
                source,
            }
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
    /// [`crate::ToolError::MissingTool`] when nothing executable is at
    /// [`WORKER_INTERPRETER_PATH`], and whatever `WorkerBundle::hash` reports
    /// once the runtime agrees.
    pub fn verified_hash(&self) -> Result<WorkerBundleHash, BuildError> {
        let interpreter = self.root.join(WORKER_INTERPRETER_PATH);
        let probe = probe_runtime(&interpreter)?;
        if probe.runtime != self.manifest.python {
            return Err(WorkerBundleError::RuntimeIdentityMismatch {
                mismatch: Box::new(RuntimeIdentityMismatch {
                    manifest: PathBuf::from(BUNDLE_MANIFEST_PATH),
                    interpreter,
                    declared: self.manifest.python.clone(),
                    observed: probe.runtime,
                }),
            }
            .into());
        }
        self.check_environment_matches_lock(&probe)?;
        self.hash()
    }

    /// Refuses an environment that is not the one `worker/requirements.lock`
    /// describes.
    ///
    /// The gap this closes is the one the ABI check leaves open. The lockfile
    /// reaches the bundle identity as *bytes*, so the hash proved what the file
    /// says and nothing about the environment beside it: a `torch` upgraded in
    /// place, or a `chatterbox-tts` the configured index satisfied at the same
    /// version, left every declared input byte-identical and every cache key
    /// where it was while the audio changed. ADR-0001 §22 forbids depending on
    /// a human-managed marker, and a lockfile nothing compares against is one.
    ///
    /// Extra *distributions* are ignored rather than refused. The reference
    /// machine's qualification virtualenv also carries the repository's
    /// pre-commit tooling, which `docs/operations/WORKER-ENVIRONMENT.md`
    /// §Regenerating the lock removes from the lock precisely because the
    /// worker does not load it — refusing it here would contradict that
    /// document and push an operator toward a second environment.
    ///
    /// Their `.pth` files are not ignored, and that is what makes the tolerance
    /// affordable. A `.pth` runs at interpreter startup whether or not anything
    /// imports its distribution, so an extra install is inert only until it
    /// ships one. Every hook must be owned by a distribution the lock pins;
    /// the pre-commit tooling ships none, so the documented shared environment
    /// still passes while a hook that could change what the worker loads does
    /// not.
    ///
    /// # Errors
    ///
    /// [`WorkerBundleError::UnreadableWorkerLockfile`] when a lock line is not
    /// an artifact-bound pin, a required resolution directive is absent, or
    /// the governed pin and its provenance marker have come apart, and
    /// [`WorkerBundleError::EnvironmentDoesNotMatchLock`]
    /// carrying the [`EnvironmentMismatch`] variant for the first fault found:
    /// [`EnvironmentMismatch::AmbiguousDistribution`] when two installs share
    /// a canonicalized name, then, per locked pin,
    /// [`EnvironmentMismatch::Absent`],
    /// [`EnvironmentMismatch::Version`],
    /// [`EnvironmentMismatch::FromIndex`],
    /// [`EnvironmentMismatch::WithoutRecordedRevision`], or
    /// [`EnvironmentMismatch::FromAnotherRevision`], then
    /// [`EnvironmentMismatch::UnownedPathHook`] or
    /// [`EnvironmentMismatch::UnlockedPathHook`] for a startup hook the lock
    /// does not account for.
    fn check_environment_matches_lock(&self, probe: &RuntimeProbe) -> Result<(), BuildError> {
        let resolved = self.resolve(WORKER_LOCKFILE_PATH)?;
        let bytes = self.read_bounded(WORKER_LOCKFILE_PATH, &resolved)?;
        let lockfile = String::from_utf8(bytes).map_err(|_| {
            unreadable_lockfile(
                WorkerLockfileLocus::WholeFile,
                WorkerLockfileErrorReason::InvalidUtf8,
            )
        })?;

        let installed = installed_by_name(&probe.distributions)?;
        let pins = parse_lockfile(&lockfile)?;

        for locked in &pins {
            let Some(found) = installed.get(locked.name.as_str()) else {
                return Err(mismatch(EnvironmentMismatch::Absent {
                    distribution: locked.name.clone(),
                    required: locked.version.clone(),
                }));
            };
            if found.version != locked.version {
                return Err(mismatch(EnvironmentMismatch::Version {
                    distribution: locked.name.clone(),
                    required: locked.version.clone(),
                    installed: found.version.clone(),
                }));
            }
            let Some(commit) = &locked.governed_commit else {
                continue;
            };
            // PEP 610 records `vcs_info.commit_id` only for a VCS install, and
            // that is the whole point of asking for one:
            // `docs/operations/WORKER-ENVIRONMENT.md` §Install the governed
            // Chatterbox source installs from the governed tree's git URL at
            // this commit, so the revision is `pip`'s observation rather than a
            // directory name an operator chose. A directory install records a
            // path and proves nothing about the bytes beneath it.
            let Some(recorded) = &found.commit else {
                return Err(mismatch(if found.recorded_source {
                    EnvironmentMismatch::WithoutRecordedRevision {
                        distribution: locked.name.clone(),
                        commit: commit.clone(),
                    }
                } else {
                    EnvironmentMismatch::FromIndex {
                        distribution: locked.name.clone(),
                        commit: commit.clone(),
                    }
                }));
            };
            if recorded != commit {
                return Err(mismatch(EnvironmentMismatch::FromAnotherRevision {
                    distribution: locked.name.clone(),
                    commit: commit.clone(),
                }));
            }
        }

        check_path_hooks_are_pinned(&pins, &probe.path_hooks)
    }

    /// Derives the bundle's identity from what the manifest declares.
    ///
    /// Hashes every declared input's bytes together with the declared runtime
    /// ABI, but only after proving the declaration is complete: see
    /// [`WorkerBundle::check_import_roots_declared`].
    ///
    /// Private because the declared runtime ABI is a claim until an interpreter
    /// is asked. [`WorkerBundle::verified_hash`] asks first, and is the only
    /// way out of this module.
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
    fn hash(&self) -> Result<WorkerBundleHash, BuildError> {
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

/// Indexes the reported distributions by their canonicalized name.
///
/// # Errors
///
/// [`WorkerBundleError::EnvironmentDoesNotMatchLock`] carrying
/// [`EnvironmentMismatch::AmbiguousDistribution`] when two installs share one
/// name. PEP 503 canonicalization is many-to-one, so this is the map the probe
/// deliberately does not build: it would keep whichever install was walked last
/// and report the other as absent.
fn installed_by_name(
    distributions: &[InstalledDistribution],
) -> Result<BTreeMap<&str, &InstalledDistribution>, BuildError> {
    let mut installed = BTreeMap::new();
    for distribution in distributions {
        if installed
            .insert(distribution.name.as_str(), distribution)
            .is_some()
        {
            return Err(mismatch(EnvironmentMismatch::AmbiguousDistribution {
                distribution: distribution.name.clone(),
            }));
        }
    }
    Ok(installed)
}

/// Refuses a `.pth` file the lockfile does not account for.
///
/// Ownership rather than contents, because that is what the lock can speak to:
/// a hook belonging to a pinned distribution is fixed by that pin like every
/// other file the distribution installs, and a hook belonging to anything else
/// is startup code inside the worker's process that nothing in the bundle
/// identity describes.
///
/// # Errors
///
/// [`WorkerBundleError::EnvironmentDoesNotMatchLock`] carrying
/// [`EnvironmentMismatch::UnownedPathHook`] when no installed distribution
/// claims the file, and [`EnvironmentMismatch::UnlockedPathHook`] when the one
/// that does is not pinned.
fn check_path_hooks_are_pinned(
    pins: &[LockedDistribution],
    hooks: &[PathHook],
) -> Result<(), BuildError> {
    let pinned: BTreeSet<&str> = pins.iter().map(|pin| pin.name.as_str()).collect();

    for hook in hooks {
        let Some(owner) = &hook.owner else {
            return Err(mismatch(EnvironmentMismatch::UnownedPathHook {
                file: hook.file.clone(),
            }));
        };
        if !pinned.contains(owner.as_str()) {
            return Err(mismatch(EnvironmentMismatch::UnlockedPathHook {
                file: hook.file.clone(),
                owner: owner.clone(),
            }));
        }
    }
    Ok(())
}

/// Reads `worker/requirements.lock` as the artifact-bound set of pins it
/// declares.
///
/// Refuses an unknown directive or an index pin without exactly one SHA-256
/// digest rather than skipping it. A version without its artifact is still
/// resolver input, not a lock; an implicit source can select different bytes
/// while every visible pin remains unchanged.
///
/// Refuses, for the same reason, any lock in which the
/// [`GOVERNED_SOURCE_MARKER`] and the [`GOVERNED_DISTRIBUTION_NAME`] pin have
/// come apart: a missing, displaced, doubled, or misattached marker. A pin
/// whose provenance is absent is a pin whose provenance is not compared, so a
/// lock that has lost the pairing would pass while admitting the index install
/// the marker exists to refuse.
///
/// The refusal names the *locus* and not the line: the line number where one
/// line is at fault, and the file where the invariant is the whole file's to
/// keep. A wrongly regenerated lock is exactly where a
/// `chatterbox-tts @ file:///...` line appears, and
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a governed model root
/// out of logs.
fn parse_lockfile(lockfile: &str) -> Result<Vec<LockedDistribution>, BuildError> {
    let mut pins = Vec::new();
    let mut pending_commit: Option<(usize, String)> = None;
    let mut governed_pin_seen = false;
    let mut directives_seen = [false; REQUIRED_LOCK_DIRECTIVES.len()];

    for (index, line) in lockfile.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();

        if let Some(commit) = line.strip_prefix(GOVERNED_SOURCE_MARKER) {
            // A second marker before any pin leaves two provenance claims and
            // one distribution to attach them to.
            if pending_commit.is_some() {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidProvenance,
                ));
            }
            pending_commit = Some((line_number, commit.trim().to_owned()));
            continue;
        }

        if line.is_empty() || line.starts_with('#') {
            // Bound to the *next* line, and to that one only. Dropping the
            // claim here instead — which is what this did — turned a marker an
            // edit had displaced into a governed pin with no provenance, and a
            // pin with no provenance is not checked at all.
            if pending_commit.is_some() {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidProvenance,
                ));
            }
            continue;
        }

        if line.starts_with("--") {
            // A directive between a marker and its pin would separate the two.
            if pending_commit.is_some() {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidProvenance,
                ));
            }
            let Some(position) = REQUIRED_LOCK_DIRECTIVES.iter().position(|&it| it == line) else {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::UnsupportedDirective,
                ));
            };
            // Repeating one is not harmless: for the two index directives the
            // last occurrence is the one that resolves, so a duplicate is a
            // second answer to a question the lock must answer once.
            if directives_seen[position] {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::DuplicateRequiredDirective,
                ));
            }
            directives_seen[position] = true;
            continue;
        }

        // `line` is trimmed and the blank and comment cases are skipped above,
        // so the first token always exists. Still a refusal rather than an
        // `expect`: no lockfile this parser reads may panic the build.
        let mut tokens = line.split_whitespace();
        let Some(pin) = tokens.next() else {
            return Err(unreadable_lockfile_line(
                line_number,
                WorkerLockfileErrorReason::MalformedPin,
            ));
        };
        let hash = tokens.next();
        let multiple_hashes = tokens.next().is_some();

        let Some((name, version)) = pin.split_once("==") else {
            return Err(unreadable_lockfile_line(
                line_number,
                WorkerLockfileErrorReason::MalformedPin,
            ));
        };

        // Neither is trimmed, and neither needs to be: `pin` stops at the first
        // space, so no whitespace can survive into a name or a version.
        let name_bytes = name.as_bytes();
        if !name_bytes
            .first()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
            || !name_bytes
                .last()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            || !name_bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || version.is_empty()
            || version.contains('=')
        {
            return Err(unreadable_lockfile_line(
                line_number,
                WorkerLockfileErrorReason::MalformedPin,
            ));
        }

        let name = canonicalize_distribution_name(name);
        let governed_commit = pending_commit.take().map(|(_, commit)| commit);
        if name == GOVERNED_DISTRIBUTION_NAME {
            // The governed pin carries no hash, and must not: it is installed
            // from a source tree at a commit rather than from a published
            // artifact, so there are no artifact bytes to bind. Its provenance
            // is `governed_commit`, checked against PEP 610 in
            // `WorkerBundle::check_environment_matches_lock`.
            if governed_pin_seen || governed_commit.is_none() || hash.is_some() {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidProvenance,
                ));
            }
            governed_pin_seen = true;
        } else if governed_commit.is_some() {
            return Err(unreadable_lockfile_line(
                line_number,
                WorkerLockfileErrorReason::InvalidProvenance,
            ));
        } else {
            // Exactly one, so the artifact is named and named once. None leaves
            // the version free to be served by any bytes the index offers under
            // it; two leave the reader no way to tell which was installed.
            //
            // Validated and dropped rather than kept: nothing this build can
            // ask the interpreter reports the hash of the artifact a
            // distribution was installed from, so a stored value would have no
            // reader. The lock's own bytes reach the bundle identity, which is
            // what makes editing a hash change the identity.
            let Some(hash) = hash.filter(|_| !multiple_hashes) else {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidArtifactHash,
                ));
            };
            let Some(digest) = hash.strip_prefix(ARTIFACT_HASH_PREFIX) else {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidArtifactHash,
                ));
            };
            // Lowercase only, for the reason `study_tts_core::is_blake3_hex`
            // gives about its own digests: an uppercase spelling is not what
            // the tool writing this file produces, so accepting it would
            // normalize away the evidence that something else wrote the line.
            if digest.len() != ARTIFACT_HASH_HEX_LENGTH
                || !digest
                    .bytes()
                    .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            {
                return Err(unreadable_lockfile_line(
                    line_number,
                    WorkerLockfileErrorReason::InvalidArtifactHash,
                ));
            }
        }

        pins.push(LockedDistribution {
            name,
            version: version.to_owned(),
            governed_commit,
        });
    }

    if let Some((line_number, _)) = pending_commit {
        return Err(unreadable_lockfile_line(
            line_number,
            WorkerLockfileErrorReason::InvalidProvenance,
        ));
    }
    // Neither fault is a line's: no line was expected to carry the governed
    // pin, and a directive that is absent is absent from the file.
    if !governed_pin_seen {
        return Err(unreadable_lockfile(
            WorkerLockfileLocus::WholeFile,
            WorkerLockfileErrorReason::InvalidProvenance,
        ));
    }
    if directives_seen.contains(&false) {
        return Err(unreadable_lockfile(
            WorkerLockfileLocus::WholeFile,
            WorkerLockfileErrorReason::MissingRequiredDirective,
        ));
    }

    Ok(pins)
}

/// Canonicalizes a distribution name the way PEP 503 defines it.
///
/// `packaging.utils.canonicalize_name` on the probe's side, written out here
/// because the comparison has two ends and only one of them runs Python. The
/// rule is the whole rule — `re.sub(r"[-_.]+", "-", name).lower()` — so
/// `HF-Xet`, `hf_xet`, and `hf.xet` are one distribution, and a lockfile and a
/// `pip freeze` routinely spell it differently.
///
/// A leading or trailing separator is *kept*, collapsed to one `-`, because
/// that is what the Python does. No valid name has one (PEP 508 forbids it),
/// but dropping them made this function disagree with its counterpart on
/// exactly the inputs a mangled lockfile produces — and two ends that
/// canonicalize differently report a distribution as absent rather than as the
/// malformed line it came from.
fn canonicalize_distribution_name(name: &str) -> String {
    let mut canonical = String::with_capacity(name.len());
    let mut previous_was_separator = false;
    for character in name.chars() {
        let is_separator = matches!(character, '-' | '_' | '.');
        if is_separator {
            if !previous_was_separator {
                canonical.push('-');
            }
        } else {
            canonical.extend(character.to_lowercase());
        }
        previous_was_separator = is_separator;
    }
    canonical
}

/// Builds the refusal for a lockfile line this build cannot read as a
/// resolution directive or artifact-bound pin.
fn unreadable_lockfile_line(line: usize, reason: WorkerLockfileErrorReason) -> BuildError {
    unreadable_lockfile(WorkerLockfileLocus::Line(line), reason)
}

/// Builds the refusal for a lockfile invariant that failed at `locus`.
fn unreadable_lockfile(
    locus: WorkerLockfileLocus,
    reason: WorkerLockfileErrorReason,
) -> BuildError {
    WorkerBundleError::UnreadableWorkerLockfile {
        path: PathBuf::from(WORKER_LOCKFILE_PATH),
        locus,
        reason,
    }
    .into()
}

/// Builds the refusal for an environment that is not the locked one.
fn mismatch(mismatch: EnvironmentMismatch) -> BuildError {
    WorkerBundleError::EnvironmentDoesNotMatchLock {
        mismatch: Box::new(mismatch),
    }
    .into()
}

/// Asks one interpreter what runtime and platform ABI it actually is.
///
/// Supervised through [`crate::process::run`] like every other child this
/// build launches, so a hung or chatty interpreter is bounded rather than
/// waited on. [`VERSION_PROBE_POLICY`] is reused deliberately: this is a
/// version discovery probe, and giving it a second deadline would be a second
/// ceiling to keep in step with
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
///
/// # Errors
///
/// [`crate::ToolError::MissingTool`] when nothing executable is at
/// `interpreter`, [`WorkerBundleError::UnreadableRuntimeIdentity`] when it
/// exits nonzero or answers with something other than the declared record, and
/// whatever supervision reports for a probe that overruns its deadline or its
/// output ceiling.
fn probe_runtime(interpreter: &Path) -> Result<RuntimeProbe, BuildError> {
    // Deliberately not `tools::resolve_executable`, which canonicalizes. A
    // virtualenv's `bin/python` is a symlink chain to the base interpreter and
    // `worker/.venv` is itself a link on the reference machine, so resolving
    // either one runs `/usr/bin/python3.12` — a system Python whose
    // `sys.prefix` is `/usr` and which has none of the locked distributions.
    // Every answer then described an environment nobody restored, and the
    // check that exists to read the restored one could not reach it.
    let resolved =
        tools::executable_in_place(interpreter).ok_or_else(|| crate::ToolError::MissingTool {
            tool: INTERPRETER_TOOL.to_owned(),
            requested: interpreter.to_path_buf(),
        })?;

    let mut command = Command::new(&resolved);
    command.args(["-I", "-c", RUNTIME_PROBE_SCRIPT]);
    let invocation = ToolInvocation::new(INTERPRETER_TOOL, ToolOperation::VersionProbe, &resolved);
    let output =
        process::run(invocation, command, VERSION_PROBE_POLICY).map_err(|error| match error {
            CommandRunError::Start(source) => BuildError::from(crate::ToolError::InspectTool {
                tool: INTERPRETER_TOOL.to_owned(),
                executable: resolved.clone(),
                source,
            }),
            CommandRunError::Supervision(error) => error.into(),
        })?;

    if !output.status.success() {
        return Err(unreadable_runtime(
            interpreter,
            &format!(
                "{}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| unreadable_runtime(interpreter, &error.to_string()))
}

/// Builds the refusal for an interpreter whose answer this build cannot read.
///
/// The detail is collapsed to one line because it can carry a Python traceback,
/// and a refusal is read in a terminal beside other refusals.
fn unreadable_runtime(interpreter: &Path, detail: &str) -> BuildError {
    WorkerBundleError::UnreadableRuntimeIdentity {
        interpreter: interpreter.to_path_buf(),
        detail: detail.split_whitespace().collect::<Vec<_>>().join(" "),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::ErrorKind;
    use std::thread;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::*;

    /// One named change to the runtime ABI, for the sensitivity property.
    type RuntimeMutation = (&'static str, fn(&mut PythonRuntimeIdentity));

    /// The repository this test suite is compiled inside.
    ///
    /// The bundle hashed by these tests is the **real** `worker/` package, not
    /// a shaped-like-it fixture: a fixture would agree with any manifest,
    /// including one that omits half the worker.
    fn repository_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A writable copy of the repository's real bundle inputs.
    ///
    /// Copied because these tests mutate inputs to observe the hash move, and a
    /// test that edited the checked-in worker would leave the working tree
    /// dirty when it failed part way through.
    fn bundle_copy() -> TempDir {
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
    fn bundle(root: &TempDir) -> WorkerBundle {
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

    /// How long [`install_executable`] waits for a script it just wrote to
    /// become runnable.
    ///
    /// Generous because the wait is normally zero: it is paid only when
    /// another thread happens to be mid-`fork` at the moment of the write.
    const EXEC_BUSY_TIMEOUT: Duration = Duration::from_secs(5);

    /// How often it retries while the kernel still reports `ETXTBSY`.
    const EXEC_BUSY_POLL: Duration = Duration::from_millis(1);

    /// Writes `script` as an executable and returns once it will actually run.
    ///
    /// The wait is the point. `execve` refuses a file any process still holds
    /// open for writing with `ETXTBSY`, and this test binary is
    /// multi-threaded: another test forking anywhere between this write and
    /// the exec under test inherits the write descriptor for the microseconds
    /// before it execs its own program, and the probe then fails with
    /// [`ErrorKind::ExecutableFileBusy`] instead of the refusal the caller is
    /// asserting. That is a race in the test scaffolding rather than in
    /// `verified_hash`, and it made five of these tests fail intermittently
    /// under `cargo test` while every one of them passed with
    /// `--test-threads=1`.
    ///
    /// Running the script once closes the window for good: nothing reopens
    /// this path for writing afterwards, so a single success proves no
    /// descriptor is left. The scripts installed here only `printf` and exit,
    /// so running one costs nothing and observes nothing.
    fn install_executable(path: &Path, script: String) {
        use std::os::unix::fs::PermissionsExt;

        fs::create_dir_all(path.parent().expect("an executable has a parent"))
            .expect("the executable's directory is creatable");
        fs::write(path, script).expect("the executable is writable");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))
            .expect("the executable is made executable");

        let deadline = Instant::now() + EXEC_BUSY_TIMEOUT;
        loop {
            match Command::new(path).output() {
                Ok(_) => return,
                Err(error) if error.kind() == ErrorKind::ExecutableFileBusy => {
                    assert!(
                        Instant::now() < deadline,
                        "`{}` was still busy after {EXEC_BUSY_TIMEOUT:?}",
                        path.display()
                    );
                    thread::sleep(EXEC_BUSY_POLL);
                }
                Err(error) => {
                    panic!("`{}` was installed but cannot run: {error}", path.display())
                }
            }
        }
    }

    /// Installs an executable at the one path `verified_hash` will look at,
    /// answering the probe with `answer` and nothing else.
    ///
    /// A script rather than a real Python because the property under test is
    /// what this build does with the answer, and a real interpreter would make
    /// the expectation depend on whichever Python the machine happens to have
    /// — which is the coupling `verified_hash` exists to refuse.
    ///
    /// Written to [`WORKER_INTERPRETER_PATH`] rather than to a name of the
    /// test's choosing, because that path is no longer a parameter: a helper
    /// that could put the interpreter anywhere would be testing an argument
    /// this function does not take.
    fn install_interpreter(root: &TempDir, answer: &str) {
        install_executable(
            &root.path().join(WORKER_INTERPRETER_PATH),
            format!("#!/bin/sh\nprintf '%s' '{answer}'\n"),
        );
    }

    /// What an interpreter in the environment the lock describes would answer.
    ///
    /// Built from the bundle's own manifest and its own lockfile rather than
    /// from values written here: the property under test is that a *matching*
    /// environment passes, and a hand-written distribution list would stop
    /// matching the moment somebody re-locked the worker — quietly, because
    /// the answer would still be a well-formed one.
    fn matching_answer(root: &TempDir, bundle: &WorkerBundle) -> String {
        answer(bundle, matching_distributions(root))
    }

    /// The distributions an environment restored exactly to the lock reports.
    ///
    /// Keyed here although the probe reports a list, because every case below
    /// spoils one named distribution; [`answer`] flattens it back.
    fn matching_distributions(root: &TempDir) -> BTreeMap<String, serde_json::Value> {
        let lockfile = fs::read_to_string(root.path().join(WORKER_LOCKFILE_PATH))
            .expect("the copied lockfile is readable");
        parse_lockfile(&lockfile)
            .expect("the checked-in lockfile is a set of exact pins")
            .into_iter()
            .map(|locked| {
                // A governed install comes from the tree's git URL at a commit,
                // so PEP 610 records `vcs_info.commit_id`; an index install
                // writes no record at all.
                let commit = locked.governed_commit.clone();
                (
                    locked.name,
                    serde_json::json!({
                        "version": locked.version,
                        "recorded_source": commit.is_some(),
                        "commit": commit,
                    }),
                )
            })
            .collect()
    }

    /// The keyed distribution set as the probe reports it: a list, each entry
    /// naming itself.
    fn reported(distributions: BTreeMap<String, serde_json::Value>) -> Vec<serde_json::Value> {
        distributions
            .into_iter()
            .map(|(name, mut entry)| {
                entry["name"] = serde_json::Value::String(name);
                entry
            })
            .collect()
    }

    /// One probe answer from an environment carrying no startup hook.
    ///
    /// The distribution cases are all about `worker/requirements.lock`, and an
    /// empty hook list keeps them about that alone.
    fn answer(bundle: &WorkerBundle, distributions: BTreeMap<String, serde_json::Value>) -> String {
        answer_with_path_hooks(bundle, reported(distributions), Vec::new())
    }

    /// One probe answer, from a runtime identity, the distributions as the
    /// probe lists them, and the `.pth` files a site directory holds.
    fn answer_with_path_hooks(
        bundle: &WorkerBundle,
        distributions: Vec<serde_json::Value>,
        path_hooks: Vec<serde_json::Value>,
    ) -> String {
        serde_json::to_string(&serde_json::json!({
            "runtime": bundle.manifest().python,
            "distributions": distributions,
            "path_hooks": path_hooks,
        }))
        .expect("a probe answer serializes")
    }

    /// The one distribution the checked-in lock requires from a governed tree.
    fn governed_distribution(root: &TempDir) -> (String, String) {
        let lockfile = fs::read_to_string(root.path().join(WORKER_LOCKFILE_PATH))
            .expect("the copied lockfile is readable");
        let mut governed = parse_lockfile(&lockfile)
            .expect("the checked-in lockfile is a set of exact pins")
            .into_iter()
            .filter_map(|locked| Some((locked.name, locked.governed_commit?)));
        let only = governed
            .next()
            .expect("the checked-in lockfile records a governed source tree for at least one pin");
        assert!(
            governed.next().is_none(),
            "this helper assumes one governed pin; add a case if the lock grows another"
        );
        only
    }

    #[test]
    fn t1_e1_an_interpreter_matching_the_manifest_hashes_the_bundle() {
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        assert_eq!(
            bundle
                .verified_hash()
                .expect("an agreeing interpreter must not block the hash"),
            bundle.hash().expect("a complete bundle hashes"),
            "verifying the runtime must not change the identity it derives"
        );
    }

    #[test]
    fn t1_e1_the_interpreter_is_probed_where_it_is_attached_not_where_it_resolves() {
        // A virtualenv's `bin/python` is a symlink chain ending at the base
        // interpreter, and `docs/operations/WORKER-ENVIRONMENT.md` §Attach it
        // at the fixed interpreter path makes `worker/.venv` a link as well.
        // Resolving either one runs the system Python, whose `sys.prefix` is
        // `/usr` and which has none of the locked distributions installed — so
        // the check that exists to read the *restored* environment answered
        // for one nobody restored, and reported the bare `linux_x86_64` the
        // manifest does not declare.
        //
        // Every other interpreter here is a regular file, which is why this
        // went unseen: canonicalizing a regular file returns the same file.
        // The stand-in below answers differently depending on the path it was
        // invoked through, which is the observable difference a script can
        // carry and the one a real virtualenv has.
        let root = bundle_copy();
        let bundle = bundle(&root);

        let mut resolved_runtime = bundle.manifest().python.clone();
        resolved_runtime.platform_tag = "linux_x86_64".to_owned();
        let resolved_answer = serde_json::to_string(&serde_json::json!({
            "runtime": resolved_runtime,
            "distributions": reported(matching_distributions(&root)),
            "path_hooks": [],
        }))
        .expect("a probe answer serializes");

        let base = root.path().join("base-interpreter");
        fs::create_dir_all(&base).expect("the base interpreter directory is creatable");
        let base_python = base.join("python");
        // `$0` is the path `Command` was given, so the branch below is the
        // stand-in for a real interpreter reading its own `sys.prefix`.
        let dispatch = format!(
            "#!/bin/sh\n\
             case \"$0\" in\n\
             *.venv*) printf '%s' '{attached}' ;;\n\
             *) printf '%s' '{resolved}' ;;\n\
             esac\n",
            attached = matching_answer(&root, &bundle),
            resolved = resolved_answer,
        );
        install_executable(&base_python, dispatch);

        let attached = root.path().join(WORKER_INTERPRETER_PATH);
        fs::create_dir_all(attached.parent().expect("the interpreter has a parent"))
            .expect("the interpreter directory is creatable");
        std::os::unix::fs::symlink(&base_python, &attached)
            .expect("the attached interpreter is linkable");

        assert_eq!(
            bundle
                .verified_hash()
                .expect("an interpreter attached by link must be probed where it is attached"),
            bundle.hash().expect("a complete bundle hashes"),
            "probing the attached interpreter must not change the identity it derives"
        );
    }

    #[test]
    fn t1_e1_an_interpreter_disagreeing_with_the_manifest_is_refused() {
        // The runtime ABI is the one declared bundle input with no file behind
        // it. Until it is observed, carrying the same bundle to another
        // interpreter patch version or platform keeps its identity while the
        // wheels it loads change.
        let root = bundle_copy();
        let bundle = bundle(&root);
        let changes: [RuntimeMutation; 4] = [
            ("implementation", |runtime| {
                runtime.implementation = "pypy".to_owned();
            }),
            ("version", |runtime| {
                runtime.version = "3.12.4".to_owned();
            }),
            ("abi_tag", |runtime| {
                runtime.abi_tag = "cp313".to_owned();
            }),
            ("platform_tag", |runtime| {
                runtime.platform_tag = "musllinux_1_2_x86_64".to_owned();
            }),
        ];

        for (field, mutate) in changes {
            let mut observed = bundle.manifest().python.clone();
            mutate(&mut observed);
            let probe_answer = serde_json::to_string(&serde_json::json!({
                "runtime": observed,
                "distributions": reported(matching_distributions(&root)),
                "path_hooks": [],
            }))
            .expect("a probe answer serializes");
            install_interpreter(&root, &probe_answer);

            let error = bundle
                .verified_hash()
                .expect_err("an interpreter that is not the declared one must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::RuntimeIdentityMismatch { mismatch }) =
                &error
            else {
                panic!("a differing `{field}` produced the wrong error: {error:?}");
            };
            // Both identities are named, because the operator's next action is
            // to decide which of the two is wrong.
            let message = mismatch.to_string();
            assert!(
                message.contains(&bundle.manifest().python.to_string())
                    && message.contains(&observed.to_string()),
                "a differing `{field}` did not name both identities: `{message}`"
            );
        }
    }

    #[test]
    fn t1_e1_an_interpreter_that_cannot_be_read_is_refused_as_such() {
        // Distinct from a mismatch: the environment is not merely the wrong one
        // but unusable, and the remedies differ.
        let root = bundle_copy();
        let bundle = bundle(&root);

        for (name, reported) in [
            ("silent", ""),
            (
                "not-json",
                "Traceback (most recent call last): ModuleNotFoundError",
            ),
            ("partial", r#"{"runtime":{"implementation":"cpython"}}"#),
            (
                "no-distributions",
                r#"{"runtime":{"implementation":"cpython","version":"3.12.3",
                    "abi_tag":"cp312","platform_tag":"manylinux_2_39_x86_64"}}"#,
            ),
            (
                "extra-field",
                r#"{"runtime":{"implementation":"cpython","version":"3.12.3",
                    "abi_tag":"cp312","platform_tag":"manylinux_2_39_x86_64"},
                    "distributions":{},"extra":true}"#,
            ),
        ] {
            install_interpreter(&root, reported);

            let error = bundle
                .verified_hash()
                .expect_err("an unreadable runtime answer must not hash");

            assert!(
                matches!(
                    error,
                    BuildError::WorkerBundle(WorkerBundleError::UnreadableRuntimeIdentity { .. })
                ),
                "the `{name}` answer produced the wrong error: {error:?}"
            );
        }
    }

    /// The probe script is Python this interpreter compiles.
    ///
    /// T4 rather than T1 despite living beside the code: it needs a Python on
    /// `PATH`, and the claim genuinely requires one — the artifact under test
    /// is Python source, and `crates/study-tts-runtime` has no other way to
    /// find out whether it parses. `manifest::tests` already carries colocated
    /// T4 tests for the same reason.
    ///
    /// The gap this closes is one that shipped. `verified_hash` is the only
    /// caller, every test of it drives a scripted stand-in interpreter, and the
    /// real one runs on a machine ordinary CI does not have — so a script that
    /// did not parse passed every test in this file. Compiled rather than run,
    /// because running it needs the locked environment and parsing it does not.
    #[test]
    fn t4_e1_the_runtime_probe_script_compiles_as_python() {
        let mut command = Command::new("python3");
        command
            .arg("-c")
            .arg("import sys; compile(sys.stdin.read(), '<probe>', 'exec')")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command
            .spawn()
            .expect("`python3` is on PATH; CI runs the worker suite with it");
        std::io::Write::write_all(
            &mut child.stdin.take().expect("the compiler takes stdin"),
            RUNTIME_PROBE_SCRIPT.as_bytes(),
        )
        .expect("the probe script is writable to the compiler");
        let output = child.wait_with_output().expect("the compiler exits");

        assert!(
            output.status.success(),
            "the runtime probe script does not parse:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn t1_e1_an_environment_that_is_not_the_locked_one_is_refused() {
        // The hole the ABI check leaves open. `worker/requirements.lock`
        // reaches the identity as bytes, so before this the hash proved what
        // that file says and nothing about the environment beside it: each
        // case below leaves every declared input byte-identical and every
        // cache key exactly where it was, while the audio changes.
        //
        // Driven from the checked-in lock rather than from a fixture, because
        // what is under test is the comparison against the real one.
        let root = bundle_copy();
        let bundle = bundle(&root);
        let (governed, commit) = governed_distribution(&root);
        let sample = matching_distributions(&root)
            .keys()
            .find(|name| **name != governed)
            .cloned()
            .expect("the lock pins more than the governed distribution");

        let upgraded_in_place = |mut installed: BTreeMap<String, serde_json::Value>| {
            installed.insert(
                sample.clone(),
                serde_json::json!({
                    "version": "0.0.0-not-locked",
                    "recorded_source": false,
                    "commit": null,
                }),
            );
            installed
        };
        let uninstalled = |mut installed: BTreeMap<String, serde_json::Value>| {
            installed.remove(&sample);
            installed
        };
        // What the document warns about by name: installing the lockfile whole
        // lets the index satisfy the pin, and the governed install that follows
        // finds the requirement already satisfied and does nothing.
        let from_index = |mut installed: BTreeMap<String, serde_json::Value>| {
            let found = installed
                .get_mut(&governed)
                .expect("the governed distribution is installed");
            found["recorded_source"] = serde_json::Value::Bool(false);
            found["commit"] = serde_json::Value::Null;
            installed
        };
        const OTHER_REVISION: &str = "0000000000000000000000000000000000000000";
        assert_ne!(
            commit, OTHER_REVISION,
            "the stand-in revision must differ from the one the lock records"
        );
        let from_another_revision = |mut installed: BTreeMap<String, serde_json::Value>| {
            installed
                .get_mut(&governed)
                .expect("the governed distribution is installed")["commit"] =
                serde_json::Value::String(OTHER_REVISION.to_owned());
            installed
        };

        // The case no directory name could ever have caught: a PEP 610 record
        // exists, so the install did not come from an index, and it records
        // `dir_info` rather than a revision. `code-<commit>` names a directory
        // and a directory holds whatever was put in it.
        let from_a_path = |mut installed: BTreeMap<String, serde_json::Value>| {
            installed
                .get_mut(&governed)
                .expect("the governed distribution is installed")["commit"] =
                serde_json::Value::Null;
            installed
        };

        type Spoil<'a> =
            &'a dyn Fn(BTreeMap<String, serde_json::Value>) -> BTreeMap<String, serde_json::Value>;
        let cases: [(&str, Spoil<'_>); 5] = [
            ("absent", &uninstalled),
            ("version", &upgraded_in_place),
            ("from-index", &from_index),
            ("from-another-revision", &from_another_revision),
            ("without-recorded-revision", &from_a_path),
        ];

        for (expected, spoil) in cases {
            install_interpreter(
                &root,
                &answer(&bundle, spoil(matching_distributions(&root))),
            );

            let error = bundle
                .verified_hash()
                .expect_err("an environment that is not the locked one must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::EnvironmentDoesNotMatchLock {
                mismatch,
            }) = &error
            else {
                panic!("the `{expected}` case produced the wrong error: {error:?}");
            };
            let named = match mismatch.as_ref() {
                EnvironmentMismatch::Absent { .. } => "absent",
                EnvironmentMismatch::Version { .. } => "version",
                EnvironmentMismatch::FromIndex { .. } => "from-index",
                EnvironmentMismatch::WithoutRecordedRevision { .. } => "without-recorded-revision",
                EnvironmentMismatch::FromAnotherRevision { .. } => "from-another-revision",
                EnvironmentMismatch::UnownedPathHook { .. } => "unowned-path-hook",
                EnvironmentMismatch::UnlockedPathHook { .. } => "unlocked-path-hook",
                EnvironmentMismatch::AmbiguousDistribution { .. } => "ambiguous",
            };
            assert_eq!(named, expected, "wrong fault reported: {mismatch}");

            // The governed model root never reaches a message.
            // `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps it out of
            // logs, and the refusal names the commit to reinstall from instead.
            let message = error.to_string();
            assert!(
                !message.contains("file:///"),
                "a refusal must not print a recorded URL: {message}"
            );
        }

        // And the environment restored exactly to the lock still hashes, so the
        // cases above fail for the reason they name rather than because the
        // check refuses everything.
        install_interpreter(&root, &matching_answer(&root, &bundle));
        assert_eq!(
            bundle
                .verified_hash()
                .expect("an environment matching the lock must hash"),
            bundle.hash().expect("a complete bundle hashes"),
            "checking the environment must not change the identity it derives"
        );

        // A distribution the lock does not name is ignored rather than
        // refused. `docs/operations/WORKER-ENVIRONMENT.md` §Regenerating the
        // lock removes this repository's pre-commit tooling from the lock
        // precisely because the worker does not load it, and the qualification
        // virtualenv still carries it — so tightening this would break the
        // reference machine while every test here went on passing. What keeps
        // that tolerance affordable is the startup-hook rule below: an extra
        // install is inert only until it ships a `.pth`.
        let mut with_extras = matching_distributions(&root);
        with_extras.insert(
            "pre-commit".to_owned(),
            serde_json::json!({"version": "4.0.1", "recorded_source": false, "commit": null}),
        );
        install_interpreter(&root, &answer(&bundle, with_extras));
        bundle
            .verified_hash()
            .expect("a distribution the lock does not name must not block the hash");
    }

    #[test]
    fn t1_e1_two_installs_canonicalizing_alike_are_refused_rather_than_collapsed() {
        // PEP 503 canonicalization is many-to-one, so `zope.interface` and
        // `zope-interface` are two installs under one name. The probe used to
        // report a map keyed by that name: the second entry replaced the first,
        // the comparison answered for whichever was walked last, and the other
        // install was never mentioned. It is reported as a list now, and the
        // collision is a refusal.
        let root = bundle_copy();
        let bundle = bundle(&root);
        let installed = matching_distributions(&root);
        let sample = installed
            .keys()
            .next()
            .cloned()
            .expect("the lock pins at least one distribution");

        let mut listed = reported(installed);
        let mut second = listed
            .iter()
            .find(|entry| entry["name"] == serde_json::Value::String(sample.clone()))
            .cloned()
            .expect("the sampled distribution is reported");
        second["version"] = serde_json::Value::String("0.0.0-second-install".to_owned());
        listed.push(second);

        install_interpreter(&root, &answer_with_path_hooks(&bundle, listed, Vec::new()));

        let error = bundle
            .verified_hash()
            .expect_err("two installs under one canonical name must not hash");

        let BuildError::WorkerBundle(WorkerBundleError::EnvironmentDoesNotMatchLock { mismatch }) =
            &error
        else {
            panic!("two installs under one canonical name produced the wrong error: {error:?}");
        };
        assert!(
            matches!(
                mismatch.as_ref(),
                EnvironmentMismatch::AmbiguousDistribution { distribution }
                    if *distribution == sample
            ),
            "expected an ambiguous distribution, got {mismatch}"
        );
    }

    #[test]
    fn t1_e1_a_startup_hook_the_lockfile_does_not_account_for_is_refused() {
        // The gap the distribution comparison alone leaves open. A `.pth` runs
        // as the interpreter starts — an `import` line executes, and any other
        // line joins `sys.path` ahead of the search that resolves `torch` — so
        // it is behavior inside the process the bundle identity describes,
        // arriving through a file no declared input covers. Extra distributions
        // are tolerated, which is precisely why their hooks cannot be.
        let root = bundle_copy();
        let bundle = bundle(&root);
        let owner = matching_distributions(&root)
            .keys()
            .next()
            .cloned()
            .expect("the lock pins at least one distribution");

        let cases: [(&str, serde_json::Value); 2] = [
            (
                "unowned-path-hook",
                serde_json::json!({"file": "_dropped_by_hand.pth", "owner": null}),
            ),
            (
                "unlocked-path-hook",
                serde_json::json!({"file": "_virtualenv.pth", "owner": "virtualenv"}),
            ),
        ];

        for (expected, hook) in cases {
            install_interpreter(
                &root,
                &answer_with_path_hooks(
                    &bundle,
                    reported(matching_distributions(&root)),
                    vec![hook],
                ),
            );

            let error = bundle
                .verified_hash()
                .expect_err("a startup hook outside the lock must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::EnvironmentDoesNotMatchLock {
                mismatch,
            }) = &error
            else {
                panic!("the `{expected}` case produced the wrong error: {error:?}");
            };
            let named = match mismatch.as_ref() {
                EnvironmentMismatch::UnownedPathHook { .. } => "unowned-path-hook",
                EnvironmentMismatch::UnlockedPathHook { .. } => "unlocked-path-hook",
                other => panic!("the `{expected}` case reported `{other}`"),
            };
            assert_eq!(named, expected, "wrong fault reported: {mismatch}");
        }

        // A hook owned by a locked distribution still hashes, which is what
        // makes this a rule rather than a ban: `setuptools` is pinned and ships
        // `distutils-precedence.pth`, so refusing every hook would refuse the
        // reference machine's own environment.
        install_interpreter(
            &root,
            &answer_with_path_hooks(
                &bundle,
                reported(matching_distributions(&root)),
                vec![serde_json::json!({"file": "governed.pth", "owner": owner})],
            ),
        );
        bundle
            .verified_hash()
            .expect("a hook owned by a locked distribution must not block the hash");
    }

    #[test]
    fn t1_e1_a_lockfile_line_that_is_not_an_exact_pin_is_refused() {
        // The lock's own header states every entry is an exact pin, so a line
        // this parser cannot read is a lock regenerated wrongly. Skipping it
        // would drop a distribution out of the comparison silently, which is
        // the failure the comparison exists to end.
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        let lockfile = root.path().join(WORKER_LOCKFILE_PATH);
        let original = fs::read_to_string(&lockfile).expect("the copied lockfile is readable");
        for malformed in [
            "chatterbox-tts @ file:///models",
            "extra===1.0",
            "==1.0",
            "extra==",
        ] {
            fs::write(&lockfile, format!("{original}{malformed}\n"))
                .expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("a lockfile line that is not a pin must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                locus,
                reason,
                ..
            }) = &error
            else {
                panic!("`{malformed}` produced the wrong error: {error:?}");
            };
            assert_eq!(
                locus,
                &WorkerLockfileLocus::Line(original.lines().count() + 1),
                "the refusal must name the offending line"
            );
            assert_eq!(reason, &WorkerLockfileErrorReason::MalformedPin);
            assert!(
                error
                    .to_string()
                    .contains("is not an exact `name==version` pin"),
                "the refusal must name the malformed-pin invariant: {error}"
            );
            // The line itself is not printed: a wrongly regenerated lock is
            // exactly where a governed model path appears.
            assert!(
                !error.to_string().contains("file:///"),
                "a refusal must not print the offending line: {error}"
            );
        }
    }

    #[test]
    fn t1_e1_every_index_pin_requires_one_artifact_hash() {
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        let path = root.path().join(WORKER_LOCKFILE_PATH);
        let original = fs::read_to_string(&path).expect("the copied lockfile is readable");
        let pin = original
            .lines()
            .find(|line| line.starts_with("audioread=="))
            .expect("the checked-in lock carries an index pin");
        let malformed = [
            pin.split_once(" --hash=")
                .expect("the checked-in index pin is hashed")
                .0
                .to_owned(),
            pin.replacen("sha256:", "sha256:short-", 1),
            format!("{pin} --hash=sha256:{}", "0".repeat(64)),
            // Only the digest's case differs, so nothing but the case can be
            // what refuses it. `pip` writes lowercase, so an uppercase digest
            // came from something else, and normalizing would hide that.
            {
                let (head, digest) = pin
                    .split_once(ARTIFACT_HASH_PREFIX)
                    .expect("the checked-in index pin is hashed");
                format!("{head}{ARTIFACT_HASH_PREFIX}{}", digest.to_uppercase())
            },
        ];

        for replacement in malformed {
            fs::write(&path, original.replacen(pin, &replacement, 1))
                .expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("an unbound or ambiguous index artifact must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                reason,
                ..
            }) = &error
            else {
                panic!("malformed pin `{replacement}` produced the wrong error: {error:?}");
            };
            assert_eq!(reason, &WorkerLockfileErrorReason::InvalidArtifactHash);
            assert!(
                error
                    .to_string()
                    .contains("lowercase SHA-256 artifact hash"),
                "the refusal must name the artifact-hash invariant: {error}"
            );
        }
    }

    #[test]
    fn t1_e1_the_lock_records_its_package_sources_and_artifact_kinds() {
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        let path = root.path().join(WORKER_LOCKFILE_PATH);
        let original = fs::read_to_string(&path).expect("the copied lockfile is readable");
        for required in REQUIRED_LOCK_DIRECTIVES {
            let without = original.replacen(&format!("{required}\n"), "", 1);
            assert_ne!(
                without, original,
                "the checked-in lock must carry `{required}`"
            );
            fs::write(&path, without).expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("a lock with an implicit source or artifact kind must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                reason,
                ..
            }) = &error
            else {
                panic!("removing `{required}` produced the wrong error: {error:?}");
            };
            assert_eq!(reason, &WorkerLockfileErrorReason::MissingRequiredDirective);
            assert!(
                error
                    .to_string()
                    .contains("omits a required resolution directive"),
                "the refusal must name the missing-directive invariant: {error}"
            );
        }

        for extra in [
            REQUIRED_LOCK_DIRECTIVES[0],
            "--trusted-host packages.example.invalid",
        ] {
            fs::write(&path, format!("{original}{extra}\n")).expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("a repeated or unknown resolution directive must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                reason,
                ..
            }) = &error
            else {
                panic!("adding `{extra}` produced the wrong error: {error:?}");
            };
            let expected = if extra == REQUIRED_LOCK_DIRECTIVES[0] {
                WorkerLockfileErrorReason::DuplicateRequiredDirective
            } else {
                WorkerLockfileErrorReason::UnsupportedDirective
            };
            assert_eq!(reason, &expected);
            assert!(
                !error.to_string().contains("not an exact"),
                "a directive refusal must not claim the line is a malformed pin: {error}"
            );
        }
    }

    #[test]
    fn t1_e1_a_governed_pin_parted_from_its_provenance_marker_is_refused() {
        // The marker is the only thing that makes the governed pin's origin
        // checkable, and a pin carrying no origin is not compared at all. So
        // every way of parting the two is a lock that would pass while
        // admitting exactly the index install the marker exists to refuse.
        // Driven from the checked-in lock, because that pairing is the one
        // under test.
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        let path = root.path().join(WORKER_LOCKFILE_PATH);
        let original = fs::read_to_string(&path).expect("the copied lockfile is readable");
        let marker = original
            .lines()
            .find(|line| line.starts_with(GOVERNED_SOURCE_MARKER))
            .expect("the checked-in lockfile marks its governed pin")
            .to_owned();
        let spoil = |replacement: &str| original.replacen(&marker, replacement, 1);

        let partings: [(&str, String); 4] = [
            ("deleted", original.replacen(&format!("{marker}\n"), "", 1)),
            ("separated from its pin", spoil(&format!("{marker}\n"))),
            ("doubled", spoil(&format!("{marker}\n{marker}"))),
            ("naming no commit", spoil(GOVERNED_SOURCE_MARKER.trim_end())),
            // A marker moved onto another distribution is covered by the
            // `doubled` and `separated` cases together: the claim either
            // reaches a pin that may not carry one, or reaches none.
        ];

        for (parting, lockfile) in partings {
            assert_ne!(
                lockfile, original,
                "the `{parting}` case must actually change the lockfile"
            );
            fs::write(&path, &lockfile).expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("a lockfile whose provenance marker has come apart must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                reason,
                ..
            }) = &error
            else {
                panic!("a marker `{parting}` produced the wrong error: {error:?}");
            };
            assert_eq!(reason, &WorkerLockfileErrorReason::InvalidProvenance);
            assert!(
                error.to_string().contains("governed-source provenance"),
                "the refusal must name the provenance invariant: {error}"
            );
        }
    }

    #[test]
    fn t1_e1_a_lockfile_fault_no_line_carries_names_no_line() {
        // Three invariants are the file's rather than a line's, and each was
        // reported against a line number anyway: `0` for bytes that are not
        // UTF-8, one past the last line for the two absences. Both sentinels
        // rendered as a real line the operator would open and find nothing
        // wrong with, and only the rendered message shows it.
        let root = bundle_copy();
        let bundle = bundle(&root);
        install_interpreter(&root, &matching_answer(&root, &bundle));

        let path = root.path().join(WORKER_LOCKFILE_PATH);
        let original = fs::read_to_string(&path).expect("the copied lockfile is readable");
        let marker = original
            .lines()
            .position(|line| line.starts_with(GOVERNED_SOURCE_MARKER))
            .expect("the checked-in lockfile marks its governed pin");
        // The marker and the pin beneath it, so what is left has no governed
        // pin rather than one whose provenance was torn off.
        let ungoverned: String = original
            .lines()
            .enumerate()
            .filter(|(index, _)| *index != marker && *index != marker + 1)
            .map(|(_, line)| format!("{line}\n"))
            .collect();

        for (fault, lockfile) in [
            ("bytes that are not UTF-8", vec![0xff]),
            (
                "a required directive absent",
                original
                    .replacen(&format!("{}\n", REQUIRED_LOCK_DIRECTIVES[0]), "", 1)
                    .into_bytes(),
            ),
            ("no governed pin at all", ungoverned.into_bytes()),
        ] {
            fs::write(&path, &lockfile).expect("the lockfile is writable");

            let error = bundle
                .verified_hash()
                .expect_err("a lockfile the whole file breaks must not hash");

            let BuildError::WorkerBundle(WorkerBundleError::UnreadableWorkerLockfile {
                locus, ..
            }) = &error
            else {
                panic!("`{fault}` produced the wrong error: {error:?}");
            };
            assert_eq!(locus, &WorkerLockfileLocus::WholeFile);
            assert!(
                !error.to_string().contains("line"),
                "`{fault}` is no line's fault, so the refusal must name none: {error}"
            );
        }
    }

    #[test]
    fn t1_e1_an_absent_interpreter_is_refused_before_the_bundle_is_read() {
        let root = bundle_copy();
        // Removed after the bundle is loaded, so the only thing wrong with this
        // bundle is that its interpreter is not installed. A late gate would
        // report the missing input instead.
        let bundle = bundle(&root);
        fs::remove_file(root.path().join("worker/requirements.lock"))
            .expect("the lockfile is removable");

        let error = bundle
            .verified_hash()
            .expect_err("a bundle with no interpreter must not hash");

        assert!(
            matches!(
                error,
                BuildError::Tool(crate::ToolError::MissingTool { .. })
            ),
            "expected the interpreter to be missing, got {error:?}"
        );
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

    fn write_manifest(root: &TempDir, manifest: &BundleManifest) {
        fs::write(
            root.path().join(BUNDLE_MANIFEST_PATH),
            serde_json::to_vec_pretty(manifest).expect("a manifest serializes"),
        )
        .expect("the manifest is writable");
    }
}
