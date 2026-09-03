//! Tier 3 and 4 tests for the E0-S0 walking skeleton: real filesystem, fake
//! worker, real FFmpeg.

use std::{
    future::Future,
    path::Path,
    pin::Pin,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread,
    time::Duration,
};

use serde_json::Value;
use sha2::{Digest, Sha256};
use study_tts_core::{
    CacheKey, JobState, LessonError, MAX_LESSON_JSON_BYTES, ReleaseError, RenderPlan,
};
use study_tts_runtime::{
    BackendDescriptor, BackendError, BuildError, BuildRequest, DurableStateError,
    FileSystemCachePublisher, FileSystemPackageWriter, IoError, ManagedPathError,
    PreviewServiceBundle, PublicationError, ResumeRequest, SynthesisReport, SynthesisRequest,
    ToolError, TtsExecutor, build_preview, build_preview_with_services, load_lesson, publish,
    resume_preview, validate_m4a_output, validate_production_manifest,
};
use study_tts_testkit::{
    DeterministicToneWorker, FIXTURE_VOICE_PROFILES, InterruptingJobRepository,
    cache_identity_fixture, walking_skeleton_fixture, write_voice_profile_root,
};
use tempfile::TempDir;

const CONCURRENCY_TEST_TIMEOUT: Duration = Duration::from_secs(10);

fn repository_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Finds the cache entry recording `cache_key` by reading what each entry says
/// it is.
///
/// The sharding scheme is private to the runtime. Deriving the path here would
/// be a second copy of that layout in a test that is supposed to be able to
/// fail when the layout is wrong, so the entry is discovered by its declared
/// identity instead.
///
/// # Panics
///
/// If no entry declares `cache_key`, or if a cache directory or artifact cannot
/// be read. Every caller has just built the preview that wrote the entry, so
/// either is a defect in the code under test rather than a condition a test
/// should tolerate.
fn find_cache_entry_dir(cache_root: &Path, cache_key: &CacheKey) -> std::path::PathBuf {
    let mut directories = vec![cache_root.to_path_buf()];

    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(&directory).expect("read cache directory") {
            let entry = entry.expect("read cache entry");
            if entry.file_type().expect("read cache entry type").is_dir() {
                directories.push(entry.path());
                continue;
            }
            if entry.file_name() != "artifact.json" {
                continue;
            }

            let artifact: Value =
                serde_json::from_slice(&std::fs::read(entry.path()).expect("read cache artifact"))
                    .expect("parse cache artifact");
            if artifact["cache_key"].as_str() == Some(cache_key.as_str()) {
                return entry
                    .path()
                    .parent()
                    .expect("a cache artifact always sits inside its entry directory")
                    .to_path_buf();
            }
        }
    }

    panic!("no cache entry declares `{cache_key}`");
}

/// Distinguishes the profile root each [`build_request`] installs.
///
/// Not shared state between tests: it only makes a name unique. Two builds in
/// one workspace must not install into one root, because installing rewrites
/// `reference.wav` while the other build is hashing it — which is a checksum
/// mismatch reported against a profile nobody tampered with. The concurrent
/// tests below are what found that.
static VOICE_ROOT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// A build request whose voices resolve, with every other field left at what
/// the skeleton uses.
///
/// The profile root lives beneath the test's own workspace so one `TempDir`
/// guard still cleans up everything the test wrote; a build never writes into
/// it, and installing operator-supplied profiles there is what lets these
/// tests keep one temporary root each. Each request gets its own root, for the
/// reason [`VOICE_ROOT_SEQUENCE`] gives; the profiles are byte-identical, so
/// two builds still derive the same conditioning hash and share a cache.
fn build_request(lesson_path: &Path, workspace: &Path) -> BuildRequest {
    let sequence = VOICE_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    build_request_with_voices(
        lesson_path,
        workspace,
        &workspace.join(format!("voices-{sequence}")),
    )
}

/// The same request with the profile root somewhere other than the workspace.
///
/// The tests that prove a refusal precedes workspace creation need it: writing
/// the profiles into the workspace would create the very directory they assert
/// was never made.
fn build_request_with_voices(
    lesson_path: &Path,
    workspace: &Path,
    voice_profile_root: &Path,
) -> BuildRequest {
    write_voice_profile_root(voice_profile_root, &FIXTURE_VOICE_PROFILES);
    BuildRequest {
        lesson_path: lesson_path.to_path_buf(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "ffmpeg".into(),
        ffprobe_executable: "ffprobe".into(),
        voice_profile_root: voice_profile_root.to_path_buf(),
    }
}

const SKELETON_JOB_ID: &str = "e0-s0-walking-skeleton";

fn resume_request(workspace: &Path) -> ResumeRequest {
    let build = build_request(&walking_skeleton_fixture(), workspace);
    ResumeRequest {
        job_id: SKELETON_JOB_ID.to_owned(),
        workspace: build.workspace,
        ffmpeg_executable: build.ffmpeg_executable,
        ffprobe_executable: build.ffprobe_executable,
        voice_profile_root: build.voice_profile_root,
    }
}

fn read_job_document(workspace: &Path) -> Value {
    serde_json::from_slice(
        &std::fs::read(
            workspace
                .join("jobs")
                .join(SKELETON_JOB_ID)
                .join("job.json"),
        )
        .expect("read job document"),
    )
    .expect("parse job document")
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

struct PausingWorker {
    inner: DeterministicToneWorker,
    first_request: AtomicBool,
    started: (Mutex<bool>, Condvar),
    released: (Mutex<bool>, Condvar),
}

struct StopOnDrop {
    stop: Arc<AtomicBool>,
}

impl StopOnDrop {
    fn new(stop: Arc<AtomicBool>) -> Self {
        Self { stop }
    }
}

impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl PausingWorker {
    fn new() -> Self {
        Self {
            inner: DeterministicToneWorker::default(),
            first_request: AtomicBool::new(true),
            started: (Mutex::new(false), Condvar::new()),
            released: (Mutex::new(false), Condvar::new()),
        }
    }

    fn wait_until_started(&self) {
        let (lock, ready) = &self.started;
        let started = lock.lock().expect("started lock");
        let (started, timeout) = ready
            .wait_timeout_while(started, CONCURRENCY_TEST_TIMEOUT, |started| !*started)
            .expect("started wait");
        assert!(!timeout.timed_out(), "worker did not start before deadline");
        drop(started);
    }

    fn release(&self) {
        let (lock, ready) = &self.released;
        *lock.lock().expect("release lock") = true;
        ready.notify_all();
    }
}

impl TtsExecutor for PausingWorker {
    fn descriptor(&self) -> BackendDescriptor {
        self.inner.descriptor()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError> {
        self.inner.validate(request)
    }

    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
        destination: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>> {
        Box::pin(async move {
            if self.first_request.swap(false, Ordering::SeqCst) {
                let (started_lock, started_ready) = &self.started;
                *started_lock.lock().expect("started lock") = true;
                started_ready.notify_all();

                let (release_lock, release_ready) = &self.released;
                let released = release_lock.lock().expect("release lock");
                let (released, timeout) = release_ready
                    .wait_timeout_while(released, CONCURRENCY_TEST_TIMEOUT, |released| !*released)
                    .expect("release wait");
                assert!(
                    !timeout.timed_out(),
                    "worker was not released before deadline"
                );
                drop(released);
            }
            self.inner.synthesize(request, destination).await
        })
    }
}

#[test]
fn t4_e0_skeleton_produces_wav_m4a_and_minimal_manifest() {
    let (workspace, result, worker) = run_skeleton();

    assert_eq!(worker.synthesis_count(), 2);
    assert!(result.master_wav.is_file());
    assert!(result.m4a.is_file());
    assert!(result.manifest.is_file());
    assert!(result.package_dir.is_dir());
    assert!(result.publication_record.is_file());
    assert_eq!(
        result.master_wav.parent(),
        Some(result.package_dir.as_path())
    );
    assert_eq!(result.m4a.parent(), Some(result.package_dir.as_path()));
    assert_eq!(result.manifest.parent(), Some(result.package_dir.as_path()));
    assert!(
        result
            .master_wav
            .starts_with(workspace.path().join("previews/e0-s0-walking-skeleton"))
    );

    let current: Value = serde_json::from_slice(
        &std::fs::read(&result.publication_record).expect("read publication record"),
    )
    .expect("parse publication record");
    assert_eq!(current["schema_version"], "0.1-skeleton-current");
    assert_eq!(current["lesson_id"], "e0-s0-walking-skeleton");
    assert!(
        current["package_path"]
            .as_str()
            .is_some_and(|path| path.starts_with("packages/"))
    );
    let manifest_checksum =
        blake3::hash(&std::fs::read(&result.manifest).expect("hash selected manifest"))
            .to_hex()
            .to_string();
    assert_eq!(
        current["manifest_blake3"].as_str(),
        Some(manifest_checksum.as_str())
    );

    let reader = hound::WavReader::open(&result.master_wav).expect("open assembled WAV");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 24_000);
    assert_eq!(spec.bits_per_sample, 32);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(
        reader.duration(),
        9_600 + 2 * 480,
        "Rust assembly must write two 2,400-frame tones plus exact 75 ms and 125 ms pauses, \
         each tone carrying the 10 ms of zero padding ADR-0001 §13.4 requires at both of its \
         exposed edges"
    );

    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&result.manifest).expect("read minimal manifest"))
            .expect("parse minimal manifest");
    assert_eq!(manifest["schema_version"], "1.0-skeleton");
    assert_eq!(manifest["release_status"], "private_preview");
    assert_eq!(manifest["lesson_id"], "e0-s0-walking-skeleton");
    assert_eq!(manifest["segments"].as_array().map(Vec::len), Some(2));
    for artifact in PACKAGE_ARTIFACTS {
        assert!(
            manifest["artifacts"][artifact.field]["blake3"].is_string(),
            "the manifest must record a checksum for `{}`",
            artifact.name
        );
    }
    assert!(manifest["tools"]["ffmpeg"]["resolved_executable"].is_string());
    assert!(
        manifest["tools"]["ffmpeg"]["version"]
            .as_str()
            .is_some_and(|version| version.starts_with("ffmpeg version"))
    );
    assert!(
        manifest["tools"]["ffprobe"]["version"]
            .as_str()
            .is_some_and(|version| version.starts_with("ffprobe version"))
    );
    let executions = manifest["tools"]["executions"]
        .as_array()
        .expect("the manifest records every execution");
    assert!(
        executions.iter().any(|execution| execution["arguments"]
            .as_array()
            .is_some_and(|arguments| arguments.iter().any(|argument| argument == "mono"))),
        "an encode carrying the pinned channel layout must be recorded"
    );
}

