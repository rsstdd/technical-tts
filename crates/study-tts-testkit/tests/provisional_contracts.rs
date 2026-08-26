//! Tier 3 and 4 tests for the E0-S4 provisional contract baseline.

use std::{
    future::Future,
    io::Write,
    path::{Path, PathBuf},
    pin::Pin,
    process::{Command, Output, Stdio},
};

use serde_json::Value;
use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, ContractDescriptor,
    ContractVersionError, ProvisionalJobSnapshot, RenderPlan, SuccessorCompatibility,
    ValidatedLesson,
};
use study_tts_runtime::{
    BackendDescriptor, BackendError, BackendValidationError, BuildError, CacheResolveRequest,
    FileSystemCachePublisher, FileSystemJobRepository, FileSystemPackageWriter, JobRepository,
    MAX_WORKER_FRAME_BYTES, PackagePreflightRequest, PackagePrepareRequest, PackageWriteRequest,
    PreviewServiceBundle, SynthesisReport, SynthesisRequest, TTS_EXECUTOR_CONTRACT_VERSION,
    TtsExecutor, WorkerFrameError, WorkerRequestFrame, WorkerResponseFrame, build_preview,
    build_preview_with_services, parse_worker_request, parse_worker_response,
    validate_executor_request,
};
use study_tts_testkit::{
    FakeCachePublisher, FakeJobCall, FakePackageCall, FakePackageWriter, FakeTtsExecutor,
    InMemoryJobRepository, RecordingCachePublisher, RecordingJobRepository, RecordingPackageWriter,
    RecordingTtsExecutor, SeamEventLog, run_cache_contract_scenario,
    run_job_repository_contract_scenario, run_package_writer_contract_scenario,
    run_tts_executor_contract_scenario, walking_skeleton_fixture,
};
use tempfile::TempDir;

fn contract_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/contracts")
        .join(name)
}

fn read_fixture(name: &str) -> Vec<u8> {
    std::fs::read(contract_fixture(name)).expect("read contract fixture")
}

fn descriptor(name: &str) -> ContractDescriptor {
    serde_json::from_slice(&read_fixture(name)).expect("parse contract descriptor")
}

fn validated_plan(executor: &FakeTtsExecutor) -> RenderPlan {
    let lesson = ValidatedLesson::from_json(
        &std::fs::read(walking_skeleton_fixture()).expect("read lesson fixture"),
    )
    .expect("validate lesson fixture");
    RenderPlan::for_lesson(&lesson, &executor.descriptor().synthesis_identity)
}

fn synthesis_request(plan: &RenderPlan) -> SynthesisRequest {
    let segment = &plan.segments[0];
    SynthesisRequest {
        request_id: "contract-request-1".to_owned(),
        segment_id: segment.id.clone(),
        spoken_text: segment.spoken_text.clone(),
        voice: segment.speaker.clone(),
        style: segment.style.clone(),
        cache_key: segment.cache_key.clone(),
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
    }
}

