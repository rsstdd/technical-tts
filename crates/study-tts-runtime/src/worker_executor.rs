//! The capacity-one [`TtsExecutor`] backed by one persistent worker process.
//!
//! ADR-0001 §10.4 keeps model-specific fields out of the lesson and planning
//! layers, and §10.1 gives each worker one in-flight request and one model load
//! per lifetime. This module is where those two meet: it owns a
//! [`WorkerClient`] behind a mutex — §10.1's "individually synchronized
//! client" — and reports a [`BackendDescriptor`] built from what the worker
//! said when it initialized, never from a constant.
//!
//! **The descriptor is the worker's answer, not this build's claim.** A
//! hard-coded model or bundle identity would name a bundle that is not the one
//! running, and every cache entry keyed on it would describe audio some other
//! worker produced. `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`
//! records the executor seam this implements.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Mutex;
use std::time::Duration;

use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, DeterminismClass,
    LanguageTag, Revision, WorkerBundleHash,
};

use crate::model_gate::verify_model_artifacts;
use crate::synthesis::{
    BackendDescriptor, BackendError, BackendValidationError, DriftedIdentity, SynthesisReport,
    SynthesisRequest, TTS_EXECUTOR_CONTRACT_VERSION, TtsExecutor, validate_executor_request,
};
use crate::worker_bundle::{WORKER_ENTRY_MODULE, WORKER_PACKAGE_ROOT, WorkerBundle};
use crate::worker_client::WorkerClient;
use crate::worker_environment::WORKER_INTERPRETER_PATH;
use crate::worker_launcher::WorkerLauncher;
use crate::worker_protocol::{
    InitializeParameters, WORKER_PROTOCOL_VERSION, WorkerRequestFrame, WorkerResponseFrame,
    WorkerSynthesisParameters,
};
use crate::{BuildError, ToolInvocation, ToolOperation};

/// Deadline for `initialize`, which loads the model once per lifetime.
///
/// Generous because it covers a cold read of the model weights, and a security
/// ceiling rather than a performance budget:
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
/// records it and names this constant.
pub const WORKER_INITIALIZE_DEADLINE: Duration = Duration::from_secs(300);

/// Deadline for one `synthesize`, `capabilities`, or `health` exchange.
///
/// One ceiling rather than three: the two metadata calls answer immediately,
/// and a separate constant for each would be configuration nobody sets.
/// Recorded in `docs/architecture/WALKING-SKELETON.md` §Provisional resource
/// ceilings, which names this constant.
pub const WORKER_REQUEST_DEADLINE: Duration = Duration::from_secs(600);

