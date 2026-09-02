//! `study-tts`, the authoring command line.
//!
//! `DELIVERY-PLAN.md` E1-S5 gives this binary two jobs: scaffold a lesson an
//! author can edit, and tell them whether the document they edited is one this
//! build will render. Every decision behind both lives in `study-tts-runtime`.
//! What is here is argument parsing and the rendering of a refusal — no
//! filesystem durability, no validation, and no second opinion about either.
//!
//! `docs/operations/AUTHORING.md` documents the loop these two commands open.

use std::{path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use study_tts_core::LessonDiagnostic;
use study_tts_runtime::{BuildError, load_lesson, scaffold_lesson};

/// Turns reviewed technical lessons into study-guide audio.
#[derive(Debug, Parser)]
#[command(name = "study-tts", version, about, long_about = None)]
struct Cli {
    /// The command to run.
    #[command(subcommand)]
    command: Command,
}

/// The command groups this build implements.
#[derive(Debug, Subcommand)]
enum Command {
    /// Author and check lesson documents.
    #[command(subcommand)]
    Lesson(LessonCommand),
}

/// What can be done to a lesson document.
#[derive(Debug, Subcommand)]
enum LessonCommand {
    /// Write a new lesson scaffold that already validates.
    New {
        /// Stable identity of the lesson, which also names its output
        /// directory.
        lesson_id: String,
        /// Where to write the scaffold; an existing file is never replaced.
        #[arg(long)]
        out: PathBuf,
    },
    /// Check a lesson document the way the build that renders it will.
    Validate {
        /// The lesson document to check.
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(report) => {
            println!("{report}");
            ExitCode::SUCCESS
        }
        // One exit code for every refusal. `DELIVERY-PLAN.md` E2-S5 owns
        // stable numeric exit classes and `--json`; inventing either here
        // would publish a contract that story has to keep.
        Err(refusal) => {
            eprintln!("{}", describe(&refusal));
            ExitCode::FAILURE
        }
    }
}

/// Runs one command, returning what to tell the author on success.
fn run(command: Command) -> Result<String, BuildError> {
    let Command::Lesson(lesson) = command;
    match lesson {
        LessonCommand::New { lesson_id, out } => {
            scaffold_lesson(&lesson_id, &out)?;
            Ok(format!("created lesson scaffold: {}", out.display()))
        }
        LessonCommand::Validate { path } => {
            load_lesson(&path)?;
            Ok(format!("valid lesson: {}", path.display()))
        }
    }
}

/// Renders one refusal for the person who has to act on it.
///
/// Only a lesson refusal is reshaped, and only because
/// [`LessonDiagnostic`]'s own `Display` cannot know that a reader sitting in an
/// editor wants the segment identity as well as the JSON Pointer: the pointer
/// names a position, and a segment that moved is still found by its identity.
/// Every other refusal already names its artifact, its invariant, and its
/// remedy owner, so restating it here would only give the two spellings room to
/// disagree.
fn describe(error: &BuildError) -> String {
    match error {
        BuildError::Lesson(diagnostic) => describe_lesson(diagnostic),
        other => other.to_string(),
    }
}

/// Locates a lesson refusal by document, field path, and segment.
///
/// The lines are what an author edits, in the order they narrow: which file,
/// which field of it, which segment that field belongs to, and only then what
/// was wrong. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and
/// access excludes spoken text from diagnostics, which is why the offending
/// value is never among them — the pointer is how the author finds it.
fn describe_lesson(diagnostic: &LessonDiagnostic) -> String {
    let mut lines = vec![format!("refused `{}`", diagnostic.document())];
    if !diagnostic.field_path().is_empty() {
        lines.push(format!("  field: {}", diagnostic.field_path()));
    }
    if let Some(segment) = diagnostic.segment_id() {
        lines.push(format!("  segment: {segment}"));
    }
    lines.push(format!("  reason: {}", diagnostic.error()));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use study_tts_core::ValidatedLesson;

    use super::*;

    const DOCUMENT: &str = "lessons/e1-s5.json";

    /// A lesson whose one recall prompt leaves `pause_after_ms` of silence.
    fn lesson_with_pause(pause_after_ms: u32) -> Vec<u8> {
        format!(
            r#"{{
              "schema_version": "3.1",
              "lesson_id": "e1-s5-diagnostic",
              "title": "e1-s5-diagnostic",
              "language": "en",
              "speakers": {{ "instructor": {{ "voice_profile": "owner-fallback-v1" }} }},
              "segments": [
                {{
                  "id": "seg-0001",
                  "speaker": "instructor",
                  "role": "recall_prompt",
                  "source_refs": [],
                  "display_text": "A prompt.",
                  "spoken_text": "A prompt.",
                  "style": "calm_explanatory",
                  "pause_after_ms": {pause_after_ms},
                  "review_status": "approved",
                  "editorial": true
                }}
              ]
            }}"#
        )
        .into_bytes()
    }

    #[test]
    fn t1_e1_validation_error_names_the_offending_field_path() {
        let refusal = ValidatedLesson::from_json(DOCUMENT, &lesson_with_pause(10))
            .expect_err("a recall prompt leaving 10 ms is refused");

        let rendered = describe(&BuildError::Lesson(refusal));

        assert!(
            rendered.contains(&format!("refused `{DOCUMENT}`")),
            "the refusal names the document it read: {rendered}"
        );
        assert!(
            rendered.contains("  field: /segments/0/pause_after_ms"),
            "the refusal names the RFC 6901 pointer to the offending field: {rendered}"
        );
        assert!(
            rendered.contains("  segment: seg-0001"),
            "the refusal names the segment the field belongs to: {rendered}"
        );
    }

    #[test]
    fn t1_e1_a_refusal_about_a_whole_document_names_no_field_path() {
        let refusal = ValidatedLesson::from_json(DOCUMENT, b"{")
            .expect_err("bytes that are not JSON are refused");

        let rendered = describe(&BuildError::Lesson(refusal));

        assert!(
            !rendered.contains("  field:"),
            "RFC 6901's empty pointer is said by saying nothing: {rendered}"
        );
        assert!(
            !rendered.contains("  segment:"),
            "a whole-document refusal belongs to no segment: {rendered}"
        );
    }
}
