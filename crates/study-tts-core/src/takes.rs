//! Versioned take-selection documents required by ADR-0001 §12.2.
//!
//! This module validates record-internal invariants. E2-S2 owns comparing a
//! selection with a live plan and cache.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CacheKey, SchemaVersion, SchemaVersionError,
    lesson::{MAX_LESSON_SEGMENTS, validate_lesson_id, validate_segment_id},
    verification::AudioDigest,
};

/// Layout version this build publishes for a takes document.
pub const TAKES_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// File-name stem of the published takes schema, per ADR-0001 §7.1.
pub const TAKES_SCHEMA_STEM: &str = "takes";

/// Largest takes document accepted, in UTF-8 bytes.
///
/// Mirrors `docs/architecture/WALKING-SKELETON.md` §Provisional resource
/// ceilings, which names this constant in return.
pub const MAX_TAKES_JSON_BYTES: usize = 8 * 1024 * 1024;

/// One segment's recorded take selection, in the ADR-0001 §12.2 shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SelectedTake {
    /// Segment this selection belongs to.
    ///
    /// Held to the same rule `crate::lesson` applies, through
    /// [`validate_segment_id`]: a selection naming an identity no lesson can
    /// carry approves a segment that cannot exist.
    #[schemars(schema_with = "crate::lesson::portable_id_json_schema")]
    pub segment_id: String,
    /// Synthesis base key — the segment's take-zero identity — this selection
    /// was made against.
    ///
    /// Present so a later build can refuse a selection whose base key no longer
    /// matches the current plan, rather than applying an approval given for
    /// different words.
    pub synthesis_base_key: CacheKey,
    /// Take the human chose.
    pub selected_take: u32,
    /// Synthesis identity of the chosen take, which names its cache entry.
    pub selected_cache_key: CacheKey,
    /// Digest of the audio that was approved.
    pub audio_blake3: AudioDigest,
}

/// A takes document as it is written on disk and before validation.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct TakesDocument {
    /// Published schema this document links to; absent is its declared default.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "schema_link_json_schema")]
    pub schema: Option<String>,
    /// Schema this document claims, as authored text.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: String,
    /// Lesson these selections belong to.
    pub lesson_id: String,
    /// One selection per segment, in the lesson's speaking order.
    ///
    /// Bounded by the lesson's own segment ceiling: ADR-0001 §12.2 requires one
    /// selection per segment, so a document carrying more selections than any
    /// lesson can have segments describes no lesson this build could plan.
    #[schemars(length(max = MAX_LESSON_SEGMENTS))]
    pub selections: Vec<SelectedTake>,
}

/// A takes document whose document-level invariants have passed validation.
///
/// Private fields prevent unchecked construction at the selection boundary.
#[derive(Clone, Debug)]
pub struct ValidatedTakes {
    authored: TakesDocument,
    schema_version: SchemaVersion,
}

