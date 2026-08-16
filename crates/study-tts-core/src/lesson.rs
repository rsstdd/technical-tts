use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Identifiers reach the filesystem through `previews/<lesson-id>/`, so they are bounded well
/// below `NAME_MAX` (255 on ext4) to leave room for the suffixes later stories append.
const MAX_IDENTIFIER_LENGTH: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub schema_version: String,
    pub lesson_id: String,
    pub title: String,
    pub segments: Vec<LessonSegment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LessonSegment {
    pub id: String,
    pub speaker: String,
    pub role: String,
    pub source_refs: Vec<String>,
    pub display_text: String,
    pub spoken_text: String,
    pub style: String,
    pub pause_after_ms: u32,
    pub review_status: ReviewStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Approved,
    Draft,
    NeedsReview,
}

#[derive(Debug, Error)]
pub enum LessonError {
    #[error("lesson JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported lesson schema version `{0}`")]
    UnsupportedSchema(String),
    #[error("lesson_id must not be empty")]
    MissingLessonId,
    #[error(
        "lesson_id `{0}` must be 1-64 ASCII letters, digits, hyphen, underscore, or dot, and must \
         not start with a dot, because it names an output directory"
    )]
    InvalidLessonId(String),
    #[error("lesson must contain at least one segment")]
    MissingSegments,
    #[error("segment ID must not be empty")]
    MissingSegmentId,
    #[error(
        "segment ID `{0}` must be 1-64 ASCII letters, digits, hyphen, underscore, or dot, and must \
         not start with a dot"
    )]
    InvalidSegmentId(String),
    #[error("segment ID `{0}` is duplicated")]
    DuplicateSegmentId(String),
    #[error("segment `{0}` has empty spoken_text")]
    MissingSpokenText(String),
    #[error("segment `{0}` has empty display_text")]
    MissingDisplayText(String),
    #[error("segment `{0}` has an empty role")]
    MissingRole(String),
    #[error("segment `{0}` must contain at least one source reference")]
    MissingSourceRefs(String),
    #[error("segment `{0}` contains an empty source reference")]
    EmptySourceRef(String),
    #[error("segment `{0}` is not approved for synthesis")]
    UnapprovedSegment(String),
    #[error("segment `{0}` must declare a speaker")]
    MissingSpeaker(String),
    #[error("segment `{0}` must declare a style")]
    MissingStyle(String),
    #[error("segment `{0}` pause exceeds the provisional 10-second limit")]
    PauseOutOfRange(String),
}

impl Lesson {
    pub fn from_json(bytes: &[u8]) -> Result<Self, LessonError> {
        let lesson: Self = serde_json::from_slice(bytes)?;
        lesson.validate()?;
        Ok(lesson)
    }

    pub fn validate(&self) -> Result<(), LessonError> {
        if self.schema_version != "0.1-skeleton" {
            return Err(LessonError::UnsupportedSchema(self.schema_version.clone()));
        }
        // An absent value and a malformed value are different authoring mistakes, so each keeps a
        // distinct error. Both identifier kinds apply the same two checks in the same order.
        if self.lesson_id.trim().is_empty() {
            return Err(LessonError::MissingLessonId);
        }
        if !is_portable_id(&self.lesson_id) {
            return Err(LessonError::InvalidLessonId(self.lesson_id.clone()));
        }
        if self.segments.is_empty() {
            return Err(LessonError::MissingSegments);
        }

        let mut ids = HashSet::with_capacity(self.segments.len());
        for segment in &self.segments {
            if segment.id.trim().is_empty() {
                return Err(LessonError::MissingSegmentId);
            }
            if !is_portable_id(&segment.id) {
                return Err(LessonError::InvalidSegmentId(segment.id.clone()));
            }
            if !ids.insert(segment.id.as_str()) {
                return Err(LessonError::DuplicateSegmentId(segment.id.clone()));
            }
            if segment.spoken_text.trim().is_empty() {
                return Err(LessonError::MissingSpokenText(segment.id.clone()));
            }
            if segment.display_text.trim().is_empty() {
                return Err(LessonError::MissingDisplayText(segment.id.clone()));
            }
            if segment.role.trim().is_empty() {
                return Err(LessonError::MissingRole(segment.id.clone()));
            }
            if segment.source_refs.is_empty() {
                return Err(LessonError::MissingSourceRefs(segment.id.clone()));
            }
            if segment
                .source_refs
                .iter()
                .any(|source_ref| source_ref.trim().is_empty())
            {
                return Err(LessonError::EmptySourceRef(segment.id.clone()));
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
            if segment.pause_after_ms > 10_000 {
                return Err(LessonError::PauseOutOfRange(segment.id.clone()));
            }
        }

        Ok(())
    }
}

/// A leading dot is rejected because it produces a hidden directory, which also makes `.`, `..`,
/// and `...` invalid without a special case for each. Length is measured in bytes, which is exact
/// here because the byte-class check restricts the value to ASCII.
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

