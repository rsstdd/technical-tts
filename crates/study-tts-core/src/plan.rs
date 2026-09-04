//! Deterministic render planning, and the identity values a plan carries.
//!
//! [`CacheKey`] and [`PlanHash`] are value objects rather than strings because
//! both are compared, written into manifests, and — in the key's case — used
//! as a path component. Parsing at the boundary is what makes the cache's
//! prefix slice total rather than usually correct.
//!
//! Plans are serialized during a build and read back during E2 recovery through
//! the runtime's versioned, fail-closed retained-plan boundary.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CanonicalValue, DeliveryStyle, MAX_LESSON_SEGMENTS, SchemaVersion, ValidatedLesson,
    canonical_digest,
    digest::{BLAKE3_HEX_LENGTH, blake3_newtype, json_schema_as_string},
    identity::{SynthesisContext, synthesis_digest},
    takes::{AppliedSelection, TakeSelectionSource},
    verification::AudioDigest,
};

/// Version of the render-plan document this build writes.
///
/// Version `1.0` was E1-S1's published shape. Version `2.0` makes
/// [`PlannedSegment::display_text`] required and narrows `style` from a free
/// string to the closed [`DeliveryStyle`] vocabulary, both of which
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
/// puts under **Breaking contract**. `ADR-0001-D005` does not permit either to
/// retain its version: that deviation's condition 2 needs the version to have
/// been introduced by an unreleased breaking move *within the same story*, and
/// `1.0` came from E1-S1. That no `plan.json` has ever been written makes the
/// migration trivial, not the increment optional. The change is recorded in
/// `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`.
///
/// Version `4.0` carries ADR-0001 §12.2's selection record: every segment
/// repeats its audio checksum, every segment records the synthesis base key
/// §13.2's edit-decision list asks for, and the document records whether its
/// selection was recorded by a reviewer or generated. All three are required
/// fields, which `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`
/// §Change classes puts under **Breaking contract**. Recorded in
/// `docs/architecture/E2-S2-INTERFACE-CHANGE-001.md`, which also records why
/// none of the three enters [`RenderPlan::plan_hash`].
pub const PLAN_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(4, 0);

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
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
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
/// against the plan the current build derived, and both `plan.json` and
/// `job.json` carry it across a restart. A recorded value that is not a digest
/// at all has to be reported as malformed rather than as a mismatch — the
/// first sends an operator to the record, the second sends them looking for a
/// lesson change that never happened. The retained-plan loader also derives
/// the hash again from the recorded segments before trusting it.
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
/// Produced by [`RenderPlan::for_lesson`] and read back only through the
/// version-gated retained-plan loader in `study_tts_runtime::job_repository`.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RenderPlan {
    /// Layout version of this document.
    ///
    /// First field because it is the first thing a reader of `plan.json` needs
    /// and the first thing the retained-plan loader reads. Typed rather than
    /// authored text, unlike a lesson's: nothing hand-writes a plan, so there
    /// is no authoring mistake to report against the original spelling.
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
    /// Whether the takes this plan carries were recorded by a reviewer or
    /// generated by the planner.
    ///
    /// ADR-0001 §12.2 permits a generated take-zero selection for a private
    /// preview and requires an explicit versioned takes file for production.
    /// Recorded here so the published package can carry the distinction rather
    /// than a later boundary having to infer it.
    ///
    /// Outside [`RenderPlan::plan_hash`], for the reason
    /// `docs/architecture/E2-S2-INTERFACE-CHANGE-001.md` §Identity effect
    /// records as invariant I-4: ratifying the take zero a build already
    /// rendered is a governance act, not a rendering change, and re-identifying
    /// the plan for it would refuse a resume and rebuild a package for
    /// byte-identical audio. Package reuse is separated on it instead, at
    /// `study_tts_runtime::manifest::ReuseExpectations`.
    pub take_selection_source: TakeSelectionSource,
    /// The segments to synthesize, in speaking order.
    #[schemars(length(max = MAX_LESSON_SEGMENTS))]
    pub segments: Vec<PlannedSegment>,
}

