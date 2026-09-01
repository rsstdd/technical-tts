//! T4: the fake worker against the published protocol, and the guarantee that
//! the PR suite never reaches for a model.
//!
//! Both tests named here are E1-S1 contract tests from `DELIVERY-PLAN.md`.
//! They cover the two halves of "a fake is usable": it must speak the contract
//! its real counterpart will speak, and running it must cost nothing the real
//! one costs — no weights, no download, no reference machine.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;
use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, DeterminismClass,
    RenderPlan, ValidatedLesson,
};
use study_tts_runtime::{
    BackendError, BackendValidationError, BuildError, BuildRequest, CachePublisher,
    CacheResolveRequest, DriftedIdentity, FileSystemCachePublisher, MAX_WORKER_FRAME_BYTES,
    MAX_WORKER_REQUEST_ID_BYTES, SynthesisRequest, THREAD_ENVIRONMENT, TtsExecutor,
    WORKER_PROTOCOL_SCHEMA_VERSION, WORKER_PROTOCOL_VERSION, WorkerConfiguration,
    WorkerFailureCode, WorkerInitializationIdentities, WorkerLauncher, WorkerRequestFrame,
    WorkerResponseFrame, WorkerTtsExecutor, build_preview, parse_worker_request,
    parse_worker_response,
};
use study_tts_testkit::{DETERMINISTIC_TONE_BUNDLE_HASH, deterministic_tone_conditioning};
use study_tts_testkit::{
    FIXTURE_VOICE_PROFILES, FakeTtsExecutor, run_tts_executor_contract_scenario,
    run_worker_restart_contract_scenario, validate_against_schema, walking_skeleton_fixture,
    write_voice_profile_root,
};
use tempfile::TempDir;

const FAKE_SESSION_DEADLINE: Duration = Duration::from_secs(2);
const FAKE_SESSION_POLL_INTERVAL: Duration = Duration::from_millis(10);

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The compiled fake worker beside this test binary.
///
/// Cargo puts an integration test and the binaries of its own package in the
/// same directory, so this locates the fake without a build script and without
/// assuming a profile name.
fn fake_worker() -> PathBuf {
    let mut directory = std::env::current_exe().expect("the test binary has a path");
    directory.pop();
    if directory.ends_with("deps") {
        directory.pop();
    }
    let executable = directory.join("fake-ndjson-worker");
    assert!(
        executable.is_file(),
        "`{}` must be built alongside this test",
        executable.display()
    );
    executable
}

/// One committed frame and the decision both ends of the protocol must make.
#[derive(Debug)]
struct ContractCase {
    case: String,
    accepted: bool,
    frame: String,
    why: String,
}

impl ContractCase {
    /// Reads one case line.
    ///
    /// Field by field rather than through a `Deserialize` derive: `serde` is
    /// not a dependency of this crate, and a fixture this crate owns does not
    /// earn one when a missing or mistyped field already fails loudly below.
    fn from_line(line: &str) -> Self {
        let case: Value = serde_json::from_str(line).expect("a contract case is JSON");
        Self {
            case: case_string(&case, "case"),
            accepted: case["accepted"]
                .as_bool()
                .expect("`accepted` is a boolean in every contract case"),
            frame: case_string(&case, "frame"),
            why: case_string(&case, "why"),
        }
    }
}

fn case_string(case: &Value, field: &str) -> String {
    case[field]
        .as_str()
        .unwrap_or_else(|| panic!("`{field}` is a string in every contract case"))
        .to_owned()
}

#[test]
fn t3_e1_both_protocol_ends_decide_the_committed_cases_alike() {
    // Every case here is one the two implementations used to disagree about, or
    // a neighbour of one. They disagreed because each suite wrote its own
    // cases: the Python end accepted a `trace_context` under the baseline
    // version, refused the extension version outright, and — having no integer
    // width and no duplicate-key rule — answered frames `serde_json` dropped.
    // A shared file is the only kind of contract test that can catch that,
    // because a rule only one end enforces is a rule the other end can send
    // past. `worker/tests/test_protocol.py::SharedContractCaseTests` reads this
    // same file.
    let path = repository_root().join("fixtures/contracts/e1-s1-worker-protocol-cases.ndjson");
    let cases: Vec<ContractCase> = fs::read_to_string(&path)
        .expect("the shared contract cases are readable")
        .lines()
        .map(ContractCase::from_line)
        .collect();
    assert!(
        cases.len() >= 20,
        "the shared cases must cover the contract, not a sample of it"
    );

    for case in &cases {
        let decision = parse_worker_request(case.frame.as_bytes());

        assert_eq!(
            decision.is_ok(),
            case.accepted,
            "`{}` must be {}: {}; got {decision:?}",
            case.case,
            if case.accepted { "accepted" } else { "refused" },
            case.why
        );
    }

    // The published schema is the third end, and these two cases are where it
    // could disagree with the other two: `maxLength` counts code points where
    // both runtimes count UTF-8 bytes, on the request identity and on the
    // `active_request_id` a cancellation echoes back.
    let schema: Value = serde_json::from_slice(
        &fs::read(repository_root().join(format!(
            "schemas/worker-protocol-v{}.schema.json",
            WORKER_PROTOCOL_SCHEMA_VERSION.major()
        )))
        .expect("the worker-protocol schema is readable"),
    )
    .expect("the worker-protocol schema is JSON");
    for name in [
        "non-ascii-request-id-under-the-character-ceiling",
        "active-request-id-past-the-ceiling",
    ] {
        let case = cases
            .iter()
            .find(|case| case.case == name)
            .unwrap_or_else(|| panic!("the shared cases must include `{name}`"));
        let document: Value =
            serde_json::from_str(&case.frame).expect("a shared case frame is JSON");
        assert!(
            validate_against_schema(&schema, &document).is_err(),
            "the published schema must refuse `{name}`, which both runtimes refuse"
        );
    }
}

#[test]
fn t4_e1_fake_worker_passes_shared_protocol_contract() {
    // The session is a committed fixture rather than frames built here: the
    // real worker will be driven by the same file, and a fixture two
    // implementations share is the only kind of contract test that can catch
    // them disagreeing.
    let session = fs::read_to_string(
        repository_root().join("fixtures/contracts/e1-s1-fake-worker-session.ndjson"),
    )
    .expect("the session fixture is readable");
    let requests: Vec<WorkerRequestFrame> = session
        .lines()
        .map(|line| parse_worker_request(line.as_bytes()).expect("a session frame is valid"))
        .collect();
    assert_eq!(requests.len(), 6, "the session must exercise every method");

    // Run inside a scratch directory. The session's `output` is a managed
    // relative path, and a worker that wrote it into the source tree would
    // leave a file behind on every run — which is how a test starts depending
    // on a previous one.
    let staging = TempDir::new().expect("create a worker staging directory");
    let stdout = drive(&staging, &["deterministic"], &session);
    let schema: Value = serde_json::from_slice(
        // Named from the published version rather than spelled out, so a
        // protocol major move does not leave this reading a file that is no
        // longer written.
        &fs::read(repository_root().join(format!(
            "schemas/worker-protocol-v{}.schema.json",
            WORKER_PROTOCOL_SCHEMA_VERSION.major()
        )))
        .expect("the worker-protocol schema is readable"),
    )
    .expect("the worker-protocol schema is JSON");
    let responses: Vec<WorkerResponseFrame> = stdout
        .lines()
        .map(|line| parse_schema_validated_response(&schema, line))
        .collect();

    assert_eq!(
        responses.len(),
        requests.len(),
        "every request must be answered exactly once"
    );

    // Correlation is the property a supervisor depends on: it matches a
    // response to the request it was waiting on by ID, and nothing else.
    for (request, response) in requests.iter().zip(&responses) {
        assert_eq!(
            response_request_id(response),
            request_request_id(request),
            "each response must carry its request's ID"
        );
        assert_eq!(
            response_protocol_version(response),
            WORKER_PROTOCOL_VERSION,
            "each response must declare the protocol version it speaks"
        );
    }

    // The fake answers every method with the frame that method defines, and no
    // failure frame: a fake that failed here would make a supervisor test pass
    // for the wrong reason.
    let WorkerResponseFrame::Initialized { identities, .. } = &responses[0] else {
        panic!(
            "initialize must be answered by initialized, got {:?}",
            responses[0]
        )
    };
    assert_eq!(
        identities,
        &WorkerInitializationIdentities {
            model_revision: "v1".parse().expect("`v1` is a revision"),
            tokenizer_revision: "none".parse().expect("`none` is a revision"),
            worker_bundle_hash: DETERMINISTIC_TONE_BUNDLE_HASH
                .parse()
                .expect("the fake bundle identity is a digest"),
            voice_conditioning_hashes: BTreeMap::from([(
                "synthetic-test-voice-v1".to_owned(),
                deterministic_tone_conditioning("synthetic-test-voice-v1"),
            )]),
        }
    );
    let mut empty_identities: Value = serde_json::from_str(
        stdout
            .lines()
            .next()
            .expect("the fake answers initialization first"),
    )
    .expect("the initialized response is JSON");
    empty_identities["identities"]["voice_conditioning_hashes"] = serde_json::json!({});
    assert!(
        validate_against_schema(&schema, &empty_identities).is_err(),
        "the schema must refuse a successful initialization with no loaded voice profile"
    );
    assert!(
        matches!(responses[1], WorkerResponseFrame::Capabilities { .. }),
        "capabilities must be answered by capabilities, got {:?}",
        responses[1]
    );
    assert!(
        matches!(responses[2], WorkerResponseFrame::Health { .. }),
        "health must be answered by health, got {:?}",
        responses[2]
    );
    assert!(matches!(
        responses[2],
        WorkerResponseFrame::Health {
            ready: true,
            model_loaded: false,
            ..
        }
    ));
    let WorkerResponseFrame::SynthesisSucceeded {
        model_revision,
        codec_revision,
        worker_bundle_hash,
        voice_conditioning_hash,
        ..
    } = &responses[3]
    else {
        panic!(
            "synthesize must be answered by synthesis_succeeded, got {:?}",
            responses[3]
        )
    };
    assert_eq!(model_revision, identities.model_revision.as_str());
    assert_eq!(codec_revision, identities.tokenizer_revision.as_str());
    assert_eq!(worker_bundle_hash, &identities.worker_bundle_hash);
    // The artifact the worker reported at synthesis is the one it reported
    // loading at initialization: a worker that swapped voices mid-session would
    // be publishing under a key its own `initialized` frame contradicts.
    assert_eq!(
        identities
            .voice_conditioning_hashes
            .get("synthetic-test-voice-v1"),
        Some(voice_conditioning_hash)
    );
    assert!(
        matches!(responses[4], WorkerResponseFrame::Cancelled { .. }),
        "cancel must be answered by cancelled, got {:?}",
        responses[4]
    );
    let WorkerRequestFrame::Cancel {
        active_request_id: requested_active_id,
        ..
    } = &requests[4]
    else {
        panic!("the fifth session frame must be cancel")
    };
    let WorkerResponseFrame::Cancelled {
        active_request_id: answered_active_id,
        ..
    } = &responses[4]
    else {
        panic!("the fifth response frame must be cancelled")
    };
    assert_eq!(
        requested_active_id.len(),
        MAX_WORKER_REQUEST_ID_BYTES,
        "the shared process session must exercise the exact active-ID ceiling"
    );
    assert_eq!(
        requested_active_id,
        request_request_id(&requests[3]),
        "the cancellation identity must name the synthesis request in the session"
    );
    assert_eq!(
        answered_active_id, requested_active_id,
        "the worker must echo a boundary identity byte for byte"
    );
    assert!(
        matches!(responses[5], WorkerResponseFrame::Shutdown { .. }),
        "shutdown must be answered by shutdown, got {:?}",
        responses[5]
    );

    let mismatched_bundle = "f".repeat(64);
    let mismatched_session = serde_json::json!({
        "method": "initialize",
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": "mismatched-bundle",
        "parameters": {
            "worker_bundle_hash": mismatched_bundle,
            "threads": 1,
            "staging_root": staging.path(),
        },
    })
    .to_string();
    let mismatch = drive(&staging, &["deterministic"], &mismatched_session);
    let mismatch_responses: Vec<WorkerResponseFrame> = mismatch
        .lines()
        .map(|line| parse_schema_validated_response(&schema, line))
        .collect();
    assert!(
        matches!(
            mismatch_responses.as_slice(),
            [WorkerResponseFrame::Failure {
                code: WorkerFailureCode::InitializationFailed,
                recoverable: false,
                ..
            }]
        ),
        "the fake must refuse a bundle identity other than its own, got {mismatch_responses:?}"
    );

    // A refusal is still a valid frame. The fault-injecting behavior is what
    // the supervisor tests drive, so it has to satisfy the same contract.
    let refusal = drive(&staging, &["failure"], &session);
    let refusals: Vec<WorkerResponseFrame> = refusal
        .lines()
        .map(|line| parse_schema_validated_response(&schema, line))
        .collect();
    assert!(
        refusals.iter().any(|frame| matches!(
            frame,
            WorkerResponseFrame::Failure {
                code: WorkerFailureCode::SynthesisFailed,
                ..
            }
        )),
        "the failure behavior must produce a typed failure frame, got {refusals:?}"
    );
}

