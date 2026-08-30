//! Authored lesson documents and their pre-planning validation boundary.
//!
//! Absent and malformed input remain distinct so authors receive the right
//! remedy before synthesis can start.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{IgnoredAny, MapAccess, Visitor},
};
use thiserror::Error;

use crate::{
    LanguageTag, MalformedLanguageTag, SchemaVersion, SchemaVersionError,
    digest::{blake3_newtype, json_schema_as_string},
    schema::check_declared_version,
    schema_uri,
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

// The four lesson-level ceilings below were added with the `3.1` document
// fields they bound, and are mirrored into the same table.
const MAX_LEARNING_OBJECTIVES: usize = 64;
const MAX_LEARNING_OBJECTIVE_BYTES: usize = 4 * 1024;
const MAX_LESSON_REFERENCES: usize = 256;
const MAX_LESSON_REFERENCE_BYTES: usize = 4 * 1024;

/// Longest trailing pause a segment may declare, in milliseconds.
///
/// Long enough for a deliberate beat, but short enough to reject a value that
/// would sound like an audio fault.
const MAX_PAUSE_AFTER_MS: u32 = 10_000;

/// Shortest silence a recall prompt may leave the listener to answer in, in
/// milliseconds.
///
/// ADR-0001 §8.2 makes "recall prompts include a deliberate response interval"
/// an invariant rather than a default, and §13.2's pause table gives a recall
/// question 1.5-4 seconds. This is that range's floor; a prompt below it gives
/// the listener nothing to answer in.
const MIN_RECALL_RESPONSE_MS: u32 = 1_500;

/// Longest silence a recall prompt may leave, in milliseconds.
///
/// The ceiling of the same ADR-0001 §13.2 range, enforced for the reason the
/// floor is. §8.2 admits "pause values remain within policy unless an override
/// is annotated" -- but the lesson format declares no override annotation, so
/// there is no way for an author to be outside policy on purpose, and every
/// value above this one is outside policy by accident. Enforcing the generic
/// [`MAX_PAUSE_AFTER_MS`] here instead would accept 4,001-10,000 ms as though
/// §13.2 had not been written. When an override annotation lands, this becomes
/// the bound it lifts rather than a bound to delete.
const MAX_RECALL_RESPONSE_MS: u32 = 4_000;

/// Layout version this build publishes for a lesson document.
///
/// Version `1.0` made synthesis-key language required and `1.1` added the
/// optional `$schema` link. Version `2.0` made [`AuthoredLesson::speakers`]
/// required, which is a breaking change because a `1.x` document declares no
/// voice at all; `2.1` added the optional [`LessonSegment::editorial`] flag.
/// Version `3.0` closed the [`LessonSegment::role`] and
/// [`LessonSegment::style`] vocabularies, which is breaking because a `2.x`
/// document may name anything at all in either; `3.1` added the optional
/// [`AuthoredLesson::learning_objectives`] and [`AuthoredLesson::source`] that
/// ADR-0001 §8.1's canonical format carries. The change classes and history
/// are recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`,
/// `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`, and
/// `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`.
pub const LESSON_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(3, 1);

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
    schema: Option<String>,
    /// Schema this document claims; an unrecognized version is refused rather
    /// than guessed at.
    ///
    /// Held as authored text rather than a [`SchemaVersion`] so a malformed
    /// version is reported by [`LessonError::UnsupportedSchema`] naming what
    /// was written, instead of by a serde message about a field type.
    #[schemars(schema_with = "schema_version_json_schema")]
    schema_version: String,
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
    ///
    /// The published `pattern` is necessary but not sufficient: `LanguageTag`
    /// is the authority, and `language_json_schema` spells its grammar loosely
    /// on purpose so an author's editor never rejects a tag this build accepts.
    /// A tag the schema admits may still be refused by
    /// [`ValidatedLesson::from_json`].
    #[schemars(schema_with = "language_json_schema")]
    pub language: String,
    /// What a listener should be able to do after the lesson.
    ///
    /// Added by lesson `3.1` as a compatible extension whose declared default
    /// is empty, so a `3.0` document stays valid. ADR-0001 §8.1 carries it in
    /// the canonical format; §8.2 sets no invariant on it beyond the count,
    /// blank-value, and length bounds validation applies, so it is review
    /// context rather than a gate.
    ///
    /// Display-only. Excluded from every identity, alongside the other review
    /// metadata listed on [`crate::SynthesisContext`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub learning_objectives: Vec<String>,
    /// Where the lesson's material came from.
    ///
    /// Added by lesson `3.1` on the same terms as
    /// [`AuthoredLesson::learning_objectives`]. Absent rather than empty when
    /// a lesson was not compiled from a hashed document: a lesson that names
    /// no source and one that names a source with nothing in it are different
    /// claims, and only the second is a provenance record.
    ///
    /// Display-only, for the reason `learning_objectives` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<LessonSource>,
    /// Voice binding for every speaker the lesson gives lines to.
    ///
    /// Required since lesson `2.0`. Without it a segment's `speaker` is a
    /// label nothing resolves, and ADR-0001 §8.2 requires a declared voice
    /// profile; the resolved conditioning artifact is a synthesis-key input
    /// under §12.5, so leaving it undeclared would key audio on a voice the
    /// document never named.
    ///
    /// A [`BTreeMap`] rather than a `HashMap` because this document's
    /// serialized form must not depend on authoring order.
    pub speakers: BTreeMap<String, SpeakerDeclaration>,
    /// The lesson's segments in speaking order.
    pub segments: Vec<LessonSegment>,
}

/// The voice one speaker is rendered with.
///
/// A struct rather than a bare profile string so a later story can add a
/// speaker-scoped setting without moving every lesson to a new major.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SpeakerDeclaration {
    /// Voice profile this speaker is rendered with.
    ///
    /// Portable because `study-tts-runtime` resolves it to a directory under
    /// the build's voice-profile root.
    #[schemars(schema_with = "portable_id_json_schema")]
    pub voice_profile: String,
}

/// Provenance of the material one lesson was written from.
///
/// A record rather than a bare digest because ADR-0001 §8.1 carries both the
/// hash and the references beside it, and a hash alone cannot be traced by a
/// reviewer who does not already hold the document it names.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct LessonSource {
    /// Digest of the reviewed source document this lesson was compiled from.
    ///
    /// Required whenever [`AuthoredLesson::source`] is present: a source
    /// record whose content hash is optional records a claim nothing can be
    /// checked against.
    pub content_hash: SourceContentHash,
    /// External references the source material cites.
    ///
    /// Optional with an empty default, because source material that cites
    /// nothing is ordinary rather than an authoring mistake. Unlike
    /// [`LessonSegment::source_refs`], these name material outside the lesson
    /// rather than blocks inside its own source document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
}

/// Identity of the source document a lesson was compiled from.
///
/// A value object rather than a `String` for the reason every other digest
/// here is one: a recorded identity a caller can set to anything records
/// nothing. Display-only — ADR-0001 §12.5 keys synthesis on `spoken_text`, and
/// recompiling a lesson from an edited source is what changes that text.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceContentHash(String);

impl SourceContentHash {
    /// The digest as it is written in a lesson document.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(SourceContentHash, MalformedSourceContentHash);

/// Remedy routing: the hash is produced by compiling the source document, so
/// the message names recompiling rather than editing the recorded value.
#[derive(Debug, Error)]
#[error(
    "source content hash `{0}` is not a BLAKE3 digest in lowercase hexadecimal; ADR-0001 §8.1 \
     records it so a lesson can be traced back to the reviewed document it was compiled from; \
     recompile the lesson from that document rather than editing the recorded value"
)]
pub struct MalformedSourceContentHash(String);

json_schema_as_string!(
    SourceContentHash,
    "SourceContentHash",
    "BLAKE3 over the reviewed source document a lesson was compiled from \
     (ADR-0001 8.1), as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

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
    /// What this segment is doing in the lesson.
    ///
    /// Closed since lesson `3.0`; see [`SegmentRole`] for why the vocabulary
    /// cannot be authored text. Review context rather than a synthesis input,
    /// with one behavioral consequence:
    /// [`SegmentRole::RecallPrompt`] carries the response-interval invariant
    /// [`LessonError::RecallPromptWithoutResponseInterval`] enforces.
    pub role: SegmentRole,
    /// Source material this segment was written from, so a claim can be traced
    /// back.
    pub source_refs: Vec<String>,
    /// Text as a reviewer reads it; display only, and outside the cache key.
    pub display_text: String,
    /// Text as it is spoken, which is what synthesis and the cache key are
    /// derived from.
    pub spoken_text: String,
    /// Delivery requested of the voice.
    ///
    /// Closed since lesson `3.0`; see [`DeliveryStyle`]. Unlike the role this
    /// is a synthesis-key input (ADR-0001 §12.5), and
    /// [`DeliveryStyle::as_str`] is the spelling that reaches the key.
    pub style: DeliveryStyle,
    /// Silence written after this segment, in milliseconds.
    pub pause_after_ms: u32,
    /// Whether a human has approved this segment for synthesis.
    pub review_status: ReviewStatus,
    /// Whether this segment is the author's own words rather than a claim
    /// traced to source material.
    ///
    /// Added by lesson `2.1` as a compatible extension whose declared default
    /// is `false`, so a `2.0` document stays valid. It spells the second half
    /// of ADR-0001 §8.2's rule that every segment "references source material
    /// or is explicitly marked editorial": `source_refs` may be empty only
    /// here.
    ///
    /// Display-only. Excluded from every identity, alongside the other review
    /// metadata listed on [`crate::SynthesisContext`].
    #[serde(default, skip_serializing_if = "is_unset")]
    pub editorial: bool,
}

/// Keeps an unset [`LessonSegment::editorial`] out of the serialized document,
/// so a `2.0` lesson round-trips through this type unchanged.
fn is_unset(editorial: &bool) -> bool {
    !*editorial
}

/// What one segment is doing in the lesson.
///
/// A closed vocabulary rather than authored text for two reasons. ADR-0001
/// §8.2 requires the role to be *declared*, and a free string declares nothing
/// a build can act on. And §8.2's recall-prompt invariant is not expressible
/// at all until a build can tell which segments are recall prompts:
/// [`SegmentRole::RecallPrompt`] is what
/// [`LessonError::RecallPromptWithoutResponseInterval`] keys on.
///
/// The variants are ADR-0001 §3.2's two speaker repertoires and §3.4's default
/// study sequence, which name the same vocabulary twice. A lesson need not use
/// every one; §3.2 is explicit that the program "will not add dialogue merely
/// to satisfy a template".
///
/// Display-only. Excluded from every identity, alongside the other review
/// metadata listed on [`crate::SynthesisContext`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SegmentRole {
    /// The question or difficulty the lesson opens on.
    Problem,
    /// Knowledge the listener needs before the explanation can land.
    Prerequisite,
    /// A term stated precisely.
    Definition,
    /// Conceptual explanation of how something works.
    Explanation,
    /// Why the explanation holds rather than merely that it does.
    WhyItWorks,
    /// A concrete worked case.
    Example,
    /// An objection or question the learner raises.
    Challenge,
    /// A mistake stated deliberately so it can be corrected.
    PlausibleError,
    /// The learner cutting in before an explanation finishes.
    Interruption,
    /// A narrower restatement the learner asked for.
    Clarification,
    /// The correction of a challenge or a plausible error.
    Correction,
    /// Algorithm steps spoken rather than read out as code.
    Pseudocode,
    /// The separately explained parts drawn back together.
    Synthesis,
    /// A concise recap of what has been covered.
    Recap,
    /// The compressed rule worth memorizing.
    CompressedRule,
    /// A retrieval-practice question, which must leave a response interval.
    RecallPrompt,
    /// The answer given after a recall prompt's silence.
    Answer,
}

/// The delivery one segment asks of its voice.
///
/// Closed for the reason [`SegmentRole`] is, plus one this vocabulary carries
/// alone: ADR-0001 §13.4 freezes one loudness reference per voice-profile hash
/// and style, and forbids a new style entering production until it has been
/// calibrated. An open field would let a lesson name a style with no frozen
/// reference behind it, which is a level decision nobody made.
///
/// Transcribed from ADR-0001 §8.1's canonical lesson and §5.1's qualification
/// fixture, which requires "calm, emphatic, and deliberately slow delivery".
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStyle {
    /// Even, unhurried delivery.
    Calm,
    /// The teaching register of ADR-0001 §8.1's canonical lesson.
    CalmExplanatory,
    /// Weighted delivery for a point that has to land.
    Emphatic,
    /// Deliberately slowed delivery.
    Deliberate,
}