/// One package artifact, as the filesystem and the manifest each name it.
struct PackageArtifact {
    /// File name inside the package directory, per ADR-0001 §12.1.
    name: &'static str,
    /// The `artifacts` key the manifest records it under.
    field: &'static str,
}

/// The six artifacts a complete E1-S4 package holds beside its manifest.
///
/// Transcribed from ADR-0001 §12.1's `output/` tree rather than read back out
/// of the runtime, so a test asserting the package is complete cannot be
/// satisfied by a runtime that changed its mind about what complete means.
const PACKAGE_ARTIFACTS: [PackageArtifact; 6] = [
    PackageArtifact {
        name: "lesson.wav",
        field: "master_wav",
    },
    PackageArtifact {
        name: "lesson.m4a",
        field: "m4a",
    },
    PackageArtifact {
        name: "lesson.mp3",
        field: "mp3",
    },
    PackageArtifact {
        name: "transcript.txt",
        field: "transcript",
    },
    PackageArtifact {
        name: "transcript.vtt",
        field: "captions",
    },
    PackageArtifact {
        name: "chapters.ffmetadata",
        field: "chapters",
    },
];

#[test]
fn t4_e1_the_published_manifest_schema_describes_what_a_package_writes() {
    // `manifest.json` is written and read entirely inside `study-tts-runtime`
    // through private functions, so there is no parser this crate can call and
    // no point committing a manifest fixture beside one: the document that
    // matters is the one a real package build produces, and a transcribed copy
    // would drift from it silently.
    //
    // This is the manifest half of the format-coverage table in
    // `crates/study-tts-testkit/tests/schemas.rs`, whose `CHECKED_ELSEWHERE`
    // names this test in return. It lives here rather than there because
    // producing the document needs real FFmpeg, which is a T4 dependency.
    let (_workspace, result, _worker) = run_skeleton();

    let manifest: Value = serde_json::from_slice(
        &std::fs::read(&result.manifest).expect("the written manifest is readable"),
    )
    .expect("the written manifest is JSON");
    let schema: Value = serde_json::from_slice(
        &std::fs::read(
            repository_root()
                .join(study_tts_runtime::SCHEMA_DIRECTORY)
                .join("manifest-v1.schema.json"),
        )
        .expect("the published manifest schema is readable"),
    )
    .expect("the published manifest schema is JSON");

    if let Err(violations) = study_tts_testkit::validate_against_schema(&schema, &manifest) {
        panic!(
            "a package build writes a manifest its own published schema refuses:\n  {}",
            violations.join("\n  ")
        );
    }
}

/// Where the two-segment fixture's segments land in the master, in frames.
///
/// Read against the fixture rather than recomputed from the code under test.
/// `TONE_FRAMES` is one tenth of a second — 2,400 frames at 24 kHz — and E1-S3
/// edge conditioning adds the 10 ms of zero padding ADR-0001 §13.4 requires at
/// each exposed edge, 240 frames twice, so each segment writes 2,880 frames of
/// speech. The fixture declares 75 ms and 125 ms pauses, which are exactly
/// 1,800 and 3,000 frames.
///
/// Each row is `(start_frame, audio_frames, pause_frames)`.
const WRITTEN_TIMELINE: [(u64, u64, u64); 2] = [(0, 2_880, 1_800), (4_680, 2_880, 3_000)];

/// Frames in the finished master: every row of [`WRITTEN_TIMELINE`] summed.
const MASTER_FRAMES: u64 = 10_560;

/// The WebVTT cue timings [`WRITTEN_TIMELINE`]'s speech boundaries produce.
///
/// Written out rather than converted in the test, for the reason the timeline
/// table is: a test that recomputed the conversion would agree with any
/// conversion, including a wrong one. Every boundary in this fixture happens to
/// land on a whole millisecond — 2,880 frames is 120 ms, 4,680 is 195 ms, 7,560
/// is 315 ms — so nothing here is rounded away.
const EXPECTED_CUES: [(&str, &str); 2] = [
    ("00:00:00.000", "00:00:00.120"),
    ("00:00:00.195", "00:00:00.315"),
];

