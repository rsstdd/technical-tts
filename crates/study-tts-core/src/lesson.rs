//! Authored lesson documents and their pre-planning validation boundary.
//!
//! Absent and malformed input remain distinct so authors receive the right
//! remedy before synthesis can start.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    LanguageTag, MalformedLanguageTag, SchemaVersion, SchemaVersionError,
    schema::check_declared_version, schema_uri,
};

/// Identifiers reach the filesystem through `previews/<lesson-id>/`, so they
/// are bounded well below `NAME_MAX` (255 on ext4) to leave room for the
/// suffixes later stories append.
const MAX_IDENTIFIER_LENGTH: usize = 64;

/// The published-schema spelling of [`is_portable_id`]'s character rule.
///
/// JSON Schema needs its own spelling so editors can reject invalid IDs. The
/// parser/schema agreement is pinned by
/// `t3_e1_the_published_lesson_schema_refuses_the_invalid_fixtures`.
const PORTABLE_ID_PATTERN: &str = r"^[A-Za-z0-9_-][A-Za-z0-9._-]*$(?![\s\S])";

/// Largest canonical lesson JSON document accepted, in UTF-8 bytes.
///
/// This provisional security ceiling mirrors
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings;
/// runtime ingestion imports it rather than maintaining a second value.
pub const MAX_LESSON_JSON_BYTES: usize = 16 * 1024 * 1024;

// The five provisional authored-input ceilings below mirror
// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
pub(crate) const MAX_LESSON_SEGMENTS: usize = 4_096;
const MAX_SEGMENT_TEXT_BYTES: usize = 64 * 1024;
const MAX_SOURCE_REFS_PER_SEGMENT: usize = 256;
const MAX_SOURCE_REF_BYTES: usize = 4 * 1024;
const MAX_AUTHORED_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// Longest trailing pause a segment may declare, in milliseconds.
///
/// Long enough for a deliberate beat, but short enough to reject a value that
/// would sound like an audio fault.
const MAX_PAUSE_AFTER_MS: u32 = 10_000;

/// Layout version this build publishes for a lesson document.
///
/// Version `1.0` made synthesis-key language required; `1.1` added the optional
/// [`AuthoredLesson::schema`] link. The change classes and history are recorded
/// in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`.
pub const LESSON_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 1);

/// File-name stem of the published lesson schema, per ADR-0001 §7.1.
///
/// Shared by document validation and schema publication to prevent drift.
pub const LESSON_SCHEMA_STEM: &str = "lesson";

/// One authored lesson, as it is written on disk and before validation.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct AuthoredLesson {
    /// Published schema this document links to, added by lesson schema `1.1`.
    ///
    /// Optional, with absent as its declared default, so a `1.0` document
    /// stays valid. When present it must name the schema for the version the
    /// document declares: a link to some other schema is a document claiming
    /// to have been checked against something it was not.
    ///
    /// Spelled `$schema` because that is the key every JSON Schema tool looks
    /// for; `deny_unknown_fields` means the rename is the only way the field
    /// can appear at all.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "schema_link_json_schema")]
    pub schema: Option<String>,
    /// Schema this document claims; an unrecognized version is refused rather
    /// than guessed at.
    ///
    /// Held as authored text rather than a [`SchemaVersion`] so a malformed
    /// version is reported by [`LessonError::UnsupportedSchema`] naming what
    /// was written, instead of by a serde message about a field type.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: String,
    /// Stable identity of the lesson, which also names its output directory.
    #[schemars(schema_with = "portable_id_json_schema")]
    pub lesson_id: String,
    /// Human-readable title; display only, and deliberately outside every cache
    /// key.
    pub title: String,
    /// Language the lesson is spoken in, as a BCP 47 tag.
    ///
    /// Unlike the title this *is* a synthesis-key input (ADR-0001 §12.5), so a
    /// lesson cannot leave it to a default: the same text in two languages is
    /// two different renders and must not share a cache entry.
    ///
    /// Authored text here and a [`LanguageTag`] on [`ValidatedLesson`], for the
    /// reason given on `schema_version`.
    #[schemars(schema_with = "language_json_schema")]
    pub language: String,
    /// The lesson's segments in speaking order.
    pub segments: Vec<LessonSegment>,
}

/// A lesson whose complete set of authoring invariants has passed validation.
///
/// Private fields prevent unchecked construction at the planning boundary.
#[derive(Clone, Debug)]
pub struct ValidatedLesson {
    authored: AuthoredLesson,
    schema_version: SchemaVersion,
    language: LanguageTag,
}