impl DeliveryStyle {
    /// The wire spelling, which is also the bytes that reach a synthesis key.
    ///
    /// Written out rather than derived from `Serialize` so the key does not
    /// depend on a serializer being available where it is computed;
    /// `t1_e1_delivery_style_spelling_matches_its_serde_form` is what holds
    /// the two together.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::CalmExplanatory => "calm_explanatory",
            Self::Emphatic => "emphatic",
            Self::Deliberate => "deliberate",
        }
    }
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
/// Each invariant has a distinct variant, and absent and malformed values
/// remain separate because their remedies differ. That includes the three
/// closed vocabularies ADR-0001 §8.2 declares: `serde` refuses them before this
/// module sees a value, so `vocabulary_refusal` classifies each refusal back
/// into its own variant on the refusal path — absent into
/// [`LessonError::MissingSegmentRole`] and its two siblings, unrecognized into
/// [`LessonError::UnknownSegmentRole`] and its two.
///
/// What remains under [`LessonError::InvalidJson`] is the document's *shape*:
/// a field of the wrong type, a field no version declares, an omitted field
/// outside those three, bytes that are not JSON at all. That is one invariant —
/// the document does not have the form the published schema declares — however
/// many fields can violate it, and each violation is located by the pointer
/// [`LessonDiagnostic::field_path`] carries.
/// `t1_e1_each_lesson_invariant_has_a_distinct_error` holds both halves: it
/// exercises all three vocabularies in all three forms each can fail in, and
/// asserts that no two invariants produce the same variant and pointer.
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
    /// The recorded source digest is not a BLAKE3 digest.
    ///
    /// Its own invariant rather than a shape refusal, for the reason the three
    /// closed vocabularies have theirs: `SourceContentHash` refuses the value
    /// at parse time and `serde` would deliver that refusal as opaque prose,
    /// leaving a hash that is not a digest indistinguishable from a
    /// `content_hash` that is not a string at all. The remedy differs — the
    /// first is recompiled from the source document, the second is a document
    /// that does not have the declared shape.
    #[error("lesson source hash is unusable: {0}")]
    MalformedSourceContentHash(#[from] MalformedSourceContentHash),
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
    /// The segment cites no source and is not marked editorial, so its claims
    /// cannot be traced back.
    #[error(
        "segment `{0}` must contain at least one source reference, or be marked `editorial` if \
         it is the author's own words"
    )]
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
    /// A segment names a role outside the closed vocabulary.
    ///
    /// Its own variant rather than a shape refusal because ADR-0001 §8.2 makes
    /// the declared role an invariant of the lesson format: the remedy is one
    /// value to correct, where [`LessonError::InvalidJson`] is a structure to
    /// correct.
    #[error(
        "segment role `{0}` is outside the vocabulary the lesson format declares; the \
         document's author must name a role the published lesson schema lists"
    )]
    UnknownSegmentRole(String),
    /// A segment asks for a delivery outside the closed vocabulary.
    ///
    /// Separate from [`LessonError::UnknownSegmentRole`] because the remedies
    /// differ: ADR-0001 §13.4 admits a style only once it has a frozen
    /// loudness reference, so correcting one is a calibration decision rather
    /// than a spelling.
    #[error(
        "segment style `{0}` is outside the vocabulary the lesson format declares; the \
         document's author must name a style the published lesson schema lists, which ADR-0001 \
         §13.4 admits only once it has a frozen loudness reference"
    )]
    UnknownDeliveryStyle(String),
    /// A segment declares a review state outside the closed vocabulary.
    ///
    /// Separate from [`LessonError::UnapprovedSegment`] because that one is a
    /// state this build understands and refuses to synthesize, where this one
    /// is a state nothing can interpret at all.
    #[error(
        "segment review_status `{0}` is outside the vocabulary the lesson format declares; the \
         document's author must name a review state the published lesson schema lists"
    )]
    UnknownReviewStatus(String),
    /// A segment declares no role at all.
    ///
    /// Separate from [`LessonError::UnknownSegmentRole`] because the remedies
    /// differ, which is the rule this enum is written to: an absent role is a
    /// field to add, where an unrecognized one is a value to correct. ADR-0001
    /// §8.2 requires the role to be *declared*, so neither is a default this
    /// build may supply.
    #[error(
        "a segment declares no role; the document's author must add a `role` naming one of the \
         values the published lesson schema lists"
    )]
    MissingSegmentRole,
    /// A segment declares no delivery style at all.
    ///
    /// Separate from [`LessonError::UnknownDeliveryStyle`] for the reason
    /// [`LessonError::MissingSegmentRole`] is separate from its own vocabulary
    /// refusal.
    #[error(
        "a segment declares no style; the document's author must add a `style` naming one of the \
         values the published lesson schema lists"
    )]
    MissingDeliveryStyle,
    /// A segment declares no review state at all.
    ///
    /// Separate from [`LessonError::UnapprovedSegment`], which is a state a
    /// human recorded and this build refuses to synthesize: an absent review
    /// state records no decision at all, so nothing was reviewed.
    #[error(
        "a segment declares no review_status; the document's author must add a `review_status` \
         naming one of the values the published lesson schema lists"
    )]
    MissingReviewStatus,
    /// No human approved this segment; synthesis accepts only
    /// `ReviewStatus::Approved`.
    #[error("segment `{0}` is not approved for synthesis")]
    UnapprovedSegment(String),
    /// No voice was named, so no voice profile can be resolved.
    #[error("segment `{0}` must declare a speaker")]
    MissingSpeaker(String),
    /// The segment speaks as somebody the lesson never bound to a voice.
    ///
    /// Separate from [`LessonError::MissingSpeaker`] because the remedies
    /// differ: this document named a speaker, it just never said who that is.
    #[error(
        "segment `{segment_id}` speaks as `{speaker}`, which `speakers` does not declare; the \
         document's author must bind that speaker to a voice profile or correct the segment"
    )]
    UndeclaredSpeaker {
        /// Segment naming the unbound speaker.
        segment_id: String,
        /// Speaker the segment named.
        speaker: String,
    },
    /// A speaker is declared with no voice profile at all.
    #[error("speaker `{0}` must declare a voice_profile")]
    MissingVoiceProfile(String),
    /// A voice profile was named but could not safely name a directory.
    ///
    /// It resolves to a directory beneath the build's voice-profile root, so
    /// it carries the same portability rule as a lesson or segment identity.
    #[error(
        "speaker `{speaker}` declares voice_profile `{voice_profile}`, which must be 1-{max} \
         ASCII letters, digits, hyphen, underscore, or dot, and must not start with a dot, \
         because it names a voice-profile directory",
        max = MAX_IDENTIFIER_LENGTH
    )]
    InvalidVoiceProfile {
        /// Speaker carrying the unusable profile reference.
        speaker: String,
        /// Profile reference as declared.
        voice_profile: String,
    },
    /// One speaker is bound to a voice more than once.
    ///
    /// Its own variant rather than a shape refusal because the document is
    /// well-formed JSON of the declared shape: RFC 8259 leaves a repeated
    /// object name undefined, and a [`BTreeMap`] resolves it by keeping the
    /// last binding. Two bindings naming different profiles therefore select
    /// a voice by parser behavior rather than by review, which ADR-0001 §12.5
    /// makes a synthesis-key input. Separate from
    /// [`LessonError::InvalidVoiceProfile`] because the remedy is a binding to
    /// delete rather than a value to correct.
    #[error(
        "speaker `{0}` is declared more than once; the document's author must leave one binding, \
         because a repeated declaration lets the voice this speaker is rendered with depend on \
         which binding a parser keeps rather than on which one a reviewer approved"
    )]
    DuplicateSpeaker(String),
    /// The pause is long enough to read as a fault in the audio rather than as
    /// phrasing.
    #[error("segment `{0}` pause exceeds the provisional {max} ms limit", max = MAX_PAUSE_AFTER_MS)]
    PauseOutOfRange(String),
    /// A recall prompt leaves the listener no silence to answer in.
    ///
    /// ADR-0001 §8.2 makes the interval an invariant of the format rather than
    /// a pacing default, because a prompt answered over by the next segment is
    /// not retrieval practice.
    #[error(
        "segment `{segment_id}` is a recall prompt with a {pause_after_ms} ms pause, leaving less \
         than the {min_ms} ms response interval ADR-0001 §8.2 requires"
    )]
    RecallPromptWithoutResponseInterval {
        /// Segment carrying the prompt.
        segment_id: String,
        /// Silence the segment declares.
        pause_after_ms: u32,
        /// Shortest interval a recall prompt may leave.
        min_ms: u32,
    },
    /// A recall prompt leaves more silence than the pause policy allows.
    ///
    /// Its own variant rather than a second reading of
    /// [`LessonError::RecallPromptWithoutResponseInterval`] because the
    /// remedies are opposite: that one is answered by lengthening the pause,
    /// this one by shortening it. ADR-0001 §13.2 bounds a recall question at
    /// four seconds and §8.2 admits a pause outside policy only when an
    /// override is annotated, which the lesson format cannot yet express.
    #[error(
        "segment `{segment_id}` is a recall prompt with a {pause_after_ms} ms pause, exceeding \
         the {max_ms} ms response interval ADR-0001 §13.2 allows; the document's author must \
         shorten it, because the lesson format declares no override annotation"
    )]
    RecallPromptResponseIntervalTooLong {
        /// Segment carrying the prompt.
        segment_id: String,
        /// Silence the segment declares.
        pause_after_ms: u32,
        /// Longest interval a recall prompt may leave.
        max_ms: u32,
    },
    /// The lesson states more objectives than this provisional build accepts.
    #[error(
        "lesson declares {found} learning objectives, exceeding the provisional limit of {max}"
    )]
    TooManyLearningObjectives {
        /// Objectives the lesson declares.
        found: usize,
        /// Largest accepted objective count.
        max: usize,
    },
    /// An objective is present but blank, which states nothing.
    #[error("lesson contains an empty learning objective")]
    EmptyLearningObjective,
    /// One objective exceeds its fixed memory ceiling.
    #[error(
        "lesson contains a {bytes}-byte learning objective, exceeding the provisional \
         {max_bytes}-byte limit"
    )]
    LearningObjectiveTooLong {
        /// UTF-8 bytes the objective contains.
        bytes: usize,
        /// Largest accepted objective length.
        max_bytes: usize,
    },
    /// The source record cites more references than this build accepts.
    #[error("lesson source cites {found} references, exceeding the provisional limit of {max}")]
    TooManyLessonReferences {
        /// References the source record cites.
        found: usize,
        /// Largest accepted reference count.
        max: usize,
    },
    /// A source reference is present but blank, which traces to nothing.
    #[error("lesson source contains an empty reference")]
    EmptyLessonReference,
    /// One source reference exceeds its fixed memory ceiling.
    #[error(
        "lesson source contains a {bytes}-byte reference, exceeding the provisional \
         {max_bytes}-byte limit"
    )]
    LessonReferenceTooLong {
        /// UTF-8 bytes the reference contains.
        bytes: usize,
        /// Largest accepted reference length.
        max_bytes: usize,
    },
    /// Authored strings collectively exceed the lesson memory envelope.
    #[error("lesson authored text exceeds the provisional {max_bytes}-byte aggregate limit")]
    AuthoredTextTooLarge {
        /// Largest accepted aggregate authored-text length.
        max_bytes: usize,
    },
}

/// One refused lesson document, located precisely enough to act on.
///
/// ADR-0001 §14 asks a diagnostic to say where a failure happened rather than
/// only what it was, and `DELIVERY-PLAN.md` E1-S2 names the three parts: the
/// document, the segment, and the field. Nothing here quotes a field's
/// *value*, because `AGENTS.md` §Security and data forbids logging source or
/// spoken text by default.
#[derive(Debug)]
pub struct LessonDiagnostic {
    document: String,
    segment_id: Option<String>,
    field_path: String,
    error: LessonError,
}

impl LessonDiagnostic {
    /// Locates a refusal about a document rather than about one of its
    /// segments.
    ///
    /// `study-tts-runtime`'s bounded lesson reader refuses
    /// [`LessonError::LessonJsonTooLarge`] before this module sees the bytes,
    /// and its publication gate applies [`validate_lesson_id`] to a manifest.
    /// Both refusals arrive through here carrying the same location a parsed
    /// one would.
    ///
    /// Boxed because a located refusal is 128 bytes, which
    /// `clippy::result_large_err` refuses in a `Result`;
    /// `study_tts_runtime::BuildError` boxes it for the same reason, so this
    /// is the one allocation rather than a second.
    pub fn about(document: &str, error: LessonError) -> Box<Self> {
        Self::located(document, None, error)
    }

    /// Locates a refusal at a pointer the caller already knows.
    ///
    /// [`ValidatedLesson::from_json`] uses it for a `serde` refusal, whose
    /// path comes from the deserializer rather than from [`field_of`].
    fn at(
        document: &str,
        segment_id: Option<String>,
        field_path: String,
        error: LessonError,
    ) -> Box<Self> {
        Box::new(Self {
            document: document.to_owned(),
            segment_id,
            field_path,
            error,
        })
    }

    /// Locates a refusal in the segment at `index`.
    ///
    /// The index comes from the validation loop, which is the only place that
    /// knows it: a segment names itself by identity, and a JSON Pointer needs
    /// its position.
    fn in_segment(document: &str, index: usize, segment_id: &str, error: LessonError) -> Box<Self> {
        Self::located(document, Some((index, segment_id)), error)
    }

    /// Composes the RFC 6901 pointer for one refusal.
    fn located(document: &str, segment: Option<(usize, &str)>, error: LessonError) -> Box<Self> {
        // `None` means the refusal is about the document as a whole, wherever
        // it was raised: the aggregate text ceiling trips inside the segment
        // loop but is not a fact about that segment, so the segment arm is
        // unreachable for it.
        let (segment_id, field_path) = match (segment, field_of(&error)) {
            (_, None) => (None, String::new()),
            (Some((index, id)), Some(field)) => {
                (Some(id.to_owned()), format!("/segments/{index}/{field}"))
            }
            (None, Some(field)) => (None, format!("/{field}")),
        };
        Box::new(Self {
            document: document.to_owned(),
            segment_id,
            field_path,
            error,
        })
    }

