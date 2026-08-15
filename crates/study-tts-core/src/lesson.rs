use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub spoken_text: String,
    pub style: String,
    pub pause_after_ms: u32,
}

#[derive(Debug, Error)]
pub enum LessonError {
    #[error("lesson JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported lesson schema version `{0}`")]
    UnsupportedSchema(String),
    #[error("lesson_id must not be empty")]
    MissingLessonId,
    #[error("lesson must contain at least one segment")]
    MissingSegments,
    #[error("segment `{0}` has a duplicate or empty ID")]
    InvalidSegmentId(String),
    #[error("segment `{0}` has empty spoken_text")]
    MissingSpokenText(String),
    #[error("segment `{0}` must declare a speaker and style")]
    MissingSynthesisSelection(String),
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
        if self.schema_version != "1.0" {
            return Err(LessonError::UnsupportedSchema(self.schema_version.clone()));
        }
        if self.lesson_id.trim().is_empty() {
            return Err(LessonError::MissingLessonId);
        }
        if self.segments.is_empty() {
            return Err(LessonError::MissingSegments);
        }

        let mut ids = HashSet::with_capacity(self.segments.len());
        for segment in &self.segments {
            if segment.id.trim().is_empty() || !ids.insert(segment.id.as_str()) {
                return Err(LessonError::InvalidSegmentId(segment.id.clone()));
            }
            if segment.spoken_text.trim().is_empty() {
                return Err(LessonError::MissingSpokenText(segment.id.clone()));
            }
            if segment.speaker.trim().is_empty() || segment.style.trim().is_empty() {
                return Err(LessonError::MissingSynthesisSelection(segment.id.clone()));
            }
            if segment.pause_after_ms > 10_000 {
                return Err(LessonError::PauseOutOfRange(segment.id.clone()));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_segment_ids() {
        let bytes = br#"{
            "schema_version":"1.0",
            "lesson_id":"duplicate",
            "title":"Duplicate",
            "segments":[
                {"id":"seg-1","speaker":"nadia","spoken_text":"one","style":"calm","pause_after_ms":0},
                {"id":"seg-1","speaker":"tom","spoken_text":"two","style":"calm","pause_after_ms":0}
            ]
        }"#;

        assert!(matches!(
            Lesson::from_json(bytes),
            Err(LessonError::InvalidSegmentId(id)) if id == "seg-1"
        ));
    }
}