/// One continuously spoken passage, the unit that is synthesized, cached, and
/// retaken.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct LessonSegment {
    /// Identity of the segment within its lesson, unique and portable as a path
    /// component.
    #[schemars(schema_with = "portable_id_json_schema")]
    pub id: String,
    /// Which voice speaks this segment.
    pub speaker: String,
    /// The speaker's role in the lesson, for review context rather than for
    /// synthesis.
    pub role: String,
    /// Source material this segment was written from, so a claim can be traced
    /// back.
    pub source_refs: Vec<String>,
    /// Text as a reviewer reads it; display only, and outside the cache key.
    pub display_text: String,
    /// Text as it is spoken, which is what synthesis and the cache key are
    /// derived from.
    pub spoken_text: String,
    /// Delivery style requested of the voice.
    pub style: String,
    /// Silence written after this segment, in milliseconds.
    pub pause_after_ms: u32,
    /// Whether a human has approved this segment for synthesis.
    pub review_status: ReviewStatus,
}

/// Whether a segment has cleared human review.
///
/// Closed vocabulary rather than a flag: an unrecognized status is a parse
/// error, so a document cannot invent a state that would be treated as approved
/// by default.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    /// A human approved this segment; the only status synthesis accepts.
    Approved,
    /// Not yet submitted for review.
    Draft,
    /// Submitted and awaiting a decision.
    NeedsReview,
}

/// Why a lesson document was refused.
///
/// Each invariant has a distinct variant; absent and malformed values remain
/// separate because their remedies differ.
#[derive(Debug, Error)]
pub enum LessonError {
    /// The input exceeds the fixed envelope within which parsing is allowed.
    #[error("lesson JSON exceeds the provisional {max_bytes}-byte limit")]
    LessonJsonTooLarge {
        /// Largest lesson document this build accepts.
        max_bytes: usize,
    },
    /// The bytes are not JSON, or not the shape this schema declares.
    #[error("lesson JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The shape parsed, but this build does not know that version and will not
    /// guess.
    ///
    /// Preserves the typed version refusal so callers retain its exact remedy.
    #[error("lesson schema version is unusable: {0}")]
    UnsupportedSchema(#[from] SchemaVersionError),
    /// The document links to a schema other than the one for the version it
    /// declares.
    ///
    /// A wrong link means the author's editor checked different rules.
    #[error(
        "lesson links to schema `{declared}` but declares version `{version}`, whose schema is \
         `{expected}`; the document's author must correct the link or the version so the document \
         is checked against the rules this build applies"
    )]
    UnexpectedSchemaLink {
        /// Link the document carries.
        declared: String,
        /// Version the document declares.
        version: SchemaVersion,
        /// Link that version requires.
        expected: String,
    },
    /// No lesson identity was supplied at all.
    #[error("lesson_id must not be empty")]
    MissingLessonId,
    /// The declared language is not a BCP 47 tag.
    ///
    /// ADR-0001 §12.5 makes language a synthesis-key input, so it cannot be
    /// defaulted or passed through unchecked.
    #[error("lesson language is unusable: {0}")]
    MalformedLanguage(#[from] MalformedLanguageTag),
    /// An identity was supplied but could not safely name a directory.
    #[error(
        "lesson_id `{0}` must be 1-{max} ASCII letters, digits, hyphen, underscore, or dot, and \
         must not start with a dot, because it names an output directory",
        max = MAX_IDENTIFIER_LENGTH
    )]
    InvalidLessonId(String),
    /// A lesson with nothing to speak is an authoring mistake, not an empty
    /// build.
    #[error("lesson must contain at least one segment")]
    MissingSegments,
    /// The lesson would create more planning and synthesis units than this
    /// provisional build accepts.
    #[error("lesson contains {found} segments, exceeding the provisional limit of {max}")]
    TooManySegments {
        /// Segments the authored lesson contains.
        found: usize,
        /// Largest segment count this build accepts.
        max: usize,
    },
    /// A segment supplied no identity at all.
    #[error("segment ID must not be empty")]
    MissingSegmentId,
    /// An identity was supplied but could not safely name a path component.
    #[error(
        "segment ID `{0}` must be 1-{max} ASCII letters, digits, hyphen, underscore, or dot, and \
         must not start with a dot",
        max = MAX_IDENTIFIER_LENGTH
    )]
    InvalidSegmentId(String),
    /// Two segments share an identity, which would collide in the cache and the
    /// manifest.
    #[error("segment ID `{0}` is duplicated")]
    DuplicateSegmentId(String),
    /// The segment has nothing to synthesize.
    #[error("segment `{0}` has empty spoken_text")]
    MissingSpokenText(String),
    /// The exact synthesis input exceeds its fixed memory ceiling.
    #[error(
        "segment `{segment_id}` spoken_text is {bytes} UTF-8 bytes, exceeding the provisional \
         {max_bytes}-byte limit"
    )]
    SpokenTextTooLong {
        /// Segment carrying the oversized field.
        segment_id: String,
        /// UTF-8 bytes the field contains.
        bytes: usize,
        /// Largest accepted field length.
        max_bytes: usize,
    },
    /// The segment has nothing for a reviewer to read against the audio.
    #[error("segment `{0}` has empty display_text")]
    MissingDisplayText(String),
    /// The review transcript field exceeds its fixed memory ceiling.
    #[error(
        "segment `{segment_id}` display_text is {bytes} UTF-8 bytes, exceeding the provisional \
         {max_bytes}-byte limit"
    )]
    DisplayTextTooLong {
        /// Segment carrying the oversized field.
        segment_id: String,
        /// UTF-8 bytes the field contains.
        bytes: usize,
        /// Largest accepted field length.
        max_bytes: usize,
    },
    /// The segment declares no role, so review context cannot be established.
    #[error("segment `{0}` has an empty role")]
    MissingRole(String),
    /// The segment cites no source, so its claims cannot be traced back.
    #[error("segment `{0}` must contain at least one source reference")]
    MissingSourceRefs(String),
    /// A segment cites more source blocks than this provisional build accepts.
    #[error(
        "segment `{segment_id}` contains {found} source references, exceeding the provisional \
         limit of {max}"
    )]
    TooManySourceRefs {
        /// Segment carrying the oversized reference list.
        segment_id: String,
        /// References the segment contains.
        found: usize,
        /// Largest accepted reference count.
        max: usize,
    },
    /// A citation is present but blank, which traces to nothing.
    #[error("segment `{0}` contains an empty source reference")]
    EmptySourceRef(String),
    /// One source reference exceeds its fixed memory ceiling.
    #[error(
        "segment `{segment_id}` contains a {bytes}-byte source reference, exceeding the \
         provisional {max_bytes}-byte limit"
    )]
    SourceRefTooLong {
        /// Segment carrying the oversized reference.
        segment_id: String,
        /// UTF-8 bytes the reference contains.
        bytes: usize,
        /// Largest accepted reference length.
        max_bytes: usize,
    },
    /// No human approved this segment; synthesis accepts only
    /// `ReviewStatus::Approved`.
    #[error("segment `{0}` is not approved for synthesis")]
    UnapprovedSegment(String),
    /// No voice was named, so no voice profile can be resolved.
    #[error("segment `{0}` must declare a speaker")]
    MissingSpeaker(String),
    /// No delivery style was named, which would leave the synthesis identity
    /// underdetermined.
    #[error("segment `{0}` must declare a style")]
    MissingStyle(String),
    /// The pause is long enough to read as a fault in the audio rather than as
    /// phrasing.
    #[error("segment `{0}` pause exceeds the provisional {max} ms limit", max = MAX_PAUSE_AFTER_MS)]
    PauseOutOfRange(String),
    /// Authored strings collectively exceed the lesson memory envelope.
    #[error("lesson authored text exceeds the provisional {max_bytes}-byte aggregate limit")]
    AuthoredTextTooLarge {
        /// Largest accepted aggregate authored-text length.
        max_bytes: usize,
    },
}