/// How to start one worker and what it contributes to a synthesis key.
///
/// The fields split by who owns the answer. The first four start a process; the
/// rest are ADR-0001 §12.5 inputs the *worker cannot report* — it does not know
/// which repository it was built from or which parameters the project chose —
/// so they are configured. Everything the worker does know
/// (`languages`, `max_text_bytes`, determinism, and every identity) is read
/// back from it instead.
#[derive(Clone, Debug)]
pub struct WorkerConfiguration {
    /// Interpreter to run, resolved and checked by the caller.
    ///
    /// Private, with every other launch field, because together they are what
    /// starts a process under a claimed identity. A caller that could set the
    /// program and the identity independently could run anything under any
    /// bundle's name, and ADR-0001 §12.5 keys every cache entry on that name.
    /// [`WorkerConfiguration::for_bundle`] is the only way to obtain one for a
    /// bundle, and it derives the identity rather than accepting it.
    program: PathBuf,
    /// Discrete arguments, never a shell string.
    arguments: Vec<String>,
    /// Directory the worker starts in, which is what `python -m` resolves the
    /// entry module against.
    working_directory: PathBuf,
    /// Environment the worker starts with, including the offline variables.
    environment: BTreeMap<String, String>,
    /// The one directory the worker may write inside, for its lifetime.
    ///
    /// Sent at `initialize` and enforced by the worker against the resolved
    /// parent of every path it is assigned. ADR-0001 10.3 confines worker
    /// writes to the assigned staging root; this is the field that tells the
    /// worker where that root is, without which it can only check the spelling
    /// of the one path it was handed.
    staging_root: PathBuf,
    /// Native threads this worker may use, from `worker/launcher.json`.
    threads: NonZeroU32,
    /// Identity of the bundle this worker must confirm it is.
    worker_bundle_hash: WorkerBundleHash,
    /// Model repository the backend loads from.
    model_repository: String,
    /// The model revision whose bytes were proven before this worker starts.
    ///
    /// Carried rather than taken from the worker's answer, because the answer
    /// is what it decides which weights to load *from*: the worker computes
    /// `model-{revision}` from the governed acquisition record and loads that
    /// directory, so a worker reporting another revision has loaded bytes
    /// [`crate::verify_model_artifacts`] never hashed.
    /// [`WorkerTtsExecutor::start`] compares the two.
    model_revision: Revision,
    /// Seed the backend samples with.
    seed: u64,
    /// Backend generation parameters, by name, in their configured spelling.
    generation_parameters: BTreeMap<String, String>,
    /// How long `initialize` may take before the worker is killed.
    ///
    /// A field rather than only a constant because a deadline that cannot be
    /// shortened cannot be tested: proving a hung worker is refused and its
    /// process tree reaped would otherwise cost
    /// [`WORKER_INITIALIZE_DEADLINE`] of wall time per run. E5-S3 owns making
    /// this configurable for operators; it arrives here early because the
    /// fault path needs it now.
    initialize_deadline: Duration,
    /// How long any other single exchange may take, for the same reason.
    request_deadline: Duration,
}

/// One persistent worker, leased for one request at a time.
#[derive(Debug)]
pub struct WorkerTtsExecutor {
    descriptor: BackendDescriptor,
    /// Voice profiles the worker said it had loaded.
    ///
    /// Kept here rather than on [`BackendDescriptor`], deliberately. The
    /// descriptor is what [`BackendDescriptor::synthesis_context`] builds every
    /// cache key from, so a field added there would move every key in the
    /// project — and what a worker *can* be asked for is not part of what its
    /// audio is. `languages` and `max_text_bytes` predate that reasoning and
    /// are left where they are rather than moved for tidiness.
    declared_voices: BTreeSet<String>,
    /// Delivery styles the worker said it has parameters for.
    declared_styles: BTreeSet<String>,
    request_deadline: Duration,
    /// Behind a mutex because ADR-0001 §10.1 gives one worker one in-flight
    /// request, and [`TtsExecutor`] takes `&self` so callers cannot serialize
    /// it for us.
    client: Mutex<WorkerClient>,
}