    /// The document this refusal came from, as the caller named it.
    ///
    /// Spelled `document` rather than `source` so it cannot be confused with
    /// [`std::error::Error::source`], which returns the [`LessonError`].
    pub fn document(&self) -> &str {
        &self.document
    }

    /// The segment the refusal is about, when it is about one.
    pub fn segment_id(&self) -> Option<&str> {
        self.segment_id.as_deref()
    }

    /// RFC 6901 JSON Pointer to the offending field.
    ///
    /// Empty for a refusal about the document as a whole, which is what RFC
    /// 6901 defines the empty pointer to mean.
    pub fn field_path(&self) -> &str {
        &self.field_path
    }

    /// The invariant that was violated.
    pub fn error(&self) -> &LessonError {
        &self.error
    }
}

// Written out rather than derived through `thiserror` because the rendering is
// conditional: a whole-document refusal has no pointer to name, and displaying
// an empty location is worse than saying nothing.
impl fmt::Display for LessonDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.field_path.as_str() {
            "" => write!(formatter, "`{}`: {}", self.document, self.error),
            path => write!(formatter, "`{}` at `{path}`: {}", self.document, self.error),
        }
    }
}

impl std::error::Error for LessonDiagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The RFC 6901 pointer each refusal names, relative to the document or to the
/// segment it happened in, or `None` for a whole-document refusal.
///
/// One exhaustive match with no `_` arm, so a new [`LessonError`] variant does
/// not compile until somebody has decided what it names.
///
/// [`LessonError::InvalidJson`] is `None` here because its pointer does not
/// come from the variant: `serde_json` reports a line and column, and
/// [`locate_shape_refusal`] takes the path from the deserializer instead. This
/// function is the fallback for the one case that has no path — bytes that are
/// not JSON at all.
///
/// The three closed-vocabulary refusals reach a caller by that same route,
/// since `serde` raises them before this module runs. Their arms record the
/// pointer the deserializer produces so the two spellings cannot disagree.
fn field_of(error: &LessonError) -> Option<String> {
    let field = match error {
        LessonError::LessonJsonTooLarge { .. }
        | LessonError::InvalidJson(_)
        | LessonError::AuthoredTextTooLarge { .. } => return None,
        LessonError::UnsupportedSchema(_) => "schema_version".to_owned(),
        LessonError::UnexpectedSchemaLink { .. } => "$schema".to_owned(),
        LessonError::MissingLessonId | LessonError::InvalidLessonId(_) => "lesson_id".to_owned(),
        LessonError::MalformedLanguage(_) => "language".to_owned(),
        LessonError::MalformedSourceContentHash(_) => "source/content_hash".to_owned(),
        LessonError::TooManyLearningObjectives { .. }
        | LessonError::EmptyLearningObjective
        | LessonError::LearningObjectiveTooLong { .. } => "learning_objectives".to_owned(),
        LessonError::TooManyLessonReferences { .. }
        | LessonError::EmptyLessonReference
        | LessonError::LessonReferenceTooLong { .. } => "source/references".to_owned(),
        LessonError::MissingSegments | LessonError::TooManySegments { .. } => "segments".to_owned(),
        // The speaker's own binding rather than the map: these errors carry
        // the name, so the pointer can name the field the author must edit.
        LessonError::MissingVoiceProfile(speaker) => {
            format!("speakers/{}/voice_profile", escape_token(speaker))
        }
        LessonError::InvalidVoiceProfile { speaker, .. } => {
            format!("speakers/{}/voice_profile", escape_token(speaker))
        }
        // The repeated binding itself, not its `voice_profile`: the field is
        // not what is wrong with it.
        LessonError::DuplicateSpeaker(speaker) => format!("speakers/{}", escape_token(speaker)),
        LessonError::MissingSegmentId
        | LessonError::InvalidSegmentId(_)
        | LessonError::DuplicateSegmentId(_) => "id".to_owned(),
        LessonError::MissingSpokenText(_) | LessonError::SpokenTextTooLong { .. } => {
            "spoken_text".to_owned()
        }
        LessonError::MissingDisplayText(_) | LessonError::DisplayTextTooLong { .. } => {
            "display_text".to_owned()
        }
        LessonError::MissingSourceRefs(_)
        | LessonError::TooManySourceRefs { .. }
        | LessonError::EmptySourceRef(_)
        | LessonError::SourceRefTooLong { .. } => "source_refs".to_owned(),
        LessonError::UnknownSegmentRole(_) | LessonError::MissingSegmentRole => "role".to_owned(),
        LessonError::UnknownDeliveryStyle(_) | LessonError::MissingDeliveryStyle => {
            "style".to_owned()
        }
        LessonError::UnknownReviewStatus(_)
        | LessonError::MissingReviewStatus
        | LessonError::UnapprovedSegment(_) => "review_status".to_owned(),
        LessonError::MissingSpeaker(_) | LessonError::UndeclaredSpeaker { .. } => {
            "speaker".to_owned()
        }
        LessonError::PauseOutOfRange(_)
        | LessonError::RecallPromptWithoutResponseInterval { .. }
        | LessonError::RecallPromptResponseIntervalTooLong { .. } => "pause_after_ms".to_owned(),
    };
    Some(field)
}

/// Locates a `serde` refusal at the field it is about.
///
/// The path comes from the deserializer, so it stays correct when a field is
/// added: a hand-written locator would be a second copy of this module's own
/// shape, and the copy is what drifts.
///
/// The segment identity and the key an omitted-field refusal is about are read
/// back from a lenient parse of the same bytes, because a document that failed
/// to deserialize has no [`LessonSegment`] to ask. That parse runs only on the
/// refusal path, and a document whose bytes are not JSON at all simply yields
/// neither.
fn locate_shape_refusal(
    document: &str,
    bytes: &[u8],
    error: serde_path_to_error::Error<serde_json::Error>,
) -> Box<LessonDiagnostic> {
    let mut field_path = String::new();
    for segment in error.path() {
        match segment {
            serde_path_to_error::Segment::Seq { index } => {
                field_path.push('/');
                field_path.push_str(&index.to_string());
            }
            serde_path_to_error::Segment::Map { key }
            | serde_path_to_error::Segment::Enum { variant: key } => {
                field_path.push('/');
                field_path.push_str(&escape_token(key));
            }
            // A path this build cannot spell is better left empty than
            // guessed at: an empty pointer means the document as a whole,
            // which is true, where a partial one would name the wrong field.
            serde_path_to_error::Segment::Unknown => {
                return LessonDiagnostic::about(
                    document,
                    LessonError::InvalidJson(error.into_inner()),
                );
            }
        }
    }

    let authored = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    if let Some(field) = omitted_field(error.inner(), authored.as_ref(), &field_path) {
        field_path.push('/');
        field_path.push_str(&escape_token(&field));
    }

    let segment_id = segment_index(&field_path).and_then(|index| {
        authored
            .as_ref()?
            .get("segments")?
            .get(index)?
            .get("id")?
            .as_str()
            .map(str::to_owned)
    });
    let refusal = vocabulary_refusal(&field_path, authored.as_ref())
        .or_else(|| source_hash_refusal(&field_path, authored.as_ref()))
        .unwrap_or_else(|| LessonError::InvalidJson(error.into_inner()));
    LessonDiagnostic::at(document, segment_id, field_path, refusal)
}

/// The key an omitted-field refusal is about, when the document lacks it.
///
/// `serde` raises a missing field against the object that should have carried
/// it, so the deserializer path stops at the parent and `DELIVERY-PLAN.md`
/// E1-S2's field path would name an object where the author needs a field. The
/// key is legible only in the message `serde::de::Error::missing_field`
/// formats, so it is confirmed against the document before it is trusted: a key
/// the parent genuinely lacks is the one to add, and anything else — another
/// refusal, a message shape this build does not know — leaves the parent
/// pointer the deserializer gave us rather than inventing a field.
///
/// That degradation is silent by construction, so the message format is pinned
/// rather than trusted: the two omitted-field cases in
/// `t1_e1_a_shape_error_is_located_at_the_field_it_is_about` assert the child
/// pointer, and a `serde` release that reworded `missing_field` fails them with
/// the parent pointer in the diff. This is the only place in the boundary that
/// reads an upstream crate's prose, and that test is why it is allowed to.
fn omitted_field(
    error: &serde_json::Error,
    authored: Option<&serde_json::Value>,
    parent_path: &str,
) -> Option<String> {
    let message = error.to_string();
    let field = message.strip_prefix("missing field `")?.split('`').next()?;
    let parent = authored?.pointer(parent_path)?.as_object()?;
    (!parent.contains_key(field)).then(|| field.to_owned())
}

/// The lesson-format invariant a located `serde` refusal is really about.
///
/// ADR-0001 §8.2's three declared vocabularies are invariants of the lesson
/// format rather than of its JSON shape, and `DELIVERY-PLAN.md` E1-S2 asks each
/// invariant for its own error. `serde` answers every one of them the same way,
/// so the authored value is read back at the pointer the deserializer gave us
/// and classified into the three refusals a field can earn: the key is absent,
/// the value is a string outside the vocabulary, or the value is some other
/// JSON type.
///
/// Only the first two are named. A wrong type is a *shape* refusal — the
/// document does not have the form the published schema declares — and it stays
/// [`LessonError::InvalidJson`] located at its field, because "this is not a
/// string" is one invariant however many fields can violate it, where "absent"
/// and "outside the vocabulary" are per-field invariants with per-field
/// remedies. `t1_e1_each_lesson_invariant_has_a_distinct_error` exercises all
/// three forms at each of the three fields, so a classifier that collapsed two
/// of them fails there rather than at whichever author met it first.
///
/// The classification reads the *document*, never `serde`'s message. The only
/// prose this boundary reads is `missing_field`'s, in [`omitted_field`], whose
/// own doc says why and which test pins it.
fn vocabulary_refusal(
    field_path: &str,
    authored: Option<&serde_json::Value>,
) -> Option<LessonError> {
    // Anchored inside a segment, so a speaker or an object keyed `role`
    // elsewhere in the document cannot be read as one of these fields.
    segment_index(field_path)?;
    let (parent_path, field) = field_path.rsplit_once('/')?;
    let declared = authored?.pointer(field_path);
    let present = authored?
        .pointer(parent_path)?
        .as_object()?
        .contains_key(field);

    match (field, declared.and_then(serde_json::Value::as_str), present) {
        ("role", Some(value), _) => Some(LessonError::UnknownSegmentRole(value.to_owned())),
        ("style", Some(value), _) => Some(LessonError::UnknownDeliveryStyle(value.to_owned())),
        ("review_status", Some(value), _) => {
            Some(LessonError::UnknownReviewStatus(value.to_owned()))
        }
        ("role", None, false) => Some(LessonError::MissingSegmentRole),
        ("style", None, false) => Some(LessonError::MissingDeliveryStyle),
        ("review_status", None, false) => Some(LessonError::MissingReviewStatus),
        _ => None,
    }
}

/// The source-digest invariant a located `serde` refusal is really about.
///
/// [`SourceContentHash`] refuses a value that is not a digest during parsing,
/// and `serde` delivers that typed refusal as prose inside
/// [`LessonError::InvalidJson`] — so without this, a recorded hash that is not
/// a digest and a `content_hash` that is not a string arrive as one
/// `(InvalidJson, /source/content_hash)` pair, and the invariant
/// [`MalformedSourceContentHash`] exists to name has no variant a caller or a
/// test can match on. The remedies differ: one is recompiled from the source
/// document, the other is a document that does not have the declared shape.
///
/// The value is reparsed from the document rather than read out of `serde`'s
/// message, for the reason [`vocabulary_refusal`] gives, and a value that
/// parses leaves the refusal alone: it was about something else.
///
/// A wrong type stays [`LessonError::InvalidJson`], as it does at the three
/// vocabularies. So does an absent `content_hash`, which is `serde`'s missing
/// field like every other required one — [`omitted_field`] has already pointed
/// it at the key the author must add.
fn source_hash_refusal(
    field_path: &str,
    authored: Option<&serde_json::Value>,
) -> Option<LessonError> {
    if field_path != "/source/content_hash" {
        return None;
    }
    authored?
        .pointer(field_path)?
        .as_str()?
        .parse::<SourceContentHash>()
        .err()
        .map(LessonError::MalformedSourceContentHash)
}

/// The segment a pointer is inside, if it is inside one.
fn segment_index(field_path: &str) -> Option<usize> {
    field_path
        .strip_prefix("/segments/")?
        .split('/')
        .next()?
        .parse()
        .ok()
}