/// Why a takes document was refused.
#[derive(Debug, Error)]
pub enum TakesError {
    /// The input exceeds the fixed envelope within which parsing is allowed.
    #[error("takes JSON exceeds the provisional {max_bytes}-byte limit")]
    TakesJsonTooLarge {
        /// Largest takes document this build accepts.
        max_bytes: usize,
    },
    /// The bytes are not JSON, or not the shape this schema declares.
    #[error("takes JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// This build does not know that version and will not guess.
    #[error("takes schema version is unusable: {0}")]
    UnsupportedSchema(#[from] SchemaVersionError),
    /// The document links to a schema other than the one for its version.
    #[error(
        "takes document links to schema `{declared}` but declares version `{version}`, whose \
         schema is `{expected}`; the document's author must correct the link or the version"
    )]
    UnexpectedSchemaLink {
        /// Link the document carries.
        declared: String,
        /// Version the document declares.
        version: SchemaVersion,
        /// Link that version requires.
        expected: String,
    },
    /// The lesson identifier is absent or could not name a directory.
    #[error("takes document lesson identity is unusable: {0}")]
    InvalidLessonId(#[from] crate::LessonError),
    /// A selection names a segment identity no lesson could carry.
    ///
    /// Kept separate from [`TakesError::InvalidLessonId`] so the author knows
    /// which identifier to fix.
    #[error("takes document segment identity is unusable: {0}")]
    InvalidSegmentId(#[source] crate::LessonError),
    /// A production release requires a selection for every segment, so an empty
    /// document is an authoring mistake rather than "no retakes".
    #[error(
        "takes document contains no selections; ADR-0001 §12.2 requires an explicit selection for \
         every segment even when every take remains zero, so the reviewer must record the \
         selections rather than omit the list"
    )]
    MissingSelections,
    /// The document cannot correspond to a lesson within the segment ceiling.
    #[error(
        "takes document records {found} selections, exceeding the provisional limit of {max}; \
         ADR-0001 §12.2 requires one selection per lesson segment, so the reviewer must record \
         the selections for a lesson this build can plan"
    )]
    TooManySelections {
        /// Selections the authored document contains.
        found: usize,
        /// Largest selection count this build accepts.
        max: usize,
    },
    /// Two selections name one segment, so which was approved is unknowable.
    #[error("segment `{0}` is selected more than once")]
    DuplicateSelection(String),
    /// Take zero names a cache key other than its synthesis base key.
    #[error(
        "segment `{segment_id}` selects take zero but its selected cache key differs from its \
         synthesis base key; the reviewer must re-record the selection from the cache entry that \
         was reviewed rather than edit either key"
    )]
    BaseTakeKeyMismatch {
        /// Segment carrying the mismatched key.
        segment_id: String,
    },
    /// A non-zero take names the take-zero synthesis base key.
    #[error(
        "segment `{segment_id}` selects take {selected_take} but its selected cache key equals its \
         synthesis base key; the reviewer must re-record the selection from the cache entry that \
         was reviewed rather than edit either key"
    )]
    RetakeUsesBaseKey {
        /// Segment carrying the take-zero key.
        segment_id: String,
        /// Non-zero take the selection claims.
        selected_take: u32,
    },
}

impl TakesDocument {
    /// Validates a takes document against the invariants that hold without a
    /// plan.
    ///
    /// Selections are *not* checked against a live plan here; ADR-0001 §12.2
    /// gives that check a plan to compare against, and this module has none.
    ///
    /// # Errors
    ///
    /// [`TakesError::UnsupportedSchema`] for a version this build cannot read,
    /// [`TakesError::UnexpectedSchemaLink`] for a link naming another schema,
    /// [`TakesError::InvalidLessonId`] for an identifier that could not name a
    /// directory, [`TakesError::MissingSelections`] for an empty document,
    /// [`TakesError::TooManySelections`] past the lesson's own segment ceiling,
    /// [`TakesError::InvalidSegmentId`] for a selection naming an identity no
    /// lesson could carry, [`TakesError::DuplicateSelection`] when two
    /// selections name one segment, [`TakesError::BaseTakeKeyMismatch`] when
    /// take zero does not name its base key, and
    /// [`TakesError::RetakeUsesBaseKey`] when a non-zero take names that key.
    pub fn validate(self) -> Result<ValidatedTakes, TakesError> {
        let schema_version: SchemaVersion = self.schema_version.parse()?;
        schema_version.accepted_by(TAKES_SCHEMA_VERSION)?;
        if let Some(declared) = &self.schema {
            let expected = crate::schema_uri(TAKES_SCHEMA_STEM, schema_version.major());
            if declared != &expected {
                return Err(TakesError::UnexpectedSchemaLink {
                    declared: declared.clone(),
                    version: schema_version,
                    expected,
                });
            }
        }
        validate_lesson_id(&self.lesson_id)?;
        if self.selections.is_empty() {
            return Err(TakesError::MissingSelections);
        }
        if self.selections.len() > MAX_LESSON_SEGMENTS {
            return Err(TakesError::TooManySelections {
                found: self.selections.len(),
                max: MAX_LESSON_SEGMENTS,
            });
        }

        let mut segments = HashSet::with_capacity(self.selections.len());
        for selection in &self.selections {
            validate_segment_id(&selection.segment_id).map_err(TakesError::InvalidSegmentId)?;
            if !segments.insert(selection.segment_id.as_str()) {
                return Err(TakesError::DuplicateSelection(selection.segment_id.clone()));
            }
            let keys_match = selection.selected_cache_key == selection.synthesis_base_key;
            let is_base_take = selection.selected_take == crate::BASE_TAKE;
            if is_base_take && !keys_match {
                return Err(TakesError::BaseTakeKeyMismatch {
                    segment_id: selection.segment_id.clone(),
                });
            }
            if !is_base_take && keys_match {
                return Err(TakesError::RetakeUsesBaseKey {
                    segment_id: selection.segment_id.clone(),
                    selected_take: selection.selected_take,
                });
            }
        }

        Ok(ValidatedTakes {
            authored: self,
            schema_version,
        })
    }
}

