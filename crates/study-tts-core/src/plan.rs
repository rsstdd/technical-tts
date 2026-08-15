use serde::{Deserialize, Serialize};

use crate::Lesson;

pub const CANONICAL_SAMPLE_RATE: u32 = 24_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RenderPlan {
    pub lesson_id: String,
    pub plan_hash: String,
    pub segments: Vec<PlannedSegment>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlannedSegment {
    pub id: String,
    pub speaker: String,
    pub spoken_text: String,
    pub style: String,
    pub pause_after_ms: u32,
    pub cache_key: String,
}

#[derive(Serialize)]
struct SynthesisIdentity<'a> {
    identity_version: &'static str,
    synthesizer: &'a str,
    speaker: &'a str,
    spoken_text: &'a str,
    style: &'a str,
    sample_rate: u32,
    channels: u16,
    sample_format: &'static str,
}

impl RenderPlan {
    pub fn for_lesson(lesson: &Lesson, synthesizer_identity: &str) -> Self {
        let segments = lesson
            .segments
            .iter()
            .map(|segment| {
                let identity = SynthesisIdentity {
                    identity_version: "e0-s0-v1",
                    synthesizer: synthesizer_identity,
                    speaker: &segment.speaker,
                    spoken_text: &segment.spoken_text,
                    style: &segment.style,
                    sample_rate: CANONICAL_SAMPLE_RATE,
                    channels: 1,
                    sample_format: "f32le",
                };
                let identity_bytes = serde_json::to_vec(&identity)
                    .expect("serializing a fixed synthesis identity cannot fail");

                PlannedSegment {
                    id: segment.id.clone(),
                    speaker: segment.speaker.clone(),
                    spoken_text: segment.spoken_text.clone(),
                    style: segment.style.clone(),
                    pause_after_ms: segment.pause_after_ms,
                    cache_key: blake3::hash(&identity_bytes).to_hex().to_string(),
                }
            })
            .collect::<Vec<_>>();
        let plan_hash = blake3::hash(
            &serde_json::to_vec(&segments).expect("serializing a render plan cannot fail"),
        )
        .to_hex()
        .to_string();

        Self {
            lesson_id: lesson.lesson_id.clone(),
            plan_hash,
            segments,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_is_stable_for_identical_inputs() {
        let lesson = Lesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid");

        let first = RenderPlan::for_lesson(&lesson, "fake-tone-v1");
        let second = RenderPlan::for_lesson(&lesson, "fake-tone-v1");

        assert_eq!(first.plan_hash, second.plan_hash);
        assert_eq!(first.segments[0].cache_key, second.segments[0].cache_key);
    }
}