/// The first speaker name the document binds to a voice more than once.
///
/// `serde` cannot raise this. [`AuthoredLesson::speakers`] is a [`BTreeMap`],
/// and a map keeps the last value for a repeated key, so a document declaring
/// one speaker under two voice profiles deserializes to whichever binding the
/// author wrote last and nothing downstream can tell that the other existed.
/// The bytes are therefore read a second time, with the names collected in
/// document order rather than into a map.
///
/// Only the first repeat is reported, because a refusal is acted on one edit
/// at a time and the second parse exists to name a binding, not to survey the
/// document.
///
/// `None` when the bytes carry no readable `speakers` object, which is a
/// refusal [`AuthoredLesson`]'s own deserialization has already made.
fn repeated_speaker(bytes: &[u8]) -> Option<String> {
    /// Just enough of a lesson to read its speaker names.
    ///
    /// Without `deny_unknown_fields` for the reason `check_declared_version`'s
    /// header is: ignoring the rest of the document is why it is read
    /// separately.
    #[derive(Deserialize)]
    struct DeclaredSpeakers {
        speakers: RepeatedName,
    }

    /// The first name a JSON object declares twice, if it declares one.
    struct RepeatedName(Option<String>);

    impl<'de> Deserialize<'de> for RepeatedName {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Names;

            impl<'de> Visitor<'de> for Names {
                type Value = RepeatedName;

                fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str("a map of speaker names")
                }

                fn visit_map<A: MapAccess<'de>>(
                    self,
                    mut entries: A,
                ) -> Result<Self::Value, A::Error> {
                    let mut declared = BTreeSet::new();
                    let mut repeated: Option<String> = None;
                    // Drained rather than returned from at the first repeat: a
                    // visitor that stops mid-map leaves the deserializer
                    // inside it, and the caller then reads a parse failure
                    // where this function found its answer.
                    while let Some((name, _)) = entries.next_entry::<String, IgnoredAny>()? {
                        if !declared.insert(name.clone()) && repeated.is_none() {
                            repeated = Some(name);
                        }
                    }
                    Ok(RepeatedName(repeated))
                }
            }

            deserializer.deserialize_map(Names)
        }
    }

    serde_json::from_slice::<DeclaredSpeakers>(bytes)
        .ok()?
        .speakers
        .0
}

/// Escapes one RFC 6901 reference token.
///
/// A speaker name is authored text under no portability rule, so it can carry
/// the two characters a pointer gives meaning to. Without this, a speaker
/// named `a/b` would produce a pointer naming a path that does not exist.
fn escape_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

impl AuthoredLesson {
    /// Creates a current lesson document with its stable published-schema link.
    ///
    /// The two `3.1` records start empty because both are optional in the
    /// format: a caller that has objectives or a source hash assigns them to
    /// the returned document rather than passing seven arguments here.
    pub fn new(
        lesson_id: String,
        title: String,
        language: String,
        speakers: BTreeMap<String, SpeakerDeclaration>,
        segments: Vec<LessonSegment>,
    ) -> Self {
        Self {
            schema: Some(schema_uri(
                LESSON_SCHEMA_STEM,
                LESSON_SCHEMA_VERSION.major(),
            )),
            schema_version: LESSON_SCHEMA_VERSION.to_string(),
            lesson_id,
            title,
            language,
            learning_objectives: Vec::new(),
            source: None,
            speakers,
            segments,
        }
    }

    /// Validates authored data before making it available to render planning.
    ///
    /// `document` names where the data came from — a path, or whatever a
    /// programmatic caller wants a refusal to name — and appears in the
    /// returned diagnostic.
    ///
    /// # Errors
    ///
    /// A [`LessonDiagnostic`] wrapping one [`LessonError`], reachable through
    /// [`LessonDiagnostic::error`]. Lesson-level violations are
    /// [`LessonError::UnsupportedSchema`],
    /// [`LessonError::UnexpectedSchemaLink`],
    /// [`LessonError::MalformedLanguage`], [`LessonError::MissingLessonId`],
    /// [`LessonError::InvalidLessonId`],
    /// [`LessonError::TooManyLearningObjectives`],
    /// [`LessonError::EmptyLearningObjective`],
    /// [`LessonError::LearningObjectiveTooLong`],
    /// [`LessonError::TooManyLessonReferences`],
    /// [`LessonError::EmptyLessonReference`],
    /// [`LessonError::LessonReferenceTooLong`],
    /// [`LessonError::MissingSegments`], [`LessonError::TooManySegments`],
    /// [`LessonError::MissingVoiceProfile`],
    /// [`LessonError::InvalidVoiceProfile`], or
    /// [`LessonError::AuthoredTextTooLarge`]. Segment validation returns
    /// [`LessonError::MissingSegmentId`],
    /// [`LessonError::InvalidSegmentId`],
    /// [`LessonError::DuplicateSegmentId`],
    /// [`LessonError::MissingSpokenText`],
    /// [`LessonError::SpokenTextTooLong`],
    /// [`LessonError::MissingDisplayText`],
    /// [`LessonError::DisplayTextTooLong`],
    /// [`LessonError::MissingSourceRefs`],
    /// [`LessonError::TooManySourceRefs`], [`LessonError::EmptySourceRef`],
    /// [`LessonError::SourceRefTooLong`],
    /// [`LessonError::UnapprovedSegment`], [`LessonError::MissingSpeaker`],
    /// [`LessonError::UndeclaredSpeaker`], [`LessonError::PauseOutOfRange`],
    /// [`LessonError::RecallPromptWithoutResponseInterval`], or
    /// [`LessonError::RecallPromptResponseIntervalTooLong`] — the two ends of
    /// ADR-0001 §13.2's recall range, separate because one is answered by
    /// lengthening the pause and the other by shortening it. Existing semantic
    /// checks preserve their relative order; resource checks occur beside the
    /// count or field they bound.
    ///
    /// Ten [`LessonError`] variants cannot be returned here, because
    /// [`ValidatedLesson::from_json`] raises them before this function is
    /// reached and a caller holding an [`AuthoredLesson`] has already parsed:
    /// [`LessonError::LessonJsonTooLarge`] and [`LessonError::InvalidJson`]
    /// for bytes that are not one document of this shape,
    /// [`LessonError::DuplicateSpeaker`] for a speaker the document binds
    /// twice, which a [`BTreeMap`] field cannot represent and so cannot be
    /// reached from an [`AuthoredLesson`] at all,
    /// [`LessonError::MalformedSourceContentHash`], which
    /// [`SourceContentHash`] refuses during deserialization so an
    /// [`AuthoredLesson`] can only hold a digest, and
    /// [`LessonError::UnknownSegmentRole`],
    /// [`LessonError::UnknownDeliveryStyle`],
    /// [`LessonError::UnknownReviewStatus`],
    /// [`LessonError::MissingSegmentRole`],
    /// [`LessonError::MissingDeliveryStyle`], or
    /// [`LessonError::MissingReviewStatus`] for the three closed vocabularies,
    /// which the deserializer refuses at their own fields like
    /// [`ReviewStatus`].
    pub fn validate(self, document: &str) -> Result<ValidatedLesson, Box<LessonDiagnostic>> {
        let (schema_version, language) = self.check(document)?;
        Ok(ValidatedLesson {
            authored: self,
            schema_version,
            language,
        })
    }

    /// Applies every invariant, locating each refusal in `document`.
    fn check(&self, document: &str) -> Result<(SchemaVersion, LanguageTag), Box<LessonDiagnostic>> {
        let (schema_version, language) = self
            .check_document()
            .map_err(|error| LessonDiagnostic::about(document, error))?;

        let mut ids = HashSet::with_capacity(self.segments.len());
        // Every lesson-level authored string seeds the aggregate, because they
        // are authored strings like any other; the ceiling is checked beside
        // each segment.
        let mut authored_text_bytes =
            self.speakers
                .iter()
                .fold(self.title.len(), |total, (speaker, declaration)| {
                    total
                        .saturating_add(speaker.len())
                        .saturating_add(declaration.voice_profile.len())
                });
        for objective in &self.learning_objectives {
            authored_text_bytes = authored_text_bytes.saturating_add(objective.len());
        }
        for reference in self.source.iter().flat_map(|source| &source.references) {
            authored_text_bytes = authored_text_bytes.saturating_add(reference.len());
        }
        for (index, segment) in self.segments.iter().enumerate() {
            self.check_segment(segment, &mut ids, &mut authored_text_bytes)
                .map_err(|error| {
                    LessonDiagnostic::in_segment(document, index, &segment.id, error)
                })?;
        }

        Ok((schema_version, language))
    }

    /// Applies the invariants that belong to the document rather than to any
    /// one segment.
    fn check_document(&self) -> Result<(SchemaVersion, LanguageTag), LessonError> {
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
        self.check_provenance()?;
        if self.segments.is_empty() {
            return Err(LessonError::MissingSegments);
        }
        if self.segments.len() > MAX_LESSON_SEGMENTS {
            return Err(LessonError::TooManySegments {
                found: self.segments.len(),
                max: MAX_LESSON_SEGMENTS,
            });
        }
        for (speaker, declaration) in &self.speakers {
            if declaration.voice_profile.trim().is_empty() {
                return Err(LessonError::MissingVoiceProfile(speaker.clone()));
            }
            if !is_portable_id(&declaration.voice_profile) {
                return Err(LessonError::InvalidVoiceProfile {
                    speaker: speaker.clone(),
                    voice_profile: declaration.voice_profile.clone(),
                });
            }
        }

        Ok((schema_version, language))
    }

    /// Bounds the two lesson-level records added by lesson `3.1`.
    ///
    /// Neither is a gate: ADR-0001 §8.2 sets no invariant on either beyond
    /// their being usable text, so these are the resource and blank-value
    /// bounds every other authored list here carries.
    fn check_provenance(&self) -> Result<(), LessonError> {
        if self.learning_objectives.len() > MAX_LEARNING_OBJECTIVES {
            return Err(LessonError::TooManyLearningObjectives {
                found: self.learning_objectives.len(),
                max: MAX_LEARNING_OBJECTIVES,
            });
        }
        if self
            .learning_objectives
            .iter()
            .any(|objective| objective.trim().is_empty())
        {
            return Err(LessonError::EmptyLearningObjective);
        }
        if let Some(objective) = self
            .learning_objectives
            .iter()
            .find(|objective| objective.len() > MAX_LEARNING_OBJECTIVE_BYTES)
        {
            return Err(LessonError::LearningObjectiveTooLong {
                bytes: objective.len(),
                max_bytes: MAX_LEARNING_OBJECTIVE_BYTES,
            });
        }

        let Some(source) = &self.source else {
            return Ok(());
        };
        if source.references.len() > MAX_LESSON_REFERENCES {
            return Err(LessonError::TooManyLessonReferences {
                found: source.references.len(),
                max: MAX_LESSON_REFERENCES,
            });
        }
        if source
            .references
            .iter()
            .any(|reference| reference.trim().is_empty())
        {
            return Err(LessonError::EmptyLessonReference);
        }
        if let Some(reference) = source
            .references
            .iter()
            .find(|reference| reference.len() > MAX_LESSON_REFERENCE_BYTES)
        {
            return Err(LessonError::LessonReferenceTooLong {
                bytes: reference.len(),
                max_bytes: MAX_LESSON_REFERENCE_BYTES,
            });
        }

        Ok(())
    }

    /// Applies every invariant one segment must satisfy on its own.
    ///
    /// `ids` and `authored_text_bytes` carry the two cross-segment facts:
    /// identity uniqueness and the running authored-text total.
    fn check_segment<'a>(
        &'a self,
        segment: &'a LessonSegment,
        ids: &mut HashSet<&'a str>,
        authored_text_bytes: &mut usize,
    ) -> Result<(), LessonError> {
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
        // ADR-0001 §8.2: a segment references source material *or* is
        // explicitly marked editorial. Every other reference bound below
        // applies either way.
        if segment.source_refs.is_empty() && !segment.editorial {
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
        if !self.speakers.contains_key(&segment.speaker) {
            return Err(LessonError::UndeclaredSpeaker {
                segment_id: segment.id.clone(),
                speaker: segment.speaker.clone(),
            });
        }
        if segment.pause_after_ms > MAX_PAUSE_AFTER_MS {
            return Err(LessonError::PauseOutOfRange(segment.id.clone()));
        }
        // Checked after the generic ceiling so an unusable pause is reported as
        // one whatever the role, and only then against the role's own range.
        if segment.role == SegmentRole::RecallPrompt {
            if segment.pause_after_ms < MIN_RECALL_RESPONSE_MS {
                return Err(LessonError::RecallPromptWithoutResponseInterval {
                    segment_id: segment.id.clone(),
                    pause_after_ms: segment.pause_after_ms,
                    min_ms: MIN_RECALL_RESPONSE_MS,
                });
            }
            if segment.pause_after_ms > MAX_RECALL_RESPONSE_MS {
                return Err(LessonError::RecallPromptResponseIntervalTooLong {
                    segment_id: segment.id.clone(),
                    pause_after_ms: segment.pause_after_ms,
                    max_ms: MAX_RECALL_RESPONSE_MS,
                });
            }
        }

        // The role and the style are closed vocabularies with fixed spellings,
        // so neither can grow this total.
        for field in [
            &segment.id,
            &segment.speaker,
            &segment.display_text,
            &segment.spoken_text,
        ] {
            *authored_text_bytes = authored_text_bytes.saturating_add(field.len());
        }
        for source_ref in &segment.source_refs {
            *authored_text_bytes = authored_text_bytes.saturating_add(source_ref.len());
        }
        if *authored_text_bytes > MAX_AUTHORED_TEXT_BYTES {
            return Err(LessonError::AuthoredTextTooLarge {
                max_bytes: MAX_AUTHORED_TEXT_BYTES,
            });
        }

        Ok(())
    }
}