impl AuthoredLesson {
    /// Validates authored data before making it available to render planning.
    ///
    /// # Errors
    ///
    /// [`LessonError::UnsupportedSchema`],
    /// [`LessonError::UnexpectedSchemaLink`],
    /// [`LessonError::MalformedLanguage`],
    /// [`LessonError::MissingLessonId`],
    /// [`LessonError::InvalidLessonId`], [`LessonError::MissingSegments`],
    /// [`LessonError::TooManySegments`], or
    /// [`LessonError::AuthoredTextTooLarge`] for a lesson-level violation.
    /// Segment validation returns
    /// [`LessonError::MissingSegmentId`],
    /// [`LessonError::InvalidSegmentId`],
    /// [`LessonError::DuplicateSegmentId`],
    /// [`LessonError::MissingSpokenText`],
    /// [`LessonError::SpokenTextTooLong`],
    /// [`LessonError::MissingDisplayText`],
    /// [`LessonError::DisplayTextTooLong`], [`LessonError::MissingRole`],
    /// [`LessonError::MissingSourceRefs`],
    /// [`LessonError::TooManySourceRefs`], [`LessonError::EmptySourceRef`],
    /// [`LessonError::SourceRefTooLong`],
    /// [`LessonError::UnapprovedSegment`], [`LessonError::MissingSpeaker`],
    /// [`LessonError::MissingStyle`], or [`LessonError::PauseOutOfRange`].
    /// Existing semantic checks preserve their relative order; resource checks
    /// occur beside the count or field they bound.
    pub fn validate(self) -> Result<ValidatedLesson, LessonError> {
        // The version decides what every later field means.
        let schema_version: SchemaVersion = self.schema_version.parse()?;
        schema_version.accepted_by(LESSON_SCHEMA_VERSION)?;
        if let Some(declared) = &self.schema {
            let expected = schema_uri(LESSON_SCHEMA_STEM, schema_version.major());
            if declared != &expected {
                return Err(LessonError::UnexpectedSchemaLink {
                    declared: declared.clone(),
                    version: schema_version,
                    expected,
                });
            }
        }
        validate_lesson_id(&self.lesson_id)?;
        let language: LanguageTag = self.language.parse()?;
        if self.segments.is_empty() {
            return Err(LessonError::MissingSegments);
        }
        if self.segments.len() > MAX_LESSON_SEGMENTS {
            return Err(LessonError::TooManySegments {
                found: self.segments.len(),
                max: MAX_LESSON_SEGMENTS,
            });
        }

        let mut ids = HashSet::with_capacity(self.segments.len());
        let mut authored_text_bytes = self.title.len();
        for segment in &self.segments {
            validate_segment_id(&segment.id)?;
            if !ids.insert(segment.id.as_str()) {
                return Err(LessonError::DuplicateSegmentId(segment.id.clone()));
            }
            if segment.spoken_text.trim().is_empty() {
                return Err(LessonError::MissingSpokenText(segment.id.clone()));
            }
            if segment.spoken_text.len() > MAX_SEGMENT_TEXT_BYTES {
                return Err(LessonError::SpokenTextTooLong {
                    segment_id: segment.id.clone(),
                    bytes: segment.spoken_text.len(),
                    max_bytes: MAX_SEGMENT_TEXT_BYTES,
                });
            }
            if segment.display_text.trim().is_empty() {
                return Err(LessonError::MissingDisplayText(segment.id.clone()));
            }
            if segment.display_text.len() > MAX_SEGMENT_TEXT_BYTES {
                return Err(LessonError::DisplayTextTooLong {
                    segment_id: segment.id.clone(),
                    bytes: segment.display_text.len(),
                    max_bytes: MAX_SEGMENT_TEXT_BYTES,
                });
            }
            if segment.role.trim().is_empty() {
                return Err(LessonError::MissingRole(segment.id.clone()));
            }
            if segment.source_refs.is_empty() {
                return Err(LessonError::MissingSourceRefs(segment.id.clone()));
            }
            if segment.source_refs.len() > MAX_SOURCE_REFS_PER_SEGMENT {
                return Err(LessonError::TooManySourceRefs {
                    segment_id: segment.id.clone(),
                    found: segment.source_refs.len(),
                    max: MAX_SOURCE_REFS_PER_SEGMENT,
                });
            }
            if segment
                .source_refs
                .iter()
                .any(|source_ref| source_ref.trim().is_empty())
            {
                return Err(LessonError::EmptySourceRef(segment.id.clone()));
            }
            if let Some(source_ref) = segment
                .source_refs
                .iter()
                .find(|source_ref| source_ref.len() > MAX_SOURCE_REF_BYTES)
            {
                return Err(LessonError::SourceRefTooLong {
                    segment_id: segment.id.clone(),
                    bytes: source_ref.len(),
                    max_bytes: MAX_SOURCE_REF_BYTES,
                });
            }
            if segment.review_status != ReviewStatus::Approved {
                return Err(LessonError::UnapprovedSegment(segment.id.clone()));
            }
            if segment.speaker.trim().is_empty() {
                return Err(LessonError::MissingSpeaker(segment.id.clone()));
            }
            if segment.style.trim().is_empty() {
                return Err(LessonError::MissingStyle(segment.id.clone()));
            }
            if segment.pause_after_ms > MAX_PAUSE_AFTER_MS {
                return Err(LessonError::PauseOutOfRange(segment.id.clone()));
            }

            for field in [
                &segment.id,
                &segment.speaker,
                &segment.role,
                &segment.display_text,
                &segment.spoken_text,
                &segment.style,
            ] {
                authored_text_bytes = authored_text_bytes.saturating_add(field.len());
            }
            for source_ref in &segment.source_refs {
                authored_text_bytes = authored_text_bytes.saturating_add(source_ref.len());
            }
            if authored_text_bytes > MAX_AUTHORED_TEXT_BYTES {
                return Err(LessonError::AuthoredTextTooLarge {
                    max_bytes: MAX_AUTHORED_TEXT_BYTES,
                });
            }
        }

        Ok(ValidatedLesson {
            authored: self,
            schema_version,
            language,
        })
    }
}

