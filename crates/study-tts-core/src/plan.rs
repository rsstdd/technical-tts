//! Deterministic render planning, and the identity values a plan carries.
//!
//! [`CacheKey`] and [`PlanHash`] are value objects rather than strings because
//! both are compared, written into manifests, and — in the key's case — used
//! as a path component. Parsing at the boundary is what makes the cache's
//! prefix slice total rather than usually correct.
//!
//! Plans are serialized and never read back. ADR-0001 §12.2 puts persisted
//! plans at E2, where a versioned fail-closed loader gives a parse boundary
//! something to mean. They carry their document version regardless: a plan is
//! written to disk as `plan.json` and a document with no version is a document
//! that cannot be refused later, so the loader E2 adds would arrive at files
//! that never said what they were.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CanonicalValue, SchemaVersion, ValidatedLesson, canonical_digest,
    digest::{BLAKE3_HEX_LENGTH, blake3_newtype, json_schema_as_string},
    identity::{SynthesisContext, synthesis_digest},
};

/// Version of the render-plan document this build writes.
///
/// `1.0` because `plan.json` has never been written: ADR-0001 §12.2 persists
/// plans at E2, and this is the shape that story will write. A schema may be
/// published before its writer exists; a *version* claiming history it does not
/// have may not.
pub const PLAN_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// File-name and URI stem of the published render-plan schema.
pub const PLAN_SCHEMA_STEM: &str = "plan";

/// The one sample rate this project renders, caches, assembles, and exports at,
/// in hertz.
///
/// Fixed rather than configurable so a cached segment, an assembled master, and
/// an export can never disagree about what a frame is. Transcribed from
/// ADR-0001 §13.1 "Canonical intermediate".
pub const CANONICAL_SAMPLE_RATE: u32 = 24_000;

/// The one channel count this project renders, caches, assembles, and exports.
///
/// A cache-key input like the sample rate and the sample format. Mono is what
/// keeps a frame one sample wide everywhere, so the assembler can concatenate
/// segments and count durations without ever reading a channel layout.
/// Transcribed from ADR-0001 §13.1 "Canonical intermediate".
pub const CANONICAL_CHANNELS: u16 = 1;

/// The one sample format this project renders, caches, assembles, and exports.
///
/// Named alongside the sample rate because both are speech-affecting inputs to
/// every cache key: changing either invalidates every cache entry in the
/// project, which is a decision rather than an edit. Transcribed from ADR-0001
/// §13.1 "Canonical intermediate".
pub const CANONICAL_SAMPLE_FORMAT: &str = "f32le";

/// The bit depth of the canonical sample format.
///
/// The WAV-side spelling of [`CANONICAL_SAMPLE_FORMAT`]: `f32le` is 32-bit
/// little-endian float, but a WAV header records the width and the float-ness
/// separately, so a validator has to check both — an integer stream of the
/// same width is not this format. Transcribed from ADR-0001 §13.1 "Canonical
/// intermediate".
pub const CANONICAL_BITS_PER_SAMPLE: u16 = 32;

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
    /// rests on, so it is published rather than left for a caller to
    /// rediscover.
    pub const LENGTH: usize = BLAKE3_HEX_LENGTH;

    /// The key as it is written to a plan, a manifest, and a cache artifact.
    ///
    /// # Examples
    ///
    /// A key is parsed at the boundary, so the cache's prefix slice is total
    /// rather than usually correct:
    ///
    /// ```rust
    /// use study_tts_core::CacheKey;
    ///
    /// let key: CacheKey = "a".repeat(CacheKey::LENGTH).parse()?;
    /// assert_eq!(key.as_str().len(), CacheKey::LENGTH);
    ///
    /// assert!("not-a-digest".parse::<CacheKey>().is_err());
    /// # Ok::<(), study_tts_core::MalformedCacheKey>(())
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(CacheKey, MalformedCacheKey);

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

