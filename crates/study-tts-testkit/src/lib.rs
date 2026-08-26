//! Test doubles and fixture paths shared by the workspace's integration tests.
//!
//! All generated audio is synthetic, and fixtures are the rights-clean records
//! registered in `docs/testing/TEST-DATA-MANIFEST.md`. Production crates may
//! depend on this crate only from tests.

mod contracts;

pub use contracts::{
    FakeCachePublisher, FakeJobCall, FakePackageCall, FakePackageWriter, InMemoryJobRepository,
    RecordingCachePublisher, RecordingJobRepository, RecordingPackageWriter, RecordingTtsExecutor,
    SeamEventLog, run_cache_contract_scenario, run_job_repository_contract_scenario,
    run_package_writer_contract_scenario, run_tts_executor_contract_scenario,
};

use std::{
    f32::consts::TAU,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use study_tts_core::{CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE};
use study_tts_runtime::{
    BackendDescriptor, BackendError, SynthesisReport, SynthesisRequest,
    TTS_EXECUTOR_CONTRACT_VERSION, TtsExecutor, validate_executor_request,
};

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
    synthesis_count: AtomicUsize,
    synthesized_texts: Mutex<Vec<String>>,
    requests: Mutex<Vec<SynthesisRequest>>,
    next_failure: Mutex<Option<BackendError>>,
}

impl FakeTtsExecutor {
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
        BackendDescriptor {
            contract_version: TTS_EXECUTOR_CONTRACT_VERSION.to_owned(),
            synthesis_identity: "deterministic-tone-worker-v1".to_owned(),
            max_text_bytes: 64 * 1024,
            deterministic_seed: true,
        }
    }

    fn capacity(&self) -> usize {
        1
    }

    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError> {
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
            worker_bundle_hash: blake3::hash(b"deterministic-tone-worker-v1")
                .to_hex()
                .to_string(),
            voice_profile_hash: blake3::hash(b"synthetic-test-voice-v1")
                .to_hex()
                .to_string(),
        })
    }
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