impl ValidatedLesson {
    /// Parses and validates a lesson document, refusing anything synthesis
    /// could not use.
    ///
    /// # Errors
    ///
    /// [`LessonError::LessonJsonTooLarge`] when the input exceeds
    /// [`MAX_LESSON_JSON_BYTES`], then [`LessonError::UnsupportedSchema`] for a
    /// version this build cannot read, and [`LessonError::InvalidJson`] when
    /// the bytes are not this document's shape. Parsed authoring data can
    /// return every lesson-level or segment-level variant documented by
    /// [`AuthoredLesson::validate`].
    pub fn from_json(bytes: &[u8]) -> Result<Self, LessonError> {
        if bytes.len() > MAX_LESSON_JSON_BYTES {
            return Err(LessonError::LessonJsonTooLarge {
                max_bytes: MAX_LESSON_JSON_BYTES,
            });
        }
        // A strict parse would misreport a future field before its version.
        check_declared_version(bytes, LESSON_SCHEMA_VERSION)?;
        let lesson: AuthoredLesson = serde_json::from_slice(bytes)?;
        lesson.validate()
    }

    /// The accepted schema version this lesson declared.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// The stable identity of this lesson.
    pub fn lesson_id(&self) -> &str {
        &self.authored.lesson_id
    }

    /// The language this lesson is spoken in, checked and case-normalized.
    pub fn language(&self) -> &LanguageTag {
        &self.language
    }

