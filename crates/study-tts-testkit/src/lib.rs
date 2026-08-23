//! Test doubles and fixture paths shared by the workspace's integration tests.
//!
//! Nothing here reaches a lesson: the worker synthesizes tones, and the
//! fixtures are the committed synthetic lessons registered in
//! `docs/testing/TEST-DATA-MANIFEST.md`.

use std::{
    f32::consts::TAU,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use study_tts_core::{CANONICAL_SAMPLE_RATE, PlannedSegment};
use study_tts_runtime::{SegmentSynthesizer, SynthesisError, SynthesisReport};

const TONE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

/// A synthesizer that writes a tone derived from the segment's cache key.
///
/// Deterministic so a cache hit is byte-identical, and counting so a test can
/// prove that a gate refused before any synthesis happened rather than merely
/// that the build failed.
#[derive(Debug, Default)]
pub struct DeterministicToneWorker {
    synthesis_count: AtomicUsize,
    synthesized_texts: Mutex<Vec<String>>,
}

impl DeterministicToneWorker {
    /// How many segments this worker has synthesized, for asserting that a gate ran
    /// first.
    pub fn synthesis_count(&self) -> usize {
        self.synthesis_count.load(Ordering::SeqCst)
    }

    /// The spoken text of every segment synthesized so far, in call order.
    pub fn synthesized_texts(&self) -> Vec<String> {
        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl SegmentSynthesizer for DeterministicToneWorker {
    fn identity(&self) -> &str {
        "deterministic-tone-worker-v1"
    }

    fn synthesize(
        &self,
        segment: &PlannedSegment,
        destination: &Path,
    ) -> Result<SynthesisReport, SynthesisError> {
        let freq = 300.0
            + f32::from(
                segment
                    .cache_key
                    .as_str()
                    .bytes()
                    .fold(0_u8, |accumulator, byte| accumulator.wrapping_add(byte)),
            );
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(destination, spec)
            .map_err(|error| SynthesisError::new(error.to_string()))?;

        for frame in 0..TONE_FRAMES {
            let phase = TAU * freq * frame as f32 / CANONICAL_SAMPLE_RATE as f32;
            writer
                .write_sample(phase.sin() * 0.2)
                .map_err(|error| SynthesisError::new(error.to_string()))?;
        }

        writer
            .finalize()
            .map_err(|error| SynthesisError::new(error.to_string()))?;

        self.synthesized_texts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(segment.spoken_text.clone());

        self.synthesis_count.fetch_add(1, Ordering::SeqCst);

        Ok(SynthesisReport {
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: 1,
            frames: TONE_FRAMES,
        })
    }
}

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
pub fn write_voice_profile_fixture(dir: &Path, spec: &VoiceProfileFixtureSpec) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create voice profile fixture directory");

    let reference_path = dir.join("reference.wav");
    let wav_spec = hound::WavSpec {
        channels: 1,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: 32,
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