    fn parse(value: &Value) -> Result<Lesson, LessonError> {
        Lesson::from_json(&serde_json::to_vec(value).expect("test lesson should serialize"))
    }

    #[test]
    fn t1_e0_valid_lesson_parses() {
        let lesson = parse(&fixture()).expect("reviewed fixture should validate");
        assert_eq!(lesson.lesson_id, "e0-s0-walking-skeleton");
        assert_eq!(lesson.segments.len(), 2);
    }

    #[test]
    fn t1_e0_duplicate_segment_id_is_rejected() {
        let bytes = br#"{
            "schema_version":"0.1-skeleton",
            "lesson_id":"duplicate",
            "title":"Duplicate",
            "segments":[
                {"id":"seg-1","speaker":"nadia","role":"explanation","source_refs":["block-1"],"display_text":"one","spoken_text":"one","style":"calm","pause_after_ms":0,"review_status":"approved"},
                {"id":"seg-1","speaker":"tom","role":"recap","source_refs":["block-2"],"display_text":"two","spoken_text":"two","style":"calm","pause_after_ms":0,"review_status":"approved"}
            ]
        }"#;

        assert!(matches!(
            Lesson::from_json(bytes),
            Err(LessonError::DuplicateSegmentId(id)) if id == "seg-1"
        ));
    }

    #[test]
    fn t1_e0_unapproved_segment_is_rejected() {
        let mut value = fixture();
        value["segments"][0]["review_status"] = Value::String("needs_review".to_owned());

        assert!(matches!(
            parse(&value),
            Err(LessonError::UnapprovedSegment(id)) if id == "seg-0001"
        ));
    }

    #[test]
    fn t1_e0_review_context_invariants_have_distinct_errors() {
        let mut value = fixture();
        value["segments"][0]["display_text"] = Value::String(String::new());
        assert!(matches!(
            parse(&value),
            Err(LessonError::MissingDisplayText(_))
        ));

        let mut value = fixture();
        value["segments"][0]["role"] = Value::String(String::new());
        assert!(matches!(parse(&value), Err(LessonError::MissingRole(_))));

        let mut value = fixture();
        value["segments"][0]["source_refs"] = Value::Array(Vec::new());
        assert!(matches!(
            parse(&value),
            Err(LessonError::MissingSourceRefs(_))
        ));

        let mut value = fixture();
        value["segments"][0]["source_refs"] = Value::Array(vec![Value::String(String::new())]);
        assert!(matches!(parse(&value), Err(LessonError::EmptySourceRef(_))));
    }

    #[test]
    fn t1_e0_synthesis_selection_invariants_have_distinct_errors() {
        let mut value = fixture();
        value["segments"][0]["speaker"] = Value::String(String::new());
        assert!(matches!(parse(&value), Err(LessonError::MissingSpeaker(_))));

        let mut value = fixture();
        value["segments"][0]["style"] = Value::String(String::new());
        assert!(matches!(parse(&value), Err(LessonError::MissingStyle(_))));

        let mut value = fixture();
        value["segments"][0]["review_status"] = Value::String("aproved".to_owned());
        assert!(matches!(parse(&value), Err(LessonError::InvalidJson(_))));
    }

    #[test]
    fn t1_e0_empty_identifiers_are_reported_as_missing_not_malformed() {
        // Whitespace-only is treated as absent for both identifier kinds, so the two branches
        // cannot drift apart the way `is_empty` versus `trim().is_empty()` previously allowed.
        for absent in ["", "   "] {
            let mut value = fixture();
            value["lesson_id"] = Value::String(absent.to_owned());
            assert!(
                matches!(parse(&value), Err(LessonError::MissingLessonId)),
                "lesson_id `{absent}` must be reported as missing"
            );

            let mut value = fixture();
            value["segments"][0]["id"] = Value::String(absent.to_owned());
            assert!(
                matches!(parse(&value), Err(LessonError::MissingSegmentId)),
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
                matches!(parse(&value), Err(LessonError::InvalidLessonId(_))),
                "lesson_id `{unsafe_id}` must be rejected"
            );

            let mut value = fixture();
            value["segments"][0]["id"] = Value::String(unsafe_id.clone());
            assert!(
                matches!(parse(&value), Err(LessonError::InvalidSegmentId(_))),
                "segment ID `{unsafe_id}` must be rejected"
            );
        }
    }

    #[test]
    fn t1_e0_portable_ids_at_the_length_bound_are_accepted() {
        // `lesson.v1` is pinned deliberately: interior dots stay legal, so a later attempt to
        // reject every dot would fail here rather than silently breaking versioned identifiers.
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
                parse(&value).is_ok(),
                "lesson_id `{safe_id}` must be accepted"
            );
        }
    }
}