/// Runs ffprobe over one artifact and returns its codec and channel count.
///
/// The build already probed each artifact; this probes them again from outside,
/// so the claim is about the files rather than about the runtime agreeing with
/// itself.
///
/// # Panics
///
/// If ffprobe cannot be run or its output cannot be read. Both are T4
/// dependencies the suite already requires.
fn probe_stream(path: &Path) -> (String, u64) {
    let output = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name,channels",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .expect("run ffprobe");
    assert!(
        output.status.success(),
        "ffprobe failed for `{}`: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let probe: Value = serde_json::from_slice(&output.stdout).expect("parse ffprobe output");
    let streams = probe["streams"]
        .as_array()
        .expect("ffprobe reports streams");
    assert_eq!(
        streams.len(),
        1,
        "`{}` must hold exactly one stream",
        path.display()
    );
    (
        streams[0]["codec_name"]
            .as_str()
            .expect("ffprobe reports a codec")
            .to_owned(),
        streams[0]["channels"]
            .as_u64()
            .expect("ffprobe reports a channel count"),
    )
}

fn read_manifest(result: &study_tts_runtime::BuildResult) -> Value {
    serde_json::from_slice(&std::fs::read(&result.manifest).expect("read package manifest"))
        .expect("parse package manifest")
}

#[test]
fn t4_e1_master_sample_count_equals_segments_plus_silence() {
    let (_workspace, result, _worker) = run_skeleton();

    assert_eq!(
        hound::WavReader::open(&result.master_wav)
            .expect("open assembled master")
            .duration() as u64,
        MASTER_FRAMES,
        "the master must hold every segment's conditioned speech plus its exact silence"
    );

    let manifest = read_manifest(&result);
    assert_eq!(manifest["total_frames"].as_u64(), Some(MASTER_FRAMES));
    let segments = manifest["segments"]
        .as_array()
        .expect("the manifest records its segments");
    assert_eq!(segments.len(), WRITTEN_TIMELINE.len());
    for (index, (start, audio, pause)) in WRITTEN_TIMELINE.into_iter().enumerate() {
        let segment = &segments[index];
        assert_eq!(
            (
                segment["start_frame"].as_u64(),
                segment["frames"].as_u64(),
                segment["pause_frames"].as_u64(),
            ),
            (Some(start), Some(audio), Some(pause)),
            "segment {index}"
        );
    }
}

#[test]
fn t4_e1_caption_boundaries_equal_written_sample_boundaries() {
    let (_workspace, result, _worker) = run_skeleton();
    let captions = std::fs::read_to_string(&result.captions).expect("read captions");

    assert!(
        captions.starts_with("WEBVTT\n"),
        "captions must be WebVTT: {captions}"
    );
    // Parsed out of the file rather than read off the timeline the build used.
    // A manifest holding the right frames beside a cue holding the wrong
    // timestamp is exactly the defect this test exists to catch, and comparing
    // the manifest to itself could not see it.
    let cues: Vec<(&str, &str)> = captions
        .lines()
        .filter_map(|line| line.split_once(" --> "))
        .collect();

    assert_eq!(
        cues,
        EXPECTED_CUES.to_vec(),
        "every cue must begin and end at the sample boundary the master was written at"
    );
}

#[test]
fn t4_e1_wav_m4a_and_mp3_pass_structural_validation() {
    let (_workspace, result, _worker) = run_skeleton();

    // The codecs ADR-0001 §13.5 and §13.3 assign each output: a float master,
    // AAC for the listening file, MP3 for the compatibility output, all mono.
    // Not §12.7, which is recovery and assigns no codec.
    const EXPECTED: [(&str, &str); 3] = [
        ("lesson.wav", "pcm_f32le"),
        ("lesson.m4a", "aac"),
        ("lesson.mp3", "mp3"),
    ];

    for (name, codec) in EXPECTED {
        let path = result.package_dir.join(name);
        assert_eq!(probe_stream(&path), (codec.to_owned(), 1), "`{name}`");
    }
}

#[test]
fn t4_e1_paths_with_spaces_and_unicode_are_supported() {
    // The workspace carries them, not the lesson ID: `PORTABLE_ID_PATTERN` in
    // `study-tts-core` forbids a space or a non-ASCII character in an ID, and
    // the workspace is where the risk actually is — every staged path and every
    // `{input_path}` and `{output_path}` FFmpeg is handed is built from it, so
    // this is the shape a shell would have broken.
    let root = TempDir::new().expect("create isolated workspace root");
    let workspace = root.path().join("prévisualisation « ✓ » dir");
    std::fs::create_dir(&workspace).expect("create awkwardly named workspace");
    let worker = DeterministicToneWorker::default();

    let result = build_preview(
        build_request(&walking_skeleton_fixture(), &workspace),
        &worker,
    )
    .expect("a workspace path with spaces and non-ASCII characters must build");

    for artifact in PACKAGE_ARTIFACTS {
        let path = result.package_dir.join(artifact.name);
        assert!(path.is_file(), "`{}` must be written", artifact.name);
    }
    assert!(result.package_dir.starts_with(&workspace));
}

#[cfg(unix)]
#[test]
fn t4_e1_ffmpeg_failure_preserves_master_and_prior_state() {
    use std::os::unix::fs::PermissionsExt;

    let (workspace, first, worker) = run_skeleton();
    let previous_current =
        std::fs::read(&first.publication_record).expect("read previous current record");
    let changed_lesson = workspace.path().join("changed.json");
    let mut lesson: Value = serde_json::from_slice(
        &std::fs::read(walking_skeleton_fixture()).expect("read walking-skeleton fixture"),
    )
    .expect("parse walking-skeleton fixture");
    lesson["segments"][0]["spoken_text"] = Value::String("Changed for the MP3 failure.".to_owned());
    std::fs::write(
        &changed_lesson,
        serde_json::to_vec_pretty(&lesson).expect("serialize changed lesson"),
    )
    .expect("write changed lesson");

    // Fails the MP3 encode specifically, after preflight has passed and the
    // M4A has succeeded. That is the interesting failure: the transaction is
    // part way through writing a package, and what must survive is the previous
    // selection, the previous package, and the new master — with no partial MP3
    // left behind.
    let failing_ffmpeg = workspace.path().join("mp3-failing-ffmpeg");
    std::fs::write(
        &failing_ffmpeg,
        b"#!/bin/sh\n\
for argument; do\n\
  if [ \"$argument\" = \"-encoders\" ]; then\n\
    echo ' A....D libmp3lame           libmp3lame MP3 (MPEG audio layer 3)'\n\
    exit 0\n\
  fi\n\
done\n\
if [ \"$1\" = \"-version\" ]; then\n\
  echo 'ffmpeg version mp3-failure'\n\
  exit 0\n\
fi\n\
for output; do :; done\n\
case \"$output\" in\n\
  *.mp3) : > \"$output\"; exit 12 ;;\n\
esac\n\
exec ffmpeg \"$@\"\n",
    )
    .expect("write MP3-failing FFmpeg wrapper");
    let mut permissions = std::fs::metadata(&failing_ffmpeg)
        .expect("wrapper metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&failing_ffmpeg, permissions).expect("make wrapper executable");

    let mut request = build_request(&changed_lesson, workspace.path());
    request.ffmpeg_executable = failing_ffmpeg;
    let error = build_preview(request, &worker).expect_err("MP3 encode failure must surface");

    assert!(
        matches!(error, BuildError::Tool(ToolError::Ffmpeg { .. })),
        "an MP3 encode failure must arrive as an FFmpeg refusal: `{error}`"
    );
    assert_eq!(
        std::fs::read(&first.publication_record).expect("read retained current record"),
        previous_current,
        "the prior selection must be untouched"
    );
    assert!(
        first.package_dir.is_dir(),
        "the prior package must be untouched"
    );
    for artifact in PACKAGE_ARTIFACTS {
        assert!(
            first.package_dir.join(artifact.name).is_file(),
            "the prior package must keep `{}`",
            artifact.name
        );
    }

    // The abandoned staging directory keeps the master the failed build
    // assembled, and holds no partial MP3: `export::encode` stages the encode
    // beside its destination and drops the guard on failure.
    let staging = workspace.path().join("jobs/e0-s0-walking-skeleton/staging");
    let staged: Vec<std::path::PathBuf> = std::fs::read_dir(&staging)
        .expect("read the abandoned staging root")
        .map(|entry| entry.expect("read staged transaction").path())
        .collect();
    let masters = staged
        .iter()
        .filter(|stage| stage.join("lesson.wav").is_file())
        .count();
    assert_eq!(masters, 1, "the new master must remain recoverable");
    for stage in &staged {
        assert!(
            !stage.join("lesson.mp3").exists(),
            "no partial MP3 may survive in `{}`",
            stage.display()
        );
    }
}

#[test]
fn t4_e1_manifest_checksums_match_every_output() {
    let (_workspace, result, _worker) = run_skeleton();
    let manifest = read_manifest(&result);

    for artifact in PACKAGE_ARTIFACTS {
        let path = result.package_dir.join(artifact.name);
        let recorded = manifest["artifacts"][artifact.field]["blake3"]
            .as_str()
            .unwrap_or_else(|| panic!("the manifest must record `{}`", artifact.name));
        let found = blake3::hash(&std::fs::read(&path).expect("read package artifact"))
            .to_hex()
            .to_string();

        assert_eq!(recorded, found, "`{}`", artifact.name);
        assert_eq!(
            manifest["artifacts"][artifact.field]["path"].as_str(),
            Some(artifact.name),
            "the manifest must name `{}` by its published path",
            artifact.name
        );
    }
}

/// Every file inside a published package is owner-only, the manifest included.
///
/// The gap this closes: four of the seven were created `0600` by `tempfile`
/// while the three text documents went through `fs::write`, which creates
/// `0666 & ~umask`. The transcript and the captions carry the whole authored
/// lesson in plaintext, so under a common `022` umask they were the only
/// world-readable files in a `private_preview` package — and their mode moved
/// with whoever ran the build.
///
/// Asserted against a really-rendered package rather than a fixture, because
/// the defect was in how the runtime creates the file, which a fixture writing
/// its own bytes would not reproduce.
#[test]
#[cfg(unix)]
fn t4_e1_every_package_file_is_owner_only() {
    use std::os::unix::fs::PermissionsExt as _;

    let (_workspace, result, _worker) = run_skeleton();

    let named: Vec<&str> = PACKAGE_ARTIFACTS
        .iter()
        .map(|artifact| artifact.name)
        .chain(std::iter::once("manifest.json"))
        .collect();
    for name in named {
        let path = result.package_dir.join(name);
        let mode = std::fs::metadata(&path)
            .unwrap_or_else(|error| panic!("`{name}` must be published: {error}"))
            .permissions()
            .mode();

        assert_eq!(mode & 0o777, 0o600, "`{name}` is mode {:o}", mode & 0o777);
    }
}

#[test]
fn t4_e1_lossy_output_is_never_source_for_another_export() {
    let (_workspace, result, _worker) = run_skeleton();
    let manifest = read_manifest(&result);
    let executions = manifest["tools"]["executions"]
        .as_array()
        .expect("the manifest records every execution");

    let mut encodes = 0;
    for execution in executions {
        let arguments: Vec<&str> = execution["arguments"]
            .as_array()
            .expect("an execution records its arguments")
            .iter()
            .map(|argument| argument.as_str().expect("an argument is a string"))
            .collect();
        let Some(input) = arguments
            .iter()
            .position(|argument| *argument == "-i")
            .and_then(|flag| arguments.get(flag + 1))
        else {
            continue;
        };
        if execution["tool"] != "ffmpeg" {
            continue;
        }
        encodes += 1;
        assert!(
            input.ends_with("lesson.wav"),
            "every export must be encoded from the canonical master, not from `{input}`"
        );
    }
    assert_eq!(
        encodes, 2,
        "both lossy exports must be encoded, each from the master"
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
    // A `voices-<n>` root is operator-supplied input this test installed
    // before the build, not something the build wrote; dropping it keeps the
    // assertion exact about what the build itself creates.
    entries.retain(|name| !name.starts_with("voices-"));
    assert_eq!(entries, ["cache", "jobs", "previews", "quarantine"]);
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
    // This byte comparison applies only to the deterministic fake executor
    // and exact Rust assembly. It does not claim byte-identical Chatterbox
    // output from repeated synthesis.
    assert_eq!(
        std::fs::read(first.master_wav).expect("read first master"),
        std::fs::read(second.master_wav).expect("read second master")
    );
    assert_eq!(first.package_dir, second.package_dir);
    assert_eq!(first.manifest, second.manifest);
    assert_eq!(first.publication_record, second.publication_record);
}

#[test]
fn t4_e0_concurrent_jobs_share_one_internally_consistent_cache_winner() {
    let root = TempDir::new().expect("create concurrent workspace");
    let first_lesson = write_lesson_with_id(root.path(), "first.json", "concurrent-first");
    let second_lesson = write_lesson_with_id(root.path(), "second.json", "concurrent-second");
    let workspace = root.path().join("workspace");
    let worker = Arc::new(DeterministicToneWorker::default());

    let (first, second) = thread::scope(|scope| {
        let first_worker = Arc::clone(&worker);
        let first_workspace = workspace.clone();
        let first = scope.spawn(move || {
            build_preview(
                build_request(&first_lesson, &first_workspace),
                first_worker.as_ref(),
            )
        });
        let second_worker = Arc::clone(&worker);
        let second_workspace = workspace.clone();
        let second = scope.spawn(move || {
            build_preview(
                build_request(&second_lesson, &second_workspace),
                second_worker.as_ref(),
            )
        });
        (
            first.join().expect("first build thread"),
            second.join().expect("second build thread"),
        )
    });

    let first = first.expect("first concurrent job");
    let second = second.expect("second concurrent job");
    assert_eq!(
        worker.synthesis_count(),
        2,
        "both lessons use the same two keys, so only one producer may synthesize each"
    );
    assert_ne!(first.package_dir, second.package_dir);
    assert!(first.manifest.is_file());
    assert!(second.manifest.is_file());
}

#[test]
fn t4_e0_live_lesson_job_lock_refuses_a_second_build() {
    let workspace = TempDir::new().expect("create lock workspace");
    let worker = Arc::new(PausingWorker::new());

    thread::scope(|scope| {
        let first_worker = Arc::clone(&worker);
        let first_workspace = workspace.path().to_path_buf();
        let first = scope.spawn(move || {
            build_preview(
                build_request(&walking_skeleton_fixture(), &first_workspace),
                first_worker.as_ref(),
            )
        });

        worker.wait_until_started();
        let error = build_preview(
            build_request(&walking_skeleton_fixture(), workspace.path()),
            worker.as_ref(),
        )
        .expect_err("the live lesson owner must be refused");
        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::LiveJobLock { .. })
        ));
        assert!(error.remedy().is_none());
        assert!(!error.to_string().contains("state corruption"));
        assert!(!error.to_string().contains("reconciliation"));

        worker.release();
        first
            .join()
            .expect("first build thread")
            .expect("first build completes");
    });
}

