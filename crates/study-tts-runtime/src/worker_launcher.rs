//! `worker/launcher.json`: the inference-affecting settings a worker
//! starts with.
//!
//! A declared bundle input (ADR-0001 §12.5 counts "launcher configuration that
//! affects inference" among the hash's inputs), so this file is read from the
//! path `worker/bundle-manifest.json` declares and nowhere else — reading it
//! from elsewhere would let the bundle hash describe a file the worker did not
//! use.
//!
//! **Both ends parse it, and neither end is the other's source.**
//! `worker/study_tts_worker/worker.py` reads it to apply the offline variables
//! into the process a backend is about to be imported into, which is a thing
//! only that process can do. Rust reads it for the settings the *parent* owns:
//! the per-worker thread allowance it puts in the child's environment and sends
//! on the `initialize` frame. `LAUNCHER_SHAPE` in `worker.py` is the same shape
//! stated in Python. Neither end tests the other; both read the *checked-in*
//! file — `t4_e1_the_checked_in_launcher_is_one_this_build_reads` here and
//! `LauncherShapeTests` in `worker/tests/test_worker.py` there — so a
//! change only one shape accepts fails the other end's suite rather than
//! passing silently.

use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::path::Path;

use serde::Deserialize;

use crate::BuildError;
use crate::error::WorkerBundleError;
use crate::worker_bundle::WORKER_LAUNCHER_PATH;

/// Launcher layout this build reads.
///
/// Refused rather than guessed at, like every other versioned record here: a
/// launcher written for a later layout may mean something different by a field
/// this build would otherwise read under the old meaning. Mirrors
/// `LAUNCHER_SCHEMA_VERSION` in `worker/study_tts_worker/worker.py`.
pub const LAUNCHER_SCHEMA_VERSION: &str = "1.1";

/// Environment variables that cap native numerical thread pools.
///
/// ADR-0001 §10.1: "The launcher sets `OMP_NUM_THREADS`, `MKL_NUM_THREADS`,
/// `OPENBLAS_NUM_THREADS`, and `NUMEXPR_NUM_THREADS` to the same per-worker
/// value." The launcher is the *parent*, which is why these are set here rather
/// than by the worker: every one of them is read as a native library loads, so
/// a process cannot usefully set them for itself, and E5-S2's pool gives each
/// worker its own allowance that no file shared by all of them could carry.
pub const THREAD_ENVIRONMENT: [&str; 4] = [
    "OMP_NUM_THREADS",
    "MKL_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "NUMEXPR_NUM_THREADS",
];

/// Offline variables ADR-0001 14 requires the launcher to set to `1`.
///
/// Mirrors `REQUIRED_OFFLINE_ENVIRONMENT` in
/// `worker/study_tts_worker/worker.py`, which names this constant in turn.
pub const REQUIRED_OFFLINE_ENVIRONMENT: [&str; 2] = ["HF_HUB_OFFLINE", "TRANSFORMERS_OFFLINE"];

/// Offline variables the launcher may carry but need not.
///
/// Mirrors `OPTIONAL_OFFLINE_ENVIRONMENT` in
/// `worker/study_tts_worker/worker.py`.
pub const OPTIONAL_OFFLINE_ENVIRONMENT: [&str; 1] = ["HF_HUB_DISABLE_PROGRESS_BARS"];

/// The variable names under which a governed root reaches a worker.
///
/// A denylist for [`crate::WorkerConfiguration::for_protocol_fake`], and
/// nothing else. [`WorkerLauncher::child_environment`] still publishes the two
/// roots under the names *the launcher declares* rather than under these, for
/// the reason it gives; this constant exists because the protocol fake reads no
/// launcher and so has no other way to recognise the arrangement it must never
/// be handed. Mirrors `model_root_environment_variable` and
/// `voice_root_environment_variable` in `worker/launcher.json`, and
/// `t4_e1_the_governed_root_variables_are_the_ones_the_launcher_declares` reads
/// that file and refuses the drift.
pub const GOVERNED_ROOT_ENVIRONMENT: [&str; 2] = ["STUDY_TTS_MODEL_ROOT", "STUDY_TTS_VOICE_ROOT"];