fn build_request(workspace: &Path) -> study_tts_runtime::BuildRequest {
    study_tts_runtime::BuildRequest {
        lesson_path: walking_skeleton_fixture(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "ffmpeg".into(),
        ffprobe_executable: "ffprobe".into(),
        voice_profile_dir: None,
    }
}

#[derive(Debug)]
struct ZeroCapacityExecutor;

impl TtsExecutor for ZeroCapacityExecutor {
    fn descriptor(&self) -> BackendDescriptor {
        BackendDescriptor {
            contract_version: TTS_EXECUTOR_CONTRACT_VERSION.to_owned(),
            synthesis_identity: "zero-capacity-contract-executor".to_owned(),
            max_text_bytes: 64 * 1024,
            deterministic_seed: true,
        }
    }

    fn capacity(&self) -> usize {
        0
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
        _destination: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>> {
        Box::pin(async move {
            Err(BackendError::Protocol {
                request_id: request.request_id,
                message: "zero-capacity executor reached synthesis".to_owned(),
            })
        })
    }
}

#[test]
fn t3_e0_contract_change_requires_version_or_explicit_compatible_extension() {
    let baseline = descriptor("e0-s4-contract-baseline.json");

    assert_eq!(
        baseline
            .assess_successor(&baseline)
            .expect("unchanged descriptor"),
        SuccessorCompatibility::Unchanged
    );
    assert_eq!(
        baseline
            .assess_successor(&descriptor("e0-s4-contract-patch.json"))
            .expect("diagnostic patch"),
        SuccessorCompatibility::DiagnosticPatch
    );
    assert_eq!(
        baseline
            .assess_successor(&descriptor("e0-s4-contract-compatible-extension.json"))
            .expect("compatible extension"),
        SuccessorCompatibility::CompatibleExtension
    );
    assert_eq!(
        baseline
            .assess_successor(&descriptor("e0-s4-contract-breaking.json"))
            .expect("breaking major increment"),
        SuccessorCompatibility::Breaking
    );
    assert!(
        serde_json::from_slice::<ContractDescriptor>(&read_fixture(
            "e0-s4-contract-malformed.json"
        ))
        .is_err()
    );
    assert!(matches!(
        baseline.assess_successor(&descriptor("e0-s4-contract-unknown-successor.json")),
        Err(ContractVersionError::VersionClassMismatch { .. })
    ));

    let mut mislabeled_unchanged = baseline.clone();
    mislabeled_unchanged.extension_default = Some("changed semantic default".to_owned());
    assert!(matches!(
        baseline.assess_successor(&mislabeled_unchanged),
        Err(ContractVersionError::VersionClassMismatch { .. })
    ));
    let mut mislabeled_patch = descriptor("e0-s4-contract-patch.json");
    mislabeled_patch.extension_default = Some("changed semantic default".to_owned());
    assert!(matches!(
        baseline.assess_successor(&mislabeled_patch),
        Err(ContractVersionError::VersionClassMismatch { .. })
    ));

    let valid = parse_worker_request(trim_newline(&read_fixture("e0-s4-worker-valid.json")));
    assert!(
        matches!(valid, Ok(WorkerRequestFrame::Synthesize { .. })),
        "valid worker frame failed: {valid:?}"
    );
    assert!(matches!(
        parse_worker_request(trim_newline(&read_fixture(
            "e0-s4-worker-compatible-extension.json"
        ))),
        Ok(WorkerRequestFrame::Synthesize { .. })
    ));
    assert!(matches!(
        parse_worker_request(trim_newline(&read_fixture(
            "e0-s4-worker-incompatible-version.json"
        ))),
        Err(WorkerFrameError::UnsupportedVersion { .. })
    ));
    assert!(matches!(
        parse_worker_request(trim_newline(&read_fixture("e0-s4-worker-malformed.json"))),
        Err(WorkerFrameError::Malformed(_))
    ));
}

#[test]
fn t4_e0_every_provisional_seam_has_a_fake() {
    let workspace = TempDir::new().expect("create contract workspace");
    let executor = FakeTtsExecutor::default();
    let plan = validated_plan(&executor);
    let request = synthesis_request(&plan);
    let report = run_tts_executor_contract_scenario(
        &executor,
        request.clone(),
        &workspace.path().join("executor.wav"),
    )
    .expect("executor contract scenario");
    assert_eq!(report.frames, CANONICAL_SAMPLE_RATE / 10);
    let requests = executor.requests();
    assert_eq!(requests.as_slice(), std::slice::from_ref(&request));

    executor.fail_next(BackendError::Execution {
        request_id: request.request_id.clone(),
        code: "injected_failure".to_owned(),
        message: "contract failure".to_owned(),
    });
    assert!(matches!(
        run_tts_executor_contract_scenario(
            &executor,
            request.clone(),
            &workspace.path().join("failure.wav")
        ),
        Err(BackendError::Execution { .. })
    ));

    let cache = FakeCachePublisher::default();
    let cache_executor = FakeTtsExecutor::default();
    let mut pending = Some(request.clone());
    let mut producer = |destination: &Path| {
        run_tts_executor_contract_scenario(
            &cache_executor,
            pending.take().expect("cache miss produces once"),
            destination,
        )
    };
    let cache_request = CacheResolveRequest {
        workspace: workspace.path().to_path_buf(),
        job_id: "contract-job".to_owned(),
        segment: plan.segments[0].clone(),
    };
    let cached = run_cache_contract_scenario(&cache, &cache_request, &mut producer)
        .expect("cache contract scenario");
    assert_eq!(cached[0], cached[1]);
    assert_eq!(cache_executor.synthesis_count(), 1);
    assert_eq!(cache.requests().len(), 2);

    let jobs = InMemoryJobRepository::default();
    let snapshot = ProvisionalJobSnapshot::planned("contract-job", plan.plan_hash.as_str());
    assert_eq!(
        run_job_repository_contract_scenario(&jobs, workspace.path(), &snapshot)
            .expect("job contract scenario"),
        snapshot
    );
    let caching = snapshot.advancing(study_tts_core::ProvisionalJobStage::Caching);
    jobs.replace(workspace.path(), &caching)
        .expect("retain second job replacement");
    assert_eq!(jobs.snapshots(), [snapshot.clone(), caching]);
    assert_eq!(
        jobs.calls(),
        [
            FakeJobCall::Claim("contract-job".to_owned()),
            FakeJobCall::Replace(study_tts_core::ProvisionalJobStage::Planned),
            FakeJobCall::Load("contract-job".to_owned()),
            FakeJobCall::Replace(study_tts_core::ProvisionalJobStage::Caching),
        ]
    );

    let packages = FakePackageWriter::new(workspace.path().join("fake-packages"));
    let preflight = PackagePreflightRequest {
        ffmpeg_executable: Path::new("study-tts-missing-ffmpeg"),
        ffprobe_executable: Path::new("study-tts-missing-ffprobe"),
    };
    let prepare = PackagePrepareRequest {
        workspace: workspace.path(),
        job_id: "contract-job",
        plan: &plan,
    };
    let write = PackageWriteRequest {
        workspace: workspace.path(),
        job_id: "contract-job",
        plan: &plan,
        cached_artifacts: &cached[..1],
    };
    let publications =
        run_package_writer_contract_scenario(&packages, &preflight, &prepare, &write)
            .expect("package contract scenario");
    assert_eq!(publications[0], publications[1]);
    assert_eq!(
        packages.calls(),
        [
            FakePackageCall::Preflight,
            FakePackageCall::Prepare,
            FakePackageCall::Write,
            FakePackageCall::Write,
        ]
    );
}

#[test]
fn t4_e0_walking_skeleton_uses_only_published_seams() {
    let workspace = TempDir::new().expect("create seam workspace");
    let events = SeamEventLog::default();
    let executor = RecordingTtsExecutor::new(FakeTtsExecutor::default(), events.clone());
    let cache = RecordingCachePublisher::new(FileSystemCachePublisher, events.clone());
    let packages = RecordingPackageWriter::new(FileSystemPackageWriter, events.clone());
    let jobs = RecordingJobRepository::new(FileSystemJobRepository, events.clone());
    let services = PreviewServiceBundle {
        executor: &executor,
        cache: &cache,
        packages: &packages,
        jobs: &jobs,
    };

    let first = build_preview_with_services(build_request(workspace.path()), services)
        .expect("first seam build");
    let second = build_preview_with_services(build_request(workspace.path()), services)
        .expect("second seam build");

    assert_eq!(executor.inner().synthesis_count(), 2);
    assert_eq!(first.package_dir, second.package_dir);
    assert_eq!(first.publication_record, second.publication_record);
    assert_eq!(first.manifest, second.manifest);
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(&second.manifest).expect("read selected manifest"))
            .expect("parse selected manifest");
    assert_eq!(manifest["release_status"], "private_preview");
    assert_eq!(manifest["schema_version"], "0.2-skeleton");
    assert_eq!(
        hound::WavReader::open(&second.master_wav)
            .expect("open master WAV")
            .duration(),
        9_600
    );

    let events = events.events();
    let validate_first = event_position(&events, "executor.validate:seg-0001");
    let preflight = event_position(&events, "package.preflight");
    let claim = event_position(&events, "job.claim");
    let prepare = event_position(&events, "package.prepare");
    let cache_first = event_position(&events, "cache.resolve:seg-0001");
    let synth_first = event_position(&events, "executor.synthesize:seg-0001");
    let package = event_position(&events, "package.write");
    assert!(validate_first < preflight);
    assert!(preflight < claim);
    assert!(claim < prepare);
    assert!(prepare < cache_first);
    assert!(cache_first < synth_first);
    assert!(synth_first < package);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.starts_with("executor.synthesize:"))
            .count(),
        2
    );
}

