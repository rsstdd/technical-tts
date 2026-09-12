//! Tier 4 tests for E2-S5's exit-code classes: real process, real filesystem.
//!
//! They live in this crate for the reason `authoring.rs` records —
//! `CARGO_BIN_EXE_study-tts` is set only for the package that declares the
//! binary, and a test that located the executable any other way would assert a
//! target-directory layout instead of a command.

use std::process::{Command, Output};

use tempfile::TempDir;

fn study_tts(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// Every refusal leaves an exit code that names the class it belongs to.
///
/// ADR-0001 §7.3 fixes the vocabulary: "Exit-code classes distinguish invalid
/// input, missing dependency, incompatible environment, worker failure,
/// audio-quality failure, cancellation, and internal error." It names the
/// classes and not the numbers, so the numbers are this crate's and the mapping
/// is what must not drift.
///
/// The wrong implementation this rejects is the one the binary shipped until
/// E2-S5: a single `ExitCode::FAILURE` for every refusal, which tells a script
/// that something went wrong and never which kind, so no caller can
/// distinguish a lesson it should fix from a tool it should install.
#[test]
fn t4_e2_each_failure_class_has_declared_exit_code() {
    let workspace = TempDir::new().expect("create a workspace");

    // Invalid input: bytes that are not a lesson.
    let malformed = workspace.path().join("malformed.json");
    std::fs::write(&malformed, b"{").expect("write a malformed lesson");
    let refused = study_tts(&["lesson", "validate", &malformed.display().to_string()]);
    assert_eq!(
        refused.status.code(),
        Some(2),
        "an unreadable lesson exits as invalid input, not as a generic failure"
    );

    // Success keeps the exit code every shell already expects.
    let scaffold = workspace.path().join("scaffold.json");
    let created = study_tts(&[
        "lesson",
        "new",
        "e2-s5-exit-codes",
        "--out",
        &scaffold.display().to_string(),
    ]);
    assert_eq!(
        created.status.code(),
        Some(0),
        "a command that did its work exits zero"
    );

    let accepted = study_tts(&["lesson", "validate", &scaffold.display().to_string()]);
    assert_eq!(
        accepted.status.code(),
        Some(0),
        "a lesson this build would render exits zero"
    );

    // A path that is not there is still invalid input rather than an internal
    // error: the operator gave a name nothing answers to.
    let absent = workspace.path().join("absent.json");
    let missing = study_tts(&["lesson", "validate", &absent.display().to_string()]);
    assert_eq!(
        missing.status.code(),
        Some(2),
        "a lesson path that resolves to nothing is invalid input"
    );
}

/// `publish` refuses, and the refusal names every gate a production release
/// would need.
///
/// `DELIVERY-PLAN.md` §11 fixes what `publish` writes in version 1.0 — nothing
/// — and E2-S5 task 7 fixes how it says so: "Refuse `publish` with a message
/// naming the missing production gates." Both halves are asserted, because
/// each without the other is a different and worse command.
///
/// The wrong implementation this rejects is the refusal the runtime already
/// had: `PrivateProfileCannotClaimProduction` alone, which tells an operator
/// that a preview cannot claim production and leaves them no way to learn what
/// could. Naming the twelve is what turns a refusal into an answer.
#[test]
fn t4_e2_publish_is_refused_with_named_missing_gates() {
    const GATES: [&str; 12] = [
        "long_form_soak",
        "content_integrity_review",
        "asr_triage_recorded",
        "worker_unloaded_before_verification",
        "explicit_take_selection",
        "frozen_loudness_references",
        "voice_identity_and_format",
        "automated_audio_checks",
        "package_provenance",
        "offline_render_verified",
        "rights_and_licensing",
        "clean_machine_operations",
    ];

    let refused = study_tts(&["publish", "e2-s5-publish"]);

    assert_eq!(
        refused.status.code(),
        Some(2),
        "a publication this build refuses is invalid input, not an internal error"
    );

    let said = String::from_utf8(refused.stderr.clone()).expect("stderr is UTF-8");
    for gate in GATES {
        assert!(
            said.contains(gate),
            "the refusal must name `{gate}`, so the operator learns what \
             production requires: {said}"
        );
    }
    assert!(
        said.contains("private"),
        "the refusal must still say a preview cannot claim production: {said}"
    );

    // The first implementation of this printed the refusal from the command
    // and again from the reporter, and every `contains` assertion above passed
    // anyway. An operator reading a refusal twice reasonably wonders whether
    // two things failed.
    assert_eq!(
        said.matches("release claim is").count(),
        1,
        "a refusal is reported exactly once: {said}"
    );
}