#[test]
fn t4_e0_corrupt_current_preview_is_refused_without_overwrite() {
    let (workspace, result, worker) = run_skeleton();
    std::fs::write(&result.publication_record, b"{corrupt").expect("corrupt current record");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("corrupt authoritative current record must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MalformedCurrentPreview { .. })
    ));
    assert_eq!(
        std::fs::read(result.publication_record).expect("current record remains"),
        b"{corrupt"
    );
}

#[test]
fn t4_e0_corrupt_publication_journal_is_refused_without_overwrite() {
    let (workspace, result, worker) = run_skeleton();
    let current = std::fs::read(&result.publication_record).expect("read current record");
    let journal = workspace
        .path()
        .join("jobs/e0-s0-walking-skeleton/publication.json");
    std::fs::write(&journal, b"{corrupt").expect("corrupt journal");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("corrupt authoritative journal must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MalformedPublicationJournal { .. })
    ));
    assert_eq!(
        std::fs::read(journal).expect("journal remains"),
        b"{corrupt"
    );
    assert_eq!(
        std::fs::read(result.publication_record).expect("current remains"),
        current
    );
}

#[test]
fn t4_e2_corrupt_job_state_is_not_overwritten() {
    let (workspace, _result, worker) = run_skeleton();
    let document = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("job.json");
    std::fs::write(&document, b"{corrupt").expect("corrupt job document");

    let build_error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("corrupt authoritative job state must be refused by a build");
    let resume_error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("corrupt authoritative job state must be refused by a resume");

    for error in [build_error, resume_error] {
        assert!(
            matches!(
                error,
                BuildError::DurableState(ref state)
                    if matches!(**state, DurableStateError::MalformedJobSnapshot { .. })
            ),
            "{error}"
        );
    }
    assert_eq!(
        std::fs::read(document).expect("job document remains"),
        b"{corrupt"
    );
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_retained_lesson_for_another_job() {
    let (workspace, _result, worker) = run_skeleton();
    let job_dir = workspace.path().join("jobs").join(SKELETON_JOB_ID);
    let other_lesson = write_lesson_with_id(workspace.path(), "other.json", "other-job");
    let retained = std::fs::read(other_lesson).expect("read other lesson");
    std::fs::write(job_dir.join("lesson.json"), &retained).expect("replace retained lesson");
    let mut document = read_job_document(workspace.path());
    document["lesson_blake3"] = Value::String(blake3::hash(&retained).to_hex().to_string());
    std::fs::write(
        job_dir.join("job.json"),
        serde_json::to_vec_pretty(&document).expect("serialize coherent altered job document"),
    )
    .expect("write coherent altered job document");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a retained lesson for another job must be refused under the claimed lock");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(
                **state,
                DurableStateError::RetainedLessonIdentityMismatch { .. }
            )
    ));
    assert!(!workspace.path().join("jobs/other-job").exists());
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_malformed_retained_plan() {
    let (workspace, _result, worker) = run_skeleton();
    let plan = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("plan.json");
    std::fs::write(&plan, b"{corrupt").expect("corrupt retained plan");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a malformed authoritative plan must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MalformedRetainedPlan { .. })
    ));
    assert_eq!(
        std::fs::read(plan).expect("retained plan remains"),
        b"{corrupt"
    );
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_retained_plan_with_a_stale_hash() {
    let (workspace, _result, worker) = run_skeleton();
    let plan = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("plan.json");
    let mut retained: RenderPlan =
        serde_json::from_slice(&std::fs::read(&plan).expect("read retained plan"))
            .expect("parse retained plan");
    retained.segments[0].display_text = "changed display text".to_owned();
    std::fs::write(
        &plan,
        serde_json::to_vec_pretty(&retained).expect("serialize altered plan"),
    )
    .expect("write altered plan");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a retained plan whose content does not derive its hash must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::RetainedPlanHashMismatch { .. })
    ));
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_retained_plan_for_another_job() {
    let (workspace, _result, worker) = run_skeleton();
    let plan = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("plan.json");
    let mut retained: RenderPlan =
        serde_json::from_slice(&std::fs::read(&plan).expect("read retained plan"))
            .expect("parse retained plan");
    retained.lesson_id = "other-job".to_owned();
    std::fs::write(
        &plan,
        serde_json::to_vec_pretty(&retained).expect("serialize altered plan"),
    )
    .expect("write altered plan");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a retained plan for another job must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::RetainedPlanIdentityMismatch { .. })
    ));
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_plan_hash_that_disagrees_with_job_state() {
    let (workspace, _result, worker) = run_skeleton();
    let plan = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("plan.json");
    let mut retained: RenderPlan =
        serde_json::from_slice(&std::fs::read(&plan).expect("read retained plan"))
            .expect("parse retained plan");
    retained.segments[0].display_text = "changed display text".to_owned();
    retained.plan_hash = retained.derived_hash();
    std::fs::write(
        &plan,
        serde_json::to_vec_pretty(&retained).expect("serialize self-consistent plan"),
    )
    .expect("write self-consistent plan");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a retained plan whose identity disagrees with job state must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::JobPlanHashMismatch { .. })
    ));
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_job_package_that_disagrees_with_selected_output() {
    let (workspace, _result, worker) = run_skeleton();
    let path = workspace
        .path()
        .join("jobs")
        .join(SKELETON_JOB_ID)
        .join("job.json");
    let mut document = read_job_document(workspace.path());
    let contradictory = "d".repeat(64);
    document["preview_package"]["package_id"] = Value::String(contradictory.clone());
    document["preview_package"]["manifest_blake3"] = Value::String(contradictory);
    let bytes = serde_json::to_vec_pretty(&document).expect("serialize contradictory job state");
    std::fs::write(&path, &bytes).expect("replace job state");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("job and selected-output identities must agree before state is replaced");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(
                **state,
                DurableStateError::JobPreviewSelectionMismatch { .. }
            )
    ));
    assert_eq!(
        std::fs::read(path).expect("job state remains"),
        bytes,
        "contradictory authoritative state must not be overwritten"
    );
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_resume_refuses_a_selected_package_for_a_different_job_plan() {
    let (workspace, _result, worker) = run_skeleton();
    let job_dir = workspace.path().join("jobs").join(SKELETON_JOB_ID);
    let plan_path = job_dir.join("plan.json");
    let job_path = job_dir.join("job.json");
    let mut retained: RenderPlan =
        serde_json::from_slice(&std::fs::read(&plan_path).expect("read retained plan"))
            .expect("parse retained plan");
    retained.segments[0].display_text = "a different plan".to_owned();
    retained.plan_hash = retained.derived_hash();
    let plan_bytes = serde_json::to_vec_pretty(&retained).expect("serialize altered plan");
    std::fs::write(&plan_path, &plan_bytes).expect("write altered plan");
    let mut document = read_job_document(workspace.path());
    document["plan_hash"] = Value::String(retained.plan_hash.as_str().to_owned());
    let job_bytes = serde_json::to_vec_pretty(&document).expect("serialize altered job state");
    std::fs::write(&job_path, &job_bytes).expect("write altered job state");

    let error = resume_preview(resume_request(workspace.path()), &worker)
        .expect_err("a selected output for another authoritative plan must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::PackagePlanMismatch { .. })
    ));
    assert_eq!(std::fs::read(job_path).expect("job remains"), job_bytes);
    assert_eq!(std::fs::read(plan_path).expect("plan remains"), plan_bytes);
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e2_job_directory_holds_validated_lesson_and_plan() {
    let (workspace, result, _worker) = run_skeleton();
    let job_dir = workspace.path().join("jobs").join(SKELETON_JOB_ID);

    let lesson = load_lesson(&job_dir.join("lesson.json")).expect("the retained lesson validates");
    let plan: RenderPlan =
        serde_json::from_slice(&std::fs::read(job_dir.join("plan.json")).expect("read plan"))
            .expect("the retained plan parses");
    let manifest = read_manifest(&result);
    let document = read_job_document(workspace.path());

    assert_eq!(lesson.lesson_id(), SKELETON_JOB_ID);
    assert_eq!(plan.lesson_id, SKELETON_JOB_ID);
    assert_eq!(plan.schema_version, study_tts_core::PLAN_SCHEMA_VERSION);
    assert_eq!(plan.plan_hash, plan.derived_hash());
    assert_eq!(manifest["plan_hash"], plan.plan_hash.as_str());
    assert_eq!(document["plan_hash"], plan.plan_hash.as_str());
    assert_eq!(document["state"], "rendered");
    assert_eq!(document["last_successful_state"], "rendered");
    assert_eq!(document["build_attempt"], 1);
    assert_eq!(document["release_status"], "private_preview");
    assert_eq!(
        document["segments"]
            .as_object()
            .expect("segments recorded")
            .len(),
        2
    );
    assert!(document["preview_package"]["manifest_blake3"].is_string());
}