json_schema_as_string!(
    CacheKey,
    "CacheKey",
    "BLAKE3 over the canonical serialization of every speech-affecting input \
     (ADR-0001 12.5), as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// Identity of a whole render plan: BLAKE3 over its serialized segments.
///
/// A value object rather than a `String` because it is a digest that reaches a
/// manifest, and a digest typed as a string is one a caller can set to
/// anything. `From<blake3::Hash>` is the only infallible constructor, so a plan
/// hash cannot be *derived* without a plan having been hashed.
///
/// Parseable because it is read back: `manifest.json` records it and
/// `study-tts-runtime`'s package reconciliation compares the recorded value
/// against the plan the current build derived, and a job snapshot carries it
/// across a restart. A recorded value that is not a digest at all has to be
/// reported as malformed rather than as a mismatch — the first sends an
/// operator to the record, the second sends them looking for a lesson change
/// that never happened. `plan.json` itself is still not persisted; ADR-0001
/// §12.2 puts that at E2, and this boundary is the manifest's rather than that
/// file's.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlanHash(String);

impl PlanHash {
    /// The hash as it is written to a manifest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(PlanHash, MalformedPlanHash);

/// Remedy routing: a plan hash is derived from the lesson the plan was built
/// from, so the message names rebuilding rather than editing the record.
#[derive(Debug, Error)]
#[error(
    "plan hash `{0}` is not a BLAKE3 digest in lowercase hexadecimal; ADR-0001 §12.2 derives it \
     from the plan's resolved segments; preserve the package it was recorded in and rebuild the \
     plan from its lesson rather than editing the recorded value"
)]
pub struct MalformedPlanHash(String);

json_schema_as_string!(
    PlanHash,
    "PlanHash",
    "BLAKE3 over the canonical serialization of a render plan's resolved \
     segments, as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// A lesson resolved into exactly what will be synthesized, derived
/// deterministically from it.
///
/// Produced only by [`RenderPlan::for_lesson`] and never read back, so it does
/// not derive `Deserialize`. Adding one now would publish an unversioned parse
/// boundary that accepts unknown fields and that no caller exercises; ADR-0001
/// §12.2 puts persisted plans at E2, and the loader that story needs is where a
/// fail-closed boundary belongs.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RenderPlan {
    /// Layout version of this document.
    ///
    /// First field because it is the first thing a reader of `plan.json` needs
    /// and the first thing the E2 loader will read. Typed rather than authored
    /// text, unlike a lesson's: nothing hand-writes a plan, so there is no
    /// authoring mistake to report against the original spelling.
    ///
    /// Outside [`RenderPlan::plan_hash`] on purpose. That hash names the
    /// segments to be synthesized, and a document layout is not one of them: a
    /// plan whose version moved while its segments did not is the same plan,
    /// and must not invalidate a cache.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: SchemaVersion,
    /// The lesson this plan was derived from.
    pub lesson_id: String,
    /// Identity of the plan as a whole, so a rebuild can be recognized as the
    /// same plan.
    pub plan_hash: PlanHash,
    /// The segments to synthesize, in speaking order.
    pub segments: Vec<PlannedSegment>,
}

/// One segment with its synthesis identity resolved.
///
/// Contains synthesis inputs, their derived cache identity, and the timeline
/// metadata needed after synthesis. `speaker`, `spoken_text`, `style`, and
/// `take` affect the cache key; `id` and `pause_after_ms` affect only the plan
/// identity. Display-only lesson metadata remains absent so presentation edits
/// change neither identity.
///
/// Serialized, never deserialized, for the reason given on [`RenderPlan`].
/// `plan_hash` is derived separately by `plan_digest`, which keeps serde
/// document-layout details out of the identity — and which destructures this
/// type without `..`, so a field added here stops compiling until somebody has
/// decided whether it belongs in the plan's identity.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
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
    /// Which take of this segment the plan selects.
    pub take: u32,
    /// This segment's synthesis identity, which names its cache entry.
    pub cache_key: CacheKey,
}

/// Take used by every planned segment.
///
/// ADR-0001 §12.2: "Take zero is the synthesis default." A planned segment's
/// cache key is therefore that segment's *synthesis base key*, and selecting
/// any other take is the takes file's decision rather than the planner's.
pub const BASE_TAKE: u32 = 0;