#[test]
fn t4_e0_executor_validation_precedes_tools_and_durable_state() {
    let root = TempDir::new().expect("create gate-order root");
    let workspace = root.path().join("not-created");
    let mut request = build_request(&workspace);
    request.ffmpeg_executable = "study-tts-missing-ffmpeg".into();

    let error = build_preview(request, &ZeroCapacityExecutor)
        .expect_err("executor validation must reject before package preflight");

    assert!(matches!(
        error,
        BuildError::Synthesis(source)
            if matches!(
                source.as_ref(),
                BackendError::InvalidRequest {
                    source: BackendValidationError::ZeroCapacity,
                    ..
                }
            )
    ));
    assert!(!workspace.exists());
}

#[test]
fn t3_e0_worker_frame_ceiling_and_unknown_fields_fail_closed() {
    let oversized = vec![b' '; MAX_WORKER_FRAME_BYTES + 1];
    assert!(matches!(
        parse_worker_request(&oversized),
        Err(WorkerFrameError::TooLarge { .. })
    ));
    assert!(matches!(
        parse_worker_request(
            concat!(
                r#"{"method":"shutdown","protocol_version":"e0.worker.0.1","#,
                r#""request_id":"id","extra":true}"#,
            )
            .as_bytes()
        ),
        Err(WorkerFrameError::Malformed(_))
    ));
    assert!(matches!(
        parse_worker_response(
            br#"{"event":"shutdown","protocol_version":"e0.worker.9.0","request_id":"id"}"#
        ),
        Err(WorkerFrameError::UnsupportedVersion { .. })
    ));
    assert!(matches!(
        parse_worker_response(
            br#"{"event":"shutdown","protocol_version":"e0.worker.0.1","request_id":""}"#
        ),
        Err(WorkerFrameError::EmptyRequestId)
    ));
    assert!(matches!(
        parse_worker_response(
            concat!(
                r#"{"event":"progress","protocol_version":"e0.worker.0.1","#,
                r#""request_id":"id","progress":1.1}"#,
            )
            .as_bytes()
        ),
        Err(WorkerFrameError::InvalidProgress { .. })
    ));
}