#[test]
fn t4_e2_no_op_rebuild_produces_identical_manifest() {
    let (workspace, first, worker) = run_skeleton();
    let manifest_before = std::fs::read(&first.manifest).expect("read first manifest");

    let second = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("a rebuild with nothing changed succeeds");

    assert_eq!(
        worker.synthesis_count(),
        2,
        "nothing changed, so nothing is resynthesized"
    );
    assert_eq!(first.package_dir, second.package_dir);
    assert_eq!(
        std::fs::read(&second.manifest).expect("read second manifest"),
        manifest_before,
        "the selected manifest is byte-identical across a no-op rebuild"
    );
    let document = read_job_document(workspace.path());
    assert_eq!(document["build_attempt"], 2);
    assert_eq!(document["abandoned_attempt"]["build_attempt"], 1);
    assert_eq!(document["abandoned_attempt"]["state"], "rendered");
    assert_eq!(document["state"], "rendered");
}

#[test]
fn t4_e2_resume_regenerates_only_missing_or_invalid_segments() {
    for invalid in [false, true] {
        let (workspace, first, worker) = run_skeleton();
        let document = read_job_document(workspace.path());
        let cache_keys: Vec<(String, CacheKey)> = document["segments"]
            .as_object()
            .expect("segments recorded")
            .iter()
            .map(|(segment_id, status)| {
                (
                    segment_id.clone(),
                    status["cache_key"]
                        .as_str()
                        .expect("a recorded cache key")
                        .parse()
                        .expect("a recorded cache key parses"),
                )
            })
            .collect();
        let cache_root = workspace.path().join("cache");
        let replaced = find_cache_entry_dir(&cache_root, &cache_keys[0].1);
        let surviving = find_cache_entry_dir(&cache_root, &cache_keys[1].1).join("audio.wav");
        let surviving_bytes = std::fs::read(&surviving).expect("read surviving audio");
        if invalid {
            std::fs::write(replaced.join("audio.wav"), b"invalid wav")
                .expect("invalidate one published cache entry");
        } else {
            std::fs::remove_dir_all(&replaced).expect("remove one published cache entry");
        }

        let resumed =
            resume_preview(resume_request(workspace.path()), &worker).expect("the resume succeeds");

        assert_eq!(
            worker.synthesis_count(),
            3,
            "exactly the {} segment is regenerated",
            if invalid { "invalid" } else { "missing" }
        );
        assert_eq!(
            std::fs::read(&surviving).expect("read surviving audio again"),
            surviving_bytes,
            "the valid entry is reused byte for byte"
        );
        assert!(
            replaced.join("audio.wav").is_file(),
            "the missing or invalid entry is republished"
        );
        if invalid {
            let take_dir = workspace
                .path()
                .join("quarantine/e0-s0-walking-skeleton")
                .join(&cache_keys[0].0)
                .join("take-0");
            let preserved = std::fs::read_dir(take_dir)
                .expect("invalid entry has a quarantine attempt")
                .map(|entry| {
                    entry
                        .expect("read quarantine attempt")
                        .path()
                        .join("cache-entry/audio.wav")
                })
                .any(|audio| {
                    std::fs::read(audio).is_ok_and(|bytes| bytes.as_slice() == b"invalid wav")
                });
            assert!(
                preserved,
                "the invalid published entry is preserved in quarantine"
            );
        }
        assert_eq!(resumed.package_dir, first.package_dir);
        let manifest = read_manifest(&resumed);
        assert_eq!(manifest["segments"].as_array().expect("segments").len(), 2);
        let document = read_job_document(workspace.path());
        assert_eq!(document["build_attempt"], 2);
        assert_eq!(document["state"], "rendered");
    }
}

#[test]
fn t4_e2_interrupt_after_cache_publish_reconciles_on_resume() {
    let workspace = TempDir::new().expect("create interrupted workspace");
    let worker = DeterministicToneWorker::default();
    // Both segments are published to the cache before the attempt records
    // `Rendered`; failing that write is the crash ADR-0001 §12.7 step 4 names.
    let jobs = InterruptingJobRepository::failing_before(JobState::Rendered);
    let error = build_preview_with_services(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        PreviewServiceBundle {
            executor: &worker,
            cache: &FileSystemCachePublisher,
            packages: &FileSystemPackageWriter,
            jobs: &jobs,
        },
    )
    .expect_err("the injected interruption must surface");
    assert!(matches!(error, BuildError::Io(IoError::FileSystem { .. })));
    assert_eq!(jobs.interruptions(), 1);
    assert_eq!(worker.synthesis_count(), 2);
    let interrupted = read_job_document(workspace.path());
    assert_eq!(interrupted["state"], "rendering");
    assert_eq!(interrupted["build_attempt"], 1);

    let resumed =
        resume_preview(resume_request(workspace.path()), &worker).expect("the resume succeeds");

    assert_eq!(
        worker.synthesis_count(),
        2,
        "artifacts published before the state advanced are reconciled, not resynthesized"
    );
    assert!(resumed.manifest.is_file());
    let document = read_job_document(workspace.path());
    assert_eq!(document["build_attempt"], 2);
    assert_eq!(document["abandoned_attempt"]["state"], "rendering");
    assert_eq!(document["state"], "rendered");
    assert!(document["preview_package"]["package_id"].is_string());
}

