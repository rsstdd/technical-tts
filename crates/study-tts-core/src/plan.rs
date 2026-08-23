use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Lesson,
    digest::{BLAKE3_HEX_LENGTH, is_blake3_hex},
};

/// The one sample rate this project renders, caches, assembles, and exports at,
/// in hertz.
///
/// Fixed rather than configurable so a cached segment, an assembled master, and
/// an export can never disagree about what a frame is.
pub const CANONICAL_SAMPLE_RATE: u32 = 24_000;

/// The synthesis identity of one segment: BLAKE3 over the canonical
/// serialization of every speech-affecting input, rendered as lowercase
/// hexadecimal (ADR-0001 §12.5).
///
/// A value object rather than a `String` because the key is not only compared,
/// it is *used as a path component*: the cache shards its entries on the key's
/// leading characters. Slicing a bare string there panics on a key shorter than
/// the shard width and on one whose byte boundary falls inside a multi-byte
/// character, and a cache artifact on disk records its key, so any JSON string
/// could reach that slice. Parsing here makes the shard slice total instead of
/// merely usually correct.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CacheKey(String);

impl CacheKey {
    /// Characters in a rendered key.
    ///
    /// Every one is ASCII, so any prefix up to this width is in bounds and on a
    /// character boundary. That is the guarantee a prefix-sharded cache layout
    /// rests on, so it is published rather than left for a caller to rediscover.
    pub const LENGTH: usize = BLAKE3_HEX_LENGTH;

    /// The key as it is written to a plan, a manifest, and a cache artifact.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Produces a key without validation, because a fresh digest cannot fail the
/// check.
///
/// This is the only infallible constructor, and it takes the hash itself rather
/// than a string, so the definition in ADR-0001 §12.5 is the one route into the
/// type that no caller can shortcut.
impl From<blake3::Hash> for CacheKey {
    fn from(hash: blake3::Hash) -> Self {
        Self(hash.to_hex().to_string())
    }
}

impl From<CacheKey> for String {
    fn from(key: CacheKey) -> Self {
        key.0
    }
}

impl TryFrom<String> for CacheKey {
    type Error = MalformedCacheKey;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if is_blake3_hex(&value) {
            return Ok(Self(value));
        }
        Err(MalformedCacheKey(value))
    }
}

impl FromStr for CacheKey {
    type Err = MalformedCacheKey;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value.to_owned())
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Remedy routing: a plan or manifest is regenerated from its lesson, never
/// hand-corrected, so the message names rebuilding rather than editing the
/// recorded value.
#[derive(Debug, Error)]
#[error(
    "cache key `{0}` is not a BLAKE3 digest in lowercase hexadecimal; ADR-0001 §12.5 \
     defines a cache key as exactly that, and the cache uses it as a directory name; rebuild \
     the plan from its lesson rather than editing the recorded key"
)]
pub struct MalformedCacheKey(String);

/// Identity of a whole render plan: BLAKE3 over its serialized segments.
///
/// A value object rather than a `String` because it is a digest that reaches a
/// manifest, and a digest typed as a string is one a caller can set to
/// anything. `From<blake3::Hash>` is the only constructor, so a plan hash
/// cannot exist without a plan having been hashed.
///
/// Deliberately not parseable. Nothing reads a plan back yet, and a `TryFrom`
/// or `Deserialize` here would be a boundary with no traffic and no test. The
/// story that persists `plan.json` (ADR-0001 §12.2) adds them with the
/// versioned, fail-closed loader that gives them meaning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(into = "String")]
pub struct PlanHash(String);

impl PlanHash {
    /// The hash as it is written to a manifest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<blake3::Hash> for PlanHash {
    fn from(hash: blake3::Hash) -> Self {
        Self(hash.to_hex().to_string())
    }
}

impl From<PlanHash> for String {
    fn from(hash: PlanHash) -> Self {
        hash.0
    }
}

impl fmt::Display for PlanHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A lesson resolved into exactly what will be synthesized, derived
/// deterministically from it.
///
/// Produced only by [`RenderPlan::for_lesson`] and never read back, so it does
/// not derive `Deserialize`. Adding one now would publish an unversioned parse
/// boundary that accepts unknown fields and that no caller exercises; ADR-0001
/// §12.2 puts persisted plans at E2, and the loader that story needs is where a
/// fail-closed boundary belongs.
#[derive(Clone, Debug, Serialize)]
pub struct RenderPlan {
    /// The lesson this plan was derived from.
    pub lesson_id: String,
    /// Identity of the plan as a whole, so a rebuild can be recognized as the same
    /// plan.
    pub plan_hash: PlanHash,
    /// The segments to synthesize, in speaking order.
    pub segments: Vec<PlannedSegment>,
}