impl WorkerConfiguration {
    /// Builds the configuration for the worker this repository ships.
    ///
    /// Every inference-affecting value comes from `worker/launcher.json`, the
    /// declared bundle input both ends read, rather than from a caller: a seed
    /// or a generation parameter a caller chose would let two builds key
    /// identical audio differently, and ADR-0001 §12.5 puts both in the
    /// synthesis key. The caller supplies only what the launcher cannot know:
    /// where the bundle root is, and where the two governed roots are. The
    /// launcher names those two by environment variable and never by path,
    /// because `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a
    /// governed location out of a committed file.
    ///
    /// **The identity is derived here, never accepted.**
    /// [`crate::WorkerBundle::verified_hash`] proves the interpreter that would
    /// run this bundle is the one its manifest declares before it returns a
    /// hash, so the identity every cache entry is keyed on describes the code
    /// that actually ran. An identity a caller passed in would be a claim about
    /// a bundle nobody checked.
    ///
    /// # Errors
    ///
    /// [`crate::WorkerBundleError::UnreadableLauncher`] or
    /// [`crate::WorkerBundleError::UnsupportedLauncher`] when the launcher is
    /// not the record this build reads, and everything
    /// [`crate::WorkerBundle::verified_hash`] reports when the bundle cannot be
    /// identified — including [`crate::ToolError::MissingTool`] when the
    /// interpreter it names is absent.
    pub fn for_bundle(
        bundle_root: &Path,
        model_root: &Path,
        voice_root: &Path,
        staging_root: &Path,
    ) -> Result<Self, BuildError> {
        // Identity before anything else, so a bundle that cannot be identified
        // never reaches the point of having a launchable configuration at all.
        let worker_bundle_hash = WorkerBundle::load(bundle_root)?.verified_hash()?;
        // And the model's bytes before that identity can launch anything. The
        // worker reads its revision out of the governed acquisition record and
        // loads whatever that names; until this ran, nothing had hashed the
        // weights, so ADR-0001 §12.5's key described a claim rather than the
        // bytes that produced the audio. Issue #66, and the model half of the
        // 2026-08-31 audit's sixth finding.
        let model_revision = verify_model_artifacts(model_root)?;
        let launcher = WorkerLauncher::read(bundle_root)?;
        // Absolute from here on — the bundle root and both governed roots. The
        // child is given a working directory of its own, so every path handed
        // to it resolves against that rather than against the parent's: a
        // relative program becomes a program somewhere nobody meant, which
        // `std` documents as unspecified rather than as a rule to rely on, and
        // a relative governed root becomes a directory the worker cannot find.
        // A caller passing relative roots is ordinary, so they are resolved
        // here rather than required of them.
        let bundle_root = &absolute(bundle_root)?;
        let model_root = &absolute(model_root)?;
        let voice_root = &absolute(voice_root)?;
        let staging_root = absolute(staging_root)?;
        Ok(Self {
            program: bundle_root.join(WORKER_INTERPRETER_PATH),
            // `-m` with the entry module rather than the package: `python -m`
            // on a package needs a `__main__.py` this bundle deliberately does
            // not ship. A module rather than a file path, so the interpreter
            // resolves what the manifest declares and a caller cannot point the
            // worker at a script somewhere else.
            arguments: vec!["-m".to_owned(), WORKER_ENTRY_MODULE.to_owned()],
            // `python -m` resolves the entry module against the working
            // directory, so the worker starts in the one directory its package
            // is importable from. A `PYTHONPATH` would do the same job and is
            // deliberately not used: `child_environment` closes the variable
            // set precisely so a launcher cannot choose what gets imported.
            working_directory: import_root(bundle_root),
            environment: launcher.child_environment(model_root, voice_root),
            staging_root,
            threads: launcher.threads,
            worker_bundle_hash,
            model_repository: launcher.model_repository.clone(),
            model_revision,
            seed: launcher.seed,
            generation_parameters: launcher.generation_parameters.clone(),
            initialize_deadline: WORKER_INITIALIZE_DEADLINE,
            request_deadline: WORKER_REQUEST_DEADLINE,
        })
    }

    /// Builds the configuration for the executable protocol fake.
    ///
    /// The contract suite drives the same executor as production against a fake
    /// that speaks the protocol without loading weights, which
    /// `DELIVERY-PLAN.md` E1-S3 task 7 asks for. It needs to choose the program
    /// the environment and the deadlines — a fault path proved by waiting one
    /// out would otherwise cost [`WORKER_INITIALIZE_DEADLINE`] of wall time per
    /// run, and the thread caps have to be observable in the child.
    ///
    /// **It cannot choose an identity.** The bundle hash is
    /// [`PROTOCOL_FAKE_BUNDLE_HASH`] and is not a parameter, so this is not a
    /// way around [`WorkerConfiguration::for_bundle`]: whatever a caller points
    /// `program` at runs under the synthetic backend's name, never under a real
    /// bundle's.
    #[must_use]
    pub fn for_protocol_fake(
        program: PathBuf,
        arguments: Vec<String>,
        environment: BTreeMap<String, String>,
        staging_root: PathBuf,
        deadline: Duration,
    ) -> Self {
        Self {
            // The fake is an absolute path and imports nothing, so its working
            // directory is its own location rather than an import root: an
            // inherited one would make these tests depend on where cargo was
            // invoked from.
            working_directory: program.parent().unwrap_or(Path::new(".")).to_path_buf(),
            program,
            arguments,
            environment,
            staging_root,
            threads: NonZeroU32::MIN,
            worker_bundle_hash: PROTOCOL_FAKE_BUNDLE_HASH
                .parse()
                .expect("the protocol fake's identity is a well-formed digest"),
            model_repository: "study-tts/deterministic-tone".to_owned(),
            // The fake loads no weights, so there is nothing to prove and this
            // is simply what it reports. It still travels through the same
            // comparison in `start`, which is what keeps that check honest.
            model_revision: "v1"
                .parse()
                .expect("the protocol fake's revision is well formed"),
            seed: 0,
            generation_parameters: BTreeMap::new(),
            initialize_deadline: deadline,
            request_deadline: deadline,
        }
    }
}