impl ValidatedLesson {
    /// Parses and validates a lesson document, refusing anything synthesis
    /// could not use.
    ///
    /// `document` names where the bytes came from and appears in every
    /// refusal, so a caller reading one knows which file to open.
    ///
    /// # Errors
    ///
    /// A [`LessonDiagnostic`] wrapping [`LessonError::LessonJsonTooLarge`]
    /// when the input exceeds [`MAX_LESSON_JSON_BYTES`], then
    /// [`LessonError::UnsupportedSchema`] for a version this build cannot
    /// read, and [`LessonError::InvalidJson`] when the bytes are not this
    /// document's shape or carry anything after the one JSON value a lesson
    /// is. Then [`LessonError::DuplicateSpeaker`], which only this entry point
    /// can raise: the parsed lesson has already resolved a repeated binding to
    /// one of the two voices it named. Parsed authoring data can return every
    /// lesson-level or segment-level variant documented by
    /// [`AuthoredLesson::validate`].
    pub fn from_json(document: &str, bytes: &[u8]) -> Result<Self, Box<LessonDiagnostic>> {
        let refuse = |error: LessonError| LessonDiagnostic::about(document, error);
        if bytes.len() > MAX_LESSON_JSON_BYTES {
            return Err(refuse(LessonError::LessonJsonTooLarge {
                max_bytes: MAX_LESSON_JSON_BYTES,
            }));
        }
        // A strict parse would misreport a future field before its version.
        check_declared_version(bytes, LESSON_SCHEMA_VERSION)
            .map_err(|error| refuse(error.into()))?;
        // Through `serde_path_to_error` so a refusal serde raises — an unknown
        // field, a wrong type, a value outside a closed vocabulary — is located
        // like every refusal this module raises itself. `serde_json` reports
        // only a line and column, and `DELIVERY-PLAN.md` E1-S2 asks for a field
        // path.
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let lesson: AuthoredLesson = serde_path_to_error::deserialize(&mut deserializer)
            .map_err(|error| locate_shape_refusal(document, bytes, error))?;
        // A lesson is one canonical JSON document, and a located deserializer
        // stops at the end of the first value rather than at the end of the
        // input the way `serde_json::from_slice` does. Without this the bytes
        // after that value are accepted unread, so a document could carry
        // content no validation, checksum, or review ever sees.
        deserializer
            .end()
            .map_err(|error| refuse(LessonError::InvalidJson(error)))?;
        // After the shape is known good, because a document that is not a
        // lesson has no speaker map to have declared twice. `lesson` cannot
        // answer this: its `BTreeMap` has already discarded one of the two
        // bindings, so the bytes are what still hold the question.
        if let Some(speaker) = repeated_speaker(bytes) {
            return Err(refuse(LessonError::DuplicateSpeaker(speaker)));
        }
        lesson.validate(document)
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

    /// What a listener should be able to do after this lesson.
    pub fn learning_objectives(&self) -> &[String] {
        &self.authored.learning_objectives
    }

    /// Where this lesson's material came from, when the document records it.
    pub fn source(&self) -> Option<&LessonSource> {
        self.authored.source.as_ref()
    }

    /// The validated segments in speaking order.
    pub fn segments(&self) -> &[LessonSegment] {
        &self.authored.segments
    }

    /// The voice each speaker is bound to, by speaker name.
    ///
    /// Every segment's speaker is a key here, which is what lets
    /// `study-tts-runtime` resolve the ADR-0001 §12.5 voice-conditioning input
    /// without a second lookup that could disagree.
    pub fn speakers(&self) -> &BTreeMap<String, SpeakerDeclaration> {
        &self.authored.speakers
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
    use crate::digest::BLAKE3_HEX_LENGTH;
    use serde_json::Value;
    use std::mem::{Discriminant, discriminant};

    fn fixture() -> Value {
        serde_json::from_slice(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture JSON should parse")
    }

    /// What a refusal in these tests names as the document it came from.
    const DOCUMENT: &str = "<test lesson>";

    /// The refused invariant, which is what every assertion below is about.
    ///
    /// Field access rather than an accessor because these tests live in the
    /// module that owns [`LessonDiagnostic`]; nothing outside it needs to move
    /// the error out of its location.
    fn parse_lesson(value: &Value) -> Result<ValidatedLesson, LessonError> {
        ValidatedLesson::from_json(
            DOCUMENT,
            &serde_json::to_vec(value).expect("test lesson should serialize"),
        )
        .map_err(|diagnostic| diagnostic.error)
    }

    fn validate_authored(authored: AuthoredLesson) -> Result<ValidatedLesson, LessonError> {
        authored
            .validate(DOCUMENT)
            .map_err(|diagnostic| diagnostic.error)
    }

    fn authored_fixture() -> AuthoredLesson {
        serde_json::from_value(fixture()).expect("fixture shape should deserialize")
    }

    /// The fixture's bytes with `nadia` bound to a voice a second time.
    ///
    /// Assembled as text because `serde_json::Value` holds a map and cannot
    /// carry a repeated key at all. That is the same reason the check under
    /// test reads the document's bytes rather than the parsed lesson.
    fn lesson_declaring_a_speaker_twice(voice_profile: &str) -> Vec<u8> {
        const SPEAKERS: &str = "\"speakers\":{";

        let mut document = serde_json::to_string(&fixture()).expect("the fixture serializes");
        let at = document
            .find(SPEAKERS)
            .expect("the fixture declares speakers");
        document.insert_str(
            at + SPEAKERS.len(),
            &format!("\"nadia\":{{\"voice_profile\":\"{voice_profile}\"}},"),
        );
        document.into_bytes()
    }

    /// The speaker bindings every programmatically built lesson below needs.
    fn sample_speakers() -> BTreeMap<String, SpeakerDeclaration> {
        BTreeMap::from([(
            "nadia".to_owned(),
            SpeakerDeclaration {
                voice_profile: "synthetic-test-voice-v1".to_owned(),
            },
        )])
    }

    #[test]
    fn t1_e1_a_speaker_declared_twice_is_refused() {
        // A `BTreeMap` keeps the last binding for a repeated key, so before
        // this refusal existed both documents below validated and `nadia` was
        // rendered with whichever profile the author wrote last — a voice
        // selected by parser behavior rather than by the review that approved
        // the lesson. The identical-profile case is refused on the same terms:
        // the document is ambiguous whatever the two bindings say, and a rule
        // that read the values would stop being a rule about the document.
        for (case, voice_profile) in [
            ("two different voices", "synthetic-test-voice-v2"),
            ("the same voice twice", "synthetic-test-voice-v1"),
        ] {
            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &lesson_declaring_a_speaker_twice(voice_profile),
            )
            .expect_err(case);

            assert!(
                matches!(
                    diagnostic.error(),
                    LessonError::DuplicateSpeaker(speaker) if speaker == "nadia"
                ),
                "`{case}` must be refused naming the repeated speaker: {diagnostic}"
            );
            assert_eq!(diagnostic.field_path(), "/speakers/nadia", "`{case}`");
            assert_eq!(diagnostic.segment_id(), None, "`{case}`");
        }
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
        for declared in ["4.0", "2.1", "0.1-skeleton"] {
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
        for declared in ["4.0", "3.2"] {
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
        prior["schema_version"] = Value::String("3.0".to_owned());
        prior
            .as_object_mut()
            .expect("the fixture is an object")
            .remove("$schema");

        let lesson = parse_lesson(&prior).expect("an earlier minor version must be accepted");

        assert_eq!(lesson.schema_version(), SchemaVersion::new(3, 0));
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
        assert_eq!(
            lesson.speakers()["nadia"].voice_profile,
            "synthetic-test-voice-v1"
        );
    }

    #[test]
    fn t1_e1_the_canonical_adr_lesson_document_is_accepted() {
        // ADR-0001 §8.1's canonical format, at the version this build
        // publishes rather than the `1.0` the ADR was written against. Every
        // key below is one the ADR shows; `deny_unknown_fields` means a field
        // this type omits is a canonical document this build refuses, which is
        // what this test exists to catch.
        let canonical = serde_json::json!({
            "$schema": schema_uri(LESSON_SCHEMA_STEM, LESSON_SCHEMA_VERSION.major()),
            "schema_version": LESSON_SCHEMA_VERSION.to_string(),
            "lesson_id": "excel-bijective-base-26",
            "title": "Excel Column Labels as Bijective Base Twenty-Six",
            "language": "en-US",
            "learning_objectives": [
                "Explain why the numeral system has no zero digit",
                "Reproduce the conversion recurrence",
            ],
            "source": {
                "content_hash": "4".repeat(BLAKE3_HEX_LENGTH),
                "references": [],
            },
            "speakers": {
                "nadia": { "voice_profile": "nadia-v1" },
                "tom": { "voice_profile": "tom-v1" },
            },
            "segments": [{
                "id": "seg-0001",
                "speaker": "nadia",
                "role": "explanation",
                "source_refs": ["block-001"],
                "display_text": "Excel column labels use bijective base 26.",
                "spoken_text": "Excel column labels use bijective base twenty-six.",
                "style": "calm_explanatory",
                "pause_after_ms": 550,
                "review_status": "approved",
            }],
        });

        let lesson = parse_lesson(&canonical).expect("the canonical ADR document must validate");

        assert_eq!(
            lesson.learning_objectives(),
            [
                "Explain why the numeral system has no zero digit",
                "Reproduce the conversion recurrence",
            ]
        );
        let source = lesson.source().expect("the document records a source");
        assert_eq!(source.content_hash.as_str(), "4".repeat(BLAKE3_HEX_LENGTH));
        assert!(source.references.is_empty());
    }

    #[test]
    fn t1_e1_delivery_style_spelling_matches_its_serde_form() {
        // The synthesis key writes `as_str` while a lesson document writes the
        // serde form. A silent disagreement would key audio on a spelling no
        // document carries. The exhaustive match makes a new variant a compile
        // error here rather than a silently untested one.
        for style in [
            DeliveryStyle::Calm,
            DeliveryStyle::CalmExplanatory,
            DeliveryStyle::Emphatic,
            DeliveryStyle::Deliberate,
        ] {
            let serialized =
                serde_json::to_string(&style).expect("a delivery style serializes as a string");

            assert_eq!(serialized, format!("\"{}\"", style.as_str()));
        }
    }

    #[test]
    fn t1_e1_a_role_or_style_outside_its_vocabulary_is_refused() {
        // ADR-0001 §8.2 requires the role and the style to be *declared*, and
        // §13.4 will not render a style with no frozen loudness reference
        // behind it. Before lesson `3.0` every value below validated.
        //
        // Each vocabulary carries its own refusal, quoting the spelling the
        // author has to correct, so a caller telling one from the other does
        // not have to parse a pointer. This test covers the spellings the two
        // vocabularies exclude.
        for role in ["", "  ", "narration", "Explanation", "recall-prompt"] {
            let mut lesson = fixture();
            lesson["segments"][0]["role"] = Value::String(role.to_owned());

            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&lesson).expect("the mutated lesson serializes"),
            )
            .expect_err("a role outside the vocabulary must be refused");

            assert!(
                matches!(diagnostic.error(), LessonError::UnknownSegmentRole(declared)
                    if declared == role),
                "role `{role}`: {diagnostic}"
            );
            assert_eq!(diagnostic.field_path(), "/segments/0/role", "role `{role}`");
            assert_eq!(diagnostic.segment_id(), Some("seg-0001"), "role `{role}`");
        }

        for style in ["", "  ", "excited", "Calm", "calm-explanatory"] {
            let mut lesson = fixture();
            lesson["segments"][0]["style"] = Value::String(style.to_owned());

            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&lesson).expect("the mutated lesson serializes"),
            )
            .expect_err("a style outside the vocabulary must be refused");

            assert!(
                matches!(diagnostic.error(), LessonError::UnknownDeliveryStyle(declared)
                    if declared == style),
                "style `{style}`: {diagnostic}"
            );
            assert_eq!(
                diagnostic.field_path(),
                "/segments/0/style",
                "style `{style}`"
            );
            assert_eq!(diagnostic.segment_id(), Some("seg-0001"), "style `{style}`");
        }
    }

    #[test]
    fn t1_e1_a_recall_prompt_must_leave_a_response_interval() {
        // ADR-0001 §8.2 makes the interval an invariant of the format and
        // §13.2 gives a recall question 1.5-4 seconds. Both edges are asserted
        // from both sides so an off-by-one cannot pass, and the same pauses are
        // checked under another role so each refusal is the prompt's rather
        // than the pause's. The ceiling matters as much as the floor: without
        // it the generic 10,000 ms limit accepted 4,001-10,000 ms, which §13.2
        // does not, and no override annotation exists to authorize.
        let prompt_with_pause = |pause_after_ms: u32| {
            let mut lesson = fixture();
            lesson["segments"][0]["role"] = Value::String("recall_prompt".to_owned());
            lesson["segments"][0]["pause_after_ms"] = Value::Number(pause_after_ms.into());
            lesson
        };

        assert!(matches!(
            parse_lesson(&prompt_with_pause(MIN_RECALL_RESPONSE_MS - 1)),
            Err(LessonError::RecallPromptWithoutResponseInterval {
                segment_id,
                pause_after_ms,
                min_ms,
            }) if segment_id == "seg-0001"
                && pause_after_ms == MIN_RECALL_RESPONSE_MS - 1
                && min_ms == MIN_RECALL_RESPONSE_MS
        ));
        parse_lesson(&prompt_with_pause(MIN_RECALL_RESPONSE_MS))
            .expect("the response-interval floor must be accepted");

        parse_lesson(&prompt_with_pause(MAX_RECALL_RESPONSE_MS))
            .expect("the response-interval ceiling must be accepted");
        assert!(matches!(
            parse_lesson(&prompt_with_pause(MAX_RECALL_RESPONSE_MS + 1)),
            Err(LessonError::RecallPromptResponseIntervalTooLong {
                segment_id,
                pause_after_ms,
                max_ms,
            }) if segment_id == "seg-0001"
                && pause_after_ms == MAX_RECALL_RESPONSE_MS + 1
                && max_ms == MAX_RECALL_RESPONSE_MS
        ));

        // The generic pause ceiling is what a non-prompt is held to, so a value
        // this test refuses for a prompt is still accepted here. That is what
        // makes the refusal above the prompt's own rule rather than a limit
        // every segment already had.
        let mut explanation = fixture();
        explanation["segments"][0]["pause_after_ms"] = Value::Number(0.into());
        parse_lesson(&explanation)
            .expect("only a recall prompt carries the response-interval floor");

        let mut long_explanation = fixture();
        long_explanation["segments"][0]["pause_after_ms"] =
            Value::Number((MAX_RECALL_RESPONSE_MS + 1).into());
        parse_lesson(&long_explanation)
            .expect("only a recall prompt carries the response-interval ceiling");
    }

