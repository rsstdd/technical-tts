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
use study_tts_core::{CacheKey, LessonError, MAX_LESSON_JSON_BYTES, ReleaseError};
use study_tts_runtime::{
    BackendDescriptor, BackendError, BuildError, BuildRequest, CacheEntryFault, CacheError,
    DurableStateError, IoError, ManagedPathError, PublicationError, SynthesisReport,
    SynthesisRequest, ToolError, TtsExecutor, build_preview, publish, validate_encoded_output,
    validate_production_manifest,
};
use study_tts_testkit::{
    DeterministicToneWorker, FIXTURE_VOICE_PROFILES, cache_identity_fixture,
    walking_skeleton_fixture, write_voice_profile_root,
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
    assert_eq!(manifest["schema_version"], "0.2-skeleton");
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
                .join("manifest-v0.schema.json"),
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
fn t4_e0_corrupt_job_snapshot_is_refused_without_overwrite() {
    let (workspace, _result, worker) = run_skeleton();
    let snapshot = workspace
        .path()
        .join("jobs/e0-s0-walking-skeleton/job.json");
    std::fs::write(&snapshot, b"{corrupt").expect("corrupt job snapshot");

    let error = build_preview(
        build_request(&walking_skeleton_fixture(), workspace.path()),
        &worker,
    )
    .expect_err("corrupt authoritative job state must be refused");

    assert!(matches!(
        error,
        BuildError::DurableState(ref state)
            if matches!(**state, DurableStateError::MalformedJobSnapshot { .. })
    ));
    assert_eq!(
        std::fs::read(snapshot).expect("job snapshot remains"),
        b"{corrupt"
    );
    assert_eq!(worker.synthesis_count(), 2);
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

    let failing_ffmpeg = workspace.path().join("failing-ffmpeg");
    std::fs::write(
        &failing_ffmpeg,
        b"#!/bin/sh\n\
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

    validate_encoded_output(Path::new("ffprobe"), &result.m4a)
        .expect("a mono AAC export must be accepted");

    // The PCM master is a valid audio file that is not a valid encoded output,
    // which is the shape an encoder failing open would produce.
    let error = validate_encoded_output(Path::new("ffprobe"), &result.master_wav)
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

        // Every one of these mutations makes the artifact describe audio this
        // build cannot consume, so they must all arrive as that fault rather
        // than merely as some cache error.
        let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
            panic!("`{field}` mutation produced the wrong variant: `{error}`");
        };
        assert!(
            matches!(**fault, CacheEntryFault::IncompatibleArtifact { .. }),
            "`{field}` mutation produced the wrong fault: `{fault}`"
        );
        let message = error.to_string();
        // A poisoned entry fails every later build, so the message must name
        // the artifact runtime reconciliation owns.
        assert!(
            message.contains(&entry_dir.display().to_string()),
            "`{field}` mutation did not name the entry directory: `{message}`"
        );
        assert!(
            message.contains("runtime reconciliation"),
            "`{field}` mutation did not route reconciliation: `{message}`"
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
    // `deny_unknown_fields` rejects this before any field is read, so it is a
    // parse failure and not an incompatible-metadata one.
    let BuildError::Cache(CacheError::UnusableCacheEntry { fault, .. }) = &error else {
        panic!("an unknown artifact field produced the wrong variant: `{error}`");
    };
    assert!(
        matches!(**fault, CacheEntryFault::UnparseableArtifact { .. }),
        "an unknown artifact field produced the wrong fault: `{fault}`"
    );
    assert!(error.to_string().contains("runtime reconciliation"));

    assert_eq!(worker.synthesis_count(), 2);
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
            if version == "0.2-skeleton"
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

    let error = validate_encoded_output(Path::new("ffprobe"), &two_stream)
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