/// Renders a declared list for a refusal message.
///
/// An empty list is said out loud for the reason `render_languages` says it:
/// the message would otherwise read as an omitted field and leave the reader
/// guessing whether the worker declared nothing or the build forgot to ask.
fn render_declared(declared: &BTreeSet<String>) -> String {
    if declared.is_empty() {
        return "none at all".to_owned();
    }
    declared
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The identity every configuration built by
/// [`WorkerConfiguration::for_protocol_fake`] runs under.
///
/// A constant rather than a parameter, and that is the whole point: with it
/// fixed, no public function in this crate accepts a caller-chosen bundle
/// identity, so there is no way to start a process under a bundle's name
/// without deriving that name from the bundle. The value is the synthetic tone
/// backend's, which no real bundle can hash to.
/// `study_tts_testkit::DETERMINISTIC_TONE_BUNDLE_HASH` is defined as this
/// constant rather than as a second copy of the literal, so the two cannot
/// drift apart.
pub const PROTOCOL_FAKE_BUNDLE_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000001";

/// Resolves `root` so it still names the same directory from another
/// working directory.
///
/// # Errors
///
/// [`BuildError::Io`] naming the root, when it cannot be resolved — which for a
/// governed root usually means it is not attached.
fn absolute(root: &Path) -> Result<PathBuf, BuildError> {
    root.canonicalize()
        .map_err(|error| crate::io_error(root, error))
}

/// The directory beneath `bundle_root` that the worker package imports from.
///
/// Derived from [`WORKER_PACKAGE_ROOT`] rather than written out, so a package
/// that moves takes its import root with it.
fn import_root(bundle_root: &Path) -> PathBuf {
    bundle_root.join(
        Path::new(WORKER_PACKAGE_ROOT)
            .parent()
            .unwrap_or(Path::new("")),
    )
}

impl WorkerTtsExecutor {
    /// Starts one worker, initializes it, and reads back what it can do.
    ///
    /// The two exchanges are ordered and both are load-bearing. `initialize`
    /// loads the model and returns the identities every cache key is built
    /// from; `capabilities` returns the envelope [`TtsExecutor::validate`]
    /// refuses against. Doing them here rather than lazily is what makes
    /// ADR-0001 §10.1's one-load-per-lifetime observable: nothing later in this
    /// type loads anything.
    ///
    /// # Errors
    ///
    /// [`BuildError::Synthesis`] carrying [`BackendError::Protocol`] when the
    /// worker cannot be started or answers with something this build cannot
    /// read, [`BackendError::Timeout`] when it does not answer in time, and
    /// [`BackendError::Execution`] when it refuses to initialize — which is
    /// what a bundle-identity disagreement arrives as.
    pub fn start(configuration: &WorkerConfiguration) -> Result<Self, BuildError> {
        // Refused here rather than at the frame: the protocol carries paths as
        // UTF-8 text, and a root this build cannot spell is a containment
        // boundary the worker would never be told about.
        let staging_root =
            configuration
                .staging_root
                .to_str()
                .ok_or_else(|| BackendError::Protocol {
                    request_id: "initialize".to_owned(),
                    message: "the assigned staging root is not UTF-8, which the protocol requires"
                        .to_owned(),
                })?;
        let invocation = ToolInvocation::new(
            "study-tts-worker",
            ToolOperation::WorkerSession,
            &configuration.program,
        );
        let mut client = WorkerClient::spawn(
            invocation,
            &configuration.program,
            &configuration.arguments,
            &configuration.working_directory,
            &configuration.environment,
        )?;

        let identities = match client.request(
            "initialize",
            &WorkerRequestFrame::Initialize {
                protocol_version: WORKER_PROTOCOL_VERSION.to_owned(),
                request_id: "initialize".to_owned(),
                parameters: InitializeParameters {
                    worker_bundle_hash: configuration.worker_bundle_hash.clone(),
                    threads: configuration.threads,
                    staging_root: staging_root.to_owned(),
                },
            },
            configuration.initialize_deadline,
        )? {
            WorkerResponseFrame::Initialized { identities, .. } => identities,
            frame => return Err(unexpected_frame("initialize", &frame).into()),
        };

        // `initialize` *sends* the bundle identity this build derived and
        // verified, so the worker's is an echo and nothing else — and it is the
        // echo, not the verified value, that reaches [`BackendDescriptor`] and
        // therefore every cache key ADR-0001 §12.5 builds. Compared before
        // `capabilities`, so a worker this build cannot name never opens a
        // session at all.
        //
        // TODO(rsstdd): the model half of this rule is issue #66. Model and
        // tokenizer revisions are still strings the worker read out of the
        // governed root's acquisition record, never digests of the bytes it
        // loaded, so changed weights keep an unchanged synthesis identity.
        if identities.worker_bundle_hash != configuration.worker_bundle_hash {
            return Err(BackendError::InvalidRequest {
                request_id: "initialize".to_owned(),
                source: BackendValidationError::BundleIdentityNotEchoed {
                    sent: configuration.worker_bundle_hash.as_str().to_owned(),
                    answered: identities.worker_bundle_hash.as_str().to_owned(),
                },
            }
            .into());
        }

        // The same rule for the model, and the half that makes
        // [`crate::verify_model_artifacts`] mean anything. That gate hashed
        // `model-{configuration.model_revision}`; the worker decides which
        // directory to load by the revision it reads from the governed
        // acquisition record and reports here. If the two disagree it has
        // loaded weights nothing proved, under a revision ADR-0001 §12.5 would
        // key the audio on.
        if identities.model_revision != configuration.model_revision {
            return Err(BackendError::InvalidRequest {
                request_id: "initialize".to_owned(),
                source: BackendValidationError::ModelRevisionNotEchoed {
                    verified: configuration.model_revision.as_str().to_owned(),
                    answered: identities.model_revision.as_str().to_owned(),
                },
            }
            .into());
        }

        let capabilities = match client.request(
            "capabilities",
            &WorkerRequestFrame::Capabilities {
                protocol_version: WORKER_PROTOCOL_VERSION.to_owned(),
                request_id: "capabilities".to_owned(),
            },
            configuration.request_deadline,
        )? {
            WorkerResponseFrame::Capabilities { capabilities, .. } => capabilities,
            frame => return Err(unexpected_frame("capabilities", &frame).into()),
        };

        let mut languages = BTreeSet::new();
        for declared in &capabilities.languages {
            // Parsed rather than trusted: a worker that declares a malformed
            // tag is refused here, not at the cache after audio exists.
            let tag: LanguageTag = declared.parse().map_err(|_| {
                protocol_failure(
                    "capabilities",
                    &format!("the worker declared the unreadable language tag `{declared}`"),
                )
            })?;
            languages.insert(tag);
        }

        // A property of the worker, so it is checked once here rather than
        // per request. ADR-0001 §12.3 fixes the canonical intermediate format;
        // a worker rendering another would have every take refused by the cache
        // *after* the model had run, which is the expensive way to learn it.
        if capabilities.sample_rate != CANONICAL_SAMPLE_RATE
            || capabilities.channels != CANONICAL_CHANNELS
            || capabilities.sample_format != CANONICAL_SAMPLE_FORMAT
        {
            return Err(BackendError::InvalidRequest {
                request_id: "capabilities".to_owned(),
                source: BackendValidationError::NonCanonicalFormat {
                    sample_rate: capabilities.sample_rate,
                    channels: capabilities.channels,
                    sample_format: capabilities.sample_format.clone(),
                },
            }
            .into());
        }

        Ok(Self {
            declared_voices: capabilities.voices.iter().cloned().collect(),
            declared_styles: capabilities.styles.iter().cloned().collect(),
            descriptor: BackendDescriptor {
                contract_version: TTS_EXECUTOR_CONTRACT_VERSION.to_owned(),
                worker_bundle_hash: identities.worker_bundle_hash,
                model_repository: configuration.model_repository.clone(),
                model_revision: identities.model_revision,
                tokenizer_revision: identities.tokenizer_revision,
                languages,
                determinism_class: if capabilities.deterministic_seed {
                    DeterminismClass::Reproducible
                } else {
                    DeterminismClass::SeededNondeterministic
                },
                seed: configuration.seed,
                generation_parameters: configuration.generation_parameters.clone(),
                max_text_bytes: usize::try_from(capabilities.max_text_bytes).unwrap_or(usize::MAX),
            },
            request_deadline: configuration.request_deadline,
            client: Mutex::new(client),
        })
    }

    /// Everything the worker has written to standard error so far.
    ///
    /// ADR-0001 §16 keeps source text and voice paths off that stream, so this
    /// is diagnostics and nothing else. It is also how
    /// `t5_e1_model_load_occurs_once_per_worker_lifetime` counts model loads
    /// without moving the protocol version.
    pub fn diagnostics(&self) -> String {
        self.locked_client().diagnostics()
    }

    /// Closes the worker down and proves its process tree is gone.
    ///
    /// # Errors
    ///
    /// [`BackendError::Protocol`] carrying what the containment boundary
    /// reported.
    pub fn shutdown(&self) -> Result<(), BackendError> {
        self.locked_client().shutdown()
    }

    /// The worker, recovering rather than panicking on a poisoned mutex.
    ///
    /// Poisoning means a caller panicked mid-exchange. The client refuses every
    /// later request on its own after an interrupted one, so recovering here
    /// surfaces that typed refusal instead of replacing a caller's panic with
    /// one of ours.
    fn locked_client(&self) -> std::sync::MutexGuard<'_, WorkerClient> {
        self.client
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Runs one synthesis exchange and reports what the worker said it wrote.
    fn synthesize_through_worker(
        &self,
        request: SynthesisRequest,
        destination: &Path,
    ) -> Result<SynthesisReport, BackendError> {
        self.validate(&request)?;
        let output = destination
            .to_str()
            .ok_or_else(|| BackendError::Destination {
                request_id: request.request_id.clone(),
                destination: destination.to_path_buf(),
                message: "the assigned staging path is not UTF-8, which the protocol requires"
                    .to_owned(),
            })?
            .to_owned();

        // Held across the whole exchange rather than taken twice: the identity
        // below counts this worker's exchanges, so reading it and using it must
        // be one critical section or two callers could compose the same one.
        let mut client = self.locked_client();
        let wire_request_id = client.next_request_id(&request.request_id)?;

        let frame = WorkerRequestFrame::Synthesize {
            protocol_version: WORKER_PROTOCOL_VERSION.to_owned(),
            request_id: wire_request_id.clone(),
            parameters: WorkerSynthesisParameters {
                text: request.spoken_text.clone(),
                // The resolved profile identity, which is what this field has
                // always meant. The planner resolves it from the lesson's
                // speaker bindings; a worker has never seen a lesson and could
                // not resolve a speaker name itself.
                voice: request.voice_profile.clone(),
                style: request.style.clone(),
                seed: self.descriptor.seed,
                take: request.take,
                output,
                trace_context: None,
            },
        };

        let response = client.request(&wire_request_id, &frame, self.request_deadline)?;
        drop(client);
        let report = match response {
            WorkerResponseFrame::SynthesisSucceeded {
                sample_rate,
                channels,
                frames,
                model_revision,
                codec_revision,
                worker_bundle_hash,
                voice_conditioning_hash,
                voice_profile,
                ..
            } => {
                // Every identity a success frame restates is compared against
                // what this executor initialized with, not just the bundle. All
                // four are ADR-0001 §12.5 key inputs, and the cache recomputes
                // the key from `self.descriptor` — so a worker that reloaded
                // underneath us produces a key describing the *initialized*
                // worker while the audio came from another one, and the entry
                // publishes because nothing downstream can see the difference.
                // This is the only place both halves are in hand at once.
                check_identity(
                    &request.request_id,
                    DriftedIdentity::WorkerBundle,
                    self.descriptor.worker_bundle_hash.as_str(),
                    worker_bundle_hash.as_str(),
                )?;
                check_identity(
                    &request.request_id,
                    DriftedIdentity::Model,
                    self.descriptor.model_revision.as_str(),
                    &model_revision,
                )?;
                check_identity(
                    &request.request_id,
                    DriftedIdentity::Codec,
                    self.descriptor.tokenizer_revision.as_str(),
                    &codec_revision,
                )?;
                // Against the request rather than the descriptor: the profile
                // is the plan's choice per segment, not a property of the
                // worker, and `synthesize` sent this exact value out.
                check_identity(
                    &request.request_id,
                    DriftedIdentity::VoiceProfile,
                    &request.voice_profile,
                    &voice_profile,
                )?;
                SynthesisReport {
                    sample_rate,
                    channels,
                    frames,
                    backend_revision: model_revision,
                    // Built from the conditioning artifact the *worker*
                    // reported, not the one the request carried. That is the
                    // whole point: the cache recomputes the synthesis key from
                    // this context and refuses publication when it is not the
                    // key the plan derived, so a worker whose voice root
                    // disagrees with the planner's is caught here rather than
                    // rendering a voice nobody asked for. Echoing the request
                    // back would make that gate pass by construction, which
                    // `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md` §Limits
                    // this change does not close recorded as owed to E1-S3.
                    context: self.descriptor.synthesis_context(
                        request.language.clone(),
                        BTreeMap::from([(request.voice.clone(), voice_conditioning_hash.clone())]),
                    ),
                    voice_conditioning_hash,
                    voice_profile,
                }
            }
            frame => return Err(unexpected_frame(&request.request_id, &frame)),
        };

        // The worker was assigned exactly one path to write, and a success
        // frame is a claim that it did. ADR-0001 §10.3 confines worker writes
        // to the assigned staging root, and this is the half of that rule Rust
        // can enforce: it cannot stop a worker writing elsewhere, but it can
        // refuse to report success for audio that is not where it asked for it.
        //
        // `symlink_metadata`, so a link dropped at the assigned path is refused
        // rather than followed. The cache re-resolves this path through
        // `managed::leaf` before it reads it and would refuse the link too;
        // this refuses it while the request that caused it is still in hand,
        // and names the worker rather than the audio.
        match std::fs::symlink_metadata(destination) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => {
                return Err(BackendError::Destination {
                    request_id: request.request_id,
                    destination: destination.to_path_buf(),
                    message: "the worker reported success but the assigned path is not a regular \
                              file"
                        .to_owned(),
                });
            }
            Err(source) => {
                return Err(BackendError::Destination {
                    request_id: request.request_id,
                    destination: destination.to_path_buf(),
                    message: format!(
                        "the worker reported success but wrote nothing readable to the path it \
                         was assigned: {source}"
                    ),
                });
            }
        }

        Ok(report)
    }
}

