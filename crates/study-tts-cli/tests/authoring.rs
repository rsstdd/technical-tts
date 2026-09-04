//! Tier 4 tests for the E1-S5 authoring commands: real process, real
//! filesystem, fake worker, real FFmpeg.
//!
//! They live in this crate rather than beside the rest of the end-to-end suite
//! in `study-tts-testkit` because `CARGO_BIN_EXE_study-tts` is only set for the
//! package that declares the binary. A test that located the executable any
//! other way would be asserting a target-directory layout instead of a
//! command.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use study_tts_runtime::{BuildRequest, SCAFFOLD_VOICE_PROFILE, build_preview};
use study_tts_testkit::{DeterministicToneWorker, write_voice_profile_root};
use tempfile::TempDir;

fn study_tts(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

/// Scaffolds one lesson and returns where it was written.
fn scaffold(directory: &Path, lesson_id: &str) -> PathBuf {
    let destination = directory.join(format!("{lesson_id}.json"));
    let output = study_tts(&[
        "lesson",
        "new",
        lesson_id,
        "--out",
        &destination.display().to_string(),
    ]);

    assert!(
        output.status.success(),
        "`lesson new` refused a scaffold: {}",
        stderr_of(&output)
    );
    assert!(
        std::fs::read(&destination)
            .expect("read the scaffold")
            .ends_with(b"\n"),
        "a file an author opens in an editor ends with a newline"
    );
    destination
}

#[test]
fn t4_e1_scaffolded_lesson_validates_without_manual_repair() {
    let directory = TempDir::new().expect("create scaffold workspace");
    let lesson = scaffold(directory.path(), "e1-s5-scaffold");

    let validated = study_tts(&["lesson", "validate", &lesson.display().to_string()]);

    assert!(
        validated.status.success(),
        "the scaffold this build wrote did not validate: {}",
        stderr_of(&validated)
    );
    assert_eq!(
        stdout_of(&validated).trim(),
        format!("valid lesson: {}", lesson.display()),
        "`lesson validate` reports the document it accepted"
    );
}

#[test]
fn t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton() {
    let directory = TempDir::new().expect("create render workspace");
    let lesson = scaffold(directory.path(), "e1-s5-render");
    let voice_profile_root = directory.path().join("voices");
    write_voice_profile_root(&voice_profile_root, &[SCAFFOLD_VOICE_PROFILE]);

    let result = build_preview(
        BuildRequest {
            lesson_path: lesson,
            workspace: directory.path().join("workspace"),
            ffmpeg_executable: "ffmpeg".into(),
            ffprobe_executable: "ffprobe".into(),
            voice_profile_root,
            retakes: std::collections::BTreeMap::new(),
        },
        &DeterministicToneWorker::default(),
    )
    .expect("render the scaffold through the walking skeleton");

    for artifact in [
        &result.master_wav,
        &result.m4a,
        &result.mp3,
        &result.transcript,
        &result.captions,
        &result.chapters,
        &result.manifest,
    ] {
        assert!(
            artifact.is_file(),
            "the package is missing `{}`",
            artifact.display()
        );
    }
}

#[test]
fn t4_e1_an_invalid_lesson_id_is_refused_before_any_file_is_created() {
    let directory = TempDir::new().expect("create refusal workspace");
    let destination = directory.path().join("spaced.json");

    let output = study_tts(&[
        "lesson",
        "new",
        "not a portable id",
        "--out",
        &destination.display().to_string(),
    ]);

    assert!(!output.status.success(), "an invalid lesson ID is refused");
    assert!(
        !destination.exists(),
        "a refused scaffold left `{}` behind",
        destination.display()
    );
}

#[test]
fn t4_e1_an_existing_destination_is_never_overwritten() {
    let directory = TempDir::new().expect("create overwrite workspace");
    let destination = directory.path().join("held.json");
    std::fs::write(&destination, b"author's own bytes").expect("write the held file");

    let output = study_tts(&[
        "lesson",
        "new",
        "e1-s5-held",
        "--out",
        &destination.display().to_string(),
    ]);

    assert!(
        !output.status.success(),
        "an existing destination is refused"
    );
    assert_eq!(
        std::fs::read(&destination).expect("read the held file"),
        b"author's own bytes",
        "the author's file survived the refusal byte for byte"
    );
}

#[cfg(unix)]
#[test]
fn t4_e1_a_scaffold_is_published_owner_readable_only() {
    use std::os::unix::fs::PermissionsExt;

    let directory = TempDir::new().expect("create mode workspace");
    let lesson = scaffold(directory.path(), "e1-s5-mode");

    let mode = std::fs::metadata(&lesson)
        .expect("read the scaffold's metadata")
        .permissions()
        .mode();

    assert_eq!(mode & 0o777, 0o600, "a scaffold is owner-readable only");
}

#[test]
fn t4_e1_an_invalid_lesson_exits_nonzero_naming_the_document() {
    let directory = TempDir::new().expect("create validation workspace");
    let lesson = directory.path().join("broken.json");
    std::fs::write(&lesson, b"{").expect("write the malformed lesson");

    let output = study_tts(&["lesson", "validate", &lesson.display().to_string()]);

    assert!(!output.status.success(), "an invalid lesson exits nonzero");
    assert!(
        stderr_of(&output).contains(&lesson.display().to_string()),
        "the refusal names the document it read: {}",
        stderr_of(&output)
    );
    assert!(
        stdout_of(&output).is_empty(),
        "a refusal writes nothing to stdout"
    );
}

#[test]
fn t4_e1_help_lists_only_the_implemented_lesson_commands() {
    let output = study_tts(&["lesson", "--help"]);
    let help = stdout_of(&output);

    assert!(output.status.success(), "`--help` succeeds");
    // The `Commands:` block rather than the whole page: prose describing what
    // `validate` checks names rendering, and a test that searched the page for
    // the word would refuse a sentence instead of a command.
    let listed: Vec<String> = help
        .split_once("Commands:\n")
        .expect("clap lists the commands")
        .1
        .lines()
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect();

    assert_eq!(
        listed,
        ["new", "validate", "help"],
        "E1-S5 publishes exactly two lesson commands, and `DELIVERY-PLAN.md` E2-S5 owns \
         everything an author might expect beside them"
    );
}

#[test]
fn t4_e1_a_refusal_never_quotes_the_lesson_text_it_read() {
    const SPOKEN: &str = "Rights policy forbids echoing this sentence into a diagnostic.";

    let directory = TempDir::new().expect("create rights workspace");
    let lesson = directory.path().join("leaky.json");
    // Valid in every respect except the recall prompt's response interval, so
    // the refusal comes from the segment that carries the text.
    std::fs::write(
        &lesson,
        format!(
            r#"{{
  "schema_version": "3.1",
  "lesson_id": "e1-s5-rights",
  "title": "e1-s5-rights",
  "language": "en",
  "speakers": {{ "instructor": {{ "voice_profile": "owner-fallback-v1" }} }},
  "segments": [
    {{
      "id": "seg-0001",
      "speaker": "instructor",
      "role": "recall_prompt",
      "source_refs": [],
      "display_text": "{SPOKEN}",
      "spoken_text": "{SPOKEN}",
      "style": "calm_explanatory",
      "pause_after_ms": 10,
      "review_status": "approved",
      "editorial": true
    }}
  ]
}}"#
        ),
    )
    .expect("write the rights-sensitive lesson");

    let output = study_tts(&["lesson", "validate", &lesson.display().to_string()]);

    assert!(!output.status.success(), "the lesson is refused");
    let rendered = format!("{}{}", stdout_of(&output), stderr_of(&output));
    assert!(
        !rendered.contains(SPOKEN),
        "`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps spoken text out of \
         diagnostics, but the refusal read: {rendered}"
    );
    assert!(
        rendered.contains("seg-0001"),
        "the refusal still locates the segment: {rendered}"
    );
}