impl RenderPlan {
    /// Derives the plan for a lesson under a given synthesis context.
    ///
    /// Deterministic: the same lesson and the same context always produce the
    /// same plan hash and the same cache keys, which is what makes a rebuild
    /// reuse its cache.
    ///
    /// # Examples
    ///
    /// Authored data cannot be planned before validation, because this
    /// function accepts only a [`ValidatedLesson`]:
    ///
    /// ```compile_fail
    /// use study_tts_core::{AuthoredLesson, RenderPlan, SynthesisContext};
    ///
    /// fn plan(authored: &AuthoredLesson, context: &SynthesisContext) {
    ///     RenderPlan::for_lesson(authored, context);
    /// }
    /// ```
    pub fn for_lesson(lesson: &ValidatedLesson, context: &SynthesisContext) -> Self {
        let segments = lesson
            .segments()
            .iter()
            .map(|segment| PlannedSegment {
                id: segment.id.clone(),
                speaker: segment.speaker.clone(),
                spoken_text: segment.spoken_text.clone(),
                style: segment.style.clone(),
                pause_after_ms: segment.pause_after_ms,
                take: BASE_TAKE,
                cache_key: synthesis_digest(context, segment, BASE_TAKE).into(),
            })
            .collect::<Vec<_>>();

        Self {
            schema_version: PLAN_SCHEMA_VERSION,
            lesson_id: lesson.lesson_id().to_owned(),
            plan_hash: plan_digest(&segments).into(),
            segments,
        }
    }
}

/// Publishes the versions of this document a build reads, rather than any
/// well-formed version string.
///
/// [`SchemaVersion`]'s own schema is a pattern, which is right for a value that
/// may be any version; this field may be one of the versions this build writes,
/// which is what [`crate::schema::accepted_versions_json_schema`] lists.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::accepted_versions_json_schema(PLAN_SCHEMA_VERSION)
}

