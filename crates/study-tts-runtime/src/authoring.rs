//! Lesson scaffolding: what `study-tts lesson new` writes, and why.
//!
//! The scaffold is a *document*, not a template engine. It exists so an author
//! starts from bytes this build has already validated, rather than from a
//! schema they must satisfy by hand on the first try. Everything the format
//! requires is present and truthful; everything the author must supply is
//! marked as theirs to replace.
//!
//! `docs/operations/AUTHORING.md` is the other end of this module: it
//! describes the scaffold, edit, review, validate, preview loop the scaffold
//! opens, and names this file.

use std::{collections::BTreeMap, path::Path};

use study_tts_core::{
    AuthoredLesson, CompileError, CompileRequest, DeliveryStyle, LessonError, LessonSegment,
    ReviewStatus, SegmentRole, SpeakerDeclaration, ValidatedLesson,
};

use crate::{
    BuildError, IoError,
    durable::{OsDurableFileSystem, RenameOutcome, write_bytes_noreplace},
};

/// The voice profile every scaffolded speaker is bound to.
///
/// `docs/adr/deviations/ADR-0001-D003-single-instructor-fallback.md` selects
/// the single-instructor configuration in which "every spoken turn uses the one
/// approved `owner-fallback-v1` profile". A scaffold naming any other profile
/// would name one no render currently qualifies.
///
/// That deviation record carries no comment naming this constant, unlike
/// every other code-to-document mirror in this workspace: it is digest-pinned
/// by the accepted E0-S2 source-provenance record under
/// `evidence/gates/g0/e0-s2/`, and editing an accepted record's subject
/// invalidates the provenance it attests. `docs/operations/AUTHORING.md`
/// carries the return reference instead.
pub const SCAFFOLD_VOICE_PROFILE: &str = "owner-fallback-v1";

/// The speaker name a scaffold gives its one voice.
///
/// `instructor` rather than a person's name, because the same deviation
/// forbids relabelling the owner profile "as Nadia, Tom, a second speaker, or
/// multiple speakers".
const SCAFFOLD_SPEAKER: &str = "instructor";

/// The language a scaffold declares.
///
/// A synthesis-key input under ADR-0001 §12.5, so it cannot be left to a
/// default; `en` is the only language the qualified voice profile records.
const SCAFFOLD_LANGUAGE: &str = "en";

/// One segment of the scaffold, in speaking order.
struct ScaffoldSegment {
    /// Identity within the lesson, stable across every edit the author makes.
    id: &'static str,
    /// What the segment is doing in the lesson.
    role: SegmentRole,
    /// The line the author replaces, which is guidance rather than content.
    text: &'static str,
    /// Silence written after the segment.
    pause_after_ms: u32,
}

/// The shape of a lesson, spoken once so an author can hear it before writing
/// one.
///
/// A prompt, its response interval, and an answer: ADR-0001 §3.4's default
/// study sequence reduced to the smallest run that exercises the one invariant
/// a role carries. `2_000` sits inside the 1 500–4 000 ms band
/// `LessonError::RecallPromptWithoutResponseInterval` and its too-long sibling
/// bound, so an author who shortens or lengthens it is told which way they
/// went.
const SCAFFOLD_SEGMENTS: [ScaffoldSegment; 3] = [
    ScaffoldSegment {
        id: "seg-0001",
        role: SegmentRole::Explanation,
        text: "Replace this line with the explanation this lesson opens on.",
        pause_after_ms: 400,
    },
    ScaffoldSegment {
        id: "seg-0002",
        role: SegmentRole::RecallPrompt,
        text: "Replace this line with the question the listener should answer.",
        pause_after_ms: 2_000,
    },
    ScaffoldSegment {
        id: "seg-0003",
        role: SegmentRole::Answer,
        text: "Replace this line with the answer to the question above.",
        pause_after_ms: 600,
    },
];