#[test]
fn t4_e0_publication_journal_lesson_mismatch_has_its_own_error() {
    let (workspace, result, worker) = run_skeleton();
    let journal = workspace
        .path()
        .join("jobs/e0-s0-walking-skeleton/publication.json");
    let mut record: Value =
        serde_json::from_slice(&std::fs::read(&journal).expect("read publication journal"))
            .expect("parse publication journal");
    record["lesson_id"] = Value::String("different-lesson".to_owned());
    std::fs::write(
        &journal,
        serde_json::to_vec_pretty(&record).expect("serialize mismatched journal"),
    )
    .expect("write mismatched journal");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a mismatched journal lesson must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(
                **state,
                DurableStateError::PublicationJournalLessonMismatch { .. }
            )
    ));
    assert!(result.publication_record.is_file());
}

#[test]
fn t4_e0_corrupt_selected_manifest_is_refused_without_replacement() {
    let (workspace, result, worker) = run_skeleton();
    let current = std::fs::read(&result.publication_record).expect("read current record");
    std::fs::write(&result.manifest, b"{}").expect("corrupt selected manifest");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a corrupt selected manifest must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MalformedPackageManifest { .. })
    ));
    assert_eq!(
        std::fs::read(result.publication_record).expect("current remains"),
        current
    );
    assert_eq!(
        std::fs::read(result.manifest).expect("manifest remains"),
        b"{}"
    );
}

#[test]
fn t4_e0_missing_selected_package_has_its_own_error() {
    let (workspace, result, worker) = run_skeleton();
    std::fs::remove_dir_all(&result.package_dir).expect("remove generated test package");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a missing selected package must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MissingPackageDirectory { .. })
    ));
}

#[test]
fn t4_e0_selected_manifest_checksum_mismatch_has_its_own_error() {
    let (workspace, result, worker) = run_skeleton();
    let mut manifest = std::fs::read(&result.manifest).expect("read selected manifest");
    manifest.push(b'\n');
    std::fs::write(&result.manifest, manifest).expect("rewrite valid manifest bytes");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a selected manifest checksum mismatch must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(
                **state,
                DurableStateError::PackageManifestChecksumMismatch { .. }
            )
    ));
}

#[test]
fn t4_e0_publication_plan_mismatch_has_its_own_error() {
    let (workspace, result, worker) = run_skeleton();
    let journal = workspace
        .path()
        .join("jobs/e0-s0-walking-skeleton/publication.json");
    let mut record: Value =
        serde_json::from_slice(&std::fs::read(&journal).expect("read publication journal"))
            .expect("parse publication journal");
    record["plan_hash"] = Value::String(blake3::hash(b"different plan").to_hex().to_string());
    std::fs::write(
        &journal,
        serde_json::to_vec_pretty(&record).expect("serialize mismatched journal"),
    )
    .expect("write mismatched journal");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a journal/package plan mismatch must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::PackagePlanMismatch { .. })
    ));
    assert!(result.publication_record.is_file());
}

#[test]
fn t4_e0_durable_unselected_package_is_selected_during_reconciliation() {
    let (workspace, result, worker) = run_skeleton();
    let journal_path = workspace
        .path()
        .join("jobs/e0-s0-walking-skeleton/publication.json");
    let mut journal: Value =
        serde_json::from_slice(&std::fs::read(&journal_path).expect("read publication journal"))
            .expect("parse publication journal");
    let manifest_blake3 = journal["transaction"]["manifest_blake3"]
        .as_str()
        .expect("completed journal checksum")
        .to_owned();
    journal["transaction"] = serde_json::json!({
        "state": "package_durable",
        "manifest_blake3": manifest_blake3,
    });
    std::fs::write(
        &journal_path,
        serde_json::to_vec_pretty(&journal).expect("serialize interrupted journal"),
    )
    .expect("write interrupted journal");
    std::fs::remove_file(&result.publication_record).expect("remove not-yet-selected pointer");

    let recovered = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("reconciliation must select the durable package");

    assert_eq!(recovered.package_dir, result.package_dir);
    assert!(recovered.publication_record.is_file());
    assert_eq!(worker.synthesis_count(), 2);
}

#[test]
fn t4_e0_current_readers_observe_only_complete_generations() {
    let (workspace, first, worker) = run_skeleton();
    let changed_lesson = workspace.path().join("reader-change.json");
    let mut lesson: Value = serde_json::from_slice(
        &std::fs::read(walking_skeleton_fixture()).expect("read walking-skeleton fixture"),
    )
    .expect("parse walking-skeleton fixture");
    lesson["segments"][1]["spoken_text"] = Value::String("A second generation.".to_owned());
    std::fs::write(
        &changed_lesson,
        serde_json::to_vec_pretty(&lesson).expect("serialize changed lesson"),
    )
    .expect("write changed lesson");

    let stop = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    thread::scope(|scope| {
        let shutdown = StopOnDrop::new(Arc::clone(&stop));
        let reader_stop = Arc::clone(&stop);
        let reader_reads = Arc::clone(&reads);
        let current_path = first.publication_record.clone();
        let preview_dir = current_path
            .parent()
            .expect("current record has preview directory")
            .to_path_buf();
        let reader = scope.spawn(move || {
            while !reader_stop.load(Ordering::SeqCst) {
                let current: Value = serde_json::from_slice(
                    &std::fs::read(&current_path).expect("read one complete current record"),
                )
                .expect("parse one complete current record");
                let package = preview_dir.join(
                    current["package_path"]
                        .as_str()
                        .expect("current package path"),
                );
                assert!(package.join("lesson.wav").is_file());
                assert!(package.join("lesson.m4a").is_file());
                assert!(package.join("manifest.json").is_file());
                reader_reads.fetch_add(1, Ordering::SeqCst);
                thread::yield_now();
            }
        });

        build_preview(build_request(&changed_lesson, workspace.path()), &worker)
            .expect("publish second generation");
        drop(shutdown);
        reader.join().expect("current reader");
    });

    assert!(reads.load(Ordering::SeqCst) > 0);
}

#[test]
fn t4_e0_legacy_flat_preview_artifacts_are_preserved() {
    let workspace = TempDir::new().expect("create legacy workspace");
    let legacy_dir = workspace.path().join("previews/e0-s0-walking-skeleton");
    std::fs::create_dir_all(&legacy_dir).expect("create legacy preview directory");
    let legacy = legacy_dir.join("lesson.wav");
    std::fs::write(&legacy, b"legacy-preview-bytes").expect("write legacy preview");

    let result = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &DeterministicToneWorker::default(),
    )
    .expect("new immutable preview should build beside legacy artifacts");

    assert_eq!(
        std::fs::read(legacy).expect("legacy preview remains"),
        b"legacy-preview-bytes"
    );
    assert!(result.package_dir.starts_with(legacy_dir.join("packages")));
}

