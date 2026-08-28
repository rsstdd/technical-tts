//! T4: the fake worker against the published protocol, and the guarantee that
//! the PR suite never reaches for a model.
//!
//! Both tests named here are E1-S1 contract tests from `DELIVERY-PLAN.md`.
//! They cover the two halves of "a fake is usable": it must speak the contract
//! its real counterpart will speak, and running it must cost nothing the real
//! one costs — no weights, no download, no reference machine.

use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde_json::Value;
use study_tts_core::ValidatedLesson;
use study_tts_runtime::{
    BuildRequest, MAX_WORKER_FRAME_BYTES, MAX_WORKER_REQUEST_ID_BYTES, TtsExecutor,
    WORKER_PROTOCOL_SCHEMA_VERSION, WORKER_PROTOCOL_VERSION, WorkerFailureCode, WorkerRequestFrame,
    WorkerResponseFrame, build_preview, parse_worker_request, parse_worker_response,
};
use study_tts_testkit::{FakeTtsExecutor, validate_against_schema, walking_skeleton_fixture};
use tempfile::TempDir;

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
    let responses: Vec<WorkerResponseFrame> = stdout
        .lines()
        .map(|line| {
            assert!(
                line.len() <= MAX_WORKER_FRAME_BYTES,
                "a response frame must stay within the declared ceiling"
            );
            // Parsed through the *published* parser, not through `serde_json`:
            // that parser is what the supervisor uses, and a fake whose frames
            // only survive a lenient read is a fake that hides a real refusal.
            parse_worker_response(line.as_bytes()).expect("a response frame is valid")
        })
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
    assert!(
        matches!(responses[0], WorkerResponseFrame::Initialized { .. }),
        "initialize must be answered by initialized, got {:?}",
        responses[0]
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
    assert!(
        matches!(responses[3], WorkerResponseFrame::SynthesisSucceeded { .. }),
        "synthesize must be answered by synthesis_succeeded, got {:?}",
        responses[3]
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

    // The reported `worker_bundle_hash` and `voice_profile_hash` need no
    // assertion here: they parse as `WorkerBundleHash` and `VoiceProfileHash`,
    // so a fake reporting a value that is not a digest would have failed the
    // parse above rather than reached this line.
    // `worker_protocol`'s own
    // `t1_e1_a_frame_naming_an_identity_that_is_not_a_digest_is_refused` proves
    // that refusal, and it fails if the fields go back to `String` while this
    // test carries on passing.

    // A refusal is still a valid frame. The fault-injecting behavior is what
    // the supervisor tests drive, so it has to satisfy the same contract.
    let refusal = drive(&staging, &["failure"], &session);
    let refusals: Vec<WorkerResponseFrame> = refusal
        .lines()
        .map(|line| parse_worker_response(line.as_bytes()).expect("a failure frame is valid"))
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

/// Runs the fake worker in `staging` over `session` and returns its stdout.
fn drive(staging: &TempDir, arguments: &[&str], session: &str) -> String {
    let mut worker = Command::new(fake_worker())
        .args(arguments)
        .current_dir(staging.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("the fake worker starts");
    worker
        .stdin
        .take()
        .expect("the fake worker takes stdin")
        .write_all(session.as_bytes())
        .expect("the session is writable to the worker");
    let output = worker.wait_with_output().expect("the fake worker exits");
    assert!(
        output.status.success(),
        "the fake worker must exit cleanly, got {}",
        output.status
    );
    String::from_utf8(output.stdout).expect("worker stdout is UTF-8")
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
        &fs::read(walking_skeleton_fixture()).expect("the lesson fixture is readable"),
    )
    .expect("the lesson fixture validates");
    assert_eq!(
        lesson.segments().len(),
        2,
        "the suite must render a real lesson, not an empty one"
    );

    let executor = FakeTtsExecutor::default();
    build_preview(
        BuildRequest {
            lesson_path: walking_skeleton_fixture(),
            workspace: workspace.path().to_path_buf(),
            ffmpeg_executable: "ffmpeg".into(),
            ffprobe_executable: "ffprobe".into(),
            voice_profile_dir: None,
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