/// Writes a validated lesson scaffold to `destination`.
///
/// The bytes are validated before they are published, so a scaffold this
/// function returns `Ok` for is one `load_lesson` accepts. Nothing already at
/// `destination` is replaced, and a refusal leaves the directory as it found
/// it.
///
/// # Errors
///
/// [`IoError::WriteJson`] when the document cannot be serialized,
/// [`IoError::DestinationExists`] when `destination` is already taken,
/// [`IoError::FileSystem`] when staging, writing, synchronizing, or renaming
/// fails — which is also how a missing parent directory is reported, since
/// creating one is deliberately not this command's business — and
/// [`crate::ManagedPathError::UnrootedDestination`] when `destination` names no
/// file at all.
///
/// [`IoError::ReadFile`], [`IoError::LessonNotRegularFile`], and
/// [`IoError::AudioAt`] are not among them, because this function reads no
/// file and touches no audio.
///
/// One [`study_tts_core::LessonError`] is reachable and the rest are not:
/// [`study_tts_core::LessonError::InvalidLessonId`], because `lesson_id` is the
/// only field a caller supplies. Every other field is this module's own, and
/// `t1_e1_the_scaffold_this_build_writes_validates` is what proves them valid
/// rather than this sentence.
pub fn scaffold_lesson(lesson_id: &str, destination: &Path) -> Result<(), BuildError> {
    let mut document =
        serde_json::to_vec_pretty(&scaffold(lesson_id)).map_err(|source| IoError::WriteJson {
            path: destination.to_path_buf(),
            source,
        })?;
    // A file a person opens in an editor ends with a newline. The validation
    // below runs over these bytes including it, so what was checked stays what
    // is written.
    document.push(b'\n');

    // The bytes are validated rather than the value, and the validated bytes
    // are the ones published. Serializing a second time after validation would
    // leave the document that was checked and the document that was written as
    // two artifacts nothing holds together.
    ValidatedLesson::from_json(&destination.display().to_string(), &document)?;

    match write_bytes_noreplace(&OsDurableFileSystem, destination, &document)? {
        RenameOutcome::Published => Ok(()),
        RenameOutcome::DestinationExists => Err(IoError::DestinationExists {
            path: destination.to_path_buf(),
        }
        .into()),
    }
}