#[test]
fn t4_e0_executable_fake_worker_exposes_deterministic_and_fault_behaviors() {
    let workspace = TempDir::new().expect("create executable-worker workspace");
    let audio = workspace.path().join("fake.wav");
    let request = worker_request_for(&audio);

    let deterministic = run_fake_worker("deterministic", &request);
    assert!(deterministic.status.success());
    assert!(deterministic.stderr.is_empty());
    assert!(matches!(
        parse_worker_response(trim_newline(&deterministic.stdout)),
        Ok(WorkerResponseFrame::SynthesisSucceeded { .. })
    ));
    assert_eq!(
        hound::WavReader::open(&audio)
            .expect("open fake-worker WAV")
            .duration(),
        CANONICAL_SAMPLE_RATE / 10
    );

    let failure = run_fake_worker("failure", &request);
    assert!(failure.status.success());
    assert!(matches!(
        parse_worker_response(trim_newline(&failure.stdout)),
        Ok(WorkerResponseFrame::Failure { .. })
    ));

    let malformed = run_fake_worker("malformed-frame", &request);
    assert!(malformed.status.success());
    assert!(parse_worker_response(trim_newline(&malformed.stdout)).is_err());

    let truncated = run_fake_worker("truncated-audio", &request);
    assert!(truncated.status.success());
    assert_eq!(std::fs::read(&audio).expect("read truncated WAV"), b"RIFF");

    let stderr = run_fake_worker("stderr", &request);
    assert!(stderr.status.success());
    assert!(String::from_utf8_lossy(&stderr.stderr).contains("fake worker diagnostic"));
    assert!(parse_worker_response(trim_newline(&stderr.stdout)).is_ok());

    let exit = Command::new(env!("CARGO_BIN_EXE_fake-ndjson-worker"))
        .arg("exit")
        .output()
        .expect("run fake-worker exit behavior");
    assert_eq!(exit.status.code(), Some(17));
}

fn trim_newline(bytes: &[u8]) -> &[u8] {
    bytes
        .strip_suffix(b"\n")
        .and_then(|bytes| bytes.strip_suffix(b"\r").or(Some(bytes)))
        .unwrap_or(bytes)
}

fn event_position(events: &[String], expected: &str) -> usize {
    events
        .iter()
        .position(|event| event == expected)
        .unwrap_or_else(|| panic!("missing seam event `{expected}` from {events:?}"))
}

fn worker_request_for(output: &Path) -> Vec<u8> {
    let mut request: Value =
        serde_json::from_slice(trim_newline(&read_fixture("e0-s4-worker-valid.json")))
            .expect("parse valid worker fixture");
    request["parameters"]["output"] = Value::String(output.display().to_string());
    serde_json::to_vec(&request).expect("serialize worker request")
}

fn run_fake_worker(behavior: &str, request: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fake-ndjson-worker"))
        .arg(behavior)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start executable fake worker");
    let mut stdin = child.stdin.take().expect("fake worker stdin");
    stdin.write_all(request).expect("write fake-worker request");
    stdin.write_all(b"\n").expect("terminate NDJSON request");
    drop(stdin);
    child
        .wait_with_output()
        .expect("collect fake-worker output")
}