impl TtsExecutor for WorkerTtsExecutor {
    fn descriptor(&self) -> BackendDescriptor {
        self.descriptor.clone()
    }

    fn capacity(&self) -> usize {
        // One, and not a configured value: ADR-0001 §10.1 gives each worker
        // process one request at a time, and a pool of more than one is
        // E5-S2's, behind the `doctor` RAM and core budgets it requires.
        1
    }

    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError> {
        // The worker-declared half of the envelope, which only this type knows:
        // `validate_executor_request` is shared with every executor and reads
        // the descriptor, and these two lists are deliberately not on it.
        let declared = if self.declared_styles.contains(&request.style) {
            None
        } else {
            Some(BackendValidationError::UndeclaredStyle {
                requested: request.style.clone(),
                declared: render_declared(&self.declared_styles),
            })
        }
        .or_else(|| {
            if self.declared_voices.contains(&request.voice_profile) {
                return None;
            }
            Some(BackendValidationError::UndeclaredVoiceProfile {
                requested: request.voice_profile.clone(),
                declared: render_declared(&self.declared_voices),
            })
        });
        if let Some(source) = declared {
            return Err(BackendError::InvalidRequest {
                request_id: request.request_id.clone(),
                source,
            });
        }

        validate_executor_request(&self.descriptor, self.capacity(), request).map_err(|source| {
            BackendError::InvalidRequest {
                request_id: request.request_id.clone(),
                source,
            }
        })
    }

    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
        destination: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>> {
        // Ready rather than deferred: the exchange is blocking and this
        // workspace has no async runtime to yield to. The boxed future is the
        // ADR-0001 §10.4 signature, which E5-S2's pool will drive for real.
        Box::pin(async move { self.synthesize_through_worker(request, destination) })
    }
}

