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
