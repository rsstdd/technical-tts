use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};
use study_tts_core::{CacheKey, LessonError};
use study_tts_runtime::{
    BuildError, BuildRequest, CacheEntryFault, build_preview, cache_entry_dir, publish,
    validate_encoded_output, validate_production_manifest,
};
use study_tts_testkit::{
    DeterministicToneWorker, cache_identity_fixture, walking_skeleton_fixture,
};
use tempfile::TempDir;

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build_request(lesson_path: &Path, workspace: &Path) -> BuildRequest {
    BuildRequest {
        lesson_path: lesson_path.to_path_buf(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "ffmpeg".into(),
        ffprobe_executable: "ffprobe".into(),
        voice_profile_dir: None,
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

fn write_lesson_with_id(root: &Path, file_name: &str, lesson_id: &str) -> std::path::PathBuf {
    let mut lesson: Value = serde_json::from_slice(
        &std::fs::read(walking_skeleton_fixture()).expect("read walking-skeleton fixture"),
    )
    .expect("parse walking-skeleton fixture");
    lesson["lesson_id"] = Value::String(lesson_id.to_owned());
    let path = root.join(file_name);
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&lesson).expect("serialize modified lesson"),
    )
    .expect("write modified lesson");
    path
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
    let worker = DeterministicToneWorker::default();

    build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("walking skeleton must not access model artifacts");

    let mut entries = std::fs::read_dir(workspace.path())
        .expect("read workspace tree")
        .map(|entry| {
            entry
                .expect("read workspace entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(entries, ["cache", "previews"]);
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

    // seg-a synthesizes; seg-b and seg-e hit it; seg-c, seg-d, and seg-f each miss for a
    // different speech-affecting reason.
    assert_eq!(
        worker.synthesis_count(),
        4,
        "segment ID, pause, role, source refs, and display text must hit; spoken text, style, and \
         speaker must miss"
    );

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(result.manifest).expect("read cache identity manifest"),
    )
    .expect("parse cache identity manifest");
    let segments = manifest["segments"].as_array().expect("manifest segments");
    assert_eq!(segments.len(), 6);

    assert_eq!(
        segments[0]["cache_key"], segments[1]["cache_key"],
        "segment ID, pause, role, and source refs must be excluded from synthesis identity"
    );
    assert_ne!(
        segments[0]["cache_key"], segments[2]["cache_key"],
        "spoken text must be included in synthesis identity"
    );
    assert_ne!(
        segments[0]["cache_key"], segments[3]["cache_key"],
        "style must be included in synthesis identity"
    );
    assert_eq!(
        segments[0]["cache_key"], segments[4]["cache_key"],
        "display-only metadata must not alter synthesis identity"
    );
    assert_ne!(
        segments[0]["cache_key"], segments[5]["cache_key"],
        "speaker must be included in synthesis identity"
    );

    // Order is meaningful while synthesis is sequential. E5-S2 introduces the configurable worker
    // pool, after which this must become a set comparison rather than a sequence comparison.
    let submitted = worker.synthesized_texts();
    assert_eq!(
        submitted,
        [
            "Same speech.",
            "Same speech!",
            "Same speech.",
            "Same speech."
        ],
        "the worker must receive spoken_text and never display_text"
    );
    assert!(
        !submitted.iter().any(|text| text.contains("Display-only")),
        "display_text must never reach the synthesizer"
    );
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
fn t4_e0_ffprobe_rejects_non_aac_input() {
    let (_workspace, result, _worker) = run_skeleton();

    validate_encoded_output(Path::new("ffprobe"), &result.m4a)
        .expect("a mono AAC export must be accepted");

    // The PCM master is a valid audio file that is not a valid encoded output, which is the shape
    // an encoder failing open would produce.
    let error = validate_encoded_output(Path::new("ffprobe"), &result.master_wav)
        .expect_err("a PCM master must not pass encoded-output validation");
    assert!(matches!(error, BuildError::InvalidEncodedOutput(_)));
}

#[test]
fn t4_e0_lesson_id_cannot_escape_the_workspace() {
    let outer = TempDir::new().expect("create traversal test root");
    let workspace = outer.path().join("workspace");
    std::fs::create_dir(&workspace).expect("create traversal workspace");
    let worker = DeterministicToneWorker::default();
    let absolute_escape = outer.path().join("absolute-escape");
    let cases = [
        ("relative.json", "../../relative-escape".to_owned()),
        (
            "absolute.json",
            absolute_escape.to_string_lossy().into_owned(),
        ),
    ];

    for (file_name, lesson_id) in cases {
        let lesson = write_lesson_with_id(outer.path(), file_name, &lesson_id);
        let error = build_preview(build_request(&lesson, &workspace), &worker)
            .expect_err("unsafe lesson ID must be rejected");
        assert!(matches!(
            error,
            BuildError::Lesson(LessonError::InvalidLessonId(_))
        ));
    }

    assert!(!outer.path().join("relative-escape").exists());
    assert!(!absolute_escape.exists());
    assert_eq!(worker.synthesis_count(), 0);
}

#[cfg(unix)]
#[test]
fn t4_e0_managed_directory_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let outer = TempDir::new().expect("create symlink test root");
    let workspace = outer.path().join("workspace");
    let escape = outer.path().join("escape");
    std::fs::create_dir(&workspace).expect("create symlink workspace");
    std::fs::create_dir(&escape).expect("create symlink escape target");
    symlink(&escape, workspace.join("previews")).expect("create previews symlink");
    let worker = DeterministicToneWorker::default();

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), &workspace),
        &worker,
    )
    .expect_err("managed-directory symlink escape must fail");

    assert!(matches!(error, BuildError::ManagedPathEscape { .. }));
    assert!(
        std::fs::read_dir(&escape)
            .expect("read escape target")
            .next()
            .is_none()
    );
    assert_eq!(worker.synthesis_count(), 0);
}