/// Refuses `identity` when the worker's answer is not what it initialized with.
///
/// # Errors
///
/// [`BackendError::IdentityDrift`] naming `identity`, both values, and the
/// request, so an operator sees which of the four disagreed rather than that
/// one of them did.
fn check_identity(
    request_id: &str,
    identity: DriftedIdentity,
    expected: &str,
    found: &str,
) -> Result<(), BackendError> {
    if expected == found {
        return Ok(());
    }
    Err(BackendError::IdentityDrift {
        request_id: request_id.to_owned(),
        identity,
        message: format!("expected `{expected}`, found `{found}`"),
    })
}

/// The refusal for a worker frame that is not the answer this exchange needs.
fn unexpected_frame(request_id: &str, frame: &WorkerResponseFrame) -> BackendError {
    protocol_failure(
        request_id,
        &format!("the worker answered with `{}`", frame.event_name()),
    )
}

/// Builds the protocol refusal for `request_id`.
fn protocol_failure(request_id: &str, message: &str) -> BackendError {
    BackendError::Protocol {
        request_id: request_id.to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolError;
    use crate::worker_bundle::tests::bundle_copy;

    #[test]
    fn t1_e1_a_bundle_configuration_derives_its_identity_rather_than_being_told_one() {
        // ADR-0001 §12.5 makes the bundle identity a term of every cache key
        // the worker's audio is stored under, so an identity a caller supplied
        // is a claim about code nobody checked — and every entry keyed on it
        // would describe audio some other bundle produced. `for_bundle`
        // therefore derives it, and derivation is what fails here: the copied
        // bundle carries every declared input and no interpreter, so
        // `verified_hash` refuses before a configuration exists to launch
        // anything with.
        let root = bundle_copy();

        let error = WorkerConfiguration::for_bundle(
            root.path(),
            Path::new("/governed/models"),
            Path::new("/governed/voices"),
            Path::new("/staging"),
        )
        .expect_err("a bundle whose interpreter is absent cannot be launched");

        let BuildError::Tool(source) = &error else {
            panic!("the wrong error was produced: {error:?}");
        };
        assert!(
            matches!(source, ToolError::MissingTool { .. }),
            "the refusal must name the missing interpreter, not {error:?}"
        );
    }

    #[test]
    fn t1_e1_a_worker_is_launched_from_the_import_root_its_package_lives_under() {
        // `python -m` resolves the module against the working directory, and
        // the parent of the package is the only directory it resolves from.
        // Derived from `WORKER_PACKAGE_ROOT` rather than written out, so the
        // two cannot disagree if the package ever moves.
        let root = Path::new("/bundle");

        let import_root = import_root(root);

        assert_eq!(
            import_root,
            root.join("worker"),
            "the worker must start from the directory its package is importable from"
        );
        assert!(
            root.join(WORKER_PACKAGE_ROOT).starts_with(&import_root),
            "the import root must contain the package it makes importable"
        );
    }
}