#[test]
fn t4_e1_fake_worker_contract_deadline_kills_and_reaps_a_hung_worker() {
    let staging = TempDir::new().expect("create a worker staging directory");
    let session = serde_json::json!({
        "method": "health",
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": "hung-worker",
    })
    .to_string();
    let mut worker =
        FakeWorkerChild::spawn(&staging, &["hang"]).expect("the hanging fake worker starts");
    let worker_id = worker.id();
    worker
        .write_session(&session)
        .expect("the session is writable to the hanging worker");

    let started = Instant::now();
    let error = worker
        .wait_with_output_until_deadline()
        .expect_err("the hanging fake worker must time out");

    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(
        started.elapsed() < FAKE_SESSION_DEADLINE + Duration::from_secs(1),
        "timeout cleanup must finish within one second of the fake-session deadline"
    );
    assert!(
        !Path::new("/proc").join(worker_id.to_string()).exists(),
        "the timed-out fake worker must be reaped, not left as process {worker_id}"
    );
}

fn parse_schema_validated_response(schema: &Value, line: &str) -> WorkerResponseFrame {
    assert!(
        line.len() <= MAX_WORKER_FRAME_BYTES,
        "a response frame must stay within the declared ceiling"
    );
    let document: Value = serde_json::from_str(line).expect("a fake response frame is JSON");
    validate_against_schema(schema, &document)
        .unwrap_or_else(|violations| panic!("the schema refused `{line}`: {violations:?}"));
    // The parser is what the supervisor uses. Schema-only acceptance would
    // let a lenient fake hide a refusal in the real boundary.
    parse_worker_response(line.as_bytes()).expect("a response frame is valid")
}

/// Runs the fake worker in `staging` over `session` and returns its stdout.
fn drive(staging: &TempDir, arguments: &[&str], session: &str) -> String {
    let mut worker = FakeWorkerChild::spawn(staging, arguments).expect("the fake worker starts");
    worker
        .write_session(session)
        .expect("the session is writable to the worker");
    let output = worker
        .wait_with_output_until_deadline()
        .expect("the fake worker exits before the session deadline");
    assert!(
        output.status.success(),
        "the fake worker must exit cleanly, got {}",
        output.status
    );
    String::from_utf8(output.stdout).expect("worker stdout is UTF-8")
}

struct FakeWorkerChild {
    child: Option<Child>,
}

impl FakeWorkerChild {
    fn spawn(staging: &TempDir, arguments: &[&str]) -> io::Result<Self> {
        let child = Command::new(fake_worker())
            .args(arguments)
            .current_dir(staging.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        Ok(Self { child: Some(child) })
    }

    fn id(&self) -> u32 {
        self.child
            .as_ref()
            .expect("a live fake-worker guard owns a child")
            .id()
    }

    fn write_session(&mut self, session: &str) -> io::Result<()> {
        let mut stdin = self.take_stdin()?;
        stdin.write_all(session.as_bytes())
    }

    fn wait_with_output_until_deadline(mut self) -> io::Result<Output> {
        let deadline = Instant::now() + FAKE_SESSION_DEADLINE;
        loop {
            if self.child_mut().try_wait()?.is_some() {
                return self
                    .child
                    .take()
                    .expect("an exited fake-worker guard owns a child")
                    .wait_with_output();
            }
            if Instant::now() >= deadline {
                self.kill_and_reap()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "fake worker exceeded the two-second session deadline",
                ));
            }
            thread::sleep(FAKE_SESSION_POLL_INTERVAL);
        }
    }

    fn take_stdin(&mut self) -> io::Result<ChildStdin> {
        self.child_mut().stdin.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::BrokenPipe, "fake worker has no open stdin")
        })
    }

    fn child_mut(&mut self) -> &mut Child {
        self.child
            .as_mut()
            .expect("a live fake-worker guard owns a child")
    }

    fn kill_and_reap(&mut self) -> io::Result<()> {
        let child = self.child_mut();
        match child.kill() {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
            Err(error) => return Err(error),
        }
        child.wait()?;
        self.child.take();
        Ok(())
    }
}

impl Drop for FakeWorkerChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn request_request_id(frame: &WorkerRequestFrame) -> &str {
    match frame {
        WorkerRequestFrame::Initialize { request_id, .. }
        | WorkerRequestFrame::Capabilities { request_id, .. }
        | WorkerRequestFrame::Health { request_id, .. }
        | WorkerRequestFrame::Synthesize { request_id, .. }
        | WorkerRequestFrame::Cancel { request_id, .. }
        | WorkerRequestFrame::Shutdown { request_id, .. } => request_id,
    }
}

fn response_request_id(frame: &WorkerResponseFrame) -> &str {
    match frame {
        WorkerResponseFrame::Initialized { request_id, .. }
        | WorkerResponseFrame::Capabilities { request_id, .. }
        | WorkerResponseFrame::Health { request_id, .. }
        | WorkerResponseFrame::Progress { request_id, .. }
        | WorkerResponseFrame::SynthesisSucceeded { request_id, .. }
        | WorkerResponseFrame::Cancelled { request_id, .. }
        | WorkerResponseFrame::Shutdown { request_id, .. }
        | WorkerResponseFrame::Failure { request_id, .. } => request_id,
    }
}

fn response_protocol_version(frame: &WorkerResponseFrame) -> &str {
    match frame {
        WorkerResponseFrame::Initialized {
            protocol_version, ..
        }
        | WorkerResponseFrame::Capabilities {
            protocol_version, ..
        }
        | WorkerResponseFrame::Health {
            protocol_version, ..
        }
        | WorkerResponseFrame::Progress {
            protocol_version, ..
        }
        | WorkerResponseFrame::SynthesisSucceeded {
            protocol_version, ..
        }
        | WorkerResponseFrame::Cancelled {
            protocol_version, ..
        }
        | WorkerResponseFrame::Shutdown {
            protocol_version, ..
        }
        | WorkerResponseFrame::Failure {
            protocol_version, ..
        } => protocol_version,
    }
}