/// Interpreter settings the worker is started with, and their values.
///
/// Declared rather than inherited, because `WorkerClient::spawn` clears the
/// environment: what is not here does not reach the child. Each one closes a
/// way the machine could otherwise change what the interpreter does before the
/// worker's own code runs.
///
/// - `PYTHONNOUSERSITE` keeps a user site-packages directory off `sys.path`, so
///   the locked bundle is the only thing importable. `docs/operations/`
///   `WORKER-ENVIRONMENT.md` requires every startup module to have a locked
///   owner, and a user-site import is precisely an unlocked one.
/// - `PYTHONDONTWRITEBYTECODE` keeps the interpreter from writing `.pyc` files
///   into the bundle it was hashed from.
/// - `PYTHONHASHSEED` fixes string hashing, which iteration order over a `set`
///   would otherwise vary with.
/// - `PYTHONUTF8` fixes the text encoding rather than reading it from a locale
///   this build no longer passes on.
pub const INTERPRETER_ENVIRONMENT: [(&str, &str); 4] = [
    ("PYTHONNOUSERSITE", "1"),
    ("PYTHONDONTWRITEBYTECODE", "1"),
    ("PYTHONHASHSEED", "0"),
    ("PYTHONUTF8", "1"),
];

/// The inference-affecting settings one worker is launched with.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerLauncher {
    /// Layout of this record, refused when it is not one this build reads.
    pub schema_version: String,
    /// Execution device the worker selects.
    pub device: String,
    /// Native threads one worker may use.
    ///
    /// `NonZeroU32` for the reason `InitializeParameters::threads` is: zero is
    /// not a smaller allowance but an unanswerable instruction, and the same
    /// count reaches the worker on that frame.
    pub threads: NonZeroU32,
    /// Seed the backend samples with.
    ///
    /// An ADR-0001 §12.5 synthesis-key input, so it is configured in this
    /// declared bundle input rather than chosen by a caller: a seed a caller
    /// picked would let two builds key identical audio differently.
    pub seed: u64,
    /// Model repository the backend loads from.
    ///
    /// The public identifier the weights were acquired from, never the
    /// governed local root they live in: an ADR-0001 §12.5 key input this
    /// committed file may carry, where
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` forbids the path.
    pub model_repository: String,
    /// Backend generation parameters, by name, in their configured spelling.
    ///
    /// **Strings, and deliberately not numbers.** These reach the synthesis key
    /// (ADR-0001 §12.5), and an identity may carry no floating point — there is
    /// no encoding of `0.05` that two builds are guaranteed to agree on. The
    /// launcher records the exact spelling, the key hashes that spelling, and
    /// the worker parses it once when it calls the backend. A `BTreeMap` so the
    /// order an author wrote them in cannot reach the digest.
    pub generation_parameters: BTreeMap<String, String>,
    /// Variables the worker puts into its own environment before importing a
    /// backend.
    ///
    /// Read here but **not applied here**: they are read as
    /// `huggingface_hub` and `transformers` load, inside the worker, so the
    /// worker applies them itself and reports on standard error that it did.
    /// Rust carries them so a reader of this type sees the whole record.
    pub offline_environment: BTreeMap<String, String>,
    /// Whether every model load is confined to local files.
    pub local_files_only: bool,
    /// Variable naming the governed model root, whose value never enters Git.
    pub model_root_environment_variable: String,
    /// Variable naming the governed voice root, whose value never enters Git.
    ///
    /// The worker resolves a voice profile beneath it. Named by variable rather
    /// than written here for the reason the model root is:
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a governed path
    /// out of Git, CI, and logs, and this file is committed.
    pub voice_root_environment_variable: String,
}

impl WorkerLauncher {
    /// Reads and checks the launcher beneath `bundle_root`.
    ///
    /// # Errors
    ///
    /// [`WorkerBundleError::UnsupportedLauncher`] when the declared layout is
    /// not [`LAUNCHER_SCHEMA_VERSION`], and
    /// [`WorkerBundleError::UnreadableLauncher`] when the file is absent or is
    /// not the record this build reads. The version is checked before the
    /// shape, because a future layout may add fields this build does not know
    /// and reporting one as unknown would send an operator to edit a record
    /// this build cannot read anyway — the same ordering `worker.py` uses.
    pub fn read(bundle_root: &Path) -> Result<Self, BuildError> {
        let path = bundle_root.join(WORKER_LAUNCHER_PATH);
        let bytes = std::fs::read(&path).map_err(|source| unreadable(&source.to_string()))?;

        #[derive(Deserialize)]
        struct DeclaredVersion {
            schema_version: String,
        }
        let declared: DeclaredVersion =
            serde_json::from_slice(&bytes).map_err(|source| unreadable(&source.to_string()))?;
        if declared.schema_version != LAUNCHER_SCHEMA_VERSION {
            return Err(WorkerBundleError::UnsupportedLauncher {
                found: declared.schema_version,
                supported: LAUNCHER_SCHEMA_VERSION,
            }
            .into());
        }
        serde_json::from_slice(&bytes).map_err(|source| unreadable(&source.to_string()))
    }

    /// The environment one worker starts with: thread caps and governed roots.
    ///
    /// The offline variables are deliberately absent: the worker applies those
    /// itself from this same file, and setting them from both ends would be two
    /// sources that can disagree about whether a render may reach a network.
    ///
    /// The two roots are passed by value at runtime and published under the
    /// variable names *this launcher declares*, never under a name written into
    /// Rust. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a governed
    /// path out of Git, CI, and logs, so the variable name is the only half of
    /// the arrangement either end can agree on in writing — which is why the
    /// launcher carries the name and the caller carries the path.
    ///
    /// The set stays closed at the caps plus these two.
    /// `t1_e1_the_thread_allowance_reaches_every_native_pool` asserts the exact
    /// size, so a third variable cannot be added without saying so here.
    pub fn child_environment(
        &self,
        model_root: &Path,
        voice_root: &Path,
    ) -> BTreeMap<String, String> {
        let mut environment: BTreeMap<String, String> = THREAD_ENVIRONMENT
            .iter()
            .map(|name| ((*name).to_owned(), self.threads.to_string()))
            .collect();
        environment.insert(
            self.model_root_environment_variable.clone(),
            model_root.display().to_string(),
        );
        environment.insert(
            self.voice_root_environment_variable.clone(),
            voice_root.display().to_string(),
        );
        // The child inherits nothing, so the offline variables the worker also
        // applies to itself are set here as well: an import that consulted a
        // model resolver before `_apply_offline_environment` ran would already
        // have decided whether it may reach the network.
        //
        // Over the allowlist, never over the file, for the reason
        // `_apply_offline_environment` gives on the Python side: iterating the
        // launcher's own entries would make `worker/launcher.json` a place to
        // set `PYTHONPATH` for the child — in a file that is a declared bundle
        // input and therefore reads as governed. Since `WorkerClient::spawn`
        // clears the environment, this loop is the *only* thing standing
        // between that file and what the interpreter imports.
        environment.extend(
            REQUIRED_OFFLINE_ENVIRONMENT
                .iter()
                .chain(OPTIONAL_OFFLINE_ENVIRONMENT.iter())
                .filter_map(|name| {
                    self.offline_environment
                        .get(*name)
                        .map(|value| ((*name).to_owned(), value.clone()))
                }),
        );
        environment.extend(
            INTERPRETER_ENVIRONMENT
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
        );
        environment
    }
}

/// The refusal for a launcher this build cannot read.
fn unreadable(detail: &str) -> BuildError {
    WorkerBundleError::UnreadableLauncher {
        path: WORKER_LAUNCHER_PATH.into(),
        detail: detail.to_owned(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;
    use crate::WorkerBundleError;

    /// The repository root, where the checked-in launcher lives.
    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// A valid launcher of the layout this build reads, before a test
    /// spoils it.
    ///
    /// One spelling, so the next layout move edits this and the cases below say
    /// what they are about rather than restating a whole record each.
    fn valid_launcher() -> serde_json::Value {
        serde_json::json!({
            "schema_version": LAUNCHER_SCHEMA_VERSION,
            "device": "cpu",
            "threads": 3,
            "seed": 42,
            "model_repository": "ResembleAI/chatterbox",
            "generation_parameters": {"cfg_weight": "0.5"},
            "offline_environment": {"HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1"},
            "local_files_only": true,
            "model_root_environment_variable": "STUDY_TTS_MODEL_ROOT",
            "voice_root_environment_variable": "STUDY_TTS_VOICE_ROOT",
        })
    }

    /// Writes `contents` as a launcher beneath a fresh root.
    fn launcher_root(contents: &str) -> TempDir {
        let root = TempDir::new().expect("create a bundle root");
        let path = root.path().join(WORKER_LAUNCHER_PATH);
        fs::create_dir_all(path.parent().expect("the launcher has a parent"))
            .expect("the launcher directory is creatable");
        fs::write(&path, contents).expect("the launcher is writable");
        root
    }

    #[test]
    fn t4_e1_the_checked_in_launcher_is_one_this_build_reads() {
        // The file both ends parse. A change that only the Python shape accepts
        // would leave Rust unable to start a worker at all, and this is the
        // cheapest place that fails when the two shapes drift apart.
        let launcher =
            WorkerLauncher::read(&repository_root()).expect("the checked-in launcher is readable");

        assert_eq!(launcher.schema_version, LAUNCHER_SCHEMA_VERSION);
        assert_eq!(launcher.device, "cpu");
        assert!(
            launcher.local_files_only,
            "ADR-0001 §14 renders offline, so the checked-in launcher must confine model loads"
        );
        for required in ["HF_HUB_OFFLINE", "TRANSFORMERS_OFFLINE"] {
            assert_eq!(
                launcher
                    .offline_environment
                    .get(required)
                    .map(String::as_str),
                Some("1"),
                "`{required}` must be set to `1` in the checked-in launcher"
            );
        }
    }

    #[test]
    fn t1_e1_a_declared_environment_carries_everything_the_child_no_longer_inherits() {
        // `WorkerClient::spawn` clears the environment, so anything the worker
        // needs is declared here or is absent. The offline variables are the
        // load-bearing half: the worker applies them to its own process, but
        // only once it is running, and a model resolver consulted during an
        // import before that point would already have decided whether it may
        // reach the network.
        let root = launcher_root(&valid_launcher().to_string());
        let launcher = WorkerLauncher::read(root.path()).expect("the launcher is readable");

        let environment = launcher
            .child_environment(Path::new("/governed/models"), Path::new("/governed/voices"));

        for (name, value) in &launcher.offline_environment {
            assert_eq!(
                environment.get(name),
                Some(value),
                "`{name}` must reach the child, which inherits nothing"
            );
        }
        for name in INTERPRETER_ENVIRONMENT.map(|(name, _)| name) {
            assert!(
                environment.contains_key(name),
                "`{name}` must reach the child, which inherits nothing"
            );
        }
    }

    #[test]
    fn t1_e1_a_launcher_cannot_choose_an_extra_variable_for_the_child() {
        // `worker/launcher.json` is a declared bundle input, so it reads as
        // governed; it is not a place to choose what the interpreter imports.
        // Since `WorkerClient::spawn` clears the environment, this allowlist is
        // the only thing between that file and the child's `sys.path`. The
        // Python end refuses such an entry at its parse
        // (`_apply_offline_environment`); this proves the entry cannot reach
        // the child even from a launcher that carries one.
        let mut document = valid_launcher();
        document["offline_environment"]["PYTHONPATH"] = serde_json::json!("/somewhere/else");
        let root = launcher_root(&document.to_string());
        let launcher = WorkerLauncher::read(root.path()).expect("the launcher is readable");

        let environment = launcher
            .child_environment(Path::new("/governed/models"), Path::new("/governed/voices"));

        assert!(
            !environment.contains_key("PYTHONPATH"),
            "a launcher entry outside the offline allowlist reached the child: {environment:?}"
        );
    }

    #[test]
    fn t1_e1_the_thread_allowance_reaches_every_native_pool() {
        // ADR-0001 §10.1 caps PyTorch and every native numerical pool at the
        // same per-worker value. Each of these is read as a native library
        // loads, so one of them left unset is one pool sized by the machine
        // rather than by the budget `doctor` checked.
        let root = launcher_root(&valid_launcher().to_string());
        let launcher = WorkerLauncher::read(root.path()).expect("the launcher is readable");

        let environment = launcher
            .child_environment(Path::new("/governed/models"), Path::new("/governed/voices"));

        for name in THREAD_ENVIRONMENT {
            assert_eq!(
                environment.get(name).map(String::as_str),
                Some("3"),
                "`{name}` must carry the launcher's per-worker thread allowance"
            );
        }
        // Under the names the launcher itself declares, never a name written
        // here: `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps the
        // governed paths out of this committed file, so the variable is the
        // only half either end can agree on in writing.
        assert_eq!(
            environment
                .get(&launcher.model_root_environment_variable)
                .map(String::as_str),
            Some("/governed/models"),
            "the worker must be told where the governed model root is"
        );
        assert_eq!(
            environment
                .get(&launcher.voice_root_environment_variable)
                .map(String::as_str),
            Some("/governed/voices"),
            "the worker must be told where the governed voice root is"
        );
        // The set stays closed, and it is now the *whole* environment rather
        // than an overlay: `WorkerClient::spawn` clears before declaring, so a
        // name missing here is a name the child does not have. The offline
        // variables are set from both ends and cannot disagree for it — both
        // read `worker/launcher.json`, and the worker applying them to itself
        // covers only the part of its own startup that runs after it does.
        assert_eq!(
            environment.len(),
            THREAD_ENVIRONMENT.len()
                + 2
                + launcher.offline_environment.len()
                + INTERPRETER_ENVIRONMENT.len(),
            "the parent sets the thread caps, the governed roots, the offline \
             variables and the interpreter settings, and nothing else: {environment:?}"
        );
    }

    #[test]
    fn t1_e1_a_launcher_this_build_cannot_read_is_refused_by_its_exact_fault() {
        // One refusal per fault, so an operator is handed the repair rather
        // than a category. The version is refused before the shape: a later
        // layout's new field must not be reported as an unknown one.
        let mut later = valid_launcher();
        later["schema_version"] = serde_json::json!("2.0");
        later["a_field_this_build_does_not_know"] = serde_json::json!(true);
        let unsupported = launcher_root(&later.to_string());
        let error = WorkerLauncher::read(unsupported.path())
            .expect_err("a launcher layout this build does not implement must be refused");
        assert!(
            matches!(
                error,
                BuildError::WorkerBundle(WorkerBundleError::UnsupportedLauncher { .. })
            ),
            "the version must be refused before the shape: {error:?}"
        );

        let mut unknown_field = valid_launcher();
        unknown_field["extra"] = serde_json::json!(1);
        let mut zero_threads = valid_launcher();
        zero_threads["threads"] = serde_json::json!(0);
        let mut missing_field = valid_launcher();
        missing_field
            .as_object_mut()
            .expect("the launcher is an object")
            .remove("generation_parameters");

        for (case, contents) in [
            ("an unknown field", unknown_field),
            ("a zero thread allowance", zero_threads),
            ("a missing field", missing_field),
        ] {
            let root = launcher_root(&contents.to_string());
            let error = WorkerLauncher::read(root.path())
                .expect_err("a launcher this build cannot read must be refused");
            assert!(
                matches!(
                    error,
                    BuildError::WorkerBundle(WorkerBundleError::UnreadableLauncher { .. })
                ),
                "`{case}` must be refused as unreadable: {error:?}"
            );
        }
    }

    #[test]
    fn t4_e1_the_governed_root_variables_are_the_ones_the_launcher_declares() {
        // The half that keeps the mirror from coming apart. `for_protocol_fake`
        // refuses an environment naming either of these, and it reads no
        // launcher — so a launcher that renamed a governed-root variable would
        // leave the fake denying a name nothing uses and admitting the one that
        // matters. Read from the shipped file rather than restated here, which
        // would be a third copy to drift.
        let launcher =
            WorkerLauncher::read(&repository_root()).expect("the checked-in launcher is readable");

        assert_eq!(
            [
                launcher.model_root_environment_variable.as_str(),
                launcher.voice_root_environment_variable.as_str(),
            ],
            GOVERNED_ROOT_ENVIRONMENT,
            "`GOVERNED_ROOT_ENVIRONMENT` must name exactly what `worker/launcher.json` declares"
        );
    }
}