/// One segment with its synthesis identity resolved.
///
/// Contains synthesis inputs, their derived cache identity, and the timeline
/// and transcript metadata needed after synthesis. `speaker`, `spoken_text`,
/// `style`, and `take` affect the cache key; `id`, `display_text`, and
/// `pause_after_ms` affect only the plan identity. The remaining lesson
/// metadata — role, citations, review state — reaches neither, because nothing
/// downstream of planning reads it.
///
/// `plan_hash` is derived separately by `plan_digest`, which keeps serde
/// document-layout details out of the identity — and which destructures this
/// type without `..`, so a field added here stops compiling until somebody has
/// decided whether it belongs in the plan's identity.
#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PlannedSegment {
    /// Identity of the segment within its lesson.
    pub id: String,
    /// Which voice speaks this segment.
    pub speaker: String,
    /// Voice profile the lesson binds that speaker to.
    ///
    /// The resolved identity, carried forward rather than re-derived, because
    /// the worker protocol's `synthesize` frame asks for a *voice profile
    /// identity* and the speaker name is not one: two speakers may share a
    /// profile, and a worker handed a speaker name would have to know a
    /// lesson's bindings to resolve it.
    ///
    /// Not a synthesis-key input. ADR-0001 §12.5 keys on the conditioning
    /// artifact's hash, which is what actually changes the audio; the profile
    /// identity names the record that artifact was resolved through, so a
    /// reviewer can reach its consent record. Renaming a profile directory
    /// must not re-render every segment it speaks.
    pub voice_profile: String,
    /// Text as a reviewer reads it, carried so the package a build writes can
    /// hold the transcript for the audio it selected.
    ///
    /// ADR-0001 §8.3 keeps this apart from `spoken_text` so a pronunciation
    /// edit cannot hide a semantic one, and §12.5 keeps it out of the cache
    /// key: correcting a transcript must not re-synthesize identical audio. It
    /// is inside the plan hash all the same, because a package whose
    /// transcript changed is a different package, and a plan identity that
    /// ignored it would let the corrected text be reconciled away as already
    /// selected.
    pub display_text: String,
    /// The exact text to speak.
    pub spoken_text: String,
    /// Delivery requested of the voice.
    ///
    /// The lesson's own closed vocabulary rather than its spelling, so the
    /// published plan schema names the four styles a build can render and a
    /// caller cannot introduce a fifth between validation and synthesis.
    pub style: DeliveryStyle,
    /// Silence written after this segment, in milliseconds.
    pub pause_after_ms: u32,
    /// Which take of this segment the plan selects.
    pub take: u32,
    /// This segment's synthesis identity, which names its cache entry.
    pub cache_key: CacheKey,
    /// This segment's take-zero identity, whatever take is selected.
    ///
    /// ADR-0001 §13.2 puts synthesis base keys in the edit-decision list beside
    /// the selected takes and selected cache keys, so a reviewer can see which
    /// selection a retake replaced. Equal to `cache_key` exactly when `take` is
    /// [`BASE_TAKE`], which is what [`RenderPlan::verify_recorded_selection`]
    /// checks on a document read back from disk.
    ///
    /// A reproducibility field, outside [`RenderPlan::plan_hash`]: it is
    /// `synthesis_digest(context, segment, BASE_TAKE)`, a pure function of
    /// inputs the identity already covers, so hashing it would add no
    /// discrimination. Derived at construction on every planning path rather
    /// than accepted from a caller.
    pub synthesis_base_key: CacheKey,
    /// Digest of the audio a reviewer approved for this segment, when one has
    /// been.
    ///
    /// ADR-0001 §12.2 requires `plan.json` to repeat the audio checksum for
    /// every segment. `None` where no takes document approved one — a segment
    /// planned at take zero because no selection was recorded has no approved
    /// audio yet, and claiming a checksum for it would describe an approval
    /// nobody gave.
    ///
    /// A reproducibility field, outside [`RenderPlan::plan_hash`]: the cache is
    /// content-addressed, so one `cache_key` names one entry with one digest,
    /// and a plan recording a different digest describes an impossible state
    /// rather than a different plan. Verified against the resolved cache entry
    /// where the entry is resolved, rather than trusted here.
    pub audio_blake3: Option<AudioDigest>,
}