    /// The human-readable lesson title.
    pub fn title(&self) -> &str {
        &self.authored.title
    }

    /// The validated segments in speaking order.
    pub fn segments(&self) -> &[LessonSegment] {
        &self.authored.segments
    }
}

/// Applies the lesson-identifier rules to a value that did not arrive inside an
/// [`AuthoredLesson`].
///
/// Production manifests reuse this boundary because the identifier names the
/// same output directory there.
///
/// # Errors
///
/// [`LessonError::MissingLessonId`] when the value is blank and
/// [`LessonError::InvalidLessonId`] when it is present but could not name a
/// directory.
pub fn validate_lesson_id(lesson_id: &str) -> Result<(), LessonError> {
    if lesson_id.trim().is_empty() {
        return Err(LessonError::MissingLessonId);
    }
    if !is_portable_id(lesson_id) {
        return Err(LessonError::InvalidLessonId(lesson_id.to_owned()));
    }
    Ok(())
}

/// Applies the segment-identity rule shared by every boundary that names one.
///
/// Takes documents reuse this boundary so they cannot approve an identity no
/// lesson can carry.
///
/// # Errors
///
/// [`LessonError::MissingSegmentId`] when the value is blank and
/// [`LessonError::InvalidSegmentId`] when it is present but could not safely
/// name a path component.
pub fn validate_segment_id(segment_id: &str) -> Result<(), LessonError> {
    if segment_id.trim().is_empty() {
        return Err(LessonError::MissingSegmentId);
    }
    if !is_portable_id(segment_id) {
        return Err(LessonError::InvalidSegmentId(segment_id.to_owned()));
    }
    Ok(())
}

/// Publishes the accepted spellings of a lesson document's version.
///
/// Derived from [`LESSON_SCHEMA_VERSION`] so schema and parser accept the same
/// finite version set.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::accepted_versions_json_schema(LESSON_SCHEMA_VERSION)
}

/// Publishes the one link a document of this major may carry.
fn schema_link_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::schema_link_json_schema(LESSON_SCHEMA_STEM, LESSON_SCHEMA_VERSION)
}

/// Publishes [`is_portable_id`] in the form an author's editor can apply.
pub(crate) fn portable_id_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "minLength": 1,
        "maxLength": MAX_IDENTIFIER_LENGTH,
        "pattern": PORTABLE_ID_PATTERN,
    })
}

/// Publishes the BCP 47 shape [`LanguageTag`] parses, to the extent one pattern
/// can carry it.
///
/// Deliberately looser than the parser so an editor never rejects a tag the
/// build accepts; it still catches common separator and subtag errors.
fn language_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "maxLength": crate::MAX_LANGUAGE_TAG_BYTES,
        "pattern": r"^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$(?![\s\S])",
    })
}

