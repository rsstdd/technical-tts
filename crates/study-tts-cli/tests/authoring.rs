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
        ["new", "validate", "compile", "help"],
        "E1-S5 published `new` and `validate`; E3-S1 adds `compile`, which ADR-0001 §7.3 \
         spells. A fourth would need a story that owns it"
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

/// A compiled draft is a document the validator reads, and the only thing it
/// still refuses is the review an author owes.
///
/// The end-to-end claim that matters: a compiler whose output the validator
/// cannot read has produced nothing usable. It runs through the real binary
/// and the real filesystem, because that is where the claim lives.
///
/// The refusal is the point rather than a disappointment. ADR-0001 §9.2 asks
/// for "reviewable draft segments" and `ValidatedLesson` accepts only
/// `approved`, so `lesson validate` tells the author exactly what remains —
/// and the exit class is `2`, invalid input, which is the author's to fix.
/// `compile_source` records why it tolerates that one refusal and no other.
#[test]
fn t4_e3_compile_writes_a_draft_whose_only_refusal_is_its_review() {
    let workspace = TempDir::new().expect("create a workspace");
    let source = workspace.path().join("lesson.md");
    let out = workspace.path().join("lesson.json");
    std::fs::write(
        &source,
        "# Caches\n\nA cache stores reusable work.\n\n- It is keyed by its inputs.\n",
    )
    .expect("write the source");

    let compiled = study_tts(&[
        "lesson",
        "compile",
        &source.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--lesson-id",
        "e3-s1-compiled",
        "--speaker",
        "narrator=synthetic-test-voice-v1",
    ]);
    assert_eq!(
        compiled.status.code(),
        Some(0),
        "compiling succeeds: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let validated = study_tts(&["lesson", "validate", &out.display().to_string()]);
    assert_eq!(
        validated.status.code(),
        Some(2),
        "a draft is refused as invalid input, not as something else"
    );
    let refusal = String::from_utf8_lossy(&validated.stderr).into_owned();
    assert!(
        refusal.contains("not approved for synthesis") && refusal.contains("review_status"),
        "the refusal names the review the author owes: {refusal}"
    );

    // Everything else about the document is already valid, which is what makes
    // the refusal above the *only* one: the compiler refuses to write a draft
    // that fails for any other reason.
    let written = std::fs::read_to_string(&out).expect("the draft is on disk");
    let document: serde_json::Value = serde_json::from_str(&written).expect("the draft is JSON");
    assert_eq!(document["schema_version"], "3.1");
    assert_eq!(document["lesson_id"], "e3-s1-compiled");
    assert_eq!(document["title"], "Caches");
    assert!(
        document["source"]["content_hash"].is_string(),
        "the document the block references index is named"
    );
    for segment in document["segments"]
        .as_array()
        .expect("the draft has segments")
    {
        assert_eq!(segment["review_status"], "draft");
        assert!(
            segment["source_refs"]
                .as_array()
                .is_some_and(|refs| !refs.is_empty()),
            "every compiled segment traces to a source block: {segment}"
        );
    }
}

/// An existing destination is never replaced by a compilation.
#[test]
fn t4_e3_compile_never_overwrites_an_existing_lesson() {
    let workspace = TempDir::new().expect("create a workspace");
    let source = workspace.path().join("lesson.md");
    let out = workspace.path().join("taken.json");
    std::fs::write(&source, "# Title\n\nProse.\n").expect("write the source");
    std::fs::write(&out, b"author's own work").expect("occupy the destination");

    let refused = study_tts(&[
        "lesson",
        "compile",
        &source.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--lesson-id",
        "e3-s1-taken",
        "--speaker",
        "narrator=synthetic-test-voice-v1",
    ]);

    assert_ne!(
        refused.status.code(),
        Some(0),
        "an occupied name is refused"
    );
    assert_eq!(
        std::fs::read_to_string(&out).expect("read the destination back"),
        "author's own work",
        "the file an author wrote is untouched"
    );
}

/// Two declared speakers and no `--voice` is refused, and nothing is written.
///
/// The refusal matters because the alternative is choosing a voice: a voice
/// profile reaches every synthesis key under ADR-0001 §12.5, so a compiler
/// that picked the first of two would silently decide whose voice a lesson
/// speaks in, and every later artifact would carry that choice.
///
/// **The last assertion is the test.** A nonzero exit and a message naming
/// `--voice` are both satisfiable by a command that complains and compiles
/// anyway; the absence of the output file is what separates refusing from
/// choosing. The version this replaces lived in `markdown.rs`, built a
/// `CompileError::SpeakerNotChosen` by hand, and asserted its `Display` — so a
/// CLI that never raised it passed unchanged.
#[test]
fn t4_e3_an_unchosen_speaker_among_several_is_refused_without_writing() {
    let workspace = TempDir::new().expect("create a workspace");
    let source = workspace.path().join("lesson.md");
    let out = workspace.path().join("lesson.json");
    std::fs::write(&source, "# Title\n\nProse.\n").expect("write the source");

    let refused = study_tts(&[
        "lesson",
        "compile",
        &source.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--lesson-id",
        "e3-s1-ambiguous",
        "--speaker",
        "first=synthetic-test-voice-v1",
        "--speaker",
        "second=synthetic-test-voice-v2",
    ]);

    assert_eq!(
        refused.status.code(),
        Some(2),
        "an ambiguous speaker is invalid input: {}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let message = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        message.contains("2 speakers") && message.contains("--voice"),
        "the refusal counts them and names the remedy: {message}"
    );
    assert!(
        !out.exists(),
        "a refused compilation writes nothing, rather than choosing a voice"
    );
}

/// Naming one of several speakers with `--voice` compiles.
///
/// The other half of the refusal above: the ambiguity is the problem, not the
/// plurality, so a lesson with two declared voices and a chosen one is fine.
#[test]
fn t4_e3_several_speakers_compile_when_one_is_chosen() {
    let workspace = TempDir::new().expect("create a workspace");
    let source = workspace.path().join("lesson.md");
    let out = workspace.path().join("lesson.json");
    std::fs::write(&source, "# Title\n\nProse.\n").expect("write the source");

    let compiled = study_tts(&[
        "lesson",
        "compile",
        &source.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--lesson-id",
        "e3-s1-chosen",
        "--speaker",
        "first=synthetic-test-voice-v1",
        "--speaker",
        "second=synthetic-test-voice-v2",
        "--voice",
        "second",
    ]);

    assert_eq!(
        compiled.status.code(),
        Some(0),
        "a chosen speaker compiles: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&out).expect("the draft is on disk"))
            .expect("the draft is JSON");
    assert_eq!(
        document["speakers"].as_object().map(serde_json::Map::len),
        Some(2),
        "both declared speakers reach the lesson"
    );
    for segment in document["segments"]
        .as_array()
        .expect("the draft has segments")
    {
        assert_eq!(
            segment["speaker"], "second",
            "every segment speaks as the chosen voice"
        );
    }
}

/// A speaker bound twice is refused, and nothing is written.
///
/// `study-tts-core` refuses a lesson document that declares one speaker twice —
/// `LessonError::DuplicateSpeaker`, whose message says a repeated declaration
/// "lets the voice this speaker is rendered with depend on which binding a
/// parser keeps rather than on which one a reviewer approved". The command line
/// is where that can happen first, and a `BTreeMap` collected from arguments
/// keeps the last binding without a word.
///
/// The wrong implementation this rejects is the one that shipped and was found
/// by running the built binary: `--speaker narrator=voice-ONE --speaker
/// narrator=voice-TWO` exited `0` and recorded `voice-TWO`. The last assertion
/// is what separates refusing from collapsing.
#[test]
fn t4_e3_a_speaker_bound_twice_is_refused_without_writing() {
    let workspace = TempDir::new().expect("create a workspace");
    let source = workspace.path().join("lesson.md");
    let out = workspace.path().join("lesson.json");
    std::fs::write(&source, "# Title\n\nProse.\n").expect("write the source");

    let refused = study_tts(&[
        "lesson",
        "compile",
        &source.display().to_string(),
        "--out",
        &out.display().to_string(),
        "--lesson-id",
        "e3-s1-duplicate",
        "--speaker",
        "narrator=synthetic-test-voice-v1",
        "--speaker",
        "narrator=synthetic-test-voice-v2",
    ]);

    assert_eq!(
        refused.status.code(),
        Some(2),
        "a repeated binding is invalid input: {}",
        String::from_utf8_lossy(&refused.stderr)
    );
    let message = String::from_utf8_lossy(&refused.stderr).into_owned();
    assert!(
        message.contains("narrator") && message.contains("more than once"),
        "the refusal names the speaker and what is wrong: {message}"
    );
    assert!(
        !out.exists(),
        "a refused compilation writes nothing, rather than keeping one binding"
    );
}