impl PlannedSegment {
    /// The identity of the worker request that renders this segment.
    ///
    /// Derived rather than assigned, and derived **here** rather than at each
    /// use, because two boundaries need the same answer and neither owns it.
    /// The executor puts it on the `synthesize` frame so the worker can
    /// correlate its reply, and the cache puts it in the quarantine path
    /// ADR-0001 §12.6 spells with an attempt and a request. Two spellings of
    /// one rule would be one rule until somebody edited one of them, and the
    /// quarantined artifact would then name a request that never existed.
    ///
    /// Unique within a plan because the cache key and the segment identity
    /// together are: two takes of one segment differ in their key, and two
    /// segments sharing a key differ in their identity.
    ///
    /// # Examples
    ///
    /// ```
    /// # use study_tts_core::{CacheKey, DeliveryStyle, PlannedSegment};
    /// let segment = PlannedSegment {
    ///     id: "segment-a".to_owned(),
    ///     speaker: "nadia".to_owned(),
    ///     voice_profile: "nadia-v1".to_owned(),
    ///     display_text: "Ten".to_owned(),
    ///     spoken_text: "Ten".to_owned(),
    ///     style: DeliveryStyle::CalmExplanatory,
    ///     pause_after_ms: 0,
    ///     take: 0,
    ///     cache_key: "0".repeat(CacheKey::LENGTH).parse()?,
    ///     synthesis_base_key: "0".repeat(CacheKey::LENGTH).parse()?,
    ///     audio_blake3: None,
    /// };
    ///
    /// assert!(segment.request_id().ends_with("-segment-a"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn request_id(&self) -> String {
        format!("e0-{}-{}", self.cache_key, self.id)
    }
}

/// Why a validated lesson could not be planned.
///
/// One variant, because planning has exactly one precondition its input type
/// cannot express: [`ValidatedLesson`] guarantees every segment names a
/// declared speaker, but not that the caller resolved that speaker's voice.
#[derive(Debug, Error)]
pub enum PlanError {
    /// A segment's speaker has no resolved voice-conditioning artifact.
    ///
    /// ADR-0001 §12.5 makes that artifact a synthesis-key input.
    /// [`CanonicalValue::optional`] would serialize the absent case as `null`
    /// and produce a well-formed key — one naming audio no voice could have
    /// produced — so planning refuses instead of deriving it.
    #[error(
        "segment `{segment_id}` speaks as `{speaker}`, whose voice profile was not resolved; \
         every speaker's conditioning artifact is an ADR-0001 §12.5 synthesis-key input, so the \
         lesson's declared profiles must be resolved before its plan is derived"
    )]
    UnresolvedSpeaker {
        /// Segment whose speaker is unresolved.
        segment_id: String,
        /// Speaker the segment names.
        speaker: String,
    },
    /// A retained segment at take zero records a base key other than its own
    /// cache key.
    #[error(
        "retained plan segment `{segment_id}` is at take zero but records a synthesis base key \
         other than its own cache key; take zero's cache key is its synthesis base key, so this \
         document was edited after it was written and the runtime owner must rebuild the plan \
         from its lesson rather than correct the recorded key"
    )]
    BaseTakeKeyMismatch {
        /// Segment carrying the mismatched key.
        segment_id: String,
    },
    /// A retained segment at a later take records its cache key as its base
    /// key.
    #[error(
        "retained plan segment `{segment_id}` is at take {take} but records its own cache key as \
         its synthesis base key; a retake has a different identity from take zero, so this \
         document was edited after it was written and the runtime owner must rebuild the plan \
         from its lesson rather than correct the recorded key"
    )]
    RetakeUsesBaseKey {
        /// Segment carrying the take-zero key.
        segment_id: String,
        /// Take that segment records.
        take: u32,
    },
}

/// Take used by every planned segment.
///
/// ADR-0001 §12.2: "Take zero is the synthesis default." A planned segment's
/// cache key is therefore that segment's *synthesis base key*, and selecting
/// any other take is the takes file's decision rather than the planner's.
pub const BASE_TAKE: u32 = 0;

impl RenderPlan {
    /// Recomputes this plan's identity from its resolved segments.
    #[must_use]
    pub fn derived_hash(&self) -> PlanHash {
        plan_digest(&self.segments).into()
    }

    /// Derives the plan for a lesson under a given synthesis context.
    ///
    /// Deterministic: the same lesson and the same context always produce the
    /// same plan hash and the same cache keys, which is what makes a rebuild
    /// reuse its cache.
    ///
    /// # Errors
    ///
    /// [`PlanError::UnresolvedSpeaker`] when `context` carries no conditioning
    /// artifact for a speaker some segment names. This is the one precondition
    /// [`ValidatedLesson`] cannot carry, because resolving a profile is
    /// filesystem work `study-tts-core` does not do.
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
    pub fn for_lesson(
        lesson: &ValidatedLesson,
        context: &SynthesisContext,
    ) -> Result<Self, PlanError> {
        Self::for_lesson_with_takes(lesson, context, &TakeSelection::implicit())
    }