/// Rejecting a leading dot covers hidden names, `.`, and `..`. The ASCII rule
/// makes the byte length equal the character length.
///
/// [`PORTABLE_ID_PATTERN`] publishes this same rule to an author's editor, and
/// names this function in return.
fn is_portable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_LENGTH
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture() -> Value {
        serde_json::from_slice(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture JSON should parse")
    }

    fn parse_lesson(value: &Value) -> Result<ValidatedLesson, LessonError> {
        ValidatedLesson::from_json(
            &serde_json::to_vec(value).expect("test lesson should serialize"),
        )
    }

    fn authored_fixture() -> AuthoredLesson {
        serde_json::from_value(fixture()).expect("fixture shape should deserialize")
    }

    #[test]
    fn t1_e1_a_lesson_without_a_usable_language_is_rejected() {
        for unusable in ["", "   ", "en--US", "en_US", "en-x-private"] {
            let mut lesson = fixture();
            lesson["language"] = Value::String(unusable.to_owned());

            assert!(
                matches!(
                    parse_lesson(&lesson),
                    Err(LessonError::MalformedLanguage(_))
                ),
                "language `{unusable}` must be refused"
            );
        }

        let mut omitted = fixture();
        omitted
            .as_object_mut()
            .expect("the fixture is an object")
            .remove("language");
        assert!(matches!(
            parse_lesson(&omitted),
            Err(LessonError::InvalidJson(_))
        ));
    }

    #[test]
    fn t1_e1_a_lesson_language_is_case_normalized_before_it_reaches_a_key() {
        let mut authored = fixture();
        authored["language"] = Value::String("EN-us".to_owned());

        let lesson = parse_lesson(&authored).expect("a valid tag in any casing validates");

        assert_eq!(lesson.language().as_str(), "en-US");
    }

    #[test]
    fn t1_e1_a_lesson_of_a_different_major_version_is_rejected() {
        for declared in ["2.0", "0.1", "0.1-skeleton"] {
            let mut lesson = fixture();
            lesson["schema_version"] = Value::String(declared.to_owned());
            lesson
                .as_object_mut()
                .expect("the fixture is an object")
                .remove("$schema");

            assert!(
                matches!(
                    parse_lesson(&lesson),
                    Err(LessonError::UnsupportedSchema(_))
                ),
                "schema version `{declared}` must be refused"
            );
        }
    }

    #[test]
    fn t1_e1_a_lesson_version_is_read_before_the_fields_that_version_added() {
        for declared in ["2.0", "1.2"] {
            let mut future = fixture();
            future["schema_version"] = Value::String(declared.to_owned());
            future["narrator_hint"] = Value::String("a field a later version added".to_owned());
            future
                .as_object_mut()
                .expect("the fixture is an object")
                .remove("$schema");

            assert!(
                matches!(
                    parse_lesson(&future),
                    Err(LessonError::UnsupportedSchema(_))
                ),
                "version `{declared}` must be refused as a version, not as its new field"
            );
        }
    }

    #[test]
    fn t1_e1_a_lesson_from_an_earlier_minor_version_is_accepted() {
        let mut prior = fixture();
        prior["schema_version"] = Value::String("1.0".to_owned());
        prior
            .as_object_mut()
            .expect("the fixture is an object")
            .remove("$schema");

        let lesson = parse_lesson(&prior).expect("an earlier minor version must be accepted");

        assert_eq!(lesson.schema_version(), SchemaVersion::new(1, 0));
    }

    #[test]
    fn t1_e1_a_lesson_link_must_name_the_schema_for_its_own_version() {
        let mut wrong = fixture();
        wrong["$schema"] = Value::String(schema_uri("takes", 1));

        assert!(matches!(
            parse_lesson(&wrong),
            Err(LessonError::UnexpectedSchemaLink { .. })
        ));
    }

    #[test]
    fn t1_e0_valid_lesson_parses() {
        let lesson = parse_lesson(&fixture()).expect("reviewed fixture should validate");
        assert_eq!(lesson.schema_version(), LESSON_SCHEMA_VERSION);
        assert_eq!(lesson.language().as_str(), "en");
        assert_eq!(lesson.lesson_id(), "e0-s0-walking-skeleton");
        assert_eq!(lesson.title(), "Walking Skeleton");
        assert_eq!(lesson.segments().len(), 2);
    }

    #[test]
    fn t3_e0_authored_lesson_serialization_preserves_the_fixture_shape() {
        let expected = fixture();
        let authored: AuthoredLesson =
            serde_json::from_value(expected.clone()).expect("fixture shape should deserialize");

        assert_eq!(
            serde_json::to_value(authored).expect("authored lesson should serialize"),
            expected
        );
    }

    #[test]
    fn t1_e0_programmatically_authored_unapproved_lesson_is_rejected() {
        let authored = AuthoredLesson {
            schema: None,
            schema_version: LESSON_SCHEMA_VERSION.to_string(),
            lesson_id: "unapproved".to_owned(),
            title: "Unapproved".to_owned(),
            language: "en".to_owned(),
            segments: vec![LessonSegment {
                id: "seg-0001".to_owned(),
                speaker: "nadia".to_owned(),
                role: "explanation".to_owned(),
                source_refs: vec!["block-001".to_owned()],
                display_text: "Review this first.".to_owned(),
                spoken_text: "Review this first.".to_owned(),
                style: "calm".to_owned(),
                pause_after_ms: 0,
                review_status: ReviewStatus::NeedsReview,
            }],
        };

        assert!(matches!(
            authored.validate(),
            Err(LessonError::UnapprovedSegment(id)) if id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e0_duplicate_segment_id_is_rejected() {
        let bytes = br#"{
            "schema_version":"1.1",
            "lesson_id":"duplicate",
            "title":"Duplicate",
            "language":"en",
            "segments":[
                {
                    "id":"seg-1","speaker":"nadia","role":"explanation",
                    "source_refs":["block-1"],"display_text":"one","spoken_text":"one",
                    "style":"calm","pause_after_ms":0,"review_status":"approved"
                },
                {
                    "id":"seg-1","speaker":"tom","role":"recap",
                    "source_refs":["block-2"],"display_text":"two","spoken_text":"two",
                    "style":"calm","pause_after_ms":0,"review_status":"approved"
                }
            ]
        }"#;

        assert!(matches!(
            ValidatedLesson::from_json(bytes),
            Err(LessonError::DuplicateSegmentId(id)) if id == "seg-1"
        ));
    }

    #[test]
    fn t1_e0_unapproved_segment_is_rejected() {
        let mut value = fixture();
        value["segments"][0]["review_status"] = Value::String("needs_review".to_owned());

        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::UnapprovedSegment(id)) if id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e0_review_context_invariants_have_distinct_errors() {
        let mut value = fixture();
        value["segments"][0]["display_text"] = Value::String(String::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::MissingDisplayText(_))
        ));

        let mut value = fixture();
        value["segments"][0]["role"] = Value::String(String::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::MissingRole(_))
        ));

        let mut value = fixture();
        value["segments"][0]["source_refs"] = Value::Array(Vec::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::MissingSourceRefs(_))
        ));

        let mut value = fixture();
        value["segments"][0]["source_refs"] = Value::Array(vec![Value::String(String::new())]);
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::EmptySourceRef(_))
        ));
    }

    #[test]
    fn t1_e0_synthesis_selection_invariants_have_distinct_errors() {
        let mut value = fixture();
        value["segments"][0]["speaker"] = Value::String(String::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::MissingSpeaker(_))
        ));

        let mut value = fixture();
        value["segments"][0]["style"] = Value::String(String::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::MissingStyle(_))
        ));

        let mut value = fixture();
        value["segments"][0]["review_status"] = Value::String("aproved".to_owned());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::InvalidJson(_))
        ));
    }

    #[test]
    fn t1_e0_empty_identifiers_are_reported_as_missing_not_malformed() {
        for absent in ["", "   "] {
            let mut value = fixture();
            value["lesson_id"] = Value::String(absent.to_owned());
            assert!(
                matches!(parse_lesson(&value), Err(LessonError::MissingLessonId)),
                "lesson_id `{absent}` must be reported as missing"
            );

            let mut value = fixture();
            value["segments"][0]["id"] = Value::String(absent.to_owned());
            assert!(
                matches!(parse_lesson(&value), Err(LessonError::MissingSegmentId)),
                "segment ID `{absent}` must be reported as missing"
            );
        }
    }

    #[test]
    fn t1_e0_non_portable_lesson_and_segment_ids_are_rejected() {
        let rejected = [
            ".".to_owned(),
            "..".to_owned(),
            "...".to_owned(),
            ".hidden".to_owned(),
            "../escape".to_owned(),
            "/tmp/escape".to_owned(),
            r"..\escape".to_owned(),
            "with space".to_owned(),
            "über".to_owned(),
            "x".repeat(MAX_IDENTIFIER_LENGTH + 1),
        ];

        for unsafe_id in rejected {
            let mut value = fixture();
            value["lesson_id"] = Value::String(unsafe_id.clone());
            assert!(
                matches!(parse_lesson(&value), Err(LessonError::InvalidLessonId(_))),
                "lesson_id `{unsafe_id}` must be rejected"
            );

            let mut value = fixture();
            value["segments"][0]["id"] = Value::String(unsafe_id.clone());
            assert!(
                matches!(parse_lesson(&value), Err(LessonError::InvalidSegmentId(_))),
                "segment ID `{unsafe_id}` must be rejected"
            );
        }
    }

    #[test]
    fn t1_e0_portable_ids_at_the_length_bound_are_accepted() {
        // `lesson.v1` is pinned deliberately: interior dots stay legal, so a
        // later attempt to reject every dot would fail here rather than
        // silently breaking versioned identifiers.
        let accepted = [
            "a".to_owned(),
            "seg-0001".to_owned(),
            "e0_s0".to_owned(),
            "lesson.v1".to_owned(),
            "x".repeat(MAX_IDENTIFIER_LENGTH),
        ];

        for safe_id in accepted {
            let mut value = fixture();
            value["lesson_id"] = Value::String(safe_id.clone());
            assert!(
                parse_lesson(&value).is_ok(),
                "lesson_id `{safe_id}` must be accepted"
            );
        }
    }

    #[test]
    fn t1_e0_lesson_json_byte_limit_accepts_the_boundary_and_precedes_parsing() {
        let mut exact = fixture();
        exact["title"] = Value::String(String::new());
        let baseline = serde_json::to_vec(&exact)
            .expect("test lesson should serialize")
            .len();
        exact["title"] = Value::String("x".repeat(MAX_LESSON_JSON_BYTES - baseline));
        let exact_bytes = serde_json::to_vec(&exact).expect("boundary lesson should serialize");
        assert_eq!(exact_bytes.len(), MAX_LESSON_JSON_BYTES);
        ValidatedLesson::from_json(&exact_bytes).expect("the byte boundary must be accepted");

        let oversized = vec![b'{'; MAX_LESSON_JSON_BYTES + 1];
        assert!(matches!(
            ValidatedLesson::from_json(&oversized),
            Err(LessonError::LessonJsonTooLarge { max_bytes })
                if max_bytes == MAX_LESSON_JSON_BYTES
        ));
    }

    #[test]
    fn t1_e0_segment_count_limit_accepts_the_boundary_and_rejects_one_more() {
        let segment = authored_fixture().segments.remove(0);
        let segments = (0..MAX_LESSON_SEGMENTS)
            .map(|index| LessonSegment {
                id: format!("seg-{index}"),
                ..segment.clone()
            })
            .collect::<Vec<_>>();
        let mut authored = authored_fixture();
        authored.segments = segments.clone();
        authored
            .validate()
            .expect("the segment-count boundary must be accepted");

        let mut authored = authored_fixture();
        authored.segments = segments;
        authored.segments.push(LessonSegment {
            id: "one-too-many".to_owned(),
            ..segment
        });
        assert!(matches!(
            authored.validate(),
            Err(LessonError::TooManySegments { found, max })
                if found == MAX_LESSON_SEGMENTS + 1 && max == MAX_LESSON_SEGMENTS
        ));
    }

    #[test]
    fn t1_e0_spoken_text_limit_counts_utf8_bytes() {
        let mut exact = authored_fixture();
        exact.segments[0].spoken_text = "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2);
        exact
            .validate()
            .expect("the spoken-text byte boundary must be accepted");

        let mut oversized = authored_fixture();
        oversized.segments[0].spoken_text = format!("{}a", "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2));
        assert!(matches!(
            oversized.validate(),
            Err(LessonError::SpokenTextTooLong { segment_id, bytes, max_bytes })
                if segment_id == "seg-0001"
                    && bytes == MAX_SEGMENT_TEXT_BYTES + 1
                    && max_bytes == MAX_SEGMENT_TEXT_BYTES
        ));
    }

    #[test]
    fn t1_e0_display_text_limit_counts_utf8_bytes() {
        let mut exact = authored_fixture();
        exact.segments[0].display_text = "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2);
        exact
            .validate()
            .expect("the display-text byte boundary must be accepted");

        let mut oversized = authored_fixture();
        oversized.segments[0].display_text = format!("{}a", "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2));
        assert!(matches!(
            oversized.validate(),
            Err(LessonError::DisplayTextTooLong { segment_id, bytes, max_bytes })
                if segment_id == "seg-0001"
                    && bytes == MAX_SEGMENT_TEXT_BYTES + 1
                    && max_bytes == MAX_SEGMENT_TEXT_BYTES
        ));
    }

    #[test]
    fn t1_e0_source_reference_limits_accept_boundaries_and_count_utf8_bytes() {
        let mut exact_count = authored_fixture();
        exact_count.segments[0].source_refs = vec!["x".to_owned(); MAX_SOURCE_REFS_PER_SEGMENT];
        exact_count
            .validate()
            .expect("the source-reference count boundary must be accepted");

        let mut oversized_count = authored_fixture();
        oversized_count.segments[0].source_refs =
            vec!["x".to_owned(); MAX_SOURCE_REFS_PER_SEGMENT + 1];
        assert!(matches!(
            oversized_count.validate(),
            Err(LessonError::TooManySourceRefs { segment_id, found, max })
                if segment_id == "seg-0001"
                    && found == MAX_SOURCE_REFS_PER_SEGMENT + 1
                    && max == MAX_SOURCE_REFS_PER_SEGMENT
        ));

        let mut exact_length = authored_fixture();
        exact_length.segments[0].source_refs = vec!["é".repeat(MAX_SOURCE_REF_BYTES / 2)];
        exact_length
            .validate()
            .expect("the source-reference byte boundary must be accepted");

        let mut oversized_length = authored_fixture();
        oversized_length.segments[0].source_refs =
            vec![format!("{}a", "é".repeat(MAX_SOURCE_REF_BYTES / 2))];
        assert!(matches!(
            oversized_length.validate(),
            Err(LessonError::SourceRefTooLong {
                segment_id,
                bytes,
                max_bytes,
            }) if segment_id == "seg-0001"
                && bytes == MAX_SOURCE_REF_BYTES + 1
                && max_bytes == MAX_SOURCE_REF_BYTES
        ));
    }

    #[test]
    fn t1_e0_programmatic_authored_text_limit_accepts_the_boundary() {
        const OTHER_AUTHORED_BYTES: usize = 7;
        let lesson_with_title = |title_bytes| AuthoredLesson {
            schema: None,
            schema_version: LESSON_SCHEMA_VERSION.to_string(),
            lesson_id: "aggregate-boundary".to_owned(),
            title: "t".repeat(title_bytes),
            language: "en".to_owned(),
            segments: vec![LessonSegment {
                id: "i".to_owned(),
                speaker: "s".to_owned(),
                role: "r".to_owned(),
                source_refs: vec!["x".to_owned()],
                display_text: "d".to_owned(),
                spoken_text: "p".to_owned(),
                style: "y".to_owned(),
                pause_after_ms: 0,
                review_status: ReviewStatus::Approved,
            }],
        };

        lesson_with_title(MAX_AUTHORED_TEXT_BYTES - OTHER_AUTHORED_BYTES)
            .validate()
            .expect("the aggregate authored-text boundary must be accepted");
        assert!(matches!(
            lesson_with_title(MAX_AUTHORED_TEXT_BYTES - OTHER_AUTHORED_BYTES + 1).validate(),
            Err(LessonError::AuthoredTextTooLarge { max_bytes })
                if max_bytes == MAX_AUTHORED_TEXT_BYTES
        ));
    }
}