#[test]
fn t4_e1_pr_suite_performs_no_model_download() {
    // The claim under test is the one ADR-0001 §14 makes about ordinary CI: a
    // pull-request run renders a whole lesson without weights, without a model
    // root, and without reaching a network.
    //
    // What a test can and cannot prove here is worth being exact about. A test
    // cannot observe the absence of a syscall it did not make, so "no network"
    // is proved by CI running this suite inside a namespace with no interfaces
    // (`.github/workflows/ci.yml`, "Run T4 suite without runtime egress").
    // What this test proves is the part that survives outside CI: a full render
    // reads no model root, creates none, and the worker that will one day load
    // a model is configured so that a download is refused rather than
    // attempted.
    let repository_models = repository_root().join("data/models");
    let before = read_recursively(&repository_models);

    let workspace = TempDir::new().expect("create a preview workspace");
    let lesson = ValidatedLesson::from_json(
        &walking_skeleton_fixture().display().to_string(),
        &fs::read(walking_skeleton_fixture()).expect("the lesson fixture is readable"),
    )
    .expect("the lesson fixture validates");
    assert_eq!(
        lesson.segments().len(),
        2,
        "the suite must render a real lesson, not an empty one"
    );

    let executor = FakeTtsExecutor::default();
    let voice_profile_root = workspace.path().join("voices");
    write_voice_profile_root(&voice_profile_root, &FIXTURE_VOICE_PROFILES);
    build_preview(
        BuildRequest {
            lesson_path: walking_skeleton_fixture(),
            workspace: workspace.path().to_path_buf(),
            ffmpeg_executable: "ffmpeg".into(),
            ffprobe_executable: "ffprobe".into(),
            voice_profile_root,
        },
        &executor,
    )
    .expect("the PR suite renders a lesson end to end");
    assert_eq!(
        executor.synthesis_count(),
        lesson.segments().len(),
        "the render must have gone through the fake executor rather than a real backend"
    );

    // The backend the PR suite renders through names no downloadable model.
    // `study-tts/deterministic-tone` is not a repository anything could fetch,
    // which is what makes the render's independence structural rather than
    // incidental.
    let descriptor = executor.descriptor();
    assert_eq!(descriptor.model_repository, "study-tts/deterministic-tone");
    assert_eq!(descriptor.tokenizer_revision.as_str(), "none");

    // Nothing appeared under the operator's model root, and none was created
    // inside the workspace. A download would have to land in one of the two.
    assert_eq!(
        read_recursively(&repository_models),
        before,
        "a PR-suite render must not write to `{}`",
        repository_models.display()
    );
    assert!(
        read_recursively(workspace.path())
            .iter()
            .all(|path| !path.contains("models")),
        "a preview build must not create a model root inside its workspace"
    );

    // Drift check on the launcher, and only that. What makes the worker
    // offline is `study_tts_worker.worker._apply_offline_environment`, which
    // puts these variables into the process before a backend could import and
    // refuses to start when the launcher does not describe an offline worker;
    // `worker/tests/test_worker.py` exercises that in a subprocess, which is
    // the only place it can be observed. Reading the values here would prove
    // nothing on its own — a file can say `HF_HUB_OFFLINE` and no process ever
    // read it, which is what this repository shipped until the worker applied
    // it.
    //
    // What this half is still worth: the Python side refuses a launcher that
    // permits fetching, and this side notices a launcher edited to *drop* a
    // variable neither ADR-0001 §14 nor `REQUIRED_OFFLINE_ENVIRONMENT` allows
    // to go missing. Read from the file rather than restated, so such an edit
    // fails here.
    let launcher: Value = serde_json::from_slice(
        &fs::read(repository_root().join("worker/launcher.json"))
            .expect("the launcher configuration is readable"),
    )
    .expect("the launcher configuration is JSON");
    assert_eq!(
        launcher.get("local_files_only").and_then(Value::as_bool),
        Some(true),
        "the worker must load only local files"
    );
    for (variable, expected) in [
        ("HF_HUB_OFFLINE", "1"),
        ("TRANSFORMERS_OFFLINE", "1"),
        ("HF_HUB_DISABLE_PROGRESS_BARS", "1"),
    ] {
        assert_eq!(
            launcher
                .get("offline_environment")
                .and_then(|environment| environment.get(variable))
                .and_then(Value::as_str),
            Some(expected),
            "the launcher must set `{variable}` per ADR-0001 §14"
        );
    }
}