    /// Derives the plan for a lesson at the takes a selection names.
    ///
    /// The take-aware constructor [`RenderPlan::for_lesson`] delegates to.
    /// ADR-0001 §11.4: "A requested alternate performance increments the
    /// segment's `take` integer, produces a new cache key, and retains the
    /// prior artifact" — the increment happens here, and the new key falls out
    /// of `take` already being a §12.5 synthesis-key input. Every segment the
    /// selection does not name stays at [`BASE_TAKE`], which is what keeps a
    /// retake's blast radius to one segment.
    ///
    /// # Errors
    ///
    /// [`PlanError::UnresolvedSpeaker`], exactly as [`RenderPlan::for_lesson`].
    pub fn for_lesson_with_takes(
        lesson: &ValidatedLesson,
        context: &SynthesisContext,
        selection: &TakeSelection<'_>,
    ) -> Result<Self, PlanError> {
        let segments = lesson
            .segments()
            .iter()
            .map(|segment| {
                // Checked before the digest rather than after, so an
                // unresolved speaker can never produce a key at all.
                if context.voice_conditioning_for(&segment.speaker).is_none() {
                    return Err(PlanError::UnresolvedSpeaker {
                        segment_id: segment.id.clone(),
                        speaker: segment.speaker.clone(),
                    });
                }
                // Resolved here rather than at the worker boundary, so a
                // segment naming a speaker the lesson never bound is refused
                // while a segment is still a plan rather than a request.
                let voice_profile = lesson
                    .speakers()
                    .get(&segment.speaker)
                    .ok_or_else(|| PlanError::UnresolvedSpeaker {
                        segment_id: segment.id.clone(),
                        speaker: segment.speaker.clone(),
                    })?
                    .voice_profile
                    .clone();
                let resolved = selection
                    .resolved
                    .get(segment.id.as_str())
                    .copied()
                    .unwrap_or(ResolvedTake {
                        take: BASE_TAKE,
                        approved_audio: None,
                    });
                let take = resolved.take;
                Ok(PlannedSegment {
                    id: segment.id.clone(),
                    speaker: segment.speaker.clone(),
                    voice_profile,
                    display_text: segment.display_text.clone(),
                    spoken_text: segment.spoken_text.clone(),
                    style: segment.style,
                    pause_after_ms: segment.pause_after_ms,
                    take,
                    cache_key: synthesis_digest(context, segment, take).into(),
                    // Derived rather than copied from the selection, so a
                    // recorded base key can never enter a plan unchecked.
                    synthesis_base_key: synthesis_digest(context, segment, BASE_TAKE).into(),
                    audio_blake3: resolved.approved_audio.cloned(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            schema_version: PLAN_SCHEMA_VERSION,
            lesson_id: lesson.lesson_id().to_owned(),
            plan_hash: plan_digest(&segments).into(),
            take_selection_source: selection.source(),
            segments,
        })
    }

    /// Checks the selection fields of a plan that was read back from disk.
    ///
    /// [`PlannedSegment::synthesis_base_key`] and
    /// [`PlannedSegment::audio_blake3`] are outside [`RenderPlan::plan_hash`],
    /// so the retained-plan loader's hash comparison does not cover them. This
    /// is the verification `synthesis_base_key` is recorded under instead: a
    /// base key is that segment's take-zero identity, so it equals `cache_key`
    /// exactly when `take` is [`BASE_TAKE`].
    ///
    /// [`PlannedSegment::audio_blake3`] is deliberately not checked here,
    /// because nothing on this path can check it: its verification is a
    /// comparison against the cache entry `cache_key` resolves to, which is
    /// filesystem work this crate does not do. `study_tts_runtime`'s render
    /// attempt performs it and refuses a disagreement.
    ///
    /// # Errors
    ///
    /// [`PlanError::BaseTakeKeyMismatch`] when a segment at take zero records a
    /// base key other than its own cache key, and
    /// [`PlanError::RetakeUsesBaseKey`] when a segment at a later take records
    /// its cache key as its base key.
    pub fn verify_recorded_selection(&self) -> Result<(), PlanError> {
        for segment in &self.segments {
            let keys_match = segment.synthesis_base_key == segment.cache_key;
            if segment.take == BASE_TAKE && !keys_match {
                return Err(PlanError::BaseTakeKeyMismatch {
                    segment_id: segment.id.clone(),
                });
            }
            if segment.take != BASE_TAKE && keys_match {
                return Err(PlanError::RetakeUsesBaseKey {
                    segment_id: segment.id.clone(),
                    take: segment.take,
                });
            }
        }
        Ok(())
    }
}

/// One segment's resolved take, and the audio a reviewer approved for it.
#[derive(Clone, Copy, Debug)]
struct ResolvedTake<'a> {
    take: u32,
    approved_audio: Option<&'a AudioDigest>,
}

/// The takes a plan is derived at, and where they came from.
///
/// One value rather than a take map beside a [`TakeSelectionSource`], because
/// the two must agree: an explicit source paired with no selections would claim
/// a reviewer approved a plan nobody recorded. The three constructors are the
/// three legitimate origins of a selection, and there is no fourth.
#[derive(Clone, Debug)]
pub struct TakeSelection<'a> {
    source: TakeSelectionSource,
    resolved: BTreeMap<&'a str, ResolvedTake<'a>>,
}

impl<'a> TakeSelection<'a> {
    /// Every segment at [`BASE_TAKE`], because no takes document was present.
    ///
    /// ADR-0001 §12.2 permits this for a private preview only;
    /// [`TakeSelectionSource::production`] is what refuses it elsewhere.
    #[must_use]
    pub fn implicit() -> Self {
        Self {
            source: TakeSelectionSource::Implicit,
            resolved: BTreeMap::new(),
        }
    }