impl ValidatedTakes {
    /// Parses and validates a takes document.
    ///
    /// # Errors
    ///
    /// [`TakesError::TakesJsonTooLarge`] when the input exceeds
    /// [`MAX_TAKES_JSON_BYTES`], then [`TakesError::UnsupportedSchema`] for a
    /// version this build cannot read, [`TakesError::InvalidJson`] when the
    /// bytes are not this document's shape, and every variant documented by
    /// [`TakesDocument::validate`].
    pub fn from_json(bytes: &[u8]) -> Result<Self, TakesError> {
        if bytes.len() > MAX_TAKES_JSON_BYTES {
            return Err(TakesError::TakesJsonTooLarge {
                max_bytes: MAX_TAKES_JSON_BYTES,
            });
        }
        // A strict parse would misreport a future field before its version.
        crate::schema::check_declared_version(bytes, TAKES_SCHEMA_VERSION)?;
        let document: TakesDocument = serde_json::from_slice(bytes)?;
        document.validate()
    }

    /// The accepted schema version this document declared.
    pub fn schema_version(&self) -> SchemaVersion {
        self.schema_version
    }

    /// The lesson these selections belong to.
    pub fn lesson_id(&self) -> &str {
        &self.authored.lesson_id
    }

    /// The validated selections, in the order they were recorded.
    pub fn selections(&self) -> &[SelectedTake] {
        &self.authored.selections
    }
}

/// Publishes exactly the versions accepted by [`ValidatedTakes`].
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::accepted_versions_json_schema(TAKES_SCHEMA_VERSION)
}