/// Every file beneath `root`, as paths relative to it.
///
/// An absent root yields nothing rather than failing, because "the model root
/// does not exist" is one of the two states this test accepts before a render
/// and requires to be unchanged after it.
fn read_recursively(root: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(
                    path.strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    found
}

// ---------------------------------------------------------------------------
// E1-S3 step 1: the capacity-one executor, driven against the protocol fake.
// ---------------------------------------------------------------------------

/// A configuration that starts the protocol fake with the identity it demands.
///
/// The bundle hash is the fake's own: `fake-ndjson-worker` refuses to
/// initialize under any other, which is the same refusal the real worker owes
/// when it is asked to be a bundle it is not.
fn fake_worker_configuration(behavior: &str) -> WorkerConfiguration {
    fake_worker_configuration_tagged(behavior, "untagged")
}

/// The same configuration, with a marker argument the fake ignores.
///
/// `parse_behavior` reads only the first argument, so the second is inert to
/// the worker and is there for the test: the process table is shared state and
/// two tests in this binary run `fake-ndjson-worker hang` at once, so a test
/// that looked for its worker by behavior alone would find somebody else's.
fn fake_worker_configuration_tagged(behavior: &str, tag: &str) -> WorkerConfiguration {
    // The fake's own constructor, which takes no bundle identity: every
    // configuration it builds runs under `PROTOCOL_FAKE_BUNDLE_HASH`, so this
    // helper cannot be a way to start something under a real bundle's name.
    // The deadline is short, because every fault path here is proved by waiting
    // one out — the shipped ceilings are `WORKER_INITIALIZE_DEADLINE` and
    // `WORKER_REQUEST_DEADLINE`, and a test that used them would spend five
    // minutes proving a hang is caught.
    WorkerConfiguration::for_protocol_fake(
        fake_worker(),
        vec![behavior.to_owned(), tag.to_owned()],
        BTreeMap::new(),
        // A stand-in root: the fake is handed the boundary like the real worker
        // is, and resolves it no more than it resolves a governed root. What
        // the boundary is *for* is proved against the real worker in
        // `t5_e1_worker_output_cannot_escape_staging_root`.
        PathBuf::from("/unused/staging"),
        FAKE_SESSION_DEADLINE,
    )
    .expect("an empty environment names no governed root")
}

/// A plan whose keys were derived from the executor actually under test.
///
/// Derived from the executor's own descriptor rather than from a constant,
/// because that is what the cache's identity gate recomputes and compares: a
/// plan built from anything else would be refused for disagreeing with the
/// worker rather than for the behavior a test is about.
fn worker_plan(executor: &WorkerTtsExecutor) -> (RenderPlan, ValidatedLesson) {
    let lesson = ValidatedLesson::from_json(
        &walking_skeleton_fixture().display().to_string(),
        &fs::read(walking_skeleton_fixture()).expect("the lesson fixture is readable"),
    )
    .expect("the lesson fixture validates");
    // Derived through the fake's own synthetic voice root, because the cache
    // now recomputes the key from what the *worker* reports: a plan built on
    // any other conditioning artifact is refused at publication, which is the
    // gate working rather than a fixture problem.
    let conditioning: BTreeMap<String, study_tts_core::VoiceConditioningHash> = lesson
        .speakers()
        .iter()
        .map(|(speaker, declaration)| {
            (
                speaker.clone(),
                deterministic_tone_conditioning(&declaration.voice_profile),
            )
        })
        .collect();
    let plan = RenderPlan::for_lesson(
        &lesson,
        &executor
            .descriptor()
            .synthesis_context(lesson.language().clone(), conditioning),
    )
    .expect("the worker context resolves every speaker the fixture declares");
    (plan, lesson)
}

/// The request for one planned segment, keyed as the plan keyed it.
fn worker_request(plan: &RenderPlan, lesson: &ValidatedLesson, index: usize) -> SynthesisRequest {
    let segment = &plan.segments[index];
    SynthesisRequest {
        request_id: format!("e1-s3-{}-{}", segment.cache_key, segment.id),
        segment_id: segment.id.clone(),
        spoken_text: segment.spoken_text.clone(),
        voice: segment.speaker.clone(),
        voice_profile: segment.voice_profile.clone(),
        voice_conditioning_hash: deterministic_tone_conditioning(&segment.voice_profile),
        style: segment.style.as_str().to_owned(),
        language: lesson.language().clone(),
        take: segment.take,
        cache_key: segment.cache_key.clone(),
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
    }
}

#[test]
fn t4_e1_the_worker_executor_reports_the_identities_its_worker_initialized_with() {
    // The descriptor is the worker's answer, not this build's claim. A
    // hard-coded identity here would name a bundle that is not the one running,
    // and ADR-0001 §12.5 makes that identity a term of every cache key the
    // audio is stored under — so every entry would describe audio some other
    // worker produced.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("deterministic"))
        .expect("the protocol fake initializes");

    let descriptor = executor.descriptor();

    assert_eq!(
        descriptor.worker_bundle_hash.as_str(),
        DETERMINISTIC_TONE_BUNDLE_HASH,
        "the bundle identity must be the one the worker reported when it initialized"
    );
    assert_eq!(descriptor.model_revision.as_str(), "v1");
    assert_eq!(descriptor.tokenizer_revision.as_str(), "none");
    assert_eq!(
        descriptor.languages,
        BTreeSet::from(["en".parse().expect("`en` is a well-formed tag")]),
        "the declared languages must come from the worker's capabilities"
    );
    assert_eq!(
        descriptor.determinism_class,
        DeterminismClass::Reproducible,
        "the fake declares a deterministic seed, so the descriptor must say so"
    );
    assert_eq!(
        executor.capacity(),
        1,
        "ADR-0001 §10.1 gives one worker process one request at a time"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_synthesis_under_a_drifted_identity_is_refused() {
    // ADR-0001 §12.5 keys every cache entry on the identities the executor read
    // back when it initialized, and the cache recomputes that key from the
    // report's context. A worker that synthesizes under a different identity
    // has been replaced or reloaded underneath the executor, so the key the
    // cache recomputes still describes the initialized worker while the audio
    // came from another one — and the entry publishes, because nothing in the
    // recomputation can see the disagreement. Comparing the bundle alone let
    // the model, the codec, and the voice profile through.
    for identity in DRIFTED_IDENTITIES {
        let behavior = drifting_behavior(identity);
        let executor = WorkerTtsExecutor::start(&fake_worker_configuration(behavior))
            .expect("the protocol fake initializes");
        let (plan, lesson) = worker_plan(&executor);
        let staging = TempDir::new().expect("create a staging root");
        let destination = staging.path().join("audio.wav");

        let report = run_tts_executor_contract_scenario(
            &executor,
            worker_request(&plan, &lesson, 0),
            &destination,
        );

        let error = report.expect_err("audio produced under a drifted identity must be refused");
        let BackendError::IdentityDrift {
            identity: reported, ..
        } = &error
        else {
            panic!("the wrong error was produced for `{behavior}`: {error:?}");
        };
        assert_eq!(
            *reported, identity,
            "the refusal must name the identity that drifted, for `{behavior}`"
        );

        executor.shutdown().expect("the worker shuts down cleanly");
    }
}

#[test]
fn t4_e1_a_request_outside_the_declared_envelope_is_refused_before_any_work() {
    // The worker answers `capabilities` with the envelope it can honor, and
    // until now the executor kept two fields of it and discarded the rest: a
    // style or a voice profile the worker never declared was sent anyway. A
    // backend asked for a voice it does not hold does not fail — it renders
    // something, and that something is published under a key naming the voice
    // nobody loaded.
    //
    // Both cases assert the gate *preceded* the work, not merely that it
    // exists: the fake writes a line per synthesis it is asked for, so an
    // executor that validated after sending would leave one behind.
    for (case, spoil, refused_by) in OUTSIDE_THE_ENVELOPE {
        let executor = WorkerTtsExecutor::start(&fake_worker_configuration("deterministic"))
            .expect("the protocol fake initializes");
        let (plan, lesson) = worker_plan(&executor);
        let staging = TempDir::new().expect("create a staging root");
        let destination = staging.path().join("audio.wav");
        let mut request = worker_request(&plan, &lesson, 0);
        spoil(&mut request);

        let refused = run_tts_executor_contract_scenario(&executor, request, &destination);

        let error = refused.expect_err("a request outside the declared envelope must be refused");
        let BackendError::InvalidRequest { source, .. } = &error else {
            panic!("`{case}` produced the wrong error: {error:?}");
        };
        assert!(
            refused_by(source),
            "`{case}` must be refused by its own invariant: {source:?}"
        );
        assert!(
            !executor.diagnostics().contains(FAKE_SYNTHESIZING_PREFIX),
            "`{case}` must be refused before the worker is asked to synthesize"
        );
        assert!(
            !destination.exists(),
            "`{case}` must leave nothing at the assigned path"
        );

        executor.shutdown().expect("the worker shuts down cleanly");
    }
}

/// One case name, the edit that steps its request outside the envelope, and the
/// invariant that must be the one to refuse it.
type EnvelopeCase = (
    &'static str,
    fn(&mut SynthesisRequest),
    fn(&BackendValidationError) -> bool,
);

/// A request field the worker never declared, and how to spoil it.
///
/// Table-driven because the two cases differ only in which declared list the
/// request steps outside of; the case name is in every assertion message.
///
/// Each case carries its own expected variant rather than sharing one
/// either-or `matches!`. `crates/AGENTS.md` gives each violated invariant its
/// own variant so a test can assert the exact failure, and an assertion that
/// accepts either one asserts neither: it holds for an executor that refuses a
/// spoiled style by naming the voice profile, and it stayed green through
/// exactly that mutation until this table was split.
///
/// The order the two lists are checked in lives in `TtsExecutor::validate`,
/// in `study-tts-runtime/src/worker_executor.rs`, whose comment names this
/// table back.
const OUTSIDE_THE_ENVELOPE: [EnvelopeCase; 2] = [
    (
        "a style the worker did not declare",
        |request| request.style = "breathless_infomercial".to_owned(),
        |source| matches!(source, BackendValidationError::UndeclaredStyle { .. }),
    ),
    (
        "a voice profile the worker did not declare",
        |request| request.voice_profile = "a-voice-nobody-loaded".to_owned(),
        |source| {
            matches!(
                source,
                BackendValidationError::UndeclaredVoiceProfile { .. }
            )
        },
    ),
];

#[test]
fn t4_e1_a_worker_rendering_a_non_canonical_format_is_refused_at_start() {
    // A property of the worker rather than of a request, so it is checked once
    // when the session opens rather than per segment. ADR-0001 §12.3 fixes the
    // canonical intermediate format; a worker rendering at another rate would
    // have every take refused by the cache after the model had already run,
    // which is the expensive way to learn it.
    let error = WorkerTtsExecutor::start(&fake_worker_configuration("non-canonical-format"))
        .expect_err("a worker that renders another format must not open a session");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::InvalidRequest { source, .. } = source.as_ref() else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert!(
        matches!(source, BackendValidationError::NonCanonicalFormat { .. }),
        "the refusal must name the format the worker declared: {source:?}"
    );
}

#[test]
fn t4_e1_a_repeated_segment_never_reuses_a_worker_request_id() {
    // ADR-0001 §10.3 requires a request identity unique per worker lifetime,
    // and `PlannedSegment::request_id` is deterministic by design — it is a
    // plan input, so a retake and a retried attempt re-ask for one segment
    // under one identity. Both are true at once only if the executor issues the
    // identity that goes on the wire, which is what this reads back.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("deterministic"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    for attempt in 0..2 {
        let destination = staging.path().join(format!("attempt-{attempt}.wav"));
        run_tts_executor_contract_scenario(
            &executor,
            worker_request(&plan, &lesson, 0),
            &destination,
        )
        .expect("one segment may be asked for more than once in a worker's lifetime");
    }

    // Shut down *before* reading, because the observation is a race otherwise:
    // the fake writes this line immediately before the response frame, and the
    // executor returns as soon as it has parsed that frame — the reader thread
    // draining standard error need not have run yet. `shutdown` joins the
    // readers, so every byte the worker wrote is in the buffer by the time it
    // returns. Read first, this test passed alone and failed under a loaded
    // suite, which is the shape of a defect rather than of an environment.
    executor.shutdown().expect("the worker shuts down cleanly");

    let diagnostics = executor.diagnostics();
    let observed: BTreeSet<&str> = diagnostics
        .lines()
        .filter_map(|line| line.strip_prefix(FAKE_SYNTHESIZING_PREFIX))
        .collect();
    assert_eq!(
        observed.len(),
        2,
        "the worker must have seen two distinct request identities, not one twice: {observed:?}"
    );
    let caller_request_id = worker_request(&plan, &lesson, 0).request_id;
    assert!(
        observed
            .iter()
            .all(|seen| seen.ends_with(&caller_request_id)),
        "each wire identity must still carry the caller's, so a worker log \
         correlates to a segment: {observed:?}"
    );
}

/// What the fake writes to stderr before it answers a synthesis request.
///
/// Two-sided with `fake-ndjson-worker.rs`, which writes it: the identity a
/// worker actually received is not otherwise observable from outside, and a
/// counter proved only against itself would agree with any implementation.
const FAKE_SYNTHESIZING_PREFIX: &str = "fake worker synthesizing request ";

/// Every identity a success frame restates, so a new one cannot go untested.
const DRIFTED_IDENTITIES: [DriftedIdentity; 4] = [
    DriftedIdentity::WorkerBundle,
    DriftedIdentity::Model,
    DriftedIdentity::Codec,
    DriftedIdentity::VoiceProfile,
];

/// The fake behavior that drifts `identity` and nothing else.
///
/// An exhaustive match rather than a lookup table: a fifth identity is a
/// compile error here, which is the half of the coverage the array above cannot
/// give on its own.
fn drifting_behavior(identity: DriftedIdentity) -> &'static str {
    match identity {
        DriftedIdentity::WorkerBundle => "drift-bundle",
        DriftedIdentity::Model => "drift-model",
        DriftedIdentity::Codec => "drift-codec",
        DriftedIdentity::VoiceProfile => "drift-voice",
    }
}

#[test]
fn t4_e1_one_worker_session_serves_more_than_one_request() {
    // The property ADR-0001 §10.1 exists for: a persistent child. An executor
    // that respawned per request would pass every other test in this file and
    // reload the model for every segment, which is the cost the whole worker
    // boundary is shaped to avoid. Two segments through one executor is the
    // smallest thing that fails if the session does not survive a request.
    // Started under `stderr`, whose one startup line is written before the
    // frame loop and therefore exactly once per process. Counting it is how
    // this observes process identity without the executor exposing a PID: a
    // respawn would write it twice. The E1-S3 test
    // `t5_e1_model_load_occurs_once_per_worker_lifetime` makes the same
    // observation against the real worker's model load.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("stderr"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    for index in 0..2 {
        let destination = staging.path().join(format!("segment-{index}.wav"));
        // The shared seam scenario, so this executor is exercised through the
        // same entry point the fake executor is: `DELIVERY-PLAN.md` E1-S3 task
        // 7 asks for one suite over both.
        let report = run_tts_executor_contract_scenario(
            &executor,
            worker_request(&plan, &lesson, index),
            &destination,
        )
        .expect("the persistent worker renders every segment of the session");
        assert_eq!(report.sample_rate, CANONICAL_SAMPLE_RATE);
        assert_eq!(report.channels, CANONICAL_CHANNELS);
        assert!(
            destination.is_file(),
            "the worker must have written the staging destination it was assigned"
        );
    }

    assert_eq!(
        executor
            .diagnostics()
            .matches("fake worker diagnostic")
            .count(),
        1,
        "one process must have served both requests; a second startup line means \
         the executor respawned and would reload a real model per segment"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_worker_failure_frame_becomes_a_typed_execution_error() {
    // The refusal must survive the boundary as the backend's own stable code,
    // not as a sentence this build wrote: `crates/AGENTS.md` requires one
    // distinct variant per violated invariant so a test can assert which.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("deterministic"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    // Started deterministic and asked to fail only now, so initialization is
    // not what failed: `fake-ndjson-worker` refuses every frame under
    // `failure`, including `initialize`.
    let failing = WorkerTtsExecutor::start(&fake_worker_configuration("failure"));
    let error = failing.expect_err("a worker that refuses to initialize must not start");
    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::Execution { code, .. } = source.as_ref() else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert_eq!(
        code,
        WorkerFailureCode::SynthesisFailed.as_str(),
        "the refusal must carry the worker's own code"
    );

    // The deterministic worker still renders, so the failure above is
    // attributable to the injected behavior rather than to the fixture.
    run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &staging.path().join("segment-0.wav"),
    )
    .expect("the deterministic worker is unaffected");

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_worker_frame_this_build_cannot_read_is_refused_as_a_protocol_failure() {
    // A frame the parser cannot read leaves the stream at a position this
    // client cannot describe. Guessing past it would risk correlating the next
    // frame to the wrong request, so it is refused.
    let error = WorkerTtsExecutor::start(&fake_worker_configuration("malformed-frame"))
        .expect_err("a worker that answers with an unreadable frame must not start");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert!(
        matches!(source.as_ref(), BackendError::Protocol { .. }),
        "an unreadable frame must be a protocol refusal: {error:?}"
    );
}

#[test]
fn t4_e1_a_worker_that_exits_without_answering_is_refused() {
    // The worker is gone rather than slow, so this must be a refusal now and
    // not a wait until the deadline: `fake-ndjson-worker exit` leaves
    // immediately, closing the protocol stream.
    let started = Instant::now();
    let error = WorkerTtsExecutor::start(&fake_worker_configuration("exit"))
        .expect_err("a worker that exits without answering must not start");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert!(
        matches!(source.as_ref(), BackendError::Protocol { .. }),
        "a closed stream must be a protocol refusal: {error:?}"
    );
    assert!(
        started.elapsed() < FAKE_SESSION_DEADLINE,
        "a closed stream must be refused when it closes, not waited out"
    );
}

#[test]
fn t4_e1_a_hung_worker_is_refused_at_its_deadline_and_its_tree_is_reaped() {
    // ADR-0001 §10.3 requires a deadline to detect a hang and the parent to
    // terminate the full child process tree. Both halves are asserted: a
    // refusal that left the worker running would keep a model resident and hold
    // the staging directory open for the life of the build.
    const TAG: &str = "e1s3-executor-hang-probe";

    let started = Instant::now();
    let error = WorkerTtsExecutor::start(&fake_worker_configuration_tagged("hang", TAG))
        .expect_err("a worker that never answers must not start");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::Timeout { timeout_ms, .. } = source.as_ref() else {
        panic!("a hang must be a timeout, not {error:?}");
    };
    assert_eq!(
        *timeout_ms,
        u64::try_from(FAKE_SESSION_DEADLINE.as_millis()).expect("the test deadline fits"),
        "the refusal must name the deadline it enforced"
    );
    assert!(
        started.elapsed() < FAKE_SESSION_DEADLINE * 4,
        "the deadline must bound the wait rather than merely be recorded"
    );

    // `start` returned an error, so the client it built was dropped, and
    // dropping it is what kills and reaps the tree. Nothing here can name the
    // PID, so the observable is that the fake's own process is gone from the
    // process table by the time the refusal is in hand.
    let survivors = Command::new("pgrep")
        .args(["-f", TAG])
        .output()
        .expect("pgrep runs on the reference platform");
    assert!(
        String::from_utf8_lossy(&survivors.stdout).trim().is_empty(),
        "a refused worker must not outlive the refusal"
    );
}

#[test]
fn t4_e1_a_synthesis_that_times_out_kills_the_worker_tree() {
    // The sibling above times out during `start`, where `start` returns an
    // error and the client is dropped — so `Drop` reaps the tree and the
    // lifecycle gap is invisible. This times out *after* a successful start,
    // where nothing is dropped: the executor is still alive and still owned by
    // this test when the process table is read. ADR-0001 §10.3 requires the
    // parent to terminate the full tree on a deadline, and a worker left
    // running holds a model resident and the staging directory open for the
    // life of the build.
    const TAG: &str = "e1s3-synthesis-hang-probe";

    let executor =
        WorkerTtsExecutor::start(&fake_worker_configuration_tagged("hang-on-synthesis", TAG))
            .expect("the fake answers initialize and capabilities before it hangs");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    let error = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &staging.path().join("segment-0.wav"),
    )
    .expect_err("a synthesis that never answers must be refused at its deadline");

    let BackendError::Timeout { timeout_ms, .. } = &error else {
        panic!("a hang must be a timeout, not {error:?}");
    };
    assert_eq!(
        *timeout_ms,
        u64::try_from(FAKE_SESSION_DEADLINE.as_millis()).expect("the test deadline fits"),
        "the refusal must name the deadline it enforced"
    );

    // Read while `executor` is still in scope, so a pass cannot be `Drop`
    // doing the work the timeout path is supposed to have done.
    let survivors = Command::new("pgrep")
        .args(["-f", TAG])
        .output()
        .expect("pgrep runs on the reference platform");
    assert!(
        String::from_utf8_lossy(&survivors.stdout).trim().is_empty(),
        "a timed-out worker must not outlive its deadline"
    );

    // And the executor must say so rather than appear usable: the stream was
    // left at a position this build cannot describe, so §10.3 requires a
    // restart before anything else is asked of it.
    let after = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &staging.path().join("segment-1.wav"),
    )
    .expect_err("a timed-out executor must refuse every later request");
    assert!(
        matches!(after, BackendError::Protocol { .. }),
        "the refusal after a timeout must name the protocol position, not {after:?}"
    );
}