    /// The takes a reviewer recorded, already reconciled with the base plan.
    ///
    /// Takes an [`AppliedSelection`] rather than a bare map because only
    /// [`crate::ValidatedTakes::reconcile_with_plan`] produces one: a selection
    /// that reaches planning has been compared with the plan it will be applied
    /// to, including ADR-0001 §12.2's synthesis-base-key refusal.
    #[must_use]
    pub fn explicit(selected: &'a AppliedSelection) -> Self {
        Self {
            source: TakeSelectionSource::Explicit,
            resolved: selected
                .selections()
                .map(|(segment_id, selection)| {
                    (
                        segment_id,
                        ResolvedTake {
                            take: selection.selected_take,
                            approved_audio: Some(&selection.audio_blake3),
                        },
                    )
                })
                .collect(),
        }
    }

    /// The takes a retained `plan.json` already established, for a resume.
    ///
    /// **Resume authority invariant.** Once a job has retained a valid
    /// `plan.json`, resume recovers its selection semantics from that plan and
    /// performs no discovery that could produce a semantically different plan
    /// from external mutable inputs. The retained plan is the already
    /// authoritative statement of what this job renders; a sibling
    /// `<lesson-stem>.takes.json` is external and may have been edited, moved,
    /// or deleted since the attempt that established the plan. Rediscovering it
    /// would synthesize a new semantic plan while claiming to continue an
    /// existing one — and [`crate::JobDocument::open_attempt`] compares no plan
    /// hashes, so the degradation would be silent rather than refused.
    ///
    /// A changed selection is therefore a new build attempt rather than a
    /// resume, which ADR-0001 §6.4 already has the edge for. The provenance is
    /// recovered too, so resuming an explicitly selected job cannot downgrade
    /// it and resuming an implicit one cannot promote it.
    #[must_use]
    pub fn recovered(retained: &'a RenderPlan) -> Self {
        Self {
            source: retained.take_selection_source,
            resolved: retained
                .segments
                .iter()
                .map(|segment| {
                    (
                        segment.id.as_str(),
                        ResolvedTake {
                            take: segment.take,
                            approved_audio: segment.audio_blake3.as_ref(),
                        },
                    )
                })
                .collect(),
        }
    }

    /// This selection with an alternate performance requested for some
    /// segments.
    ///
    /// ADR-0001 §11.4: "A requested alternate performance increments the
    /// segment's `take` integer, produces a new cache key, and retains the
    /// prior artifact." A request supersedes whatever was selected for that
    /// segment and carries no approved checksum, because nobody has reviewed
    /// audio that has not been rendered yet.
    ///
    /// Requesting one makes the whole selection
    /// [`TakeSelectionSource::Implicit`], and that is the point rather than a
    /// side effect: the resulting plan is a candidate a reviewer has not
    /// accepted, so it must not inherit an earlier document's authority to back
    /// a production release. Accepting the candidate into a takes file is what
    /// makes the next build explicit again.
    #[must_use]
    pub fn with_retakes(mut self, retakes: &'a BTreeMap<String, u32>) -> Self {
        if retakes.is_empty() {
            return self;
        }
        for (segment_id, take) in retakes {
            self.resolved.insert(
                segment_id.as_str(),
                ResolvedTake {
                    take: *take,
                    approved_audio: None,
                },
            );
        }
        self.source = TakeSelectionSource::Implicit;
        self
    }

