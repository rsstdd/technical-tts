use std::path::Path;

use serde_json::Value;
use study_tts_runtime::{
    BuildError, BuildRequest, build_preview, publish, validate_production_manifest,
};
use study_tts_testkit::{
    DeterministicToneWorker, cache_identity_fixture, walking_skeleton_fixture,
};
use tempfile::TempDir;

fn build_request(lesson_path: &Path, workspace: &Path) -> BuildRequest {
    BuildRequest {
        lesson_path: lesson_path.to_path_buf(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "ffmpeg".into(),
        ffprobe_executable: "ffprobe".into(),
    }
}

fn run_skeleton() -> (
    TempDir,
    study_tts_runtime::BuildResult,
    DeterministicToneWorker,
) {
    let workspace = TempDir::new().expect("create isolated skeleton workspace");
    let worker = DeterministicToneWorker::default();
    let result = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("walking skeleton should build");

    (workspace, result, worker)
}

#[test]
fn t4_e0_skeleton_produces_wav_m4a_and_minimal_manifest() {
    let (workspace, result, worker) = run_skeleton();

    assert_eq!(worker.synthesis_count(), 2);
    assert!(result.master_wav.is_file());
    assert!(result.m4a.is_file());
    assert!(result.manifest.is_file());
    assert!(
        result
            .master_wav
            .starts_with(workspace.path().join("previews/e0-s0-walking-skeleton"))
    );

    let reader = hound::WavReader::open(&result.master_wav).expect("open assembled WAV");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 24_000);
    assert_eq!(spec.bits_per_sample, 32);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(
        reader.duration(),
        9_600,
        "Rust assembly must write two 2,400-frame tones plus exact 75 ms and 125 ms pauses"
    );

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&result.manifest).expect("read minimal manifest"))
            .expect("parse minimal manifest");
    assert_eq!(manifest["schema_version"], "0.1-skeleton");
    assert_eq!(manifest["release_status"], "private_preview");
    assert_eq!(manifest["lesson_id"], "e0-s0-walking-skeleton");
    assert_eq!(manifest["segments"].as_array().map(Vec::len), Some(2));
    assert!(manifest["artifacts"]["master_wav"]["blake3"].is_string());
    assert!(manifest["artifacts"]["m4a"]["blake3"].is_string());
    assert!(manifest["tools"]["ffmpeg"]["resolved_executable"].is_string());
    assert!(
        manifest["tools"]["ffmpeg"]["version"]
            .as_str()
            .is_some_and(|version| version.starts_with("ffmpeg version"))
    );
    assert!(
        manifest["tools"]["ffmpeg"]["arguments"]
            .as_array()
            .is_some_and(|arguments| arguments.iter().any(|argument| argument == "mono"))
    );
    assert!(
        manifest["tools"]["ffprobe"]["version"]
            .as_str()
            .is_some_and(|version| version.starts_with("ffprobe version"))
    );
}

#[test]
fn t4_e0_skeleton_runs_without_model_artifacts() {
    let workspace = TempDir::new().expect("create isolated no-model workspace");
    let model_trap = workspace.path().join("models");
    std::fs::write(&model_trap, b"model access is forbidden in E0-S0")
        .expect("create model-path trap");
    let worker = DeterministicToneWorker::default();

    build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("walking skeleton must not access model artifacts");

    assert_eq!(
        std::fs::read(model_trap).expect("read unchanged model-path trap"),
        b"model access is forbidden in E0-S0"
    );
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e0_cache_hit_avoids_synthesis_and_is_byte_identical() {
    let (workspace, first, worker) = run_skeleton();
    let second = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("cache rebuild should succeed");

    assert_eq!(
        worker.synthesis_count(),
        2,
        "cache hits must avoid synthesis"
    );
    // This byte comparison applies only to the deterministic fake synthesizer and exact Rust
    // assembly. It does not claim byte-identical Chatterbox output from repeated synthesis.
    assert_eq!(
        std::fs::read(first.master_wav).expect("read first master"),
        std::fs::read(second.master_wav).expect("read second master")
    );
}

#[test]
fn t4_e0_cache_identity_proves_hits_and_speech_affecting_misses() {
    let workspace = TempDir::new().expect("create isolated cache workspace");
    let worker = DeterministicToneWorker::default();

    let result = build_preview(
        build_request(&cache_identity_fixture(), workspace.path()),
        &worker,
    )
    .expect("cache identity fixture should build");

    assert_eq!(
        worker.synthesis_count(),
        3,
        "pause, role, source, and segment ID must hit; spoken text and style must miss"
    );
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(result.manifest).expect("read cache identity manifest"),
    )
    .expect("parse cache identity manifest");
    let segments = manifest["segments"].as_array().expect("manifest segments");
    assert_eq!(segments[0]["cache_key"], segments[1]["cache_key"]);
    assert_ne!(segments[0]["cache_key"], segments[2]["cache_key"]);
    assert_ne!(segments[0]["cache_key"], segments[3]["cache_key"]);
}

#[test]
fn t4_e0_external_tool_preflight_names_missing_binary() {
    let workspace = TempDir::new().expect("create isolated preflight workspace");
    let worker = DeterministicToneWorker::default();
    let mut request = build_request(&walking_skeleton_fixture(), workspace.path());
    request.ffmpeg_executable = "study-tts-missing-ffmpeg".into();

    let error = build_preview(request, &worker).expect_err("missing FFmpeg must fail preflight");
    assert!(matches!(
        error,
        BuildError::MissingTool { ref tool, .. } if tool == "FFmpeg"
    ));
    assert!(error.to_string().contains("study-tts-missing-ffmpeg"));

    let mut request = build_request(&walking_skeleton_fixture(), workspace.path());
    request.ffprobe_executable = "study-tts-missing-ffprobe".into();
    let error = build_preview(request, &worker).expect_err("missing ffprobe must fail preflight");
    assert!(matches!(
        error,
        BuildError::MissingTool { ref tool, .. } if tool == "ffprobe"
    ));
    assert!(error.to_string().contains("study-tts-missing-ffprobe"));
    assert_eq!(
        worker.synthesis_count(),
        0,
        "preflight must run before synthesis"
    );
}

#[test]
fn t4_e0_private_preview_cannot_enter_production_publication() {
    let (_workspace, result, _worker) = run_skeleton();
    let manifest_bytes = std::fs::read(&result.manifest).expect("read preview manifest");

    assert!(matches!(
        publish(&result),
        Err(BuildError::PublicationRefused { .. })
    ));
    assert!(matches!(
        validate_production_manifest(&manifest_bytes),
        Err(BuildError::UnsupportedProductionManifest { ref version })
            if version == "0.1-skeleton"
    ));
}