/// Kills whatever still carries `tag`, however the test ended.
///
/// `Drop` rather than a trailing statement, because an assertion failure skips
/// the statement — and what this guards is by construction outside every
/// containment this build has, so nothing else will collect it.
struct TaggedSurvivors(&'static str);

impl Drop for TaggedSurvivors {
    fn drop(&mut self) {
        let _ = Command::new("pkill").args(["-9", "-f", self.0]).status();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn t4_e1_a_timeout_whose_tree_escaped_containment_says_so() {
    // The timeout path calls `shutdown`, and `shutdown` takes the child and the
    // ownership state with it: a tree it could not prove gone is a tree nothing
    // downstream can name. Discarding that result — which this path did until
    // the sixth remediation — hands the caller a bare deadline for an event
    // ADR-0001 §10.3 exists to refuse.
    //
    // The escape is the residual `ADR-0001-D008` records, in deliberately its
    // silent shape: `setsid -f` reparents the process to init before the
    // supervisor enumerates, so no group kill and no recorded pidfd reaches it
    // and `wait_for_containment` reports success without having seen it. What
    // remains observable is the worker's standard output, which it still holds
    // after the worker is gone — so that is what the refusal has to carry.
    const TAG: &str = "e1s3-escaped-containment-probe";

    let _survivors = TaggedSurvivors(TAG);
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration_tagged(
        "hang-on-synthesis-escaping-containment",
        TAG,
    ))
    .expect("the fake answers initialize and capabilities before it hangs");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    let error = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &staging.path().join("segment-0.wav"),
    )
    .expect_err("a synthesis that never answers must be refused at its deadline");

    let BackendError::Timeout {
        containment_failure,
        ..
    } = &error
    else {
        panic!("a hang must be a timeout, not {error:?}");
    };
    let detail = containment_failure.as_deref().unwrap_or_else(|| {
        panic!("a timeout that could not prove the tree gone must say so, got {error:?}")
    });
    assert!(
        detail.contains("standard output"),
        "the containment failure must name what outlived the worker, got {detail:?}"
    );
    assert!(
        error.to_string().contains("was not contained"),
        "both halves must reach one message, got {error}"
    );
}

#[test]
fn t4_e1_a_timeout_does_not_report_a_protocol_tail_as_an_escaped_tree() {
    // `shutdown` runs two checks with different subjects, and until this test
    // the timeout path reported both as the same event. An unterminated tail is
    // an ADR-0001 §17.7 fault about bytes on standard output; a worker tree
    // that survived termination is an ADR-0001 §10.3 fault about processes.
    // Naming the first as the second sends an operator looking for a resident
    // model that was never there, and makes the message useless as evidence
    // when a tree really does survive.
    const TAG: &str = "e1s3-timeout-tail-probe";

    let executor = WorkerTtsExecutor::start(&fake_worker_configuration_tagged(
        "hang-on-synthesis-leaving-bytes",
        TAG,
    ))
    .expect("the fake answers initialize and capabilities before it hangs");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");

    let error = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &staging.path().join("segment-0.wav"),
    )
    .expect_err("a synthesis that never answers must be refused at its deadline");

    let BackendError::Timeout {
        containment_failure,
        ..
    } = &error
    else {
        panic!("a hang must be a timeout, not {error:?}");
    };
    assert!(
        containment_failure.is_none(),
        "an unterminated tail is not a tree that survived termination, but the \
         refusal reported {containment_failure:?}"
    );
}