#[cfg(unix)]
#[test]
fn t4_e0_ffmpeg_failure_leaves_previous_current_preview_unchanged() {
    use std::os::unix::fs::PermissionsExt;

    let (workspace, first, worker) = run_skeleton();
    let previous_current =
        std::fs::read(&first.publication_record).expect("read previous current record");
    let changed_lesson = workspace.path().join("changed.json");
    let mut lesson: Value = serde_json::from_slice(
        &std::fs::read(walking_skeleton_fixture()).expect("read walking-skeleton fixture"),
    )
    .expect("parse walking-skeleton fixture");
    lesson["segments"][0]["spoken_text"] = Value::String("Changed speech.".to_owned());
    std::fs::write(
        &changed_lesson,
        serde_json::to_vec_pretty(&lesson).expect("serialize changed lesson"),
    )
    .expect("write changed lesson");

    // The wrapper answers preflight — both the version probe and the encoder
    // inventory — and fails only the encode. Without the inventory arm this
    // test would pass on a refusal raised before any durable work started,
    // which proves nothing about preserving a package the encode was part way
    // through writing.
    let failing_ffmpeg = workspace.path().join("failing-ffmpeg");
    std::fs::write(
        &failing_ffmpeg,
        b"#!/bin/sh\n\
for argument; do\n\
  if [ \"$argument\" = \"-encoders\" ]; then\n\
    echo ' A....D libmp3lame           libmp3lame MP3 (MPEG audio layer 3)'\n\
    exit 0\n\
  fi\n\
done\n\
if [ \"$1\" = \"-version\" ]; then\n\
  echo 'ffmpeg version test-failure'\n\
  exit 0\n\
fi\n\
exit 12\n",
    )
    .expect("write failing FFmpeg wrapper");
    let mut permissions = std::fs::metadata(&failing_ffmpeg)
        .expect("wrapper metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&failing_ffmpeg, permissions).expect("make wrapper executable");

    let mut request = build_request(&changed_lesson, workspace.path());
    request.ffmpeg_executable = failing_ffmpeg;
    let error = build_preview(request, &worker).expect_err("FFmpeg encode failure must surface");

    assert!(matches!(error, BuildError::Tool(ToolError::Ffmpeg { .. })));
    assert_eq!(
        std::fs::read(&first.publication_record).expect("read retained current record"),
        previous_current
    );
    assert!(first.package_dir.is_dir());
}

#[cfg(unix)]
#[test]
fn t4_e0_ffprobe_failure_leaves_previous_current_preview_unchanged() {
    use std::os::unix::fs::PermissionsExt;

    let (workspace, first, worker) = run_skeleton();
    let previous_current =
        std::fs::read(&first.publication_record).expect("read previous current record");
    let failing_ffprobe = workspace.path().join("failing-ffprobe");
    std::fs::write(
        &failing_ffprobe,
        b"#!/bin/sh\n\
if [ \"$1\" = \"-version\" ]; then\n\
  echo 'ffprobe version test-failure'\n\
  exit 0\n\
fi\n\
exit 12\n",
    )
    .expect("write failing ffprobe wrapper");
    let mut permissions = std::fs::metadata(&failing_ffprobe)
        .expect("wrapper metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&failing_ffprobe, permissions).expect("make wrapper executable");

    let mut request = build_request(&walking_skeleton_fixture(), workspace.path());
    request.ffprobe_executable = failing_ffprobe;
    let error = build_preview(request, &worker).expect_err("ffprobe failure must surface");

    assert!(matches!(error, BuildError::Tool(ToolError::Ffprobe { .. })));
    assert_eq!(
        std::fs::read(&first.publication_record).expect("read retained current record"),
        previous_current
    );
    assert!(first.package_dir.is_dir());
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

    // seg-a synthesizes; seg-b and seg-e hit it; seg-c, seg-d, and seg-f each
    // miss for a different speech-affecting reason.
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

    // Order is meaningful while synthesis is sequential. E5-S2 introduces the
    // configurable worker pool, after which this must become a set comparison
    // rather than a sequence comparison.
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

/// An FFmpeg with no MP3 encoder is refused before anything is synthesized.
///
/// The gap this closes: `tools::inspect` reads only the first line of
/// `-version`, which is identical whether or not `libmp3lame` was compiled in.
/// Without an inventory probe the refusal would arrive after a full render, and
/// on a real lesson that is minutes of synthesis thrown away.
#[cfg(unix)]
#[test]
fn t4_e1_missing_mp3_encoder_fails_before_synthesis_and_durable_work() {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().expect("create isolated preflight root");
    let workspace = root.path().join("not-created");
    let encoderless = root.path().join("encoderless-ffmpeg");
    // A plausible inventory that simply does not offer the encoder. The `aac`
    // row is what keeps this from passing for a wrapper that lists nothing at
    // all, and the description mentioning MP3 is what a substring search would
    // wrongly accept.
    std::fs::write(
        &encoderless,
        b"#!/bin/sh\n\
for argument; do\n\
  if [ \"$argument\" = \"-encoders\" ]; then\n\
    echo ' Encoders:'\n\
    echo ' A....D aac                  AAC (Advanced Audio Coding)'\n\
    echo ' A....D wrapped              a decoder that mentions libmp3lame in prose'\n\
    exit 0\n\
  fi\n\
done\n\
if [ \"$1\" = \"-version\" ]; then\n\
  echo 'ffmpeg version encoderless'\n\
  exit 0\n\
fi\n\
exit 1\n",
    )
    .expect("write encoderless FFmpeg wrapper");
    let mut permissions = std::fs::metadata(&encoderless)
        .expect("wrapper metadata")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&encoderless, permissions).expect("make wrapper executable");
    let worker = DeterministicToneWorker::default();
    let mut request = build_request_with_voices(
        &walking_skeleton_fixture(),
        &workspace,
        &root.path().join("voices"),
    );
    request.ffmpeg_executable = encoderless;

    let error =
        build_preview(request, &worker).expect_err("an FFmpeg with no MP3 encoder must be refused");

    assert!(
        matches!(
            error,
            BuildError::Tool(ToolError::MissingEncoder { ref encoder, .. })
                if encoder == &"libmp3lame"
        ),
        "a missing encoder must be its own refusal rather than a generic encode failure: `{error}`"
    );
    assert!(
        error.to_string().contains("libmp3lame"),
        "the refusal must name the encoder to install: `{error}`"
    );
    assert_eq!(
        worker.synthesis_count(),
        0,
        "the encoder inventory must be checked before synthesis"
    );
    assert!(
        !workspace.exists(),
        "the encoder inventory must be checked before any durable state is created"
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
        BuildError::Tool(ToolError::MissingTool { ref tool, .. }) if tool == "FFmpeg"
    ));
    assert!(error.to_string().contains("study-tts-missing-ffmpeg"));

    let mut request = build_request(&walking_skeleton_fixture(), workspace.path());
    request.ffprobe_executable = "study-tts-missing-ffprobe".into();
    let error = build_preview(request, &worker).expect_err("missing ffprobe must fail preflight");
    assert!(matches!(
        error,
        BuildError::Tool(ToolError::MissingTool { ref tool, .. }) if tool == "ffprobe"
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

    validate_m4a_output(Path::new("ffprobe"), &result.m4a)
        .expect("a mono AAC export must be accepted");

    // The PCM master is a valid audio file that is not a valid encoded output,
    // which is the shape an encoder failing open would produce.
    let error = validate_m4a_output(Path::new("ffprobe"), &result.master_wav)
        .expect_err("a PCM master must not pass encoded-output validation");
    // The probe is readable; what it describes is wrong. Naming the codec it
    // actually found is what tells an operator the encoder failed open rather
    // than that ffprobe misbehaved.
    assert!(
        matches!(
            error,
            BuildError::Tool(ToolError::UnexpectedEncodedStream { ref codec, .. })
                if codec.as_deref() != Some("aac")
        ),
        "a PCM master produced `{error}`"
    );
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
            &error,
            BuildError::Lesson(diagnostic)
                if matches!(diagnostic.error(), LessonError::InvalidLessonId(_))
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

    assert!(matches!(
        error,
        BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
    ));
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

    assert!(matches!(
        error,
        BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
    ));
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
        &error,
        BuildError::Lesson(diagnostic)
            if matches!(diagnostic.error(), LessonError::UnapprovedSegment(_))
    ));
    assert_eq!(worker.synthesis_count(), 0);
    // The observed half of `t1_e1_unreviewed_lesson_fails_before_worker_start`,
    // which proves the same ordering by construction. The backend is never
    // reached at all — not even for its descriptor, which is the build's first
    // touch of the executor — so a worker that starts on first use has not
    // started. Moving that descriptor call above the lesson gate in
    // `build_preview_with_services` fails here and nowhere else.
    assert_eq!(
        worker.touch_count(),
        0,
        "an unapproved lesson must be refused before the backend is reached"
    );
}

#[test]
fn t4_e0_oversized_lesson_fails_before_tools_workspace_and_synthesis() {
    let root = TempDir::new().expect("create oversized-lesson test root");
    let lesson = root.path().join("oversized.json");
    std::fs::write(&lesson, vec![b'{'; MAX_LESSON_JSON_BYTES + 1]).expect("write oversized lesson");
    let workspace = root.path().join("workspace");
    let worker = DeterministicToneWorker::default();
    let mut request = build_request_with_voices(&lesson, &workspace, &root.path().join("voices"));
    request.ffmpeg_executable = "study-tts-missing-ffmpeg".into();
    request.ffprobe_executable = "study-tts-missing-ffprobe".into();

    let error = build_preview(request, &worker).expect_err("oversized lesson must fail");

    assert!(matches!(
        &error,
        BuildError::Lesson(diagnostic)
            if matches!(
                diagnostic.error(),
                LessonError::LessonJsonTooLarge { max_bytes } if *max_bytes == MAX_LESSON_JSON_BYTES
            )
    ));
    assert_eq!(worker.synthesis_count(), 0);
    assert!(!workspace.exists());
}

#[cfg(unix)]
#[test]
fn t4_e0_lesson_fifo_fails_before_tools_workspace_and_synthesis() {
    use rustix::fs::{CWD, Mode, mkfifoat};

    let root = TempDir::new().expect("create lesson-FIFO test root");
    let lesson = root.path().join("lesson.json");
    mkfifoat(CWD, &lesson, Mode::RUSR | Mode::WUSR).expect("create lesson FIFO");
    let workspace = root.path().join("workspace");
    let worker = DeterministicToneWorker::default();
    let mut request = build_request_with_voices(&lesson, &workspace, &root.path().join("voices"));
    request.ffmpeg_executable = "study-tts-missing-ffmpeg".into();
    request.ffprobe_executable = "study-tts-missing-ffprobe".into();

    let error = build_preview(request, &worker).expect_err("a lesson FIFO must be refused");

    assert!(matches!(
        error,
        BuildError::Io(IoError::LessonNotRegularFile { path }) if path == lesson
    ));
    assert_eq!(worker.synthesis_count(), 0);
    assert!(!workspace.exists());
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
    let entry_dir = find_cache_entry_dir(&workspace.path().join("cache"), &cache_key);
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
    for (index, (field, replacement)) in mutations.into_iter().enumerate() {
        let mut mutated = original.clone();
        mutated[field] = replacement;
        std::fs::write(
            &artifact_path,
            serde_json::to_vec_pretty(&mutated).expect("serialize corrupt artifact"),
        )
        .expect("write corrupt artifact");

        build_preview(
            build_request(&walking_skeleton_fixture(), workspace.path()),
            &worker,
        )
        .expect("corrupt cache metadata is quarantined and regenerated");
        assert_eq!(
            worker.synthesis_count(),
            3 + index,
            "`{field}` metadata was rejected from reuse"
        );
    }

    let mut unknown_field = original;
    unknown_field["unexpected"] = Value::Bool(true);
    std::fs::write(
        &artifact_path,
        serde_json::to_vec_pretty(&unknown_field).expect("serialize unknown cache field"),
    )
    .expect("write cache artifact with unknown field");
    build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect("an unknown artifact field is quarantined and regenerated");
    assert_eq!(worker.synthesis_count(), 7);
}

#[test]
fn t4_e0_private_preview_cannot_enter_production_publication() {
    let (_workspace, result, _worker) = run_skeleton();
    let manifest_bytes = std::fs::read(&result.manifest).expect("read preview manifest");

    // The refusal is the release profile's, not a sentence this build writes:
    // a preview holds no gate evidence, so it cannot claim production however
    // many gates are implemented.
    assert!(matches!(
        publish(&result),
        Err(BuildError::Publication(PublicationError::Release(
            ReleaseError::PrivateProfileCannotClaimProduction
        )))
    ));
    assert!(matches!(
        validate_production_manifest(&manifest_bytes),
        Err(BuildError::Publication(
            PublicationError::UnsupportedProductionManifest { ref version }
        ))
            if version == "1.0-skeleton"
    ));
}

#[test]
fn t3_e0_registered_fixture_checksums_match_test_data_manifest() {
    let repository_root = repository_root();
    let manifest =
        std::fs::read_to_string(repository_root.join("docs/testing/TEST-DATA-MANIFEST.md"))
            .expect("read test-data manifest");
    // Every directory under `fixtures/` is discovered, not a hand-written list.
    // A list is exempt by omission: `fixtures/listening` would have been
    // unregistered and unnoticed the day it was added, which is the opposite of
    // what the rule below says. Discovering the directories makes the rule true
    // rather than approximately true.
    let mut fixture_directories: Vec<String> = std::fs::read_dir(repository_root.join("fixtures"))
        .expect("read the fixtures root")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| format!("fixtures/{}", entry.file_name().to_string_lossy()))
        .collect();
    fixture_directories.sort();
    assert!(
        !fixture_directories.is_empty(),
        "the fixtures root holds no directories to check"
    );

    // Every committed fixture is discovered rather than listed, so a new
    // fixture cannot be added without a manifest row.
    let mut checked = 0_usize;
    for directory in &fixture_directories {
        for entry in std::fs::read_dir(repository_root.join(directory))
            .expect("read registered fixture directory")
        {
            let entry = entry.expect("read registered fixture entry");
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{directory}/{file_name}");
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
    }

    // Guards against a misresolved path making the loop vacuous.
    assert!(
        checked >= 20,
        "expected at least twenty committed audio, lesson, and contract fixtures, checked {checked}"
    );
}

/// The cache is inside the workspace, so its interior needs the same
/// containment as the roots above it: a planted link is how a build is made to
/// read or write somewhere it was never given.
#[test]
fn t4_e0_cache_directory_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let outer = TempDir::new().expect("create cache symlink test root");
    let workspace = outer.path().join("workspace");
    let escape = outer.path().join("escape");
    std::fs::create_dir_all(workspace.join("cache")).expect("create cache root");
    std::fs::create_dir(&escape).expect("create escape target");
    symlink(&escape, workspace.join("cache").join("segments")).expect("plant segments symlink");

    let worker = DeterministicToneWorker::default();
    let error = build_preview(
        build_request(&walking_skeleton_fixture(), &workspace),
        &worker,
    )
    .expect_err("a symlinked cache directory must be refused");

    assert!(
        matches!(
            error,
            BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
        ),
        "a symlinked cache directory produced `{error}`"
    );
    assert_eq!(
        std::fs::read_dir(&escape)
            .expect("read escape target")
            .count(),
        0,
        "nothing may be written through the link"
    );
    assert_eq!(worker.synthesis_count(), 0);
}