/// Compiles a Markdown source document into a lesson beside it.
///
/// Here rather than in `study-tts-core` because this reads and writes files,
/// which is the boundary [`scaffold_lesson`] and [`crate::load_lesson`] already
/// sit on: `crates/AGENTS.md` §Routing table puts durable domain decisions in
/// core and the operations around them here. The parse and the lesson assembly
/// are core's — [`study_tts_core::compile_lesson`] — and this supplies the
/// bytes, validates the result, and publishes it.
///
/// The compiled document is **validated before it is written**, on the bytes
/// that are written, for the reason [`scaffold_lesson`] gives: serializing
/// again after validating would leave the document that was checked and the
/// document that landed as two artifacts nothing holds together.
///
/// # Why one refusal is tolerated and no other is
///
/// ADR-0001 §9.2 requires compilation to "split material into reviewable
/// **draft** segments", and [`ValidatedLesson`] refuses any segment that is not
/// `Approved` — a rule stricter than §8.2's own text, which bars unreviewed
/// material from "a production build". A compiled draft therefore cannot be a
/// [`ValidatedLesson`], and both ways to make it one are wrong: emitting
/// `approved` asserts a review nobody performed, and loosening the gate would
/// let an unapproved lesson reach a render.
///
/// So the draft is validated and exactly one outcome is accepted besides
/// success — [`study_tts_core::LessonError::UnapprovedSegment`], which is the
/// author's remaining work and what `study-tts lesson validate` tells them.
/// Every other refusal is a defect in this compiler and refuses the write, so
/// nothing malformed for any other reason reaches disk. The gate is not
/// weakened; the compiler stops claiming its output has passed it.
///
/// An existing destination is never replaced, which is
/// [`IoError::DestinationExists`] — a compiled lesson an author has since
/// edited is not this command's to overwrite.
///
/// # Errors
///
/// [`IoError::ReadFile`] when the source cannot be read,
/// [`IoError::LessonNotRegularFile`] when its name holds something else,
/// [`study_tts_core::CompileError::SourceNotUtf8`] when it read but is not
/// UTF-8,
/// [`study_tts_core::LessonError::LessonJsonTooLarge`] when it exceeds the
/// authored-document ceiling, whatever [`study_tts_core::compile_lesson`]
/// reports as a [`study_tts_core::CompileError`], whatever
/// [`ValidatedLesson::from_json`] reports about the compiled result **other
/// than** [`study_tts_core::LessonError::UnapprovedSegment`], and
/// [`IoError::DestinationExists`] when the destination is taken.
pub fn compile_source(
    source: &Path,
    destination: &Path,
    request: CompileRequest,
) -> Result<(), BuildError> {
    // The same bounded reader an authored lesson gets, and the same ceiling: a
    // source document is operator input, and a file read into memory to
    // discover it was too large has already cost what the ceiling refuses.
    let bytes = crate::pipeline::read_lesson(source)?;
    // `from_utf8` rather than `from_utf8_lossy`: a lossy conversion would
    // replace the invalid bytes with U+FFFD and compile a lesson whose spoken
    // text silently differs from the document the author wrote, and
    // `source.content_hash` would then name a document that produces different
    // text on a build that refused instead.
    let document = String::from_utf8(bytes).map_err(|error| {
        BuildError::Compile(Box::new(CompileError::SourceNotUtf8 {
            offset: error.utf8_error().valid_up_to(),
        }))
    })?;

    let compiled = study_tts_core::compile_lesson(&document, request)
        .map_err(|error| BuildError::Compile(Box::new(error)))?;

    let mut json = serde_json::to_vec_pretty(&compiled).map_err(|source| IoError::WriteJson {
        path: destination.to_path_buf(),
        source,
    })?;
    json.push(b'\n');

    match ValidatedLesson::from_json(&destination.display().to_string(), &json) {
        Ok(_) => {}
        Err(diagnostic) if matches!(diagnostic.error(), LessonError::UnapprovedSegment(_)) => {}
        Err(other) => return Err(other.into()),
    }

    match write_bytes_noreplace(&OsDurableFileSystem, destination, &json)? {
        RenameOutcome::Published => Ok(()),
        RenameOutcome::DestinationExists => Err(IoError::DestinationExists {
            path: destination.to_path_buf(),
        }
        .into()),
    }
}