#[cfg(unix)]
#[test]
fn t4_e0_leaf_symlink_escape_is_rejected_before_creating_anything() {
    use std::os::unix::fs::symlink;

    let outer = TempDir::new().expect("create symlink test root");
    let workspace = outer.path().join("workspace");
    let escape_target = outer.path().join("never-created");
    std::fs::create_dir_all(workspace.join("previews")).expect("create previews root");
    symlink(
        &escape_target,
        workspace.join("previews/e0-s0-walking-skeleton"),
    )
    .expect("create leaf symlink");
    let worker = DeterministicToneWorker::default();

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), &workspace),
        &worker,
    )
    .expect_err("leaf symlink escape must fail");

    assert!(matches!(error, BuildError::ManagedPathEscape { .. }));
    assert!(
        !escape_target.exists(),
        "the escape target must never be created"
    );
    assert_eq!(worker.synthesis_count(), 0);
}

#[test]
fn t4_e0_unapproved_content_fails_before_tools_and_synthesis() {
    let workspace = TempDir::new().expect("create unapproved-content workspace");
    let lesson = workspace.path().join("unapproved.json");
    let mut value: Value = serde_json::from_slice(
        &std::fs::read(walking_skeleton_fixture()).expect("read lesson fixture"),
    )
    .expect("parse lesson fixture");
    value["segments"][0]["review_status"] = Value::String("draft".to_owned());
    std::fs::write(
        &lesson,
        serde_json::to_vec_pretty(&value).expect("serialize unapproved lesson"),
    )
    .expect("write unapproved lesson");
    let worker = DeterministicToneWorker::default();
    let mut request = build_request(&lesson, workspace.path());
    request.ffmpeg_executable = "study-tts-missing-ffmpeg".into();

    let error = build_preview(request, &worker).expect_err("unapproved lesson must fail");

    assert!(matches!(
        error,
        BuildError::Lesson(LessonError::UnapprovedSegment(_))
    ));
    assert_eq!(worker.synthesis_count(), 0);
}