/// Recovery must validate a matching stage as a real managed directory before
/// it reads or publishes any files beneath that name.
#[cfg(unix)]
#[test]
fn t4_e0_recovered_cache_stage_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let (workspace, result, worker) = run_skeleton();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result.manifest).expect("read preview manifest"))
            .expect("parse preview manifest");
    let cache_key: CacheKey = manifest["segments"][0]["cache_key"]
        .as_str()
        .expect("segment cache key")
        .parse()
        .expect("the manifest records a well-formed cache key");
    let entry_dir = find_cache_entry_dir(&workspace.path().join("cache"), &cache_key);
    let shard = entry_dir.parent().expect("cache entry has a shard");

    let outside = TempDir::new().expect("create outside stage target");
    std::fs::copy(
        entry_dir.join("audio.wav"),
        outside.path().join("audio.wav"),
    )
    .expect("copy valid audio outside the workspace");
    std::fs::copy(
        entry_dir.join("artifact.json"),
        outside.path().join("artifact.json"),
    )
    .expect("copy valid artifact outside the workspace");
    std::fs::remove_dir_all(&entry_dir).expect("remove the published test entry");
    let recovered_stage = shard.join(format!(".cache-stage-{}-planted", cache_key.as_str()));
    symlink(outside.path(), &recovered_stage).expect("plant recovered-stage symlink");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("a recovered-stage symlink must be refused");

    assert!(matches!(
        error,
        BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
    ));
    assert!(recovered_stage.is_symlink());
    assert!(outside.path().join("audio.wav").is_file());
    assert!(outside.path().join("artifact.json").is_file());
    assert_eq!(worker.synthesis_count(), 2);
}

/// A cache entry's own files are read back and trusted, so a link planted at
/// one is a way to feed the build bytes from outside the workspace.
#[test]
fn t4_e0_cache_file_symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;

    let (workspace, result, _worker) = run_skeleton();
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(result.manifest).expect("read preview manifest"))
            .expect("parse preview manifest");
    let cache_key: CacheKey = manifest["segments"][0]["cache_key"]
        .as_str()
        .expect("segment cache key")
        .parse()
        .expect("the manifest records a well-formed cache key");
    let entry_dir = find_cache_entry_dir(&workspace.path().join("cache"), &cache_key);

    let outside = workspace.path().join("outside.json");
    std::fs::write(&outside, b"{}").expect("write outside file");

    for record in ["artifact.json", "audio.wav"] {
        let planted = entry_dir.join(record);
        std::fs::remove_file(&planted).expect("remove the real cache record");
        symlink(&outside, &planted).expect("plant a cache record symlink");

        let worker = DeterministicToneWorker::default();
        let error = build_preview(
            build_request(&walking_skeleton_fixture(), workspace.path()),
            &worker,
        )
        .expect_err("a symlinked cache record must be refused");

        assert!(
            matches!(
                error,
                BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
            ),
            "a symlinked `{record}` produced `{error}`"
        );
        std::fs::remove_file(&planted).expect("remove the planted symlink");
        assert_eq!(
            std::fs::read(&outside).expect("read outside file"),
            b"{}",
            "the linked-to file must not be written through"
        );
    }
}

/// A second stream is invisible to a probe that asks only for the first one,
/// and the first one is the one this build writes correctly — so the check has
/// to count, not sample.
#[test]
fn t4_e0_multi_stream_output_is_rejected() {
    let (workspace, result, _worker) = run_skeleton();
    let two_stream = workspace.path().join("two-stream.m4a");

    let encoded = std::process::Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&result.master_wav)
        .arg("-i")
        .arg(&result.master_wav)
        .args([
            "-map", "0:a", "-map", "1:a", "-ac", "1", "-c:a", "aac", "-b:a", "96k",
        ])
        .arg(&two_stream)
        .status()
        .expect("run ffmpeg to build a two-stream export");
    assert!(
        encoded.success(),
        "ffmpeg must produce the two-stream fixture"
    );

    let error = validate_m4a_output(Path::new("ffprobe"), &two_stream)
        .expect_err("a two-stream export must not pass verification");

    assert!(
        matches!(
            error,
            BuildError::Tool(ToolError::UnexpectedEncodedStreamCount {
                found: 2,
                required: 1,
                ..
            })
        ),
        "a two-stream export produced `{error}`"
    );
}