    #[test]
    fn t3_e1_recall_response_interval_matches_adr() {
        assert_eq!(
            (MIN_RECALL_RESPONSE_MS, MAX_RECALL_RESPONSE_MS),
            (1_500, 4_000)
        );
    }

    #[test]
    fn t3_e1_provisional_lesson_resource_ceilings_match_walking_skeleton_document() {
        const CASES: [(&str, usize, usize); 10] = [
            (
                "canonical lesson JSON",
                MAX_LESSON_JSON_BYTES,
                16 * 1024 * 1024,
            ),
            ("segments per lesson", MAX_LESSON_SEGMENTS, 4_096),
            (
                "learning objectives per lesson",
                MAX_LEARNING_OBJECTIVES,
                64,
            ),
            (
                "one learning objective",
                MAX_LEARNING_OBJECTIVE_BYTES,
                4 * 1024,
            ),
            (
                "references per lesson source record",
                MAX_LESSON_REFERENCES,
                256,
            ),
            (
                "one lesson source reference",
                MAX_LESSON_REFERENCE_BYTES,
                4 * 1024,
            ),
            (
                "display/spoken text per segment",
                MAX_SEGMENT_TEXT_BYTES,
                64 * 1024,
            ),
            (
                "source references per segment",
                MAX_SOURCE_REFS_PER_SEGMENT,
                256,
            ),
            ("one source reference", MAX_SOURCE_REF_BYTES, 4 * 1024),
            (
                "aggregate authored text",
                MAX_AUTHORED_TEXT_BYTES,
                16 * 1024 * 1024,
            ),
        ];

        for (resource, actual, expected) in CASES {
            assert_eq!(actual, expected, "{resource}");
        }
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
    fn t1_e1_generated_lesson_includes_the_stable_schema_uri() {
        let fixture = authored_fixture();
        let generated = AuthoredLesson::new(
            fixture.lesson_id,
            fixture.title,
            fixture.language,
            fixture.speakers,
            fixture.segments,
        );

        let document =
            serde_json::to_value(generated).expect("a generated lesson should serialize");

        assert_eq!(
            document["$schema"],
            "https://schemas.study-tts.example/lesson-v3.schema.json"
        );
        assert_eq!(document["schema_version"], "3.1");
    }

    #[test]
    fn t1_e0_programmatically_authored_unapproved_lesson_is_rejected() {
        let authored = AuthoredLesson {
            schema: None,
            schema_version: LESSON_SCHEMA_VERSION.to_string(),
            lesson_id: "unapproved".to_owned(),
            title: "Unapproved".to_owned(),
            language: "en".to_owned(),
            learning_objectives: Vec::new(),
            source: None,
            speakers: sample_speakers(),
            segments: vec![LessonSegment {
                id: "seg-0001".to_owned(),
                speaker: "nadia".to_owned(),
                role: SegmentRole::Explanation,
                source_refs: vec!["block-001".to_owned()],
                display_text: "Review this first.".to_owned(),
                spoken_text: "Review this first.".to_owned(),
                style: DeliveryStyle::Calm,
                pause_after_ms: 0,
                review_status: ReviewStatus::NeedsReview,
                editorial: false,
            }],
        };

        assert!(matches!(
            validate_authored(authored),
            Err(LessonError::UnapprovedSegment(id)) if id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e0_duplicate_segment_id_is_rejected() {
        let bytes = br#"{
            "schema_version":"3.1",
            "lesson_id":"duplicate",
            "title":"Duplicate",
            "language":"en",
            "speakers":{"nadia":{"voice_profile":"synthetic-test-voice-v1"},
                        "tom":{"voice_profile":"synthetic-test-voice-v2"}},
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
            ValidatedLesson::from_json(DOCUMENT, bytes).map_err(|diagnostic| diagnostic.error),
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

        // The role is a closed vocabulary since lesson `3.0`, so a blank one
        // is outside it rather than absent — the same way `review_status` is.
        let mut value = fixture();
        value["segments"][0]["role"] = Value::String(String::new());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::UnknownSegmentRole(_))
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
            Err(LessonError::UnknownDeliveryStyle(_))
        ));

        let mut value = fixture();
        value["segments"][0]["review_status"] = Value::String("aproved".to_owned());
        assert!(matches!(
            parse_lesson(&value),
            Err(LessonError::UnknownReviewStatus(_))
        ));
    }

    /// One authoring mistake, and the refusal it must produce.
    ///
    /// The expectation is a value read against `lesson.rs`, not a `matches!`
    /// copied out of it: comparing discriminants means a case that produced
    /// some *other* variant fails even though both are `LessonError`.
    type Invariant = (&'static str, fn(&mut Value), LessonError, &'static str);

    /// One way a closed-vocabulary field can fail, and the refusal it earns.
    ///
    /// The mutation returns the field it broke, so the case and its pointer
    /// cannot be written down separately and drift apart.
    type VocabularyRefusal = (&'static str, fn(&mut Value) -> &'static str, LessonError);

    /// Every form each of ADR-0001 §8.2's three declared vocabularies can be
    /// violated in.
    ///
    /// Three fields by three forms, written out rather than generated: the
    /// point of the table is that a reader can see all nine refusals are
    /// different, and a generated cross-product would hide a classifier that
    /// answered two cells alike.
    ///
    /// A function rather than a `const` because a `serde_json::Error` can only
    /// be had by failing a parse, which is also how this file spells its
    /// expected [`LessonError::UnsupportedSchema`] and
    /// [`LessonError::MalformedLanguage`].
    fn vocabulary_refusals() -> [VocabularyRefusal; 9] {
        [
            (
                "a role outside the vocabulary",
                |lesson| {
                    lesson["segments"][0]["role"] = Value::String("narration".to_owned());
                    "role"
                },
                LessonError::UnknownSegmentRole(String::new()),
            ),
            (
                "a style outside the vocabulary",
                |lesson| {
                    lesson["segments"][0]["style"] = Value::String("excited".to_owned());
                    "style"
                },
                LessonError::UnknownDeliveryStyle(String::new()),
            ),
            (
                "a review state outside the vocabulary",
                |lesson| {
                    lesson["segments"][0]["review_status"] = Value::String("aproved".to_owned());
                    "review_status"
                },
                LessonError::UnknownReviewStatus(String::new()),
            ),
            (
                "no role at all",
                |lesson| {
                    remove_segment_field(lesson, "role");
                    "role"
                },
                LessonError::MissingSegmentRole,
            ),
            (
                "no style at all",
                |lesson| {
                    remove_segment_field(lesson, "style");
                    "style"
                },
                LessonError::MissingDeliveryStyle,
            ),
            (
                "no review state at all",
                |lesson| {
                    remove_segment_field(lesson, "review_status");
                    "review_status"
                },
                LessonError::MissingReviewStatus,
            ),
            (
                "a role that is not a string",
                |lesson| {
                    lesson["segments"][0]["role"] = Value::from(3);
                    "role"
                },
                LessonError::InvalidJson(shape_error()),
            ),
            (
                "a style that is not a string",
                |lesson| {
                    lesson["segments"][0]["style"] = Value::Bool(true);
                    "style"
                },
                LessonError::InvalidJson(shape_error()),
            ),
            (
                "a review state that is not a string",
                |lesson| {
                    lesson["segments"][0]["review_status"] = Value::Array(Vec::new());
                    "review_status"
                },
                LessonError::InvalidJson(shape_error()),
            ),
        ]
    }

    /// Any `serde_json` failure, for a table cell that only compares
    /// discriminants.
    ///
    /// The specific error is irrelevant — `discriminant` looks at the variant
    /// and never inside it — but `LessonError::InvalidJson` has no other
    /// constructor, because a shape refusal belongs to the deserializer.
    fn shape_error() -> serde_json::Error {
        serde_json::from_str::<u8>("\"not a number\"").expect_err("a string is not a `u8`")
    }

    /// Deletes one key from the fixture's first segment.
    fn remove_segment_field(lesson: &mut Value, field: &str) {
        lesson["segments"][0]
            .as_object_mut()
            .expect("a fixture segment is an object")
            .remove(field);
    }

    #[test]
    fn t1_e1_each_lesson_invariant_has_a_distinct_error() {
        // Three guards, because none alone is enough. `field_of` is an
        // exhaustive match with no `_` arm, so adding a `LessonError` variant
        // does not compile until somebody decides which field it names — which
        // brings them to this file. The variant count below is what then fails
        // until they add its case here, and the distinctness assertion is what
        // fails if the case they add is answered by a refusal another invariant
        // already produces.
        let cases: [Invariant; 34] = [
            (
                "unknown major",
                |lesson| {
                    lesson["schema_version"] = Value::String("4.0".to_owned());
                    lesson
                        .as_object_mut()
                        .expect("the fixture is an object")
                        .remove("$schema");
                },
                LessonError::UnsupportedSchema(
                    "3".parse::<SchemaVersion>()
                        .expect_err("a one-component version is refused"),
                ),
                "/schema_version",
            ),
            (
                "link naming another schema",
                |lesson| lesson["$schema"] = Value::String(schema_uri("takes", 1)),
                LessonError::UnexpectedSchemaLink {
                    declared: String::new(),
                    version: LESSON_SCHEMA_VERSION,
                    expected: String::new(),
                },
                "/$schema",
            ),
            (
                "blank lesson identity",
                |lesson| lesson["lesson_id"] = Value::String(String::new()),
                LessonError::MissingLessonId,
                "/lesson_id",
            ),
            (
                "lesson identity that could not name a directory",
                |lesson| lesson["lesson_id"] = Value::String("../escape".to_owned()),
                LessonError::InvalidLessonId(String::new()),
                "/lesson_id",
            ),
            (
                "language outside BCP 47",
                |lesson| lesson["language"] = Value::String("en_US".to_owned()),
                LessonError::MalformedLanguage(
                    "en_US"
                        .parse::<LanguageTag>()
                        .expect_err("`en_US` is refused"),
                ),
                "/language",
            ),
            (
                "nothing to speak",
                |lesson| lesson["segments"] = Value::Array(Vec::new()),
                LessonError::MissingSegments,
                "/segments",
            ),
            (
                "more segments than this build plans",
                |lesson| {
                    let segment = lesson["segments"][0].clone();
                    let segments = (0..=MAX_LESSON_SEGMENTS)
                        .map(|index| {
                            let mut copy = segment.clone();
                            copy["id"] = Value::String(format!("seg-{index}"));
                            copy
                        })
                        .collect();
                    lesson["segments"] = Value::Array(segments);
                },
                LessonError::TooManySegments { found: 0, max: 0 },
                "/segments",
            ),
            (
                "speaker declared with no voice profile",
                |lesson| {
                    lesson["speakers"]["nadia"]["voice_profile"] = Value::String(String::new());
                },
                LessonError::MissingVoiceProfile(String::new()),
                "/speakers/nadia/voice_profile",
            ),
            (
                "voice profile that could not name a directory",
                |lesson| {
                    lesson["speakers"]["nadia"]["voice_profile"] =
                        Value::String("../escape".to_owned());
                },
                LessonError::InvalidVoiceProfile {
                    speaker: String::new(),
                    voice_profile: String::new(),
                },
                "/speakers/nadia/voice_profile",
            ),
            (
                "blank segment identity",
                |lesson| lesson["segments"][0]["id"] = Value::String(String::new()),
                LessonError::MissingSegmentId,
                "/segments/0/id",
            ),
            (
                "segment identity that could not name a path component",
                |lesson| lesson["segments"][0]["id"] = Value::String("../escape".to_owned()),
                LessonError::InvalidSegmentId(String::new()),
                "/segments/0/id",
            ),
            (
                "two segments sharing one identity",
                |lesson| lesson["segments"][1]["id"] = lesson["segments"][0]["id"].clone(),
                LessonError::DuplicateSegmentId(String::new()),
                "/segments/1/id",
            ),
            (
                "nothing to synthesize",
                |lesson| lesson["segments"][0]["spoken_text"] = Value::String("  ".to_owned()),
                LessonError::MissingSpokenText(String::new()),
                "/segments/0/spoken_text",
            ),
            (
                "spoken text past its ceiling",
                |lesson| {
                    lesson["segments"][0]["spoken_text"] =
                        Value::String("x".repeat(MAX_SEGMENT_TEXT_BYTES + 1));
                },
                LessonError::SpokenTextTooLong {
                    segment_id: String::new(),
                    bytes: 0,
                    max_bytes: 0,
                },
                "/segments/0/spoken_text",
            ),
            (
                "nothing for a reviewer to read",
                |lesson| lesson["segments"][0]["display_text"] = Value::String("  ".to_owned()),
                LessonError::MissingDisplayText(String::new()),
                "/segments/0/display_text",
            ),
            (
                "display text past its ceiling",
                |lesson| {
                    lesson["segments"][0]["display_text"] =
                        Value::String("x".repeat(MAX_SEGMENT_TEXT_BYTES + 1));
                },
                LessonError::DisplayTextTooLong {
                    segment_id: String::new(),
                    bytes: 0,
                    max_bytes: 0,
                },
                "/segments/0/display_text",
            ),
            (
                "a claim traced to nothing",
                |lesson| lesson["segments"][0]["source_refs"] = Value::Array(Vec::new()),
                LessonError::MissingSourceRefs(String::new()),
                "/segments/0/source_refs",
            ),
            (
                "more citations than this build accepts",
                |lesson| {
                    lesson["segments"][0]["source_refs"] =
                        Value::Array(vec![
                            Value::String("block-001".to_owned());
                            MAX_SOURCE_REFS_PER_SEGMENT + 1
                        ]);
                },
                LessonError::TooManySourceRefs {
                    segment_id: String::new(),
                    found: 0,
                    max: 0,
                },
                "/segments/0/source_refs",
            ),
            (
                "a citation that traces to nothing",
                |lesson| {
                    lesson["segments"][0]["source_refs"] =
                        Value::Array(vec![Value::String("  ".to_owned())]);
                },
                LessonError::EmptySourceRef(String::new()),
                "/segments/0/source_refs",
            ),
            (
                "no human approval",
                |lesson| lesson["segments"][0]["review_status"] = Value::String("draft".to_owned()),
                LessonError::UnapprovedSegment(String::new()),
                "/segments/0/review_status",
            ),
            (
                "no voice named",
                |lesson| lesson["segments"][0]["speaker"] = Value::String(String::new()),
                LessonError::MissingSpeaker(String::new()),
                "/segments/0/speaker",
            ),
            (
                "a voice the document never bound",
                |lesson| lesson["segments"][0]["speaker"] = Value::String("ghost".to_owned()),
                LessonError::UndeclaredSpeaker {
                    segment_id: String::new(),
                    speaker: String::new(),
                },
                "/segments/0/speaker",
            ),
            (
                "a citation past its ceiling",
                |lesson| {
                    lesson["segments"][0]["source_refs"] =
                        Value::Array(vec![Value::String("x".repeat(MAX_SOURCE_REF_BYTES + 1))]);
                },
                LessonError::SourceRefTooLong {
                    segment_id: String::new(),
                    bytes: 0,
                    max_bytes: 0,
                },
                "/segments/0/source_refs",
            ),
            (
                "a pause that would read as a fault",
                |lesson| {
                    lesson["segments"][0]["pause_after_ms"] =
                        Value::Number((MAX_PAUSE_AFTER_MS + 1).into());
                },
                LessonError::PauseOutOfRange(String::new()),
                "/segments/0/pause_after_ms",
            ),
            (
                "a recall prompt the listener cannot answer in",
                |lesson| {
                    lesson["segments"][0]["role"] = Value::String("recall_prompt".to_owned());
                    lesson["segments"][0]["pause_after_ms"] =
                        Value::Number((MIN_RECALL_RESPONSE_MS - 1).into());
                },
                LessonError::RecallPromptWithoutResponseInterval {
                    segment_id: String::new(),
                    pause_after_ms: 0,
                    min_ms: 0,
                },
                "/segments/0/pause_after_ms",
            ),
            (
                "a recall prompt left open past the pause policy",
                |lesson| {
                    lesson["segments"][0]["role"] = Value::String("recall_prompt".to_owned());
                    lesson["segments"][0]["pause_after_ms"] =
                        Value::Number((MAX_RECALL_RESPONSE_MS + 1).into());
                },
                LessonError::RecallPromptResponseIntervalTooLong {
                    segment_id: String::new(),
                    pause_after_ms: 0,
                    max_ms: 0,
                },
                "/segments/0/pause_after_ms",
            ),
            (
                "more objectives than this build accepts",
                |lesson| {
                    lesson["learning_objectives"] =
                        Value::Array(vec![
                            Value::String("Explain it".to_owned());
                            MAX_LEARNING_OBJECTIVES + 1
                        ]);
                },
                LessonError::TooManyLearningObjectives { found: 0, max: 0 },
                "/learning_objectives",
            ),
            (
                "an objective that states nothing",
                |lesson| {
                    lesson["learning_objectives"] =
                        Value::Array(vec![Value::String("  ".to_owned())]);
                },
                LessonError::EmptyLearningObjective,
                "/learning_objectives",
            ),
            (
                "an objective past its ceiling",
                |lesson| {
                    lesson["learning_objectives"] = Value::Array(vec![Value::String(
                        "x".repeat(MAX_LEARNING_OBJECTIVE_BYTES + 1),
                    )]);
                },
                LessonError::LearningObjectiveTooLong {
                    bytes: 0,
                    max_bytes: 0,
                },
                "/learning_objectives",
            ),
            (
                "more source references than this build accepts",
                |lesson| {
                    lesson["source"]["references"] =
                        Value::Array(vec![
                            Value::String("https://example.test/paper".to_owned());
                            MAX_LESSON_REFERENCES + 1
                        ]);
                },
                LessonError::TooManyLessonReferences { found: 0, max: 0 },
                "/source/references",
            ),
            (
                "a source reference that traces to nothing",
                |lesson| {
                    lesson["source"]["references"] =
                        Value::Array(vec![Value::String("  ".to_owned())]);
                },
                LessonError::EmptyLessonReference,
                "/source/references",
            ),
            (
                "a source reference past its ceiling",
                |lesson| {
                    lesson["source"]["references"] = Value::Array(vec![Value::String(
                        "x".repeat(MAX_LESSON_REFERENCE_BYTES + 1),
                    )]);
                },
                LessonError::LessonReferenceTooLong {
                    bytes: 0,
                    max_bytes: 0,
                },
                "/source/references",
            ),
            // The two forms `/source/content_hash` can fail in, together
            // because they are what tell the classifier from the shape
            // refusal: a string the digest rule refuses earns its own
            // invariant, while a value of the wrong type is the document not
            // having the declared shape, which stays `InvalidJson` for the
            // reason `vocabulary_refusals` gives.
            (
                "a source hash that is not a digest",
                |lesson| {
                    lesson["source"]["content_hash"] = Value::String("not-a-digest".to_owned());
                },
                LessonError::MalformedSourceContentHash(
                    "not-a-digest"
                        .parse::<SourceContentHash>()
                        .expect_err("`not-a-digest` is not a BLAKE3 digest"),
                ),
                "/source/content_hash",
            ),
            (
                "a source hash that is not a string",
                |lesson| {
                    lesson["source"]["content_hash"] = Value::Number(7.into());
                },
                LessonError::InvalidJson(shape_error()),
                "/source/content_hash",
            ),
        ];

        let mut seen = Vec::with_capacity(cases.len());
        for (case, mutate, expected, field_path) in cases {
            let mut lesson = fixture();
            mutate(&mut lesson);
            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&lesson).expect("the mutated lesson serializes"),
            )
            .expect_err(case);

            assert_eq!(
                discriminant(diagnostic.error()),
                discriminant(&expected),
                "`{case}` was refused by the wrong invariant: {diagnostic}"
            );
            assert_eq!(diagnostic.field_path(), field_path, "`{case}` field path");
            assert_eq!(diagnostic.document(), DOCUMENT, "`{case}` document");
            seen.push(refusal(&diagnostic));
        }