    /// Where this selection came from, as the plan records it.
    #[must_use]
    pub const fn source(&self) -> TakeSelectionSource {
        self.source
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
            voice_profile,
            display_text,
            spoken_text,
            style,
            pause_after_ms,
            take,
            cache_key,
            // Deliberately not hashed, and the reasons differ.
            // `synthesis_base_key` is `synthesis_digest(context, segment,
            // BASE_TAKE)`, a pure function of inputs `cache_key` already
            // covers, so hashing it would add no discrimination — it is
            // verified by derivation instead. `audio_blake3` names the digest
            // of the entry `cache_key` already identifies, so a plan recording
            // a different one describes an impossible state rather than a
            // different plan — it is verified against the resolved entry
            // instead. `docs/architecture/E2-S2-INTERFACE-CHANGE-001.md`
            // §Identity effect records both as invariants I-2 and I-3, and
            // names this destructure in return.
            synthesis_base_key: _,
            audio_blake3: _,
        } = segment;

        CanonicalValue::object([
            ("id", id.as_str().into()),
            ("speaker", speaker.as_str().into()),
            // In the plan identity though not in the cache key: two profile
            // directories holding identical conditioning artifacts derive one
            // key, and a plan that rebound a segment between them would
            // otherwise read as unchanged while its consent trail moved.
            ("voice_profile", voice_profile.as_str().into()),
            ("display_text", display_text.as_str().into()),
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
        ValidatedLesson::from_json(
            "fixtures/lessons/e0-s0-two-segment.json",
            include_bytes!("../../../fixtures/lessons/e0-s0-two-segment.json"),
        )
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
        let plan = RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("the fixture context resolves every speaker");
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
        let plan = RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("the fixture context resolves every speaker");

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
    fn t1_e2_a_retained_plan_whose_recorded_base_key_is_not_its_own_is_refused() {
        // The identity hash covers `take` and `cache_key` and deliberately not
        // `synthesis_base_key`, so an edit to the recorded base key alone
        // leaves `derived_hash` agreeing with the document. This check is the
        // reason that field may sit outside the identity at all, and without a
        // case here nothing would notice if it stopped running.
        let plan = RenderPlan::for_lesson(&fixture_lesson(), &sample_context())
            .expect("the fixture context resolves every speaker");
        assert!(plan.verify_recorded_selection().is_ok());

        let other: CacheKey = "c"
            .repeat(CacheKey::LENGTH)
            .parse()
            .expect("a repeated hexadecimal digit is a cache key");

        let mut base_take = plan.clone();
        base_take.segments[0].synthesis_base_key = other.clone();
        assert_eq!(
            base_take.derived_hash(),
            base_take.plan_hash,
            "the edit must leave the identity agreeing, or this check is redundant"
        );
        assert!(matches!(
            base_take.verify_recorded_selection(),
            Err(PlanError::BaseTakeKeyMismatch { segment_id }) if segment_id == "seg-0001"
        ));

        let mut retake = plan.clone();
        retake.segments[1].take = BASE_TAKE + 1;
        retake.segments[1].synthesis_base_key = retake.segments[1].cache_key.clone();
        assert!(matches!(
            retake.verify_recorded_selection(),
            Err(PlanError::RetakeUsesBaseKey { segment_id, take })
                if segment_id == "seg-0002" && take == BASE_TAKE + 1
        ));
    }

    #[test]
    fn t1_e0_plan_is_stable_for_identical_inputs() {
        let lesson = fixture_lesson();

        let first = RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("the fixture context resolves every speaker");
        let second = RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("the fixture context resolves every speaker");

        // Pinned so an accidental change to the identity definition or the
        // canonical byte form is a failure here rather than a silent cache-wide
        // invalidation. These moved from their E0 values when E1-S1 adopted the
        // complete ADR-0001 §12.5 input set, again within E1-S1 when the cache
        // artifact gained its provenance record and `CACHE_SCHEMA_VERSION` —
        // itself a key input — moved to `1.0`, and again when E1-S2 resolved
        // voice references and moved `SYNTHESIS_IDENTITY_VERSION` to
        // `e1-s2-v1`. The second segment's key and the plan hash moved once
        // more within E1-S2, when `sample_context` gained the second speaker
        // `RenderPlan::for_lesson` now requires resolved; that is a fixture
        // change, not a production identity change. Each move recomputed these
        // values rather than relaxing the assertion.
        //
        // The plan hash moved once more when `PlannedSegment` began carrying
        // `display_text`; the two cache keys below deliberately did not, which
        // is the separation ADR-0001 §8.3 and §12.5 ask for and the reason a
        // transcript correction reuses every cached segment.
        //
        // It moved again in E1-S3, when the segment began carrying the resolved
        // `voice_profile` the worker protocol asks for by name. The cache keys
        // below again did not, and that is the same separation: §12.5 keys on
        // the conditioning artifact's hash, so renaming a profile directory
        // re-plans without re-rendering a single segment.
        //
        // All three moved together, still within E1-S3, when `artifact.json`
        // gained the required `edge_conditioning` record and
        // `CACHE_SCHEMA_VERSION` — itself a §12.5 key input — moved to `2.0`.
        // Unlike the two moves above this one *is* a cache-wide invalidation,
        // which is what the constant being a key input is for: an entry written
        // without the conditioning ADR-0001 §13.4 requires recorded cannot be
        // reused by a build that requires it.
        // `E1-S3-INTERFACE-CHANGE-002` records the move, and supersedes the
        // `abd889db…` plan hash `E1-S3-INTERFACE-CHANGE-001` §Plan document
        // cites.
        //
        // All three moved together again at E1-S5, and this one is a cache-wide
        // invalidation for two independent reasons rather than one.
        // `SynthesisContext` gained `model_artifacts_hash`, so the key follows
        // the model's bytes rather than the name of the acquisition they
        // arrived under — issue #66 — and `SYNTHESIS_IDENTITY_VERSION` moved to
        // `e1-s5-v1` because the input list changed, which is the lever that
        // constant exists to be. `CACHE_SCHEMA_VERSION` moved to `3.0` in the
        // same change, because `ArtifactProvenance` has to record the new input
        // or an entry cannot recompute the key it is published under.
        // `E1-S5-INTERFACE-CHANGE-002` records all of it.
        assert_eq!(
            first.plan_hash.as_str(),
            "e7d4a8c9de93cdb52a45e08f1667cf397453b6653363046411a61abb8fb666d1"
        );
        assert_eq!(
            first.segments[0].cache_key.as_str(),
            "05f1f5f546d723c8ccbf74922487f83e9479e5c4977928f8e396c85d05ba5e6e"
        );
        assert_eq!(
            first.segments[1].cache_key.as_str(),
            "6aad0b954673dcc918b39aa75764a3f0a989aa069c0ca494e8973f0ecaaf3822"
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

        let first = RenderPlan::for_lesson(&lesson, &baseline)
            .expect("the fixture context resolves every speaker");
        for (input, changed) in [
            ("worker_bundle_hash", rebuilt_worker),
            ("model_revision", newer_model),
            ("tokenizer_revision", newer_tokenizer),
        ] {
            let second = RenderPlan::for_lesson(&lesson, &changed)
                .expect("the fixture context resolves every speaker");

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

    #[test]
    fn t2_e1_plan_is_stable_for_identical_lesson_input() {
        // `DELIVERY-PLAN.md` E1-S2. Two documents carrying the same lesson
        // must plan to the same identities however they were written, or a
        // reformatting pass would invalidate every cache entry the lesson
        // owns. `AuthoredLesson` holds `speakers` in a `BTreeMap` for exactly
        // this reason, and the plan hash is derived from the canonical byte
        // form rather than from serde's output; neither claim is checkable
        // without a differently written copy of one lesson to compare against.
        let document: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("the fixture parses as JSON");
        assert!(
            !document["segments"]
                .as_array()
                .expect("the fixture has segments")
                .is_empty(),
            "a lesson with no segments would make every assertion below vacuous"
        );

        let planned = |bytes: &[u8], expectation: &str| {
            let lesson =
                ValidatedLesson::from_json("fixtures/lessons/e0-s0-two-segment.json", bytes)
                    .expect(expectation);
            RenderPlan::for_lesson(&lesson, &sample_context())
                .expect("the fixture context resolves every speaker")
        };

        let rewritten = reversed_key_order(&document);
        // Without this the property is vacuous: two identical byte strings
        // plan identically for reasons that have nothing to do with the claim.
        assert_ne!(
            rewritten.as_bytes(),
            include_bytes!("../../../fixtures/lessons/e0-s0-two-segment.json").as_slice(),
            "the rewritten document must actually be written differently"
        );

        let first = planned(
            include_bytes!("../../../fixtures/lessons/e0-s0-two-segment.json"),
            "the fixture is valid",
        );
        let second = planned(rewritten.as_bytes(), "the reordered fixture is valid");

        assert_eq!(first.plan_hash, second.plan_hash);
        for (left, right) in first.segments.iter().zip(&second.segments) {
            assert_eq!(left.id, right.id, "segment order must be preserved");
            assert_eq!(
                left.cache_key, right.cache_key,
                "`{}` must keep its cache entry across a rewrite",
                left.id
            );
        }
    }

    /// Re-emits a document with every object's keys written in reverse order.
    ///
    /// `serde_json::Value` holds an object's keys sorted, so serializing one
    /// cannot produce a differently ordered document; this is what lets the
    /// property above hand the parser the same lesson written another way.
    /// Arrays keep their order, because a lesson's segment order is its
    /// speaking order rather than a spelling.
    fn reversed_key_order(value: &Value) -> String {
        match value {
            Value::Object(fields) => {
                let entries: Vec<String> = fields
                    .iter()
                    .rev()
                    .map(|(key, field)| {
                        let key = Value::String(key.clone());
                        format!("{key}:{}", reversed_key_order(field))
                    })
                    .collect();
                format!("{{{}}}", entries.join(","))
            }
            Value::Array(items) => {
                let entries: Vec<String> = items.iter().map(reversed_key_order).collect();
                format!("[{}]", entries.join(","))
            }
            scalar => scalar.to_string(),
        }
    }

    #[test]
    fn t1_e1_a_speaker_without_a_resolved_voice_cannot_be_planned() {
        // ADR-0001 §12.5 makes the conditioning artifact a synthesis-key
        // input, and `CanonicalValue::optional` would have serialized its
        // absence into a perfectly well-formed key — one naming audio no voice
        // produced. Refusing is the only outcome that leaves no such key for a
        // later build to publish under.
        let lesson = fixture_lesson();
        let mut unresolved = sample_context();
        unresolved.voice_conditioning_hashes.remove("nadia");

        let error = RenderPlan::for_lesson(&lesson, &unresolved)
            .expect_err("a speaker with no resolved voice must not be planned");

        assert!(
            matches!(
                &error,
                PlanError::UnresolvedSpeaker { segment_id, speaker }
                    if segment_id == "seg-0001" && speaker == "nadia"
            ),
            "expected an unresolved-speaker refusal, got {error}"
        );
        // The same lesson under the complete context plans, which is what
        // proves the missing artifact is the only thing wrong with it.
        RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("only the unresolved voice may refuse this lesson");
    }

    #[test]
    fn t1_e1_display_text_reaches_the_plan_without_reaching_a_cache_key() {
        // `DELIVERY-PLAN.md` E1-S2 task 4 asks for display text to be
        // preserved separately, and the package writer is handed the plan and
        // the cached audio and nothing else — so a plan that dropped it would
        // leave nothing downstream able to write a transcript.
        //
        // The two identities must move differently: correcting a transcript is
        // a new package (the plan hash moves) rendered from the same audio
        // (every cache key stands), which is ADR-0001 §8.3 and §12.5 read
        // together. An assertion on either one alone would pass for a plan
        // that put display text in both or in neither.
        let context = sample_context();
        let baseline = RenderPlan::for_lesson(&fixture_lesson(), &context)
            .expect("the fixture context resolves every speaker");

        let mut document: Value = serde_json::from_slice(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("fixture JSON should parse");
        document["segments"][0]["display_text"] =
            Value::String("A cache stores work you can reuse.".to_owned());
        let corrected = ValidatedLesson::from_json(
            "<corrected transcript>",
            &serde_json::to_vec(&document).expect("the corrected lesson serializes"),
        )
        .expect("only the display text changed");
        let corrected = RenderPlan::for_lesson(&corrected, &context)
            .expect("the fixture context resolves every speaker");

        assert_eq!(
            corrected.segments[0].display_text,
            "A cache stores work you can reuse."
        );
        assert_ne!(
            corrected.plan_hash, baseline.plan_hash,
            "a corrected transcript must be a different package"
        );
        for (segment, unchanged) in corrected.segments.iter().zip(&baseline.segments) {
            assert_eq!(
                segment.cache_key, unchanged.cache_key,
                "a corrected transcript must reuse the audio for `{}`",
                segment.id
            );
        }
    }
}
