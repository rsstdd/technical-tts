//! Tier 4 tests for E2-S5 task 4: what the CLI must never print.
//!
//! In this crate for the reason `authoring.rs` records —
//! `CARGO_BIN_EXE_study-tts` is set only for the package that declares the
//! binary.

use std::{
    path::Path,
    process::{Command, Output},
};

use tempfile::TempDir;

/// Content a refusal would quote if anything quoted the document it read.
///
/// Distinctive enough that a substring match cannot pass by accident, and
/// shaped like the three things
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access
/// excludes: spoken text, source text, and a voice-reference path.
const SPOKEN: &str = "zQ-spoken-text-that-must-never-be-logged";
const DISPLAY: &str = "zQ-display-text-that-must-never-be-logged";
const VOICE: &str = "zQ-voice-reference-profile";

fn study_tts_in(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .current_dir(directory)
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// A lesson carrying all three, and one refusable field.
///
/// `pause_after_ms` of 10 after a recall prompt is what E1-S5's own
/// diagnostic test uses to provoke a refusal, so this fixture is rejected for
/// a reason unrelated to the text it carries — which is the point. A lesson
/// refused *because* of its text would not prove the text stays unprinted.
fn lesson_carrying_sensitive_text() -> String {
    format!(
        r#"{{
          "schema_version": "3.1",
          "lesson_id": "e2-s5-redaction",
          "title": "e2-s5-redaction",
          "language": "en",
          "speakers": {{ "instructor": {{ "voice_profile": "{VOICE}" }} }},
          "segments": [
            {{
              "id": "seg-0001",
              "speaker": "instructor",
              "role": "recall_prompt",
              "source_refs": [],
              "display_text": "{DISPLAY}",
              "spoken_text": "{SPOKEN}",
              "style": "calm_explanatory",
              "pause_after_ms": 10,
              "review_status": "approved",
              "editorial": true
            }}
          ]
        }}"#
    )
}

/// Neither stream carries lesson content, on the default path.
///
/// E2-S5 task 4 says "redact source text, spoken text, and voice-reference
/// paths **by default**", and the word that does the work is the last one: a
/// flag that can clean the output is not redaction, because the operator who
/// needed it has already printed the thing.
///
/// Both output modes are checked, because `--json` and prose are built by
/// different code and only one of them was tested before this.
///
/// The wrong implementation this rejects is the most natural refusal anyone
/// writes: one that quotes the value it refused, so the author can see what
/// was wrong. `describe_lesson` deliberately prints the RFC 6901 pointer
/// instead — the pointer is how the author finds the value in their own file,
/// which they may read and the log may not.
#[test]
fn t4_e2_logs_exclude_sensitive_fixture_content() {
    let workspace = TempDir::new().expect("create a workspace");
    let lesson = workspace.path().join("lesson.json");
    std::fs::write(&lesson, lesson_carrying_sensitive_text()).expect("write the lesson");

    // The fixture must actually carry the text, or every assertion below
    // passes for a document that had nothing to leak.
    let written = std::fs::read_to_string(&lesson).expect("read the lesson back");
    for secret in [SPOKEN, DISPLAY, VOICE] {
        assert!(written.contains(secret), "the fixture carries {secret}");
    }

    for arguments in [
        vec!["lesson", "validate", "lesson.json"],
        vec!["--json", "lesson", "validate", "lesson.json"],
    ] {
        let refused = study_tts_in(workspace.path(), &arguments);
        assert_ne!(
            refused.status.code(),
            Some(0),
            "the fixture is refused, so there is a refusal to inspect"
        );

        let printed = format!(
            "{}{}",
            String::from_utf8(refused.stdout).expect("stdout is UTF-8"),
            String::from_utf8(refused.stderr).expect("stderr is UTF-8"),
        );

        for secret in [SPOKEN, DISPLAY, VOICE] {
            assert!(
                !printed.contains(secret),
                "`{arguments:?}` printed {secret}: {printed}"
            );
        }

        // What a refusal *must* carry, so redaction cannot be satisfied by
        // saying nothing useful. The pointer is what an author navigates by.
        assert!(
            printed.contains("/segments/0/pause_after_ms"),
            "`{arguments:?}` must still locate the offending field: {printed}"
        );
    }
}

/// A scaffold names its voice profile and no lesson text, because there is
/// none yet to name.
///
/// The success path, checked because redaction that only covers refusals
/// leaves the more common output unguarded.
#[test]
fn t4_e2_a_successful_command_prints_no_lesson_content() {
    let workspace = TempDir::new().expect("create a workspace");

    let created = study_tts_in(
        workspace.path(),
        &[
            "--json",
            "lesson",
            "new",
            "e2-s5-scaffold",
            "--out",
            "s.json",
        ],
    );
    assert_eq!(created.status.code(), Some(0), "the scaffold is written");

    let scaffold = std::fs::read_to_string(workspace.path().join("s.json"))
        .expect("the scaffold exists to be read");
    let printed = String::from_utf8(created.stdout).expect("stdout is UTF-8");

    // Every spoken and display string the scaffold authored stays in the file.
    let document: serde_json::Value =
        serde_json::from_str(&scaffold).expect("the scaffold is JSON");
    let segments = document["segments"]
        .as_array()
        .expect("a scaffold plans segments");
    assert!(
        !segments.is_empty(),
        "a scaffold with no segments proves nothing"
    );

    for segment in segments {
        for field in ["spoken_text", "display_text"] {
            let text = segment[field]
                .as_str()
                .expect("every scaffolded segment carries both texts");
            assert!(
                !printed.contains(text),
                "the success record quotes {field}: {text:?}"
            );
        }
    }
}
