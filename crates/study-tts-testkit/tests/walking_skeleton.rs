use std::{
    process::Command,
    time::{Duration, Instant},
};

use serde_json::Value;
use study_tts_runtime::{BuildRequest, build_preview};
use study_tts_testkit::{DeterministicToneWorker, walking_skeleton_fixture};
use tempfile::TempDir;

fn run_skeleton() -> (
    TempDir,
    study_tts_runtime::BuildResult,
    DeterministicToneWorker,
) {
    let workspace = TempDir::new().expect("create isolated skeleton workspace");
    let worker = DeterministicToneWorker::default();
    let result = build_preview(
        BuildRequest {
            lesson_path: walking_skeleton_fixture(),
            workspace: workspace.path().to_path_buf(),
            ffmpeg_executable: "ffmpeg".into(),
        },
        &worker,
    )
    .expect("walking skeleton should build");

    (workspace, result, worker)
}

#[test]
fn t4_e0_skeleton_produces_wav_m4a_and_minimal_manifest() {
    let (_workspace, result, worker) = run_skeleton();

    assert_eq!(worker.synthesis_count(), 2);
    assert!(result.master_wav.is_file());
    assert!(result.m4a.is_file());
    assert!(result.manifest.is_file());

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

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,channels",
            "-of",
            "json",
        ])
        .arg(&result.m4a)
        .output()
        .expect("run ffprobe");
    assert!(
        probe.status.success(),
        "ffprobe failed: {}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let probe_json: Value = serde_json::from_slice(&probe.stdout).expect("parse ffprobe JSON");
    assert_eq!(probe_json["streams"][0]["codec_name"], "aac");
    assert_eq!(probe_json["streams"][0]["channels"], 1);

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&result.manifest).expect("read minimal manifest"))
            .expect("parse minimal manifest");
    assert_eq!(manifest["schema_version"], "0.1-skeleton");
    assert_eq!(manifest["release_status"], "private_preview");
    assert_eq!(manifest["lesson_id"], "e0-s0-walking-skeleton");
    assert_eq!(manifest["segments"].as_array().map(Vec::len), Some(2));
    assert!(manifest["artifacts"]["master_wav"]["blake3"].is_string());
    assert!(manifest["artifacts"]["m4a"]["blake3"].is_string());
}

#[test]
fn t4_e0_skeleton_runs_offline_without_model_artifacts() {
    let (workspace, first, worker) = run_skeleton();

    assert!(!workspace.path().join("models").exists());
    assert_eq!(worker.synthesis_count(), 2);

    let second = build_preview(
        BuildRequest {
            lesson_path: walking_skeleton_fixture(),
            workspace: workspace.path().to_path_buf(),
            ffmpeg_executable: "ffmpeg".into(),
        },
        &worker,
    )
    .expect("offline cache rebuild should succeed");

    assert_eq!(
        worker.synthesis_count(),
        2,
        "cache hits must avoid synthesis"
    );
    assert_eq!(
        std::fs::read(first.master_wav).expect("read first master"),
        std::fs::read(second.master_wav).expect("read second master")
    );
}

#[test]
fn t4_e0_skeleton_completes_within_integration_tier_budget() {
    let started = Instant::now();
    let (_workspace, _result, _worker) = run_skeleton();

    assert!(
        started.elapsed() < Duration::from_secs(5 * 60),
        "walking skeleton exceeded the five-minute T4 budget"
    );
}

#[test]
fn cache_reuses_identical_synthesis_inputs_across_segment_ids() {
    let workspace = TempDir::new().expect("create isolated skeleton workspace");
    let lesson_path = workspace.path().join("duplicate-speech.json");
    std::fs::write(
        &lesson_path,
        br#"{
          "schema_version":"1.0",
          "lesson_id":"duplicate-speech",
          "title":"Duplicate Speech",
          "segments":[
            {"id":"seg-a","speaker":"nadia","spoken_text":"Same speech.","style":"calm","pause_after_ms":25},
            {"id":"seg-b","speaker":"nadia","spoken_text":"Same speech.","style":"calm","pause_after_ms":50}
          ]
        }"#,
    )
    .expect("write duplicate-speech lesson");
    let worker = DeterministicToneWorker::default();

    build_preview(
        BuildRequest {
            lesson_path,
            workspace: workspace.path().to_path_buf(),
            ffmpeg_executable: "ffmpeg".into(),
        },
        &worker,
    )
    .expect("identical synthesis identities should share one cache artifact");

    assert_eq!(worker.synthesis_count(), 1);
}
