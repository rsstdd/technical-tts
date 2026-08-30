//! Test doubles and fixture paths shared by the workspace's integration tests.
//!
//! All generated audio is synthetic, and fixtures are the rights-clean records
//! registered in `docs/testing/TEST-DATA-MANIFEST.md`. Production crates may
//! depend on this crate only from tests.

mod contracts;
mod json_schema;

pub use json_schema::validate_against_schema;

pub use contracts::{
    FakeCachePublisher, FakeJobCall, FakePackageCall, FakePackageWriter, InMemoryJobRepository,
    RecordingCachePublisher, RecordingJobRepository, RecordingPackageWriter, RecordingTtsExecutor,
    SeamEventLog, run_cache_contract_scenario, run_job_repository_contract_scenario,
    run_package_writer_contract_scenario, run_tts_executor_contract_scenario,
};

use std::{
    collections::{BTreeMap, BTreeSet},
    f32::consts::TAU,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use study_tts_core::{
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE, DeterminismClass,
    LanguageTag,
};
use study_tts_runtime::{
    BackendDescriptor, BackendError, SynthesisReport, SynthesisRequest,
    TTS_EXECUTOR_CONTRACT_VERSION, TtsExecutor, validate_executor_request,
};

/// Bundle identity the deterministic tone executor reports.
///
/// A fixed well-formed digest rather than a hash of anything: this executor has
/// no Python bundle to hash, and a constant keeps its cache keys stable across
/// runs while staying distinct from any real bundle's.
pub const DETERMINISTIC_TONE_BUNDLE_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000001";

/// Voice-profile identity the deterministic tone executor reports.
///
/// Fixed synthetic bytes for the same reason as the bundle hash: this executor
/// resolves no voice profile, and a constant keeps what it reports stable while
/// staying distinct from any real profile's.
pub const DETERMINISTIC_TONE_VOICE_PROFILE_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000002";

/// The one language the deterministic tone executor declares.
///
/// English-only, matching ADR-0002's qualified baseline. The tone carries no
/// speech at all, so declaring more would be a claim nothing backs — and the
/// point of a declared capability is that a request outside it is refused
/// before synthesis rather than answered with confident nonsense.
pub const DETERMINISTIC_TONE_LANGUAGE: &str = "en";

/// Frames in every synthetic tone this crate writes: one tenth of a second.
///
/// Short because no test here measures duration; what they measure is that the
/// same inputs produce the same bytes.
const TONE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

/// An observable executor that writes a tone derived from each synthesis key.
///
/// Deterministic bytes prove cache reuse, while call history and one-shot
/// failures let contract tests observe refusal ordering and error propagation.
#[derive(Debug, Default)]
pub struct FakeTtsExecutor {
    touch_count: AtomicUsize,
    synthesis_count: AtomicUsize,
    synthesized_texts: Mutex<Vec<String>>,
    requests: Mutex<Vec<SynthesisRequest>>,
    next_failure: Mutex<Option<BackendError>>,
}

impl FakeTtsExecutor {
    /// Returns every [`TtsExecutor`] call this executor has received.
    ///
    /// What `synthesis_count` cannot say: a gate that runs before *synthesis*
    /// still runs after the build has asked the backend for its descriptor,
    /// and a real worker that starts its process on first use would have
    /// started by then. Zero here is the observable form of "the backend was
    /// never reached", which is as close to "no worker started" as a seam that
    /// receives an already-constructed executor can get — construction itself
    /// belongs to the caller.
    ///
    /// Counts self-calls too: `validate` consults `descriptor`. Tests assert
    /// zero, where that cannot matter.
    pub fn touch_count(&self) -> usize {
        self.touch_count.load(Ordering::SeqCst)
    }

    /// Returns completed synthesis calls so tests can prove a gate ran first.
    pub fn synthesis_count(&self) -> usize {
        self.synthesis_count.load(Ordering::SeqCst)
    }

    /// The spoken text of every segment synthesized so far, in call order.
    ///
    /// A poisoned lock is recovered from rather than panicked on: poisoning
    /// means a test already failed while holding it, and panicking here would
    /// replace that failure with this one.
    pub fn synthesized_texts(&self) -> Vec<String> {
        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Returns successfully synthesized requests in call order.
    pub fn requests(&self) -> Vec<SynthesisRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Injects one typed failure into the next validated synthesis call.
    pub fn fail_next(&self, error: BackendError) {
        *self
            .next_failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(error);
    }
}

impl TtsExecutor for FakeTtsExecutor {
    fn descriptor(&self) -> BackendDescriptor {
        self.touch_count.fetch_add(1, Ordering::SeqCst);
        BackendDescriptor {
            contract_version: TTS_EXECUTOR_CONTRACT_VERSION.to_owned(),
            // Fixed, well-formed stand-ins for the real bundle and model
            // identities. The fake synthesizes a tone rather than loading a
            // model, so these exist to make its keys stable and distinct from
            // any real backend's, not to describe an artifact on disk.
            worker_bundle_hash: DETERMINISTIC_TONE_BUNDLE_HASH
                .parse()
                .expect("the fake bundle hash is a well-formed digest"),
            model_repository: "study-tts/deterministic-tone".to_owned(),
            model_revision: "v1".parse().expect("`v1` is a revision"),
            tokenizer_revision: "none".parse().expect("`none` is a revision"),
            languages: BTreeSet::from([tone_language()]),
            determinism_class: DeterminismClass::Reproducible,
            seed: 0,
            generation_parameters: BTreeMap::new(),
            max_text_bytes: 64 * 1024,
        }
    }

    fn capacity(&self) -> usize {
        self.touch_count.fetch_add(1, Ordering::SeqCst);
        1
    }

    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError> {
        self.touch_count.fetch_add(1, Ordering::SeqCst);
        validate_executor_request(&self.descriptor(), self.capacity(), request).map_err(|source| {
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
        self.touch_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move { self.synthesize_tone(request, destination) })
    }
}

impl FakeTtsExecutor {
    fn synthesize_tone(
        &self,
        request: SynthesisRequest,
        destination: &Path,
    ) -> Result<SynthesisReport, BackendError> {
        self.validate(&request)?;
        if let Some(error) = self
            .next_failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
        {
            return Err(error);
        }
        let freq = 300.0
            + f32::from(
                request
                    .cache_key
                    .as_str()
                    .bytes()
                    .fold(0_u8, |accumulator, byte| accumulator.wrapping_add(byte)),
            );
        let spec = hound::WavSpec {
            channels: CANONICAL_CHANNELS,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(destination, spec).map_err(|error| {
            BackendError::Destination {
                request_id: request.request_id.clone(),
                destination: destination.to_path_buf(),
                message: error.to_string(),
            }
        })?;

        for frame in 0..TONE_FRAMES {
            let phase = TAU * freq * frame as f32 / CANONICAL_SAMPLE_RATE as f32;
            writer
                .write_sample(phase.sin() * 0.2)
                .map_err(|error| BackendError::Destination {
                    request_id: request.request_id.clone(),
                    destination: destination.to_path_buf(),
                    message: error.to_string(),
                })?;
        }

        writer
            .finalize()
            .map_err(|error| BackendError::Destination {
                request_id: request.request_id.clone(),
                destination: destination.to_path_buf(),
                message: error.to_string(),
            })?;

        let language = request.language.clone();
        let voice = request.voice.clone();
        let conditioning = request.voice_conditioning_hash.clone();
        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.spoken_text.clone());
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request);

        self.synthesis_count.fetch_add(1, Ordering::SeqCst);

        Ok(SynthesisReport {
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: CANONICAL_CHANNELS,
            frames: TONE_FRAMES,
            backend_revision: "deterministic-tone-v1".to_owned(),
            // Built from this executor's own descriptor and the request it was
            // handed, which is what a real worker does: it reports the inputs
            // it loaded, not the ones it was told about. Reporting anything
            // else here — as an earlier version of this fake did, with a bundle
            // hash unrelated to its descriptor's — is exactly the drift the
            // cache's identity gate refuses, so a fake that did it could never
            // publish and would stop being a usable double.
            // The conditioning artifact comes from the request rather than
            // from anywhere this fake could invent one: a real worker reports
            // the artifact it loaded, and echoing the requested one is the
            // closest a fake that loads nothing can honestly get. A hash made
            // up here would name a cache entry no voice produced, and the
            // cache's identity gate would refuse it — which is the point.
            //
            // The echo is also why that gate proves nothing yet.
            // `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md` §Limits this
            // change does not close records it as owed to `DELIVERY-PLAN.md`
            // E1-S3: the Chatterbox worker must report the artifact it read
            // from disk, never the value it was handed, or the comparison
            // stays a tautology that this suite cannot catch.
            context: self
                .descriptor()
                .synthesis_context(language, BTreeMap::from([(voice, conditioning)])),
            voice_profile_hash: DETERMINISTIC_TONE_VOICE_PROFILE_HASH
                .parse()
                .expect("the fake voice profile hash is a well-formed digest"),
        })
    }
}

/// The language [`FakeTtsExecutor`] declares and reports.
///
/// Parsed once here rather than at each use so the constant and the tag cannot
/// disagree; `DETERMINISTIC_TONE_LANGUAGE` is checked to be well formed by the
/// same parse every lesson goes through.
fn tone_language() -> LanguageTag {
    DETERMINISTIC_TONE_LANGUAGE
        .parse()
        .expect("the fake executor language is a well-formed tag")
}

/// Backward-compatible test name for the deterministic E0 fake executor.
pub type DeterministicToneWorker = FakeTtsExecutor;

/// Options for a synthetic, rights-clean voice-profile fixture.
///
/// No real voice audio is involved: `reference.wav` is a generated tone and
/// `conditionals.pt` is fixed synthetic bytes, per the CI rule that real voice
/// references never enter Git or CI
/// (`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access).
/// Statuses are plain strings so tests can write invalid or unknown values
/// directly.
#[derive(Clone, Debug)]
pub struct VoiceProfileFixtureSpec {
    /// Value of `profile_id` in `profile.json`.
    pub profile_id: String,
    /// Value of `consent_status` in `consent.json`.
    pub consent_status: String,
    /// Value of `approval` in `profile.json`.
    pub approval: String,
    /// Whether `consent.json` is written at all.
    pub write_consent: bool,
}

impl Default for VoiceProfileFixtureSpec {
    fn default() -> Self {
        Self {
            profile_id: "synthetic-test-voice-v1".to_owned(),
            consent_status: "granted".to_owned(),
            approval: "approved".to_owned(),
            write_consent: true,
        }
    }
}

/// Writes a voice profile directory in the ADR-0001 §12.1 layout into `dir`.
///
/// Produces `profile.json`, `reference.wav`, `conditionals.pt`, and (unless
/// disabled) `consent.json`, with self-consistent BLAKE3 checksums, and returns
/// the profile directory. Registered as `voice-profile-synthetic-v1` in
/// `docs/testing/TEST-DATA-MANIFEST.md`; the two must stay in step.
///
/// # Panics
///
/// If any of those files cannot be written or read back for hashing. Callers
/// are tests writing into a fresh temporary directory, where a filesystem
/// failure is a broken test environment rather than a case to handle.
pub fn write_voice_profile_fixture(dir: &Path, spec: &VoiceProfileFixtureSpec) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create voice profile fixture directory");

    let reference_path = dir.join("reference.wav");
    let wav_spec = hound::WavSpec {
        channels: CANONICAL_CHANNELS,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&reference_path, wav_spec)
        .expect("create synthetic reference audio");
    for frame in 0..TONE_FRAMES {
        let phase = TAU * 440.0 * frame as f32 / CANONICAL_SAMPLE_RATE as f32;
        writer
            .write_sample(phase.sin() * 0.2)
            .expect("write synthetic reference audio");
    }
    writer
        .finalize()
        .expect("finalize synthetic reference audio");

    let conditionals_path = dir.join("conditionals.pt");
    std::fs::write(&conditionals_path, b"synthetic-conditionals-v1")
        .expect("write synthetic conditionals");

    let reference_hash = hash_fixture_file(&reference_path);
    let conditionals_hash = hash_fixture_file(&conditionals_path);

    let profile = serde_json::json!({
        "schema_version": "0.1-voice",
        "profile_id": spec.profile_id,
        "reference_wav_blake3": reference_hash,
        "conditionals_blake3": conditionals_hash,
        "extractor_identity": "synthetic-extractor-v1",
        "approval": spec.approval,
    });
    std::fs::write(
        dir.join("profile.json"),
        serde_json::to_vec_pretty(&profile).expect("serialize profile record"),
    )
    .expect("write profile record");

    if spec.write_consent {
        let consent = serde_json::json!({
            "schema_version": "0.1-voice",
            "declaration": "Synthetic test fixture; generated tone, no human voice.",
            "permitted_use": ["private_synthesis"],
            "reference_wav_blake3": reference_hash,
            "created": "2026-08-23",
            "consent_status": spec.consent_status,
            "rights_record_id": "rights-voice-owner-fallback-v1",
        });
        std::fs::write(
            dir.join("consent.json"),
            serde_json::to_vec_pretty(&consent).expect("serialize consent record"),
        )
        .expect("write consent record");
    }

    dir.to_path_buf()
}

/// Writes one default synthetic profile per identifier beneath `root`.
///
/// A build resolves `speakers[*].voice_profile` to `<root>/<profile_id>/`, so
/// this is the shape every pipeline test needs: the profile identifiers the
/// committed lesson fixtures name, installed where the build will look.
///
/// # Panics
///
/// On the same terms as [`write_voice_profile_fixture`], which it calls.
pub fn write_voice_profile_root(root: &Path, profile_ids: &[&str]) -> PathBuf {
    for profile_id in profile_ids {
        write_voice_profile_fixture(
            &root.join(profile_id),
            &VoiceProfileFixtureSpec {
                profile_id: (*profile_id).to_owned(),
                ..VoiceProfileFixtureSpec::default()
            },
        );
    }
    root.to_path_buf()
}

/// The voice profiles the committed lesson fixtures declare.
///
/// Named here rather than repeated per test so a fixture gaining a speaker is
/// one edit. Mirrors the `speakers` blocks in `fixtures/lessons/`.
pub const FIXTURE_VOICE_PROFILES: [&str; 2] =
    ["synthetic-test-voice-v1", "synthetic-test-voice-v2"];

/// Hashes a fixture file so the record written beside it agrees with its
/// bytes.
///
/// Panics on the same terms as [`write_voice_profile_fixture`], which is its
/// only caller.
fn hash_fixture_file(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read fixture file for hashing");
    blake3::hash(&bytes).to_hex().to_string()
}

/// The two-segment lesson the walking-skeleton tests build.
pub fn walking_skeleton_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lessons/e0-s0-two-segment.json")
}

/// A lesson whose segments differ by one speech-affecting field each, for
/// cache-identity tests.
pub fn cache_identity_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lessons/e0-s0-cache-identity.json")
}