#[test]
fn t4_e1_worker_output_outside_the_assigned_path_is_refused() {
    // ADR-0001 §10.3 confines worker writes to the assigned staging root, and
    // §12.6 refuses a cache entry whose audio is not what the plan asked for.
    // `escape-staging` writes a perfectly valid take to a path two directories
    // up and reports success, which is the shape a traversal takes when the
    // worker is the hostile party rather than the lesson.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("escape-staging"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");
    let assigned = staging.path().join("nested").join("take");
    fs::create_dir_all(&assigned).expect("the assigned directory is creatable");
    let destination = assigned.join("audio.wav");

    let report = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &destination,
    );

    // The escape has to have actually happened, or this test passes for a
    // worker that did nothing at all — which is what asserting only the
    // assigned path's absence would accept. Asserted first, so a later
    // assertion cannot be read as covering it.
    // Two directories up from the assigned file, which is where
    // `Behavior::EscapeStaging` puts it.
    let escaped = staging.path().join("nested").join("escaped-take.wav");
    assert!(
        escaped.is_file(),
        "the fixture must have written outside the assigned path, or this test \
         proves nothing about traversal"
    );

    // The worker claimed success, so the refusal cannot come from the frame.
    // It comes from the file the executor was promised not being there, which
    // is the only claim in a success frame that this boundary can check.
    //
    // This is detection, not prevention, and the distinction is the point: no
    // check the parent can run stops a worker writing where its uid may write.
    // The parent's half is refusing to *report success* for audio that is not
    // where it asked for it, so nothing keyed on that request is ever
    // published. `t5_e1_worker_output_cannot_escape_staging_root` is the other
    // half — the real worker, asked to write outside its root, refusing.
    let error = report.expect_err("audio written outside the assigned path must not be accepted");
    let BackendError::Destination {
        destination: named, ..
    } = &error
    else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert_eq!(
        named, &destination,
        "the refusal must name the path the worker was assigned"
    );
    assert!(
        !destination.exists(),
        "nothing may appear at the assigned path when the worker wrote elsewhere"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_symlink_at_the_assigned_path_is_refused_rather_than_followed() {
    // The other half of §10.3 containment, and the one an inventory cannot
    // catch: the worker writes the path it was assigned, faithfully, and a link
    // planted there sends those bytes somewhere else entirely. The audio then
    // exists, the worker reported success honestly, and the only thing that
    // separates this from a good take is that the assigned path is a link.
    //
    // `symlink_metadata` is what refuses it. Following the link and finding a
    // valid WAV would publish audio from outside the staging transaction into a
    // cache entry, and the cache re-resolves through `managed::leaf` afterwards
    // — but by then the take has been reported as good.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("deterministic"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let staging = TempDir::new().expect("create a staging root");
    let outside = staging.path().join("outside.wav");
    let destination = staging.path().join("audio.wav");
    std::os::unix::fs::symlink(&outside, &destination).expect("the link is creatable");

    let report = run_tts_executor_contract_scenario(
        &executor,
        worker_request(&plan, &lesson, 0),
        &destination,
    );

    // The link was followed by the worker — asserted, so this test cannot pass
    // against a fake that simply failed to write.
    assert!(
        outside.is_file(),
        "the fixture must have written through the link, or this proves nothing"
    );
    let error = report.expect_err("a link at the assigned path must not be accepted");
    let BackendError::Destination {
        destination: named,
        message,
        ..
    } = &error
    else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert_eq!(named, &destination);
    assert!(
        message.contains("not a regular file"),
        "the refusal must say the assigned path is not a regular file: {message}"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_the_launcher_thread_allowance_reaches_the_worker_process() {
    // ADR-0001 §10.1 caps PyTorch and every native numerical pool at the same
    // per-worker value, and the launching parent is what sets them: each is
    // read as a native library loads, so a worker cannot usefully set them for
    // itself. A cap the parent computed and never passed on is a cap that
    // exists only in its intention, which is what this reads back out of the
    // child's own environment.
    let launcher =
        WorkerLauncher::read(&repository_root()).expect("the checked-in launcher is readable");
    // Built through `child_environment`, so the caps under test are the ones a
    // real launch would carry, then stripped of the two governed-root variables
    // it publishes: `for_protocol_fake` refuses those by name, because a
    // stand-in root and a real one are the same string to it and admitting
    // either would leave the fake a way to start the real worker over a
    // governed root nothing had gated. Nothing here reads them; the exact size
    // of the declared set is pinned by
    // `t1_e1_the_thread_allowance_reaches_every_native_pool`.
    let mut environment =
        launcher.child_environment(Path::new("/unused/models"), Path::new("/unused/voices"));
    environment.remove(&launcher.model_root_environment_variable);
    environment.remove(&launcher.voice_root_environment_variable);
    let configuration = WorkerConfiguration::for_protocol_fake(
        fake_worker(),
        vec!["deterministic".to_owned(), "untagged".to_owned()],
        environment,
        PathBuf::from("/unused/staging"),
        FAKE_SESSION_DEADLINE,
    )
    .expect("the stripped environment names no governed root");

    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");
    let diagnostics = executor.diagnostics();

    for name in THREAD_ENVIRONMENT {
        assert!(
            diagnostics.contains(&format!("{name}={}", launcher.threads)),
            "`{name}` must reach the worker at the launcher's allowance; got: {diagnostics}"
        );
    }

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_worker_frame_past_the_protocol_ceiling_is_refused_before_it_is_kept() {
    // ADR-0001 §10.3 caps the message length, and the cap has to bind while the
    // frame is being read rather than after: a worker that never sends a
    // newline would otherwise be handed memory until this process dies, which
    // is the denial of service the ceiling exists to stop. The observable is
    // the refusal; that it arrives at all is what proves the reader stopped.
    let error = WorkerTtsExecutor::start(&fake_worker_configuration("oversized-frame"))
        .expect_err("a frame past the protocol ceiling must not be read");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::Protocol { message, .. } = source.as_ref() else {
        panic!("an oversized frame must be a protocol refusal: {error:?}");
    };
    assert!(
        message.contains("larger than the protocol ceiling"),
        "the refusal must name the invariant it enforced: {message}"
    );
}

#[test]
fn t4_e1_a_worker_answering_another_request_is_refused_rather_than_believed() {
    // ADR-0001 §10.3 puts a request ID on every frame so the supervisor can
    // correlate. An uncorrelated answer is not a late answer to ignore: it
    // means the two ends disagree about which request is in flight, and
    // believing it would eventually publish one segment's audio under another
    // segment's key.
    let error = WorkerTtsExecutor::start(&fake_worker_configuration("foreign-request-id"))
        .expect_err("an answer to a request nobody made must not be believed");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::Protocol { message, .. } = source.as_ref() else {
        panic!("an uncorrelated answer must be a protocol refusal: {error:?}");
    };
    assert!(
        message.contains("answered a different request"),
        "the refusal must name the invariant it enforced: {message}"
    );
}

// ---------------------------------------------------------------------------
// E1-S3 step 3: validated publication and quarantine, through a real worker
// session. Every name below is copied character for character from
// `DELIVERY-PLAN.md` §E1-S3 and must not be renamed.
// ---------------------------------------------------------------------------

/// Resolves one planned segment through the filesystem cache and a live worker.
///
/// The producer is the executor itself, through the same seam scenario the fake
/// executor is driven by, so the cache cannot tell the two apart — which is
/// what `DELIVERY-PLAN.md` §E1-S3 task 7 asks of the shared suite.
fn resolve_through_worker(
    executor: &WorkerTtsExecutor,
    workspace: &Path,
    job_id: &str,
    plan: &RenderPlan,
    lesson: &ValidatedLesson,
    index: usize,
) -> Result<study_tts_runtime::ValidatedCachedArtifact, BuildError> {
    let mut reached = 0_usize;
    resolve_counting_synthesis(
        executor,
        workspace,
        job_id,
        plan,
        lesson,
        index,
        &mut reached,
    )
}

/// The same resolve, reporting how many times the cache reached the worker.
///
/// A hit is not observable from the artifact — a correct hit and a re-render of
/// identical bytes return the same value — so the only honest evidence that an
/// entry was *reused* is that the producer was never called. `reached` is that
/// evidence, and the E1-S3 hit test rests on it rather than on the artifact.
fn resolve_counting_synthesis(
    executor: &WorkerTtsExecutor,
    workspace: &Path,
    job_id: &str,
    plan: &RenderPlan,
    lesson: &ValidatedLesson,
    index: usize,
    reached: &mut usize,
) -> Result<study_tts_runtime::ValidatedCachedArtifact, BuildError> {
    let request = worker_request(plan, lesson, index);
    let mut producer = |destination: &Path| {
        *reached += 1;
        run_tts_executor_contract_scenario(executor, request.clone(), destination)
    };
    FileSystemCachePublisher.resolve(
        &CacheResolveRequest {
            workspace: workspace.to_path_buf(),
            job_id: job_id.to_owned(),
            segment: plan.segments[index].clone(),
        },
        &mut producer,
    )
}

/// Every quarantined attempt directory beneath one job, in discovery order.
fn quarantined_attempts(workspace: &Path, job_id: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![workspace.join("quarantine").join(job_id)];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("attempt-"))
            {
                found.push(path);
            } else {
                pending.push(path);
            }
        }
    }
    found.sort();
    found
}

#[test]
fn t4_e1_identical_synthesis_identity_produces_cache_hit() {
    // The property the whole cache exists for. Resolving the same planned
    // segment twice must reach the worker once: the second resolve is answered
    // from the published entry, validated rather than trusted.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("stderr"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let workspace = TempDir::new().expect("create a workspace");

    let mut reached = 0_usize;
    let first = resolve_counting_synthesis(
        &executor,
        workspace.path(),
        "hit-job",
        &plan,
        &lesson,
        0,
        &mut reached,
    )
    .expect("the first resolve publishes a cache entry");
    let second = resolve_counting_synthesis(
        &executor,
        workspace.path(),
        "hit-job",
        &plan,
        &lesson,
        0,
        &mut reached,
    )
    .expect("the second resolve reuses it");

    // The claim the test is named for. Equal artifacts alone would also be true
    // of a cache that re-rendered identical bytes every time and never reused
    // anything, so the count is what separates a hit from a coincidence.
    assert_eq!(
        reached, 1,
        "the second resolve must be answered from the published entry, not the worker"
    );
    assert_eq!(
        first.cache_key(),
        second.cache_key(),
        "one synthesis identity names one entry"
    );
    assert_eq!(
        first.audio_blake3(),
        second.audio_blake3(),
        "a hit must return the published bytes, not a re-render of them"
    );
    assert_eq!(first.entry_dir(), second.entry_dir());
    assert!(
        quarantined_attempts(workspace.path(), "hit-job").is_empty(),
        "a clean hit quarantines nothing"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_speech_affecting_change_produces_cache_miss() {
    // The other half: a change to an ADR-0001 §12.5 input must not be answered
    // from the entry the old inputs published. The two fixture segments differ
    // in `spoken_text` alone, so the key is the only thing that can separate
    // them, and a cache that keyed on anything coarser would return the first
    // segment's audio for the second.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("stderr"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let workspace = TempDir::new().expect("create a workspace");
    assert_ne!(
        plan.segments[0].spoken_text, plan.segments[1].spoken_text,
        "the fixture must differ in a speech-affecting input"
    );

    let first = resolve_through_worker(&executor, workspace.path(), "miss-job", &plan, &lesson, 0)
        .expect("the first segment publishes");
    let second = resolve_through_worker(&executor, workspace.path(), "miss-job", &plan, &lesson, 1)
        .expect("the second segment publishes separately");

    assert_ne!(
        first.cache_key(),
        second.cache_key(),
        "different spoken text must derive a different synthesis identity"
    );
    assert_ne!(
        first.entry_dir(),
        second.entry_dir(),
        "two identities must not share one entry"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_invalid_audio_never_produces_cache_hit() {
    // ADR-0001 §12.6 admits an entry only after its audio validates. A worker
    // that writes four bytes and reports a full take is the case that separates
    // "the worker said so" from "the file is so": accepting it would publish a
    // cache entry that can never be decoded, under a key claiming real speech.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("truncated-audio"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let workspace = TempDir::new().expect("create a workspace");

    let refused = resolve_through_worker(
        &executor,
        workspace.path(),
        "invalid-job",
        &plan,
        &lesson,
        0,
    )
    .expect_err("invalid audio must not publish");
    assert!(
        matches!(refused, BuildError::Audio(_)),
        "the refusal must name the audio rather than the protocol: {refused:?}"
    );

    // The entry must be absent rather than present-and-broken: a second resolve
    // is a miss that reaches the worker again, not a hit on quarantined bytes.
    let again = resolve_through_worker(
        &executor,
        workspace.path(),
        "invalid-job",
        &plan,
        &lesson,
        0,
    )
    .expect_err("a refused entry must never become a hit");
    assert!(matches!(again, BuildError::Audio(_)));

    let published = workspace.path().join("cache").join("segments");
    let entries = fs::read_dir(&published)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    assert_eq!(
        entries, 1,
        "the shard directory may exist, but no entry may be published inside it"
    );
    assert!(
        !quarantined_attempts(workspace.path(), "invalid-job").is_empty(),
        "the invalid attempt must be preserved for a person to read"
    );

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_file_the_worker_left_in_the_stage_is_never_published() {
    // The staging directory *becomes* the published entry: §12.6's transaction
    // renames the whole stage into place. So a worker that drops anything
    // beside the audio it was asked for has that file published inside the
    // cache entry, under a key that describes speech and nothing else. The
    // assigned path being correct is not enough — what the stage contains is
    // what gets published, and that is the thing to check.
    // Both kinds of leftover, because the check reads directory entries and a
    // directory is the one an `is_file` filter would have walked straight past.
    for behavior in ["litter-staging", "litter-staging-directory"] {
        let executor = WorkerTtsExecutor::start(&fake_worker_configuration(behavior))
            .expect("the protocol fake initializes");
        let (plan, lesson) = worker_plan(&executor);
        let workspace = TempDir::new().expect("create a workspace");

        let refused =
            resolve_through_worker(&executor, workspace.path(), "litter-job", &plan, &lesson, 0)
                .expect_err("a stage holding more than the assigned audio must not publish");

        assert!(
            matches!(refused, BuildError::Cache(_)),
            "`{behavior}` must be refused by the cache transaction it stopped: {refused:?}"
        );
        let published = workspace.path().join("cache").join("segments");
        for shard in fs::read_dir(&published)
            .expect("the shard directory is readable")
            .flatten()
        {
            for entry in fs::read_dir(shard.path())
                .expect("a shard is readable")
                .flatten()
            {
                let name = entry.file_name();
                assert!(
                    name.to_string_lossy().starts_with(".cache-stage-"),
                    "`{behavior}` published `{}` from a littered stage",
                    name.to_string_lossy()
                );
            }
        }
        assert!(
            !quarantined_attempts(workspace.path(), "litter-job").is_empty(),
            "`{behavior}` must preserve the refused attempt for a person to read"
        );

        executor.shutdown().expect("the worker shuts down cleanly");
    }
}

#[test]
fn t4_e1_invalid_audio_uses_unique_quarantine_path() {
    // ADR-0001 §12.6 requires a collision-free quarantine directory and forbids
    // overwriting or deleting one. Two failures of the *same* segment and take
    // are the case that tests it: the attempt number and the request identity
    // are both derived from the plan, so they repeat exactly, and only the
    // nonce keeps the second failure from landing on the first one's evidence.
    let executor = WorkerTtsExecutor::start(&fake_worker_configuration("truncated-audio"))
        .expect("the protocol fake initializes");
    let (plan, lesson) = worker_plan(&executor);
    let workspace = TempDir::new().expect("create a workspace");

    for _ in 0..2 {
        resolve_through_worker(&executor, workspace.path(), "unique-job", &plan, &lesson, 0)
            .expect_err("invalid audio must not publish");
    }

    let attempts = quarantined_attempts(workspace.path(), "unique-job");
    assert_eq!(
        attempts.len(),
        2,
        "each failure keeps its own evidence: {attempts:?}"
    );
    assert_ne!(
        attempts[0], attempts[1],
        "two attempts must not share a path"
    );

    // The layout ADR-0001 §12.6 names, read back off disk rather than
    // restated: job, segment, take, then the attempt and the request that
    // produced it.
    let segment = &plan.segments[0];
    let expected_parent = workspace
        .path()
        .join("quarantine")
        .join("unique-job")
        .join(&segment.id)
        .join(format!("take-{}", segment.take));
    for attempt in &attempts {
        assert_eq!(
            attempt.parent(),
            Some(expected_parent.as_path()),
            "a quarantined attempt must sit under its job, segment, and take"
        );
        let name = attempt
            .file_name()
            .and_then(|name| name.to_str())
            .expect("an attempt directory is named");
        assert!(
            name.starts_with(&format!("attempt-1-{}-", segment.request_id())),
            "the attempt must name the request that produced it: {name}"
        );
        assert!(
            attempt.join("cache-entry").is_dir(),
            "the failed transaction itself must be preserved, not just its directory"
        );
    }

    executor.shutdown().expect("the worker shuts down cleanly");
}

#[test]
fn t4_e1_a_worker_starts_with_only_the_environment_it_was_declared() {
    // ADR-0001 §10.3 and the offline-operation claim both rest on the worker
    // running in an environment this build chose. Overlaying the declared
    // variables onto an inherited one leaves `PYTHONPATH`, `PYTHONHOME`,
    // `sitecustomize`, user-site configuration, and proxy settings able to
    // change what the child imports and reaches *before* it applies its own
    // offline settings, so the parent clears the environment rather than adding
    // to it.
    //
    // The ambient environment is this process's own rather than one planted
    // here: `unsafe_code = "forbid"` puts `set_var` out of reach, and
    // `rust-testing` refuses a test that mutates a global anyway. The guard
    // below is what keeps that from making the assertion vacuous.
    let inherited: BTreeSet<String> = std::env::vars().map(|(name, _)| name).collect();
    assert!(
        inherited.len() > 1,
        "this test proves nothing unless the parent holds an ambient environment to leak"
    );

    let declared = BTreeMap::from([("STUDY_TTS_DECLARED".to_owned(), "yes".to_owned())]);
    let configuration = WorkerConfiguration::for_protocol_fake(
        fake_worker(),
        vec!["deterministic".to_owned(), "environment".to_owned()],
        declared.clone(),
        PathBuf::from("/unused/staging"),
        FAKE_SESSION_DEADLINE,
    )
    .expect("the declared environment names no governed root");
    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");

    let diagnostics = executor.diagnostics();
    // Names rather than values, because a governed root reaches the child as a
    // value and `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a
    // governed location out of a log.
    let reported = diagnostics
        .lines()
        .find_map(|line| line.strip_prefix("fake worker environment names: "))
        .expect("the fake reports the environment names it started with");
    let names: BTreeSet<&str> = reported
        .split(',')
        .filter(|name| !name.is_empty())
        .collect();

    assert_eq!(
        names,
        declared.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        "the child must hold exactly the declared environment, got {names:?}"
    );
}

#[test]
fn t4_e1_a_worker_is_asked_to_leave_before_it_is_killed() {
    // The protocol has had a `shutdown` frame since E1-S1 and nothing sent it:
    // shutting down closed standard input and went straight to terminating the
    // process group, which on Unix is `SIGKILL`. A worker killed where it
    // stands has no chance to release what it holds, and ADR-0001 §17.7 asks
    // for graceful shutdown rather than only for containment. The kill stays as
    // the backstop for a worker that will not leave — proved separately by
    // `t4_e1_a_hung_worker_is_refused_and_its_process_tree_reaped`.
    let configuration = fake_worker_configuration_tagged("deterministic", "graceful-shutdown");
    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");

    executor.shutdown().expect("the worker shuts down cleanly");

    assert!(
        executor
            .diagnostics()
            .contains("fake worker leaving on a shutdown frame"),
        "the worker must be asked to leave before it is killed, got {:?}",
        executor.diagnostics()
    );
}

#[test]
fn t4_e1_a_worker_survives_being_shut_down_and_started_again() {
    // ADR-0001 §17.7 asks a worker to be restartable, and both suites started
    // one worker, rendered once, and dropped it — which cannot tell a
    // restartable worker from one that only ever ran once. The second lifetime
    // is the whole test: a worker that left a lock, a staged file, or a
    // resident model behind fails there and nowhere else.
    //
    // The same scenario drives the real worker in the T5 qualification
    // instrument, so the two ends are exercised by one function rather than by
    // two suites that each wrote their own idea of a restart.
    let staging = TempDir::new().expect("create a restart staging directory");
    let configuration = fake_worker_configuration_tagged("deterministic", "restart");
    // The plan is derived from a worker of the same configuration, because the
    // cache keys it carries are recomputed from what the worker reports.
    let (plan, lesson) = {
        let executor =
            WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");
        let derived = worker_plan(&executor);
        executor.shutdown().expect("the probe worker shuts down");
        derived
    };

    let [first, second] = run_worker_restart_contract_scenario(
        &configuration,
        &worker_request(&plan, &lesson, 0),
        &staging.path().join("first.wav"),
        &staging.path().join("second.wav"),
    )
    .expect("a worker must survive being restarted");

    // The second lifetime is a *new* process that loaded the same bundle, so
    // every identity a cache key is built from must be the one the first
    // reported. A restart that changed any of them would file the same audio
    // under two keys.
    assert_eq!(
        first.report.context, second.report.context,
        "a restarted worker must report the identities the first one did"
    );
    assert!(
        first
            .diagnostics
            .contains("fake worker leaving on a shutdown frame"),
        "the first lifetime must have been asked to leave, not killed"
    );
    for outcome in [&first, &second] {
        assert!(
            outcome
                .diagnostics
                .contains("fake worker environment names: \n")
                || outcome
                    .diagnostics
                    .contains("fake worker environment names: STUDY"),
            "each lifetime starts from a declared environment, got {:?}",
            outcome.diagnostics
        );
    }
}

// `/proc` is where both this test and `ProcessOwnership` prove ancestry, and
// descendant tracking is `#[cfg(target_os = "linux")]` on the production side
// too: elsewhere the process group is the whole containment story.
#[cfg(target_os = "linux")]
#[test]
fn t4_e1_a_gracefully_shut_down_worker_leaves_no_descendant_behind() {
    // `WorkerClient::shutdown` promises in its own words to prove the process
    // tree is gone, and the graceful path did not: a worker that took the
    // invitation had its recorded ownership discarded and its children never
    // enumerated, signalled, or observed. ADR-0001 §10.3 makes the parent
    // responsible for the whole tree, and a backend helper that outlives the
    // build holds the staging directory open and a model resident.
    //
    // The kill path already proves this for a worker that will not leave, in
    // `t4_e1_a_hung_worker_is_refused_at_its_deadline_and_its_tree_is_reaped`.
    // The graceful path is where a supervisor is most tempted to believe the
    // tree left with the child it was watching.
    let configuration = fake_worker_configuration_tagged("spawn-descendant", "descendant");
    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");

    executor.shutdown().expect("the worker shuts down cleanly");

    // Read after `shutdown`, which joins the reader threads: the fake writes
    // this line at startup, but only a joined reader has certainly drained it.
    let descendant = descendant_pid(&executor.diagnostics());
    assert!(
        !parked_descendant_is_live(descendant),
        "shutdown must contain the worker's descendants, but {descendant} survived it"
    );
}

// The window the test above cannot reach: a descendant that did not exist when
// the supervisor enumerated the tree. Linux-gated for the same reason.
#[cfg(target_os = "linux")]
#[test]
fn t4_e1_a_descendant_started_during_shutdown_is_contained() {
    // Descendants are enumerated once, before the worker is asked to leave,
    // because `/proc/<pid>/task/*/children` is gone the moment it exits. A
    // worker that starts a helper *after* that look and then leaves gracefully
    // was reached by nothing: it held no recorded pidfd, and the group was not
    // signalled because the direct child had already been reaped.
    //
    // ADR-0001 §10.3 makes this build responsible for the whole tree, not for
    // the part of it that existed when it last looked.
    let configuration =
        fake_worker_configuration_tagged("spawn-descendant-at-shutdown", "late-descendant");
    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");

    executor.shutdown().expect("the worker shuts down cleanly");

    let descendant = descendant_pid(&executor.diagnostics());
    assert!(
        !parked_descendant_is_live(descendant),
        "shutdown must contain a descendant started after enumeration, but {descendant} survived it"
    );
}

#[test]
fn t4_e1_trailing_bytes_past_the_last_frame_are_refused() {
    // The protocol reader forwarded completed lines and dropped whatever was
    // left unterminated at end of stream, and nothing read the channel after
    // the last request — so a worker could write anything it liked past its
    // final frame and the session still looked clean. ADR-0001 §17.7 requires
    // standard output to carry protocol messages and nothing else, and a check
    // that cannot see the last bytes on the stream is not that requirement.
    let configuration = fake_worker_configuration_tagged("trailing-bytes", "trailing");
    let executor = WorkerTtsExecutor::start(&configuration).expect("the protocol fake initializes");

    let error = executor
        .shutdown()
        .expect_err("bytes past the last frame must be refused, not discarded");

    let BackendError::Protocol {
        request_id,
        message,
    } = &error
    else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert_eq!(request_id, "shutdown", "the refusal names the exchange");
    assert!(
        message.contains("34 byte"),
        "the refusal must name what was left on the stream: {message}"
    );
}

#[test]
fn t4_e1_a_worker_echoing_another_bundle_identity_is_refused_at_start() {
    // The supervisor sends the bundle identity it verified for itself and the
    // worker answers with one of its own, which reaches `BackendDescriptor` and
    // therefore every cache key ADR-0001 §12.5 builds. The two were never
    // compared, so a worker could file audio under an identity the supervisor
    // had not proven — the whole point of verifying the bundle first.
    //
    // Refused at `start`, so no executor exists and nothing downstream of it
    // can have run.
    let configuration =
        fake_worker_configuration_tagged("drift-bundle-at-initialize", "echo-drift");

    let error = WorkerTtsExecutor::start(&configuration)
        .expect_err("a worker answering under another bundle identity must not open a session");

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::InvalidRequest { source, .. } = source.as_ref() else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert!(
        matches!(
            source,
            BackendValidationError::BundleIdentityNotEchoed { .. }
        ),
        "the refusal must name the identity the worker answered with: {source:?}"
    );
}

/// The PID the `spawn-descendant` fake announced for the child it started.
#[cfg(target_os = "linux")]
fn descendant_pid(diagnostics: &str) -> u32 {
    const PREFIX: &str = "fake worker descendant pid: ";

    diagnostics
        .lines()
        .find_map(|line| line.strip_prefix(PREFIX))
        .unwrap_or_else(|| panic!("the fake must announce its descendant, got {diagnostics:?}"))
        .trim()
        .parse()
        .expect("the announced descendant PID is a number")
}

/// Whether `pid` is still the parked descendant, rather than gone or recycled.
///
/// The command line is read rather than only the directory, so a PID reused
/// between `shutdown` returning and this question cannot be counted as a
/// descendant that survived. A reaped process has no `/proc` entry and a zombie
/// has an empty command line; neither holds a file or a model, which is what
/// containment is about.
#[cfg(target_os = "linux")]
fn parked_descendant_is_live(pid: u32) -> bool {
    fs::read(format!("/proc/{pid}/cmdline")).is_ok_and(|cmdline| {
        cmdline
            .split(|byte| *byte == 0)
            .any(|argument| argument == b"descendant-park")
    })
}

#[test]
fn t4_e1_a_worker_reporting_another_model_revision_is_refused_at_start() {
    // `verify_model_artifacts` hashes the declared artifacts of one revision
    // before a worker is started, and the worker decides which weights to load
    // from the revision it reads out of the governed acquisition record. A
    // worker answering with another has loaded bytes this build never hashed,
    // under the revision ADR-0001 §12.5 keys the audio on — which is what made
    // the verification worth doing rather than a check that guards a directory
    // nobody promised to read.
    //
    // The companion to the bundle-identity echo case above, and the second
    // half of the 2026-08-31 audit's sixth finding.
    let configuration =
        fake_worker_configuration_tagged("drift-model-at-initialize", "model-echo-drift");

    let error = WorkerTtsExecutor::start(&configuration).expect_err(
        "a worker answering under an unverified model revision must not open a session",
    );

    let BuildError::Synthesis(source) = &error else {
        panic!("the wrong error was produced: {error:?}");
    };
    let BackendError::InvalidRequest { source, .. } = source.as_ref() else {
        panic!("the wrong error was produced: {error:?}");
    };
    assert!(
        matches!(
            source,
            BackendValidationError::ModelRevisionNotEchoed { .. }
        ),
        "the refusal must name the revision the worker answered with: {source:?}"
    );
}