/// Derives the identity of a whole plan from its resolved segments.
///
/// Hashes the canonical byte form rather than a `Serialize` output, so the
/// bytes are defined by [`crate::canonical_bytes`] instead of by serde's field
/// order. Nothing on that path can fail, which is why neither this function nor
/// [`RenderPlan::for_lesson`] carries a `# Panics` section.
///
/// The mapping below *is* the plan's identity, and it is held to
/// [`PlannedSegment`] by the destructure rather than by anyone remembering;
/// that type names this function in return.
fn plan_digest(segments: &[PlannedSegment]) -> blake3::Hash {
    canonical_digest(&CanonicalValue::array(segments.iter().map(|segment| {
        // Without `..`, so a field added to `PlannedSegment` is a compile error
        // here until it is either hashed or deliberately left out. Omitting one
        // silently is a plan whose identity stops describing it.
        let PlannedSegment {
            id,
            speaker,
            spoken_text,
            style,
            pause_after_ms,
            take,
            cache_key,
        } = segment;

        CanonicalValue::object([
            ("id", id.as_str().into()),
            ("speaker", speaker.as_str().into()),
            ("spoken_text", spoken_text.as_str().into()),
            ("style", style.as_str().into()),
            ("pause_after_ms", (*pause_after_ms).into()),
            ("take", (*take).into()),
            ("cache_key", cache_key.as_str().into()),
        ])
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{digest::is_blake3_hex, identity::sample_context};
    use serde_json::Value;

    /// The reviewed two-segment lesson every property below plans.
    fn fixture_lesson() -> ValidatedLesson {
        ValidatedLesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture should be valid")
    }

    #[test]
    fn t1_e0_cache_keys_that_cannot_name_a_shard_directory_are_rejected() {
        // The cache shards its entries on the key's leading characters. While
        // this field was a `String`, the first two of these panicked in that
        // slice — `""` and `"a"` are out of bounds, and `日` puts byte index 2
        // inside a character — and the rest reached the filesystem as directory
        // names this program never produced.
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
            // disk records its key, and that record is how a malformed key
            // would otherwise reach the shard slice.
            assert!(
                serde_json::from_value::<CacheKey>(Value::String(malformed.to_owned())).is_err(),
                "a recorded cache key `{malformed}` must not deserialize"
            );
        }
    }

    #[test]
    fn t1_e0_a_planned_cache_key_is_recorded_as_a_plain_string() {
        let lesson = fixture_lesson();
        let plan = RenderPlan::for_lesson(&lesson, &sample_context());
        let cache_key = &plan.segments[0].cache_key;

        // Manifests and cache artifacts already on disk hold the key as a bare
        // JSON string. Wrapping it in a value object must not change that, or
        // every existing artifact becomes unreadable.
        let recorded = serde_json::to_value(cache_key).expect("a cache key serializes");
        assert_eq!(recorded, Value::String(cache_key.as_str().to_owned()));
        assert_eq!(
            &serde_json::from_value::<CacheKey>(recorded).expect("the recorded form parses back"),
            cache_key
        );
    }

    #[test]
    fn t1_e0_a_plan_hash_is_a_digest_recorded_as_a_plain_string() {
        let lesson = fixture_lesson();
        let plan = RenderPlan::for_lesson(&lesson, &sample_context());

        // `From<blake3::Hash>` is the only constructor, so the recorded value
        // is a digest by construction rather than by a check someone has to
        // remember.
        assert!(
            is_blake3_hex(plan.plan_hash.as_str()),
            "`{}` is not a BLAKE3 digest",
            plan.plan_hash
        );
        // The manifest holds it as a bare JSON string; wrapping it in a value
        // object must not change what `manifest.json` looks like.
        assert_eq!(
            serde_json::to_value(&plan.plan_hash).expect("a plan hash serializes"),
            Value::String(plan.plan_hash.as_str().to_owned())
        );
    }

    #[test]
    fn t1_e0_plan_is_stable_for_identical_inputs() {
        let lesson = fixture_lesson();

        let first = RenderPlan::for_lesson(&lesson, &sample_context());
        let second = RenderPlan::for_lesson(&lesson, &sample_context());

        // Pinned so an accidental change to the identity definition or the
        // canonical byte form is a failure here rather than a silent cache-wide
        // invalidation. These moved from their E0 values when E1-S1 adopted the
        // complete ADR-0001 §12.5 input set, and again within E1-S1 when the
        // cache artifact gained its provenance record and
        // `CACHE_SCHEMA_VERSION` — itself a key input — moved to `1.0`.
        assert_eq!(
            first.plan_hash.as_str(),
            "eaac5a9c480376062a9b4d5c779884fff44356e7351b4ee87ecc8eed468e1501"
        );
        assert_eq!(
            first.segments[0].cache_key.as_str(),
            "bf7b27ab8ec9607f9c34ce5e96af4b4ff4645de9ca114c66d0351b7ed3eaa603"
        );
        assert_eq!(
            first.segments[1].cache_key.as_str(),
            "ef7fe5ff9625cce6e03a278cec269b419047b00272e4e12b995f70aad41a3cb0"
        );

        assert_eq!(first.plan_hash, second.plan_hash);
        assert_eq!(first.segments[0].cache_key, second.segments[0].cache_key);
    }

    #[test]
    fn t1_e0_synthesizer_identity_participates_in_the_cache_key() {
        let lesson = fixture_lesson();

        // The synthesizer's identity is no longer one opaque string: ADR-0001
        // §12.5 names the bundle, model, and tokenizer separately, and each has
        // to reach the key on its own.
        let baseline = sample_context();
        let mut rebuilt_worker = baseline.clone();
        rebuilt_worker.worker_bundle_hash =
            "3".repeat(64).parse().expect("a digest of threes parses");
        let mut newer_model = baseline.clone();
        newer_model.model_revision = "fedcba9876543210fedcba9876543210fedcba98"
            .parse()
            .expect("a hex revision parses");
        let mut newer_tokenizer = baseline.clone();
        newer_tokenizer.tokenizer_revision = "tokenizer-2026-02"
            .parse()
            .expect("a dated tokenizer revision parses");

        let first = RenderPlan::for_lesson(&lesson, &baseline);
        for (input, changed) in [
            ("worker_bundle_hash", rebuilt_worker),
            ("model_revision", newer_model),
            ("tokenizer_revision", newer_tokenizer),
        ] {
            let second = RenderPlan::for_lesson(&lesson, &changed);

            assert_ne!(
                first.segments[0].cache_key, second.segments[0].cache_key,
                "changing `{input}` must change the segment's cache key"
            );
            assert_ne!(
                first.plan_hash, second.plan_hash,
                "changing `{input}` must change the plan hash"
            );
        }
    }
}
