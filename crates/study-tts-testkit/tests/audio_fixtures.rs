//! T4: the canonical-audio gate, driven by committed bytes rather than by
//! bytes the test just wrote.
//!
//! A test that writes a WAV with `hound` and validates it with `hound` proves
//! only that one library agrees with itself. `fixtures/audio/` holds pinned
//! files instead, so a change to the writer, the validator, or the format
//! constant in ADR-0001 §13.1 shows up here as a failure rather than as two
//! halves of the code moving together.

use std::{
    fs,
    path::{Path, PathBuf},
};

use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, DeterminismClass,
    PlannedSegment, SynthesisContext,
};
use study_tts_runtime::{
    AudioError, BackendError, BuildError, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, SynthesisReport,
};
use tempfile::TempDir;

fn audio_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/audio")
        .join(name)
}

/// The inputs the producer below reports having used.
fn context() -> SynthesisContext {
    SynthesisContext {
        worker_bundle_hash: "1".repeat(64).parse().expect("a digest of ones parses"),
        model_repository: "study-tts/deterministic-tone".to_owned(),
        model_revision: "v1".parse().expect("`v1` is a revision"),
        tokenizer_revision: "none".parse().expect("`none` is a revision"),
        language: "en".parse().expect("`en` is a well-formed language tag"),
        determinism_class: DeterminismClass::Reproducible,
        seed: 0,
        generation_parameters: std::collections::BTreeMap::new(),
        voice_conditioning_hashes: std::collections::BTreeMap::new(),
    }
}

/// A segment whose cache key is the one [`context`] derives for it.
fn segment() -> PlannedSegment {
    let mut planned = PlannedSegment {
        id: "seg-0001".to_owned(),
        speaker: "synthetic-test-voice-v1".to_owned(),
        spoken_text: "A cache stores reusable work.".to_owned(),
        style: "calm_explanatory".to_owned(),
        pause_after_ms: 0,
        take: study_tts_core::BASE_TAKE,
        // Replaced below; a hand-written key would be refused by the identity
        // gate before the audio gate this test is about could run.
        cache_key: "0".repeat(64).parse().expect("a digest of zeros parses"),
    };
    planned.cache_key = context().key_for(&planned);
    planned
}

/// Publishes one committed fixture as if a worker had just written it.
fn publish(fixture: &str) -> Result<(), BuildError> {
    let workspace = TempDir::new().expect("create a cache workspace");
    let source = audio_fixture(fixture);
    let frames = hound::WavReader::open(&source)
        .expect("the fixture is readable as WAV")
        .duration();

    let mut producer = |destination: &Path| {
        fs::copy(&source, destination).map_err(|error| BackendError::Destination {
            request_id: "audio-fixture".to_owned(),
            destination: destination.to_path_buf(),
            message: error.to_string(),
        })?;
        Ok(SynthesisReport {
            // Reported from the fixture's own header, so a refusal below is the
            // audio gate refusing the format rather than the report gate
            // refusing a miscount.
            sample_rate: CANONICAL_SAMPLE_RATE,
            channels: CANONICAL_CHANNELS,
            frames,
            backend_revision: "audio-fixture-v1".to_owned(),
            context: context(),
            voice_profile_hash: blake3::hash(b"synthetic-test-voice-v1").into(),
        })
    };

    FileSystemCachePublisher
        .resolve(
            &CacheResolveRequest {
                workspace: workspace.path().to_path_buf(),
                job_id: "audio-fixture-job".to_owned(),
                segment: segment(),
            },
            &mut producer,
        )
        .map(|_| ())
}

#[test]
fn t4_e1_the_canonical_audio_fixture_publishes() {
    publish("e1-s1-canonical-tone.wav").expect("the canonical fixture must satisfy the audio gate");

    // The fixture really is the format ADR-0001 §13.1 fixes, read from the file
    // rather than assumed: a fixture regenerated against a changed constant
    // would otherwise pass this suite while describing a different format.
    let spec = hound::WavReader::open(audio_fixture("e1-s1-canonical-tone.wav"))
        .expect("the canonical fixture is readable")
        .spec();
    assert_eq!(spec.sample_rate, CANONICAL_SAMPLE_RATE);
    assert_eq!(spec.channels, CANONICAL_CHANNELS);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(CANONICAL_SAMPLE_FORMAT, "f32le");
}

#[test]
fn t4_e1_non_canonical_audio_fixtures_are_refused() {
    // Each fixture differs from the canonical one in exactly one property, so a
    // refusal is attributable to that property. The integer variant is the one
    // that matters most: it has the canonical bit depth, so a validator
    // checking only the width would accept a stream that is not float at all.
    for fixture in [
        "e1-s1-noncanonical-48k.wav",
        "e1-s1-noncanonical-integer.wav",
    ] {
        let error = publish(fixture).expect_err("a non-canonical fixture must be refused");

        assert!(
            matches!(error, BuildError::Audio(AudioError::UnusableAudio { .. })),
            "`{fixture}` must be refused as unusable audio, got {error:?}"
        );
    }
}