#[test]
fn t4_e0_cache_metadata_mismatch_is_rejected() {
    let (workspace, result, worker) = run_skeleton();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result.manifest).expect("read preview manifest"))
            .expect("parse preview manifest");
    let cache_key: CacheKey = manifest["segments"][0]["cache_key"]
        .as_str()
        .expect("segment cache key")
        .parse()
        .expect("the manifest records a well-formed cache key");
    // The sharding scheme is owned by `cache::entry_dir`; changing it must not require editing
    // this test.
    let entry_dir = cache_entry_dir(&workspace.path().join("cache"), &cache_key);
    let artifact_path = entry_dir.join("artifact.json");
    let original: Value =
        serde_json::from_slice(&std::fs::read(&artifact_path).expect("read cache artifact"))
            .expect("parse cache artifact");

    let mutations = [
        ("schema_version", Value::String("future".to_owned())),
        ("sample_rate", Value::from(48_000)),
        ("channels", Value::from(2)),
        ("sample_format", Value::String("s16le".to_owned())),
    ];
    for (field, replacement) in mutations {
        let mut mutated = original.clone();
        mutated[field] = replacement;
        std::fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&mutated).expect("serialize corrupt artifact"),
        )
        .expect("write corrupt artifact");

        let error = build_preview(
            build_request(&walking_skeleton_fixture(), workspace.path()),
            &worker,
        )
        .expect_err("corrupt cache metadata must be rejected");

        // Every one of these mutations makes the artifact describe audio this build cannot
        // consume, so they must all arrive as that fault rather than merely as some cache error.
        let BuildError::UnusableCacheEntry { fault, .. } = &error else {
            panic!("`{field}` mutation produced the wrong variant: `{error}`");
        };
        assert!(
            matches!(**fault, CacheEntryFault::IncompatibleArtifact { .. }),
            "`{field}` mutation produced the wrong fault: `{fault}`"
        );
        let message = error.to_string();
        // A poisoned entry fails every later build, so the message must name what to delete.
        assert!(
            message.contains(&entry_dir.display().to_string()),
            "`{field}` mutation did not name the entry directory: `{message}`"
        );
        assert!(
            message.contains("delete"),
            "`{field}` mutation did not state the remedy: `{message}`"
        );
    }

    let mut unknown_field = original;
    unknown_field["unexpected"] = Value::Bool(true);
    std::fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&unknown_field).expect("serialize unknown cache field"),
    )
    .expect("write cache artifact with unknown field");
    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("an unknown artifact field must be rejected");
    // `deny_unknown_fields` rejects this before any field is read, so it is a parse failure and
    // not an incompatible-metadata one.
    let BuildError::UnusableCacheEntry { fault, .. } = &error else {
        panic!("an unknown artifact field produced the wrong variant: `{error}`");
    };
    assert!(
        matches!(**fault, CacheEntryFault::UnparseableArtifact { .. }),
        "an unknown artifact field produced the wrong fault: `{fault}`"
    );
    assert!(error.to_string().contains("delete"));

    assert_eq!(worker.synthesis_count(), 2);
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

#[test]
fn t3_e0_registered_fixture_checksums_match_test_data_manifest() {
    let repository_root = repository_root();
    let manifest =
        std::fs::read_to_string(repository_root.join("docs/testing/TEST-DATA-MANIFEST.md"))
            .expect("read test-data manifest");
    let lessons = repository_root.join("fixtures/lessons");

    // Every committed fixture is discovered rather than listed, so a new fixture cannot be added
    // without a manifest row.
    let mut checked = 0_usize;
    for entry in std::fs::read_dir(&lessons).expect("read lesson fixtures") {
        let entry = entry.expect("read lesson fixture entry");
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let relative = format!("fixtures/lessons/{file_name}");
        let bytes = std::fs::read(entry.path()).expect("read registered fixture");
        let checksum = format!("{:x}", Sha256::digest(bytes));

        let rows: Vec<&str> = manifest
            .lines()
            .filter(|line| line.contains(&relative))
            .collect();
        assert_eq!(
            rows.len(),
            1,
            "`{relative}` must have exactly one test-data row, found {}",
            rows.len()
        );
        assert!(
            rows[0].contains(&format!("SHA-256 `{checksum}`")),
            "test-data checksum is stale for {relative}; update its row to SHA-256 `{checksum}`"
        );
        checked += 1;
    }

    // Guards against a misresolved path making the loop vacuous.
    assert!(
        checked >= 2,
        "expected at least two committed lesson fixtures, checked {checked}"
    );
}