/// Publishes the one link a document of this major may carry.
///
/// The published half of [`TakesError::UnexpectedSchemaLink`]: without it the
/// schema admits any string, and an author's editor stays green on a link the
/// build refuses.
fn schema_link_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::schema_link_json_schema(TAKES_SCHEMA_STEM, TAKES_SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(fill: &str) -> CacheKey {
        fill.repeat(CacheKey::LENGTH)
            .parse()
            .expect("a repeated hex digit is a well-formed key")
    }

    fn document() -> TakesDocument {
        TakesDocument {
            schema: None,
            schema_version: TAKES_SCHEMA_VERSION.to_string(),
            lesson_id: "e0-s0-walking-skeleton".to_owned(),
            selections: vec![SelectedTake {
                segment_id: "seg-0001".to_owned(),
                synthesis_base_key: key("a"),
                selected_take: crate::BASE_TAKE,
                selected_cache_key: key("a"),
                audio_blake3: blake3::hash(b"audio").into(),
            }],
        }
    }

    #[test]
    fn t1_e1_a_base_take_selection_must_name_its_base_key() {
        let mut mismatched = document();
        mismatched.selections[0].selected_cache_key = key("b");

        assert!(matches!(
            mismatched.validate(),
            Err(TakesError::BaseTakeKeyMismatch { segment_id })
                if segment_id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e1_a_retake_selection_must_not_name_its_base_key() {
        let mut mismatched = document();
        mismatched.selections[0].selected_take = crate::BASE_TAKE + 1;

        assert!(matches!(
            mismatched.validate(),
            Err(TakesError::RetakeUsesBaseKey {
                segment_id,
                selected_take: 1,
            }) if segment_id == "seg-0001"
        ));

        let mut retake = document();
        retake.selections[0].selected_take = crate::BASE_TAKE + 1;
        retake.selections[0].selected_cache_key = key("b");
        assert!(retake.validate().is_ok());
    }

    #[test]
    fn t1_e1_two_selections_for_one_segment_are_refused() {
        let mut duplicated = document();
        let repeat = duplicated.selections[0].clone();
        duplicated.selections.push(repeat);

        assert!(matches!(
            duplicated.validate(),
            Err(TakesError::DuplicateSelection(id)) if id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e1_a_selection_naming_an_identity_no_lesson_can_carry_is_refused() {
        for unusable in ["", "   ", "../escape", "seg/0001", ".hidden"] {
            let mut document = document();
            document.selections[0].segment_id = unusable.to_owned();

            assert!(
                matches!(document.validate(), Err(TakesError::InvalidSegmentId(_))),
                "`{unusable}` must be refused here because a lesson refuses it"
            );
        }
    }

    #[test]
    fn t1_e1_takes_selection_ceiling_accepts_the_boundary_and_is_the_lesson_ceiling() {
        let selection = document().selections.remove(0);
        let numbered = |count: usize| {
            let mut document = document();
            document.selections = (0..count)
                .map(|index| SelectedTake {
                    segment_id: format!("seg-{index:05}"),
                    ..selection.clone()
                })
                .collect();
            document
        };

        assert!(numbered(MAX_LESSON_SEGMENTS).validate().is_ok());

        assert!(matches!(
            numbered(MAX_LESSON_SEGMENTS + 1).validate(),
            Err(TakesError::TooManySelections { found, max })
                if found == MAX_LESSON_SEGMENTS + 1 && max == MAX_LESSON_SEGMENTS
        ));
    }

    #[test]
    fn t1_e1_an_empty_takes_document_is_refused() {
        let mut empty = document();
        empty.selections.clear();

        assert!(matches!(
            empty.validate(),
            Err(TakesError::MissingSelections)
        ));
    }

    #[test]
    fn t1_e1_takes_json_byte_limit_accepts_the_boundary_and_precedes_parsing() {
        assert!(matches!(
            ValidatedTakes::from_json(&vec![b'{'; MAX_TAKES_JSON_BYTES]),
            Err(TakesError::InvalidJson(_)),
        ));

        assert!(matches!(
            ValidatedTakes::from_json(&vec![b'{'; MAX_TAKES_JSON_BYTES + 1]),
            Err(TakesError::TakesJsonTooLarge { max_bytes })
                if max_bytes == MAX_TAKES_JSON_BYTES
        ));
    }

    #[test]
    fn t1_e1_a_takes_document_link_must_name_its_own_schema() {
        let mut wrong = document();
        wrong.schema = Some(crate::schema_uri("lesson", 1));

        assert!(matches!(
            wrong.validate(),
            Err(TakesError::UnexpectedSchemaLink { .. })
        ));

        let mut right = document();
        right.schema = Some(crate::schema_uri(
            TAKES_SCHEMA_STEM,
            TAKES_SCHEMA_VERSION.major(),
        ));
        assert!(right.validate().is_ok());
    }

    #[test]
    fn t1_e1_a_takes_version_is_read_before_the_fields_that_version_added() {
        for declared in ["2.0", "1.1"] {
            let mut future = serde_json::to_value(document()).expect("a takes document serializes");
            future["schema_version"] = serde_json::Value::String(declared.to_owned());
            future["review_notes"] =
                serde_json::Value::String("a field a later version added".to_owned());

            let error = ValidatedTakes::from_json(
                &serde_json::to_vec(&future).expect("the spoiled document serializes"),
            )
            .expect_err("a version this build cannot read must be refused as such");

            assert!(
                matches!(error, TakesError::UnsupportedSchema(_)),
                "version `{declared}` must be refused as a version, not as its new field: {error:?}"
            );
        }
    }

    #[test]
    fn t1_e1_a_takes_document_of_a_different_major_is_refused() {
        let mut future = document();
        future.schema_version = "2.0".to_owned();

        assert!(matches!(
            future.validate(),
            Err(TakesError::UnsupportedSchema(
                SchemaVersionError::UnsupportedMajor { .. }
            ))
        ));
    }
}