        // Written out rather than added to the table above, because the case
        // is a repeated JSON key and `serde_json::Value` is a map: the one
        // invariant whose document cannot be built by mutating the fixture.
        let diagnostic = ValidatedLesson::from_json(
            DOCUMENT,
            &lesson_declaring_a_speaker_twice("synthetic-test-voice-v2"),
        )
        .expect_err("one speaker bound to two voices");
        assert_eq!(
            discriminant(diagnostic.error()),
            discriminant(&LessonError::DuplicateSpeaker(String::new()))
        );
        assert_eq!(diagnostic.field_path(), "/speakers/nadia");
        seen.push(refusal(&diagnostic));

        // `serde` refuses all three closed vocabularies before this module sees
        // a value, so each is classified back into its own invariant on the
        // refusal path by `vocabulary_refusal`. Every field is exercised in
        // every form it can fail in — absent, outside the vocabulary, and not a
        // string at all — because sampling one form per field is what let a
        // missing role and an unknown role share a refusal while a test asking
        // only for the unknown one passed. A wrong type stays `InvalidJson` on
        // purpose: it is the document not having the shape the schema declares,
        // which is one invariant however many fields can violate it, and it is
        // told apart from the other two by its pointer.
        for (form, mutate, expected) in vocabulary_refusals() {
            let mut lesson = fixture();
            let field = mutate(&mut lesson);
            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&lesson).expect("the mutated lesson serializes"),
            )
            .expect_err(form);

            assert_eq!(
                discriminant(diagnostic.error()),
                discriminant(&expected),
                "`{form}` was refused by the wrong invariant: {diagnostic}"
            );
            assert_eq!(
                diagnostic.field_path(),
                format!("/segments/0/{field}"),
                "`{form}` field path"
            );
            assert_eq!(diagnostic.segment_id(), Some("seg-0001"), "`{form}`");
            seen.push(refusal(&diagnostic));
        }

        // Two refusals are about no field at all: bytes that were never parsed,
        // and an aggregate no single field exceeds.
        let oversized = vec![b'{'; MAX_LESSON_JSON_BYTES + 1];
        let diagnostic = ValidatedLesson::from_json(DOCUMENT, &oversized)
            .expect_err("more bytes than this build parses");
        assert!(matches!(
            diagnostic.error(),
            LessonError::LessonJsonTooLarge { .. }
        ));
        assert_eq!(diagnostic.field_path(), "");
        assert_eq!(diagnostic.segment_id(), None);
        seen.push(refusal(&diagnostic));

        let mut aggregate = authored_fixture();
        aggregate.title = "t".repeat(MAX_AUTHORED_TEXT_BYTES);
        let diagnostic = aggregate
            .validate(DOCUMENT)
            .expect_err("authored text past the aggregate ceiling");
        assert_eq!(
            discriminant(diagnostic.error()),
            discriminant(&LessonError::AuthoredTextTooLarge { max_bytes: 0 })
        );
        assert_eq!(diagnostic.field_path(), "");
        seen.push(refusal(&diagnostic));

        // The claim the delivery plan names: one invariant, one refusal. The
        // key is the *located* refusal, because a variant alone cannot carry
        // the claim — `serde` answers all three closed vocabularies with
        // `InvalidJson`, and the pointer is what a caller reads to tell them
        // apart. Two invariants agreeing on both halves is what fails here.
        for (index, left) in seen.iter().enumerate() {
            for right in &seen[index + 1..] {
                assert_ne!(left, right, "two cases share one located refusal");
            }
        }

        // And every variant is still reached. A new `LessonError` variant
        // arrives in this file through `field_of`, whose match has no `_` arm;
        // this count is what then fails until it has a case above.
        let mut variants = Vec::with_capacity(seen.len());
        for (variant, _) in &seen {
            if !variants.contains(variant) {
                variants.push(*variant);
            }
        }
        assert_eq!(
            variants.len(),
            declared_lesson_error_variants(),
            "every `LessonError` variant needs a case"
        );
    }

    /// How many variants [`LessonError`] declares, read from this module.
    ///
    /// Derived rather than written down: a number kept by hand agrees with any
    /// enum, so a variant added together with its `field_of` arm would leave
    /// the count assertion above passing while no case exercised the refusal.
    /// Parser drift cannot weaken that assertion — it can only report a count
    /// the case table does not match, which fails.
    ///
    /// A variant is a four-space-indented capitalized name; every other line at
    /// that indentation is a doc comment, an attribute, a field, or a closing
    /// brace. `crates/study-tts-testkit/tests/error_documentation.rs` reads an
    /// enum out of its module the same way, and the reading is repeated rather
    /// than shared because this crate cannot depend on the testkit that depends
    /// on it.
    fn declared_lesson_error_variants() -> usize {
        let (_, body) = include_str!("lesson.rs")
            .split_once("pub enum LessonError {")
            .expect("this module declares `LessonError`");
        let (body, _) = body
            .split_once("\n}\n")
            .expect("`LessonError` has a closing brace");
        body.lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("    ")?;
                let name = rest[..rest.find(['(', '{', ','])?].trim_end();
                let is_variant =
                    name.starts_with(char::is_uppercase) && name.chars().all(char::is_alphanumeric);
                is_variant.then_some(())
            })
            .count()
    }

    /// The refusal a caller actually receives: which invariant, and where.
    ///
    /// Both halves, because `LessonError::InvalidJson` is one variant covering
    /// every shape the deserializer refuses, so the variant alone cannot say
    /// which invariant was violated.
    fn refusal(diagnostic: &LessonDiagnostic) -> (Discriminant<LessonError>, String) {
        (
            discriminant(diagnostic.error()),
            diagnostic.field_path().to_owned(),
        )
    }

    /// Text that a normalizer, a serializer, or a hash could quietly change.
    ///
    /// Every entry is either a Unicode shape with more than one valid encoding
    /// or a protected term ADR-0001 §9.3 forbids rewriting. The lesson format
    /// carries hand-authored `spoken_text` (`DELIVERY-PLAN.md` E1-S2 task 1),
    /// so anything here reaching synthesis altered would be a pronunciation
    /// change nobody reviewed.
    const HOSTILE_TEXT: [(&str, &str); 10] = [
        ("precomposed", "Caf\u{e9} r\u{e9}sum\u{e9}."),
        ("decomposed", "Cafe\u{301} re\u{301}sume\u{301}."),
        (
            "right to left",
            "\u{627}\u{644}\u{627}\u{62e}\u{62a}\u{628}\u{627}\u{631}",
        ),
        ("zero-width joiner", "\u{1f469}\u{200d}\u{1f4bb} at work."),
        ("zero-width space", "sub\u{200b}word"),
        ("CJK", "\u{6f22}\u{5b57}\u{30c6}\u{30b9}\u{30c8}"),
        ("qualified identifier", "Number.MAX_SAFE_INTEGER"),
        ("path-shaped identifier", "std::collections::BTreeMap"),
        ("complexity notation", "O(n log n) in the worst case."),
        ("escape-shaped", "a \"quoted\" \\ backslash\tand a tab"),
    ];

    #[test]
    fn t2_e1_unicode_and_protected_terms_survive_round_trip() {
        let context = crate::identity::sample_context();

        for (case, text) in HOSTILE_TEXT {
            let mut authored = fixture();
            authored["segments"][0]["spoken_text"] = Value::String(text.to_owned());
            authored["segments"][0]["display_text"] = Value::String(format!("display: {text}"));
            let bytes = serde_json::to_vec(&authored).expect("the lesson serializes");

            let lesson = ValidatedLesson::from_json(DOCUMENT, &bytes)
                .unwrap_or_else(|error| panic!("`{case}` must be valid: {error}"));

            // Bytes in, bytes out: a normalizer between the two would change
            // what a reviewer approved.
            assert_eq!(lesson.segments()[0].spoken_text, text, "parsed `{case}`");

            // Reserializing must reproduce the document, so an edit-and-save
            // pass cannot alter approved text.
            let reserialized =
                serde_json::to_vec(&lesson.authored).expect("the lesson reserializes");
            assert_eq!(
                ValidatedLesson::from_json(DOCUMENT, &reserialized)
                    .expect("the reserialized lesson is valid")
                    .segments()[0]
                    .spoken_text,
                text,
                "round-tripped `{case}`"
            );

            // And what planning carries into synthesis is the same bytes
            // again, which is the only copy the worker ever sees.
            let plan = crate::RenderPlan::for_lesson(&lesson, &context)
                .expect("the fixture context resolves every speaker");
            assert_eq!(plan.segments[0].spoken_text, text, "planned `{case}`");
        }

        // Two encodings of one rendered string are two different sounds to
        // ask for, so they must not share a cache entry. This is the case a
        // silent normalization would break, and it would break it by making
        // these equal.
        let key_for = |text: &str| {
            let mut authored = fixture();
            authored["segments"][0]["spoken_text"] = Value::String(text.to_owned());
            let lesson = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&authored).expect("the lesson serializes"),
            )
            .expect("the lesson is valid");
            crate::RenderPlan::for_lesson(&lesson, &context)
                .expect("the fixture context resolves every speaker")
                .segments[0]
                .cache_key
                .clone()
        };
        assert_ne!(key_for(HOSTILE_TEXT[0].1), key_for(HOSTILE_TEXT[1].1));
    }

    #[test]
    fn t1_e1_a_field_path_escapes_the_name_it_points_through() {
        // A speaker name is authored text under no portability rule, so it can
        // carry the two characters RFC 6901 gives meaning to. Unescaped, a
        // speaker named `a/b` would produce a pointer naming a path no
        // document has.
        let mut lesson = fixture();
        lesson["speakers"]["a/b~c"] = serde_json::json!({ "voice_profile": "" });

        let diagnostic = ValidatedLesson::from_json(
            DOCUMENT,
            &serde_json::to_vec(&lesson).expect("the lesson serializes"),
        )
        .expect_err("a speaker declared with no voice profile must be refused");

        assert!(matches!(
            diagnostic.error(),
            LessonError::MissingVoiceProfile(speaker) if speaker == "a/b~c"
        ));
        assert_eq!(diagnostic.field_path(), "/speakers/a~1b~0c/voice_profile");
    }

    /// One malformed document, and where the refusal must point.
    ///
    /// `serde` raises all of these before this module sees the data, so
    /// without a located deserializer each would arrive as "invalid JSON at
    /// line N" with no field and no segment. `DELIVERY-PLAN.md` E1-S2 requires
    /// source, segment ID, and field path, and this is the half of that
    /// requirement `field_of` cannot reach.
    type ShapeRefusal = (
        &'static str,
        fn(&mut Value),
        &'static str,
        Option<&'static str>,
    );

    #[test]
    fn t1_e1_a_shape_error_is_located_at_the_field_it_is_about() {
        let cases: [ShapeRefusal; 6] = [
            (
                "a closed-vocabulary field carrying no string at all",
                |lesson| lesson["segments"][1]["review_status"] = Value::from(3),
                "/segments/1/review_status",
                Some("seg-0002"),
            ),
            (
                "a segment field of the wrong type",
                |lesson| lesson["segments"][0]["pause_after_ms"] = Value::String("75".to_owned()),
                "/segments/0/pause_after_ms",
                Some("seg-0001"),
            ),
            (
                // Deliberately not one of the three closed vocabularies: those
                // are classified out of `InvalidJson` into their own missing
                // variants, and the distinctness test is where their pointers
                // are asserted. This case pins that the *deserializer* locates
                // an omitted key, which needs a field whose absence is still a
                // shape refusal.
                "a segment field the document omits",
                |lesson| {
                    lesson["segments"][1]
                        .as_object_mut()
                        .expect("a segment is an object")
                        .remove("spoken_text");
                },
                "/segments/1/spoken_text",
                Some("seg-0002"),
            ),
            (
                "a lesson field the document omits",
                |lesson| {
                    lesson
                        .as_object_mut()
                        .expect("a lesson is an object")
                        .remove("title");
                },
                "/title",
                None,
            ),
            (
                "a field no version of this document declares",
                |lesson| lesson["difficulty"] = Value::String("intermediate".to_owned()),
                "/difficulty",
                None,
            ),
            (
                "a speaker binding of the wrong shape",
                |lesson| lesson["speakers"]["nadia"] = Value::String("nadia-v1".to_owned()),
                "/speakers/nadia",
                None,
            ),
        ];

        for (case, mutate, field_path, segment_id) in cases {
            let mut lesson = fixture();
            mutate(&mut lesson);

            let diagnostic = ValidatedLesson::from_json(
                DOCUMENT,
                &serde_json::to_vec(&lesson).expect("the mutated lesson serializes"),
            )
            .expect_err(case);

            assert!(
                matches!(diagnostic.error(), LessonError::InvalidJson(_)),
                "`{case}` must be refused by the parser: {diagnostic}"
            );
            assert_eq!(diagnostic.document(), DOCUMENT, "`{case}` document");
            assert_eq!(diagnostic.field_path(), field_path, "`{case}` field path");
            assert_eq!(diagnostic.segment_id(), segment_id, "`{case}` segment");
        }
    }

    #[test]
    fn t1_e1_bytes_that_are_not_json_name_the_document_and_nothing_else() {
        // The one shape refusal with no field to name: there is no document to
        // point into, so the empty pointer RFC 6901 defines as the whole
        // document is the honest answer rather than a guessed field.
        let diagnostic = ValidatedLesson::from_json(DOCUMENT, b"{not json")
            .expect_err("bytes that are not JSON must be refused");

        assert!(matches!(diagnostic.error(), LessonError::InvalidJson(_)));
        assert_eq!(diagnostic.document(), DOCUMENT);
        assert_eq!(diagnostic.field_path(), "");
        assert_eq!(diagnostic.segment_id(), None);
        assert!(
            diagnostic.to_string().contains("line"),
            "a refusal with no field must still say where to look: {diagnostic}"
        );
    }

    #[test]
    fn t1_e1_content_after_the_lesson_document_is_refused() {
        // A located deserializer stops at the end of the first JSON value, so
        // the boundary has to reject the rest itself. Accepting a valid lesson
        // followed by anything would leave bytes in the file that no
        // validation, checksum, or review ever reads.
        let mut bytes = serde_json::to_vec(&fixture()).expect("the fixture serializes");
        bytes.extend_from_slice(b" {}");

        let diagnostic = ValidatedLesson::from_json(DOCUMENT, &bytes)
            .expect_err("a document carrying a second JSON value must be refused");

        assert!(
            matches!(diagnostic.error(), LessonError::InvalidJson(_)),
            "trailing content must be refused by the parser: {diagnostic}"
        );
        assert_eq!(diagnostic.document(), DOCUMENT);
        assert_eq!(diagnostic.field_path(), "");
        assert_eq!(diagnostic.segment_id(), None);
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
        ValidatedLesson::from_json(DOCUMENT, &exact_bytes)
            .expect("the byte boundary must be accepted");

        let oversized = vec![b'{'; MAX_LESSON_JSON_BYTES + 1];
        assert!(matches!(
            ValidatedLesson::from_json(DOCUMENT, &oversized)
                .map_err(|diagnostic| diagnostic.error),
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
        validate_authored(authored).expect("the segment-count boundary must be accepted");

        let mut authored = authored_fixture();
        authored.segments = segments;
        authored.segments.push(LessonSegment {
            id: "one-too-many".to_owned(),
            ..segment
        });
        assert!(matches!(
            validate_authored(authored),
            Err(LessonError::TooManySegments { found, max })
                if found == MAX_LESSON_SEGMENTS + 1 && max == MAX_LESSON_SEGMENTS
        ));
    }

    #[test]
    fn t1_e0_spoken_text_limit_counts_utf8_bytes() {
        let mut exact = authored_fixture();
        exact.segments[0].spoken_text = "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2);
        validate_authored(exact).expect("the spoken-text byte boundary must be accepted");

        let mut oversized = authored_fixture();
        oversized.segments[0].spoken_text = format!("{}a", "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2));
        assert!(matches!(
            validate_authored(oversized),
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
        validate_authored(exact).expect("the display-text byte boundary must be accepted");

        let mut oversized = authored_fixture();
        oversized.segments[0].display_text = format!("{}a", "é".repeat(MAX_SEGMENT_TEXT_BYTES / 2));
        assert!(matches!(
            validate_authored(oversized),
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
        validate_authored(exact_count)
            .expect("the source-reference count boundary must be accepted");

        let mut oversized_count = authored_fixture();
        oversized_count.segments[0].source_refs =
            vec!["x".to_owned(); MAX_SOURCE_REFS_PER_SEGMENT + 1];
        assert!(matches!(
            validate_authored(oversized_count),
            Err(LessonError::TooManySourceRefs { segment_id, found, max })
                if segment_id == "seg-0001"
                    && found == MAX_SOURCE_REFS_PER_SEGMENT + 1
                    && max == MAX_SOURCE_REFS_PER_SEGMENT
        ));

        let mut exact_length = authored_fixture();
        exact_length.segments[0].source_refs = vec!["é".repeat(MAX_SOURCE_REF_BYTES / 2)];
        validate_authored(exact_length)
            .expect("the source-reference byte boundary must be accepted");

        let mut oversized_length = authored_fixture();
        oversized_length.segments[0].source_refs =
            vec![format!("{}a", "é".repeat(MAX_SOURCE_REF_BYTES / 2))];
        assert!(matches!(
            validate_authored(oversized_length),
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
        // One byte for each of the speaker name, its voice profile, and the
        // segment's four counted fields plus its one source reference. The
        // role and the style are closed vocabularies, so neither counts.
        const OTHER_AUTHORED_BYTES: usize = 7;
        let lesson_with_title = |title_bytes| AuthoredLesson {
            schema: None,
            schema_version: LESSON_SCHEMA_VERSION.to_string(),
            lesson_id: "aggregate-boundary".to_owned(),
            title: "t".repeat(title_bytes),
            language: "en".to_owned(),
            learning_objectives: Vec::new(),
            source: None,
            speakers: BTreeMap::from([(
                "s".to_owned(),
                SpeakerDeclaration {
                    voice_profile: "v".to_owned(),
                },
            )]),
            segments: vec![LessonSegment {
                id: "i".to_owned(),
                speaker: "s".to_owned(),
                role: SegmentRole::Explanation,
                source_refs: vec!["x".to_owned()],
                display_text: "d".to_owned(),
                spoken_text: "p".to_owned(),
                style: DeliveryStyle::Calm,
                pause_after_ms: 0,
                review_status: ReviewStatus::Approved,
                editorial: false,
            }],
        };

        validate_authored(lesson_with_title(
            MAX_AUTHORED_TEXT_BYTES - OTHER_AUTHORED_BYTES,
        ))
        .expect("the aggregate authored-text boundary must be accepted");
        assert!(matches!(
            validate_authored(lesson_with_title(
                MAX_AUTHORED_TEXT_BYTES - OTHER_AUTHORED_BYTES + 1
            )),
            Err(LessonError::AuthoredTextTooLarge { max_bytes })
                if max_bytes == MAX_AUTHORED_TEXT_BYTES
        ));
    }
}
