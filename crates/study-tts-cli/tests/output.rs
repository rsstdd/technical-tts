//! Tier 3 tests for E2-S5's structured output: the envelope every command
//! emits under `--json`, pinned against a committed golden.
//!
//! In this crate for the reason `authoring.rs` records —
//! `CARGO_BIN_EXE_study-tts` is set only for the package that declares the
//! binary.

use std::process::{Command, Output};

use tempfile::TempDir;

fn study_tts(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// Runs the binary from inside `directory`, so a path an assertion compares is
/// the relative one the operator typed rather than a temporary directory that
/// differs on every run. The refusal still names the document, which is the
/// tree's settled position: the path is the operator's own and the content
/// never is. `t4_e1_a_refusal_never_quotes_the_lesson_text_it_read` holds the
/// other half.
fn study_tts_in(directory: &std::path::Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// The envelope is the committed shape, in both directions.
///
/// A golden a reviewer reads rather than a generated schema compared against
/// itself. `E2-S5`'s accepted plan review settles why there is no published
/// schema here: `--json` is a serialization boundary this project never reads
/// back, and every entry in `PUBLISHED_SCHEMAS` is a durable document or the
/// worker wire format. Stability is still owed — ADR-0001 §7.3 requires
/// `--json` on every command — and this is what owes it.
///
/// The wrong implementation this rejects: an envelope that grows a field and
/// breaks a caller silently. A `contains` assertion would pass for that; a
/// whole-document comparison does not.
#[test]
fn t3_e2_cli_json_output_matches_its_committed_golden() {
    let workspace = TempDir::new().expect("create a workspace");
    std::fs::write(workspace.path().join("malformed.json"), b"{")
        .expect("write a malformed lesson");

    let refused = study_tts_in(
        workspace.path(),
        &["--json", "lesson", "validate", "malformed.json"],
    );

    let emitted = String::from_utf8(refused.stdout.clone()).expect("stdout is UTF-8");
    let actual: serde_json::Value = serde_json::from_str(&emitted)
        .unwrap_or_else(|error| panic!("`--json` emits JSON: {error}; got {emitted}"));

    let golden_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/contracts/e2-s5-cli-output-refusal.json"
    );
    let golden_bytes = std::fs::read(golden_path).expect("read the committed golden");
    let golden: serde_json::Value =
        serde_json::from_slice(&golden_bytes).expect("the golden is JSON");

    assert_eq!(
        actual, golden,
        "the emitted envelope is the committed one; regenerate the golden only when the \
         envelope is meant to move"
    );
}

/// The envelope carries no lesson content, and the refusal path is where it
/// would leak first.
///
/// `RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access fixes the whole
/// vocabulary a diagnostic may carry: "hashes, IDs, timings, states, and error
/// classes". A refusal that quoted the value it refused would be the most
/// natural thing to write and is the thing this forbids.
#[test]
fn t3_e2_cli_json_output_carries_no_lesson_content() {
    const SECRET: &str = "the-spoken-text-nobody-may-log";

    let workspace = TempDir::new().expect("create a workspace");
    let lesson = workspace.path().join("lesson.json");
    std::fs::write(
        &lesson,
        format!(r#"{{"schema_version":"3.1","spoken_text":"{SECRET}"}}"#),
    )
    .expect("write a lesson carrying content");

    let refused = study_tts(&[
        "--json",
        "lesson",
        "validate",
        &lesson.display().to_string(),
    ]);

    let emitted = String::from_utf8(refused.stdout.clone()).expect("stdout is UTF-8");
    let said = String::from_utf8(refused.stderr.clone()).expect("stderr is UTF-8");

    assert!(
        !emitted.contains(SECRET),
        "the envelope must not carry lesson content: {emitted}"
    );
    assert!(
        !said.contains(SECRET),
        "neither may the human-readable refusal: {said}"
    );
}

/// Every command emits the envelope, and no command is exempt.
///
/// ADR-0001 §7.3: "Every command supports human-readable output and `--json`."
/// A story that adds the flag command by command satisfies it until the
/// eleventh command forgets, so this enumerates the surface rather than
/// sampling it.
///
/// `render` and `resume` are driven to a refusal that happens **before** any
/// worker starts — a bundle root with no `bundle-manifest.json` — so a T4 test
/// covers them without a model, which `docs/testing/TEST-STRATEGY.md` forbids
/// it from downloading.
///
/// The wrong implementation this rejects is the one this surface grew into:
/// four commands added in one pass with no test of their own, because the
/// envelope was proved on a single refusal and assumed for the rest.
#[test]
fn t4_e2_every_mvp_command_has_stable_structured_output() {
    let workspace = TempDir::new().expect("create a workspace");
    let root = workspace.path();
    let absent = root.join("absent");
    let absent = absent.display().to_string();
    let here = root.display().to_string();
    let digest = "a".repeat(64);

    let invocations: [(&str, Vec<&str>); 12] = [
        (
            "lesson new",
            vec!["lesson", "new", "e2-s5-all", "--out", "new.json"],
        ),
        ("lesson validate", vec!["lesson", "validate", "new.json"]),
        ("publish", vec!["publish", "e2-s5-all"]),
        ("cache prune", vec!["cache", "prune", "--workspace", &here]),
        (
            "cache verify",
            vec!["cache", "verify", "--workspace", &here],
        ),
        ("report", vec!["report", "--workspace", &here, "e2-s5-all"]),
        ("doctor", vec!["doctor", "--workspace", &here]),
        (
            "takes accept",
            vec![
                "takes",
                "accept",
                "--workspace",
                &here,
                "--lesson-id",
                "e2-s5-all",
                "--out",
                "accepted.takes.json",
            ],
        ),
        (
            "inspect",
            vec!["inspect", "--workspace", &here, "e2-s5-all"],
        ),
        (
            "review",
            vec![
                "review",
                "--workspace",
                &here,
                "--lesson-id",
                "e2-s5-all",
                "--manifest",
                &digest,
                "--reviewer",
                "r",
                "--reviewer-role",
                "owner",
                "--playback-environment",
                "speakers",
                "--disposition",
                "accepted",
            ],
        ),
        (
            "render",
            vec![
                "render",
                "new.json",
                "--workspace",
                &here,
                "--bundle-root",
                &absent,
                "--model-root",
                &absent,
                "--voice-root",
                &absent,
                "--hardware-environment",
                "test-env",
            ],
        ),
        (
            "resume",
            vec![
                "resume",
                "e2-s5-all",
                "--workspace",
                &here,
                "--bundle-root",
                &absent,
                "--model-root",
                &absent,
                "--voice-root",
                &absent,
                "--hardware-environment",
                "test-env",
            ],
        ),
    ];

    // The table must cover the surface, not a subset of it. The first version
    // of this test omitted `resume` — which is the failure it exists to
    // prevent, committed inside the test itself. `--help` is the surface's own
    // account of what exists, so it is what the table is checked against.
    let help = String::from_utf8(study_tts_in(root, &["--help"]).stdout).expect("help is UTF-8");
    let listed: Vec<&str> = help
        .lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| *name != "help")
        .collect();
    assert!(!listed.is_empty(), "`--help` lists the commands: {help}");
    for command in listed {
        assert!(
            invocations
                .iter()
                .any(|(name, _)| *name == command || name.starts_with(&format!("{command} "))),
            "`{command}` is on the surface and absent from this table"
        );
    }

    for (name, arguments) in invocations {
        let mut with_json = vec!["--json"];
        with_json.extend(arguments);
        let output = study_tts_in(root, &with_json);

        let emitted = String::from_utf8(output.stdout).expect("stdout is UTF-8");
        let record: serde_json::Value = serde_json::from_str(&emitted)
            .unwrap_or_else(|error| panic!("`{name}` emitted no envelope: {error}; got {emitted}"));

        assert_eq!(record["command"], name, "`{name}` names itself");
        assert_eq!(
            record["output_version"], "1.0-cli-output",
            "`{name}` carries the envelope version"
        );
        assert!(
            record["outcome"] == "succeeded" || record["outcome"] == "refused",
            "`{name}` reports a closed outcome, got {}",
            record["outcome"]
        );

        // A refusal carries its class and the exit code the process used, so a
        // caller reading JSON and a caller reading `$?` cannot disagree.
        if record["outcome"] == "refused" {
            assert!(
                record["error_class"].is_string(),
                "`{name}` names the boundary that refused"
            );
            assert_eq!(
                record["exit_code"].as_u64(),
                output.status.code().map(|code| code as u64),
                "`{name}` reports the exit code it actually left"
            );
        }
    }
}