/// One segment with its synthesis identity resolved.
///
/// Carries only speech-affecting fields plus the identity derived from them;
/// display-only lesson metadata is deliberately absent, because anything here
/// would change the cache key.
///
/// Serialized, never deserialized, for the reason given on [`RenderPlan`]. Its
/// serialization is load-bearing: `plan_hash` is BLAKE3 over exactly these
/// bytes.
#[derive(Clone, Debug, Serialize)]
pub struct PlannedSegment {
    /// Identity of the segment within its lesson.
    pub id: String,
    /// Which voice speaks this segment.
    pub speaker: String,
    /// The exact text to speak.
    pub spoken_text: String,
    /// Delivery style requested of the voice.
    pub style: String,
    /// Silence written after this segment, in milliseconds.
    pub pause_after_ms: u32,
    /// This segment's synthesis identity, which names its cache entry.
    pub cache_key: CacheKey,
}

/// Every speech-affecting input, named field by field.
///
/// A derived hash over `LessonSegment` would silently absorb each future field
/// and let the exclusion property regress without a compile error, so the list
/// is explicit. `identity_version` is the single lever for invalidating every
/// cache entry when this definition changes.
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
    /// Derives the plan for a lesson under a given synthesizer identity.
    ///
    /// Deterministic: the same lesson and the same synthesizer always produce the
    /// same plan hash and the same cache keys, which is what makes a rebuild reuse
    /// its cache.
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
                    cache_key: blake3::hash(&identity_bytes).into(),
                }
            })
            .collect::<Vec<_>>();
        let plan_hash = blake3::hash(
            &serde_json::to_vec(&segments).expect("serializing a render plan cannot fail"),
        )
        .into();

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
    use serde_json::Value;

    #[test]
    fn t1_e0_cache_keys_that_cannot_name_a_shard_directory_are_rejected() {
        // The cache shards its entries on the key's leading characters. While this
        // field was a `String`, the first two of these panicked in that slice — `""`
        // and `"a"` are out of bounds, and `日` puts byte index 2 inside a character —
        // and the rest reached the filesystem as directory names this program never
        // produced.
        let too_short = "a".repeat(CacheKey::LENGTH - 1);
        let too_long = "a".repeat(CacheKey::LENGTH + 1);
        let uppercase = "A".repeat(CacheKey::LENGTH);
        let outside_hex = "g".repeat(CacheKey::LENGTH);

        for malformed in [
            "",
            "a",
            "日",
            too_short.as_str(),
            too_long.as_str(),
            uppercase.as_str(),
            outside_hex.as_str(),
        ] {
            assert!(
                malformed.parse::<CacheKey>().is_err(),
                "`{malformed}` must not parse as a cache key"
            );
            // Deserialization is the boundary that matters: a cache artifact on
            // disk records its key, and that record is how a malformed key would
            // otherwise reach the shard slice.
            assert!(
                serde_json::from_value::<CacheKey>(Value::String(malformed.to_owned())).is_err(),
                "a recorded cache key `{malformed}` must not deserialize"
            );
        }
    }

    #[test]
    fn t1_e0_a_planned_cache_key_is_recorded_as_a_plain_string() {
        let lesson = Lesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid");
        let plan = RenderPlan::for_lesson(&lesson, "fake-tone-v1");
        let cache_key = &plan.segments[0].cache_key;

        // Manifests and cache artifacts already on disk hold the key as a bare JSON
        // string. Wrapping it in a value object must not change that, or every
        // existing artifact becomes unreadable.
        let recorded = serde_json::to_value(cache_key).expect("a cache key serializes");
        assert_eq!(recorded, Value::String(cache_key.as_str().to_owned()));
        assert_eq!(
            &serde_json::from_value::<CacheKey>(recorded).expect("the recorded form parses back"),
            cache_key
        );
    }

    #[test]
    fn t1_e0_a_plan_hash_is_a_digest_recorded_as_a_plain_string() {
        let lesson = Lesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid");
        let plan = RenderPlan::for_lesson(&lesson, "fake-tone-v1");

        // `From<blake3::Hash>` is the only constructor, so the recorded value is a
        // digest by construction rather than by a check someone has to remember.
        assert!(
            is_blake3_hex(plan.plan_hash.as_str()),
            "`{}` is not a BLAKE3 digest",
            plan.plan_hash
        );
        // The manifest holds it as a bare JSON string; wrapping it in a value object
        // must not change what `manifest.json` looks like.
        assert_eq!(
            serde_json::to_value(&plan.plan_hash).expect("a plan hash serializes"),
            Value::String(plan.plan_hash.as_str().to_owned())
        );
    }

    #[test]
    fn t1_e0_plan_is_stable_for_identical_inputs() {
        let lesson = Lesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid");

        let first = RenderPlan::for_lesson(&lesson, "fake-tone-v1");
        let second = RenderPlan::for_lesson(&lesson, "fake-tone-v1");

        assert_eq!(first.plan_hash, second.plan_hash);
        assert_eq!(first.segments[0].cache_key, second.segments[0].cache_key);
    }

    #[test]
    fn t1_e0_synthesizer_identity_participates_in_the_cache_key() {
        let lesson = Lesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid");

        let first = RenderPlan::for_lesson(&lesson, "fake-tone-v1");
        let second = RenderPlan::for_lesson(&lesson, "fake-tone-v2");

        assert_ne!(first.segments[0].cache_key, second.segments[0].cache_key);
        assert_ne!(first.plan_hash, second.plan_hash);
    }
}