/// The document [`scaffold_lesson`] publishes.
///
/// Split out so the scaffold's validity is a unit test rather than a claim
/// resting on the filesystem being writable.
fn scaffold(lesson_id: &str) -> AuthoredLesson {
    let speakers = BTreeMap::from([(
        SCAFFOLD_SPEAKER.to_owned(),
        SpeakerDeclaration {
            voice_profile: SCAFFOLD_VOICE_PROFILE.to_owned(),
        },
    )]);

    let segments = SCAFFOLD_SEGMENTS
        .iter()
        .map(|segment| LessonSegment {
            id: segment.id.to_owned(),
            speaker: SCAFFOLD_SPEAKER.to_owned(),
            role: segment.role,
            // Empty because `editorial` is set: ADR-0001 §8.2 lets a segment
            // cite nothing only when it is the author's own words, and
            // placeholder guidance is nobody else's.
            source_refs: Vec::new(),
            display_text: segment.text.to_owned(),
            spoken_text: segment.text.to_owned(),
            // The one style the worker declares, and the only one ADR-0001
            // §13.4 has a frozen loudness reference for.
            style: DeliveryStyle::CalmExplanatory,
            pause_after_ms: segment.pause_after_ms,
            // Approved, so the scaffold renders as written. Synthesis accepts
            // no other status, and a scaffold an author must hand-approve
            // before hearing anything is one that failed at the job E1-S5
            // gives it; that is what
            // `t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton`
            // reads. `docs/operations/AUTHORING.md` carries the other half of
            // the rule: an edited segment returns to `needs_review` and is
            // approved again by a person.
            review_status: ReviewStatus::Approved,
            editorial: true,
        })
        .collect();

    AuthoredLesson::new(
        lesson_id.to_owned(),
        // The identifier rather than a sentence. `PORTABLE_ID_PATTERN` forbids
        // spaces, so a lesson ID is never a title a person would write; and a
        // fabricated prose title would be lesson content this build authored
        // and the author never reviewed.
        lesson_id.to_owned(),
        SCAFFOLD_LANGUAGE.to_owned(),
        speakers,
        segments,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::TempDir;

    use study_tts_core::LessonError;

    use super::*;

    const DOCUMENT: &str = "scaffold";

    fn validate(lesson_id: &str) -> Result<ValidatedLesson, Box<study_tts_core::LessonDiagnostic>> {
        let document = serde_json::to_vec_pretty(&scaffold(lesson_id)).expect("serialize scaffold");
        ValidatedLesson::from_json(DOCUMENT, &document)
    }

    #[test]
    fn t1_e1_the_scaffold_this_build_writes_validates() {
        validate("e1-s5-scaffold").expect("the scaffold validates without manual repair");
    }

    #[test]
    fn t1_e1_a_scaffold_is_approved_so_it_renders_as_written() {
        let lesson = validate("e1-s5-approved").expect("the scaffold validates");

        for segment in lesson.segments() {
            assert_eq!(
                segment.review_status,
                ReviewStatus::Approved,
                "segment `{}` would refuse synthesis",
                segment.id
            );
        }
    }

    #[test]
    fn t1_e1_a_scaffold_lesson_id_is_validated_before_anything_is_written() {
        let refusal = validate("not a portable id").expect_err("a spaced identifier is refused");

        assert!(
            matches!(refusal.error(), LessonError::InvalidLessonId(_)),
            "expected an invalid-identifier refusal, got {refusal}"
        );
    }

    /// A request naming one speaker, for the compile tests here.
    fn compile_request() -> CompileRequest {
        CompileRequest {
            lesson_id: "e3-s1-encoding".to_owned(),
            language: "en".to_owned(),
            speakers: BTreeMap::from([(
                "narrator".to_owned(),
                SpeakerDeclaration {
                    voice_profile: SCAFFOLD_VOICE_PROFILE.to_owned(),
                },
            )]),
            speaker: "narrator".to_owned(),
            role: SegmentRole::Explanation,
            style: DeliveryStyle::CalmExplanatory,
            pause_after_ms: 75,
        }
    }

    /// A source document that is not UTF-8 is refused as a compile problem.
    ///
    /// The wrong implementation this rejects is the one that shipped: an
    /// `IoError::ReadFile` carrying a hand-built `io::Error`, naming a file
    /// that read without error. It sent an operator to check permissions and
    /// paths for a document whose encoding is the fault.
    ///
    /// The offset is asserted because it is what locates the problem without
    /// the message carrying the bytes — the rights constraint `markdown.rs`
    /// states for every diagnostic in that module.
    #[test]
    fn t1_e3_a_source_document_that_is_not_utf8_is_refused_as_a_compile_problem() {
        let workspace = TempDir::new().expect("create a workspace");
        let source = workspace.path().join("lesson.md");
        let destination = workspace.path().join("out.json");
        // Valid Markdown, then a lone continuation byte, which no UTF-8
        // sequence can begin with. It sits at byte 15.
        std::fs::write(&source, b"# Title\n\nProse \xa0 here.\n").expect("write the source");

        let error = compile_source(&source, &destination, compile_request())
            .expect_err("a non-UTF-8 document cannot be compiled");

        match error {
            BuildError::Compile(compile) => match *compile {
                CompileError::SourceNotUtf8 { offset } => {
                    assert_eq!(offset, 15, "the refusal locates the first invalid byte");
                }
                other => panic!("the refusal is about the encoding, got {other:?}"),
            },
            other => panic!("a read that succeeded is not an I/O failure, got {other:?}"),
        }
        assert!(
            !destination.exists(),
            "a refused compilation writes nothing"
        );
    }
}
