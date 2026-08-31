//! The speech-affecting inputs that name a cached synthesis take.
//!
//! ADR-0001 §12.5 lists exactly what belongs in a synthesis key and what does
//! not. This module is that list in Rust, plus the terms [`segment_digest`]
//! accounts for one by one because §12.5 does not name them:
//! [`SynthesisContext`] holds the inputs that come from the environment rather
//! than the lesson, and [`synthesis_digest`] combines them with one segment's
//! speech-affecting fields under [`crate::canonical_digest`].
//!
//! The field list is written out rather than derived from a struct. A hash over
//! a whole `LessonSegment` would silently absorb every field added later, and
//! the property that display-only data stays out of the key would regress
//! without a compile error. Naming each field means adding one is a decision
//! somebody makes on purpose.
//!
//! Verification identities live in [`crate::verification`] and share nothing
//! with this module but the canonical byte form. That separation is the point
//! of ADR-0001 §12.5: re-running ASR must never re-run synthesis.

use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::{blake3_newtype, json_schema_as_string};
use crate::lesson::LessonSegment;
use crate::plan::{CacheKey, PlannedSegment};
use crate::{
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE,
    CanonicalValue, LanguageTag, VoiceConditioningHash, canonical_digest,
};

/// Version of the synthesis-key definition itself.
///
/// The single lever that invalidates every cache entry when the list of inputs
/// below changes. It moved from `e0-s0-v1` when E1-S1 replaced the six-field
/// walking-skeleton identity with the complete ADR-0001 §12.5 input set, and
/// again when E1-S2 resolved voice references so `voice_conditioning_hash`
/// stopped serializing as absent for every speaker. The reasoning is recorded
/// in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` and
/// `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`.
pub const SYNTHESIS_IDENTITY_VERSION: &str = "e1-s2-v1";

/// Version of the cache-entry record format.
///
/// A distinct lever from [`SYNTHESIS_IDENTITY_VERSION`]: that one versions what
/// the key means, this one versions the artifact the key names. ADR-0001 §12.5
/// lists both, so a change to either must invalidate reuse. Defined here rather
/// than in the cache because `study-tts-runtime` reads it as an identity input
/// and a second copy could drift; `crates/study-tts-runtime/src/cache.rs`
/// imports this constant and names this module in return.
pub const CACHE_SCHEMA_VERSION: &str = "2.0";

/// Whether a backend reproduces bytes for a fixed seed.
///
/// A closed vocabulary rather than a boolean because it is recorded in a
/// durable identity, and ADR-0001 §12.5 is explicit that a seed alone proves
/// nothing: "identical seeds do not guarantee identical output across
/// dependency, platform, or execution changes."
///
/// The two variants are provisional in the sense that no third has been needed;
/// which class standard Chatterbox belongs to is settled by ADR-0002's measured
/// evidence when E1-S3 wires the real backend, not here.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub enum DeterminismClass {
    /// The same request and seed reproduce the same bytes on the same bundle.
    Reproducible,
    /// The seed constrains sampling, but bytes may still differ between runs.
    SeededNondeterministic,
}

impl DeterminismClass {
    /// The value as it is written into an identity and a manifest.
    ///
    /// Pinned to the serde representation by
    /// `t1_e1_determinism_class_spelling_matches_its_serde_form`, so the
    /// identity and the recorded JSON cannot drift apart.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reproducible => "reproducible",
            Self::SeededNondeterministic => "seeded_nondeterministic",
        }
    }
}

/// The deterministic identity of the executable worker bundle.
///
/// A value object rather than a `String` for the reason [`crate::CacheKey`] is
/// one: it is a digest that reaches a durable identity, and a digest typed as a
/// string is one any caller can set to anything. `study-tts-runtime` computes
/// it from the declared bundle inputs; this crate only accepts it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorkerBundleHash(String);

impl WorkerBundleHash {
    /// The hash as it is written into an identity, a manifest, and a frame.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(WorkerBundleHash, MalformedWorkerBundleHash);

/// Remedy routing: the bundle hash is derived mechanically from the worker's
/// declared inputs (ADR-0001 §12.5), so the message names recomputing it rather
/// than editing the recorded value.
#[derive(Debug, Error)]
#[error(
    "worker bundle hash `{0}` is not a BLAKE3 digest in lowercase hexadecimal; ADR-0001 §12.5 \
     derives this hash mechanically from the declared worker inputs; recompute it from the \
     worker bundle rather than editing the recorded value"
)]
pub struct MalformedWorkerBundleHash(String);

json_schema_as_string!(
    WorkerBundleHash,
    "WorkerBundleHash",
    "BLAKE3 over the worker bundle's declared inputs and runtime ABI \
     (ADR-0001 12.5), as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// Longest model or tokenizer revision this boundary accepts, in bytes.
///
/// A provisional ceiling on what an unchecked string may carry into every cache
/// key a job derives. Comfortably above a 64-hex digest or a released tag, and
/// far below anything a mistyped configuration produces.
pub const MAX_REVISION_BYTES: usize = 64;

/// Refs that name whatever a repository holds today rather than fixed bytes.
///
/// Compared case-insensitively, and deliberately short: it catches the mistake
/// an operator actually makes — pasting a branch where a pinned commit belongs
/// — and claims nothing beyond it. No string can prove itself immutable, and
/// what settles that for this project is the pinned rights record under
/// `evidence/rights/`, not this list.
const MOVING_REFS: [&str; 5] = ["dev", "head", "latest", "main", "master"];

/// A model or tokenizer revision, checked before it can reach a cache key.
///
/// ADR-0001 §12.5 makes both revisions synthesis-key inputs, which is what
/// makes this a value object rather than a `String`. An empty revision, one
/// that differs from another only by surrounding whitespace, and a branch name
/// all hash to a perfectly well-formed key — and a key whose inputs can change
/// while the key does not is a false hit, which ships audio the identity does
/// not describe.
///
/// Constructed only by parsing, so the check cannot be stepped around,
/// including at the `serde` boundary: a hand-edited cache manifest is refused
/// as malformed rather than compared and reported as a key mismatch, which
/// would tell an operator their audio had been tampered with when their
/// revision had.
///
/// # Examples
///
/// ```rust
/// use study_tts_core::Revision;
///
/// let pinned: Revision = "1b475dffa71fb191cb6d5901215eb6f55635a9b6".parse()?;
/// assert_eq!(pinned.as_str(), "1b475dffa71fb191cb6d5901215eb6f55635a9b6");
/// assert!("main".parse::<Revision>().is_err());
/// # Ok::<(), study_tts_core::MalformedRevision>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Revision(String);

impl Revision {
    /// The revision as it is written into an identity and a manifest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<Revision> for String {
    fn from(revision: Revision) -> Self {
        revision.0
    }
}

impl TryFrom<String> for Revision {
    type Error = MalformedRevision;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl FromStr for Revision {
    type Err = MalformedRevision;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty()
            || value.len() > MAX_REVISION_BYTES
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(MalformedRevision::Unusable {
                revision: value.to_owned(),
            });
        }
        if MOVING_REFS
            .iter()
            .any(|moving| value.eq_ignore_ascii_case(moving))
        {
            return Err(MalformedRevision::MovingRef {
                revision: value.to_owned(),
            });
        }
        Ok(Self(value.to_owned()))
    }
}

/// Why a string is not a [`Revision`].
///
/// Two variants because the two faults have two remedies: one is a revision to
/// correct, the other is a ref to resolve. Both name the worker/runtime owner
/// that `docs/governance/ROUTING-TABLES.md` §Failure routing gives worker
/// failures, because a revision is configuration that owner pins rather than a
/// value recorded from a measurement.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum MalformedRevision {
    /// Empty, over [`MAX_REVISION_BYTES`], or carrying a character no pinned
    /// revision contains — whitespace above all, which would let one revision
    /// reach a key under two spellings.
    #[error(
        "model or tokenizer revision `{revision}` is not usable; it must be 1 to {} printable \
         ASCII characters with no spaces; the worker/runtime owner must correct the pinned \
         revision the backend is configured with",
        MAX_REVISION_BYTES
    )]
    Unusable {
        /// The string that was offered as a revision.
        revision: String,
    },

    /// A branch or alias, which names different bytes at different times.
    #[error(
        "model or tokenizer revision `{revision}` names a moving ref rather than fixed bytes; \
         ADR-0001 §12.5 hashes this revision into every cache key derived with it, so the \
         worker/runtime owner must pin the commit this ref resolves to"
    )]
    MovingRef {
        /// The ref that was offered as a revision.
        revision: String,
    },
}

/// The speech-affecting inputs that do not come from the lesson.
///
/// Every field is an ADR-0001 §12.5 synthesis-key input. Nothing display-only
/// belongs here: the lesson title, source formatting, and a segment's
/// `display_text`, `role`, `source_refs`, `editorial`, and `pause_after_ms` are
/// all excluded by construction because this type cannot see them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisContext {
    /// Identity of the executable worker bundle that will synthesize.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Model repository the backend loads from.
    pub model_repository: String,
    /// Pinned model revision.
    ///
    /// A [`Revision`] rather than a `String` because it reaches the key: the
    /// type is where "never a moving tag" stops being a comment a reader has
    /// to trust and becomes a value that cannot be built from one.
    pub model_revision: Revision,
    /// Tokenizer or codec revision, which changes how text becomes audio.
    ///
    /// Typed for the reason [`SynthesisContext::model_revision`] is, and with
    /// the same consequence: different tokenizer bytes under one key is audio
    /// the key does not describe.
    pub tokenizer_revision: Revision,
    /// Spoken language of the lesson, checked and case-normalized.
    ///
    /// A [`LanguageTag`] rather than a `String` because it reaches the key:
    /// `en-US` and `en-us` are one language, and as authored bytes they would
    /// be two cache entries holding identical audio.
    pub language: LanguageTag,
    /// Whether a fixed seed is expected to reproduce bytes.
    pub determinism_class: DeterminismClass,
    /// Seed handed to the backend's sampler.
    pub seed: u64,
    /// Backend generation parameters, by name.
    ///
    /// Values are the exact configured spelling rather than a parsed number,
    /// because the parameters that matter here are floating point and
    /// [`CanonicalValue`] admits no float. Two spellings of one value —
    /// `"0.5"` and `"0.50"` — therefore produce different keys. That is the
    /// safe direction: an unnecessary cache miss costs one re-synthesis, while
    /// a false hit ships audio the identity does not describe.
    pub generation_parameters: BTreeMap<String, String>,
    /// Voice-conditioning artifact hash for each speaker, by speaker name.
    ///
    /// Populated from the lesson's `speakers` declarations by
    /// `study-tts-runtime`'s voice gate, which resolves each one before
    /// planning. Absent for a speaker the map does not carry, and absent and
    /// empty serialize differently, so an unresolved speaker can never
    /// silently match a resolved one.
    ///
    /// Typed for the reason [`SynthesisContext::worker_bundle_hash`] is, and
    /// the consequence here is the same: these reach the key, so a `String`
    /// that was not a digest produced a well-formed key naming audio no
    /// conditioning artifact could have made.
    pub voice_conditioning_hashes: BTreeMap<String, VoiceConditioningHash>,
}

impl SynthesisContext {
    /// The voice-conditioning artifact hash for one speaker, if resolved.
    pub fn voice_conditioning_for(&self, speaker: &str) -> Option<&VoiceConditioningHash> {
        self.voice_conditioning_hashes.get(speaker)
    }

    /// Recomputes the synthesis identity of an already-planned segment.
    ///
    /// The planner derives a key from the context it *intends* to use; an
    /// executor reports the context it *did* use. Running both through this one
    /// function is what lets a caller compare the two as a single value rather
    /// than field by field — and a field-by-field comparison is exactly the
    /// check that silently stops covering a field somebody adds later.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use study_tts_core::{PlanError, RenderPlan, SynthesisContext};
    /// # use study_tts_core::ValidatedLesson;
    /// fn reproducible(
    ///     lesson: &ValidatedLesson,
    ///     context: &SynthesisContext,
    /// ) -> Result<(), PlanError> {
    ///     let plan = RenderPlan::for_lesson(lesson, context)?;
    ///     for segment in &plan.segments {
    ///         assert_eq!(&context.key_for(segment), &segment.cache_key);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub fn key_for(&self, segment: &PlannedSegment) -> CacheKey {
        segment_digest(
            self,
            &segment.speaker,
            &segment.spoken_text,
            segment.style.as_str(),
            segment.take,
        )
        .into()
    }
}

/// Derives the synthesis identity of one segment at one take.
///
/// Take zero is the synthesis default and its digest is the segment's
/// *synthesis base key* in ADR-0001 §12.2 terms; a takes file selects a
/// non-zero take against that base.
///
/// The inputs are ADR-0001 §12.5's and the five [`segment_digest`] accounts
/// for beyond them; the canonical byte form is [`crate::canonical_bytes`], so
/// the result is stable across rebuilds.
pub(crate) fn synthesis_digest(
    context: &SynthesisContext,
    segment: &LessonSegment,
    take: u32,
) -> blake3::Hash {
    segment_digest(
        context,
        &segment.speaker,
        &segment.spoken_text,
        segment.style.as_str(),
        take,
    )
}

/// The one place the ADR-0001 §12.5 input list is written out.
///
/// Takes the four speech-affecting segment fields rather than a segment type,
/// so an authored [`LessonSegment`] and an already-planned [`PlannedSegment`]
/// reach the same bytes. Two functions that each listed the inputs would agree
/// until one of them was edited.
///
/// Five terms below are not in §12.5's list, and each only ever splits a key
/// that would otherwise be shared. That is the safe direction: a spare cache
/// miss costs one re-synthesis, while a false hit ships audio the key does not
/// describe.
///
/// - `identity_version` is [`SYNTHESIS_IDENTITY_VERSION`], the lever that
///   invalidates every entry when this list itself changes.
/// - `sample_rate`, `channels`, and `bits_per_sample` spell out what §12.5
///   calls the target intermediate sample format. ADR-0001 §13.1 defines the
///   canonical intermediate as all four together, and an integer stream of the
///   same width is not the same format.
/// - `speaker` is here even though `voice_conditioning_hash` now resolves,
///   because two speakers may lawfully share one voice profile. Without it
///   they would share a key, and a later story that gives one of them its own
///   profile would find the other's audio already published under it.
fn segment_digest(
    context: &SynthesisContext,
    speaker: &str,
    spoken_text: &str,
    style: &str,
    take: u32,
) -> blake3::Hash {
    let generation_parameters = CanonicalValue::Object(
        context
            .generation_parameters
            .iter()
            .map(|(name, value)| (name.clone(), CanonicalValue::from(value.clone())))
            .collect(),
    );

    canonical_digest(&CanonicalValue::object([
        ("identity_version", SYNTHESIS_IDENTITY_VERSION.into()),
        ("cache_schema_version", CACHE_SCHEMA_VERSION.into()),
        (
            "worker_bundle_hash",
            context.worker_bundle_hash.as_str().into(),
        ),
        ("model_repository", context.model_repository.as_str().into()),
        ("model_revision", context.model_revision.as_str().into()),
        (
            "tokenizer_revision",
            context.tokenizer_revision.as_str().into(),
        ),
        // Still optional, though `RenderPlan::for_lesson` refuses to derive a
        // key without it: `key_for` recomputes an *executor's* reported
        // identity, and a report that dropped the artifact must produce a key
        // that differs rather than a panic. Absent and empty serialize
        // differently, so the mismatch is what the cache sees.
        (
            "voice_conditioning_hash",
            CanonicalValue::optional(
                context
                    .voice_conditioning_for(speaker)
                    .map(VoiceConditioningHash::as_str),
            ),
        ),
        ("speaker", speaker.into()),
        ("language", context.language.as_str().into()),
        ("spoken_text", spoken_text.into()),
        ("style", style.into()),
        ("generation_parameters", generation_parameters),
        ("seed", context.seed.into()),
        (
            "determinism_class",
            context.determinism_class.as_str().into(),
        ),
        ("take", take.into()),
        ("sample_rate", CANONICAL_SAMPLE_RATE.into()),
        ("channels", CANONICAL_CHANNELS.into()),
        ("sample_format", CANONICAL_SAMPLE_FORMAT.into()),
        ("bits_per_sample", CANONICAL_BITS_PER_SAMPLE.into()),
    ]))
}

/// One fixture revision, parsed rather than asserted well formed.
#[cfg(test)]
fn revision(value: &str) -> Revision {
    value.parse().expect("a fixture revision is well formed")
}

/// A fully populated context for tests in this crate.
///
/// Every field is set to a distinguishable value so a test that mutates one
/// proves that field reaches the key, rather than proving only that some field
/// does. Lives beside the definition so a new input has to be given a value
/// here, which is what makes the sensitivity property exhaustive.
#[cfg(test)]
pub(crate) fn sample_context() -> SynthesisContext {
    SynthesisContext {
        worker_bundle_hash: "1".repeat(64).parse().expect("a digest of ones parses"),
        model_repository: "example/standard-chatterbox".to_owned(),
        model_revision: revision("0123456789abcdef0123456789abcdef01234567"),
        tokenizer_revision: revision("tokenizer-2026-01"),
        language: "en".parse().expect("`en` is a well-formed language tag"),
        determinism_class: DeterminismClass::SeededNondeterministic,
        seed: 42,
        generation_parameters: BTreeMap::from([
            ("cfg_weight".to_owned(), "0.5".to_owned()),
            ("exaggeration".to_owned(), "0.5".to_owned()),
        ]),
        // Both speakers the committed lesson fixtures declare, because
        // `RenderPlan::for_lesson` now refuses a lesson whose speaker this map
        // does not carry. Distinct digests, so a test comparing two speakers'
        // keys is comparing two voices rather than one repeated.
        voice_conditioning_hashes: BTreeMap::from([
            (
                "nadia".to_owned(),
                "2".repeat(64).parse().expect("a digest of twos parses"),
            ),
            (
                "tom".to_owned(),
                "3".repeat(64).parse().expect("a digest of threes parses"),
            ),
        ]),
    }
}

/// A segment whose speaker resolves to a voice hash in [`sample_context`].
#[cfg(test)]
pub(crate) fn sample_segment() -> LessonSegment {
    LessonSegment {
        id: "seg-0001".to_owned(),
        speaker: "nadia".to_owned(),
        role: crate::SegmentRole::Explanation,
        source_refs: vec!["block-001".to_owned()],
        display_text: "A cache stores reusable work.".to_owned(),
        spoken_text: "A cache stores reusable work.".to_owned(),
        style: crate::DeliveryStyle::CalmExplanatory,
        pause_after_ms: 75,
        review_status: crate::ReviewStatus::Approved,
        editorial: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BASE_TAKE, DeliveryStyle, ReviewStatus, SegmentRole};

    /// One named change to a synthesis input, for the sensitivity property.
    type ContextMutation = (&'static str, fn(&mut SynthesisContext));

    /// One named change to a segment field, for the sensitivity property.
    type SegmentMutation = (&'static str, fn(&mut LessonSegment));

    #[test]
    fn t1_e1_only_a_pinned_revision_reaches_a_synthesis_key() {
        // Each refusal below hashes into a perfectly well-formed key while
        // naming model bytes that can differ or that another spelling of the
        // same revision would name differently. Both are false hits, and a
        // false hit ships audio the key does not describe.
        let too_long = "x".repeat(MAX_REVISION_BYTES + 1);
        let at_ceiling = "x".repeat(MAX_REVISION_BYTES);

        for value in ["", " ", "v1 ", " v1", "a b", "v\u{e9}1", too_long.as_str()] {
            assert_eq!(
                value.parse::<Revision>(),
                Err(MalformedRevision::Unusable {
                    revision: value.to_owned()
                }),
                "`{value}` must be refused as unusable"
            );
        }

        // Case-insensitively, because a ref does not stop moving when it is
        // shouted.
        for value in ["dev", "HEAD", "latest", "main", "Master"] {
            assert_eq!(
                value.parse::<Revision>(),
                Err(MalformedRevision::MovingRef {
                    revision: value.to_owned()
                }),
                "`{value}` must be refused as a moving ref"
            );
        }

        // The shapes a pin actually takes here, and the ceiling from the
        // accepting side so an off-by-one cannot pass: a commit (ADR-0002
        // pins one), a release tag, the `none` a backend with no separate
        // tokenizer reports.
        for value in [
            "1b475dffa71fb191cb6d5901215eb6f55635a9b6",
            "v0.1.2",
            "none",
            at_ceiling.as_str(),
        ] {
            assert_eq!(
                value
                    .parse::<Revision>()
                    .as_ref()
                    .map(Revision::as_str)
                    .map_err(ToString::to_string),
                Ok(value),
                "`{value}` is a pinned revision and must be accepted"
            );
        }
    }

    #[test]
    fn t2_e1_every_speech_affecting_field_changes_synthesis_key() {
        // Destructured without `..` so adding a field to either type is a
        // compile error here until somebody decides which list it belongs in.
        // That is what keeps this property exhaustive rather than merely long.
        let SynthesisContext {
            worker_bundle_hash: _,
            model_repository: _,
            model_revision: _,
            tokenizer_revision: _,
            language: _,
            determinism_class: _,
            seed: _,
            generation_parameters: _,
            voice_conditioning_hashes: _,
        } = sample_context();
        let LessonSegment {
            id: _,
            speaker: _,
            role: _,
            source_refs: _,
            display_text: _,
            spoken_text: _,
            style: _,
            pause_after_ms: _,
            review_status: _,
            editorial: _,
        } = sample_segment();

        let baseline = synthesis_digest(&sample_context(), &sample_segment(), BASE_TAKE);

        // ADR-0001 §12.5 synthesis-key inputs that come from the environment.
        let context_inputs: [ContextMutation; 9] = [
            ("worker_bundle_hash", |context| {
                context.worker_bundle_hash =
                    "9".repeat(64).parse().expect("a digest of nines parses");
            }),
            ("model_repository", |context| {
                context.model_repository = "example/other-chatterbox".to_owned();
            }),
            ("model_revision", |context| {
                context.model_revision = revision("fedcba9876543210fedcba9876543210fedcba98");
            }),
            ("tokenizer_revision", |context| {
                context.tokenizer_revision = revision("tokenizer-2026-02");
            }),
            ("language", |context| {
                context.language = "de".parse().expect("`de` is a well-formed language tag");
            }),
            ("determinism_class", |context| {
                context.determinism_class = DeterminismClass::Reproducible;
            }),
            ("seed", |context| {
                context.seed = 43;
            }),
            ("generation_parameters", |context| {
                context
                    .generation_parameters
                    .insert("cfg_weight".to_owned(), "0.6".to_owned());
            }),
            ("voice_conditioning_hashes", |context| {
                context.voice_conditioning_hashes.insert(
                    "nadia".to_owned(),
                    "7".repeat(64).parse().expect("a digest of sevens parses"),
                );
            }),
        ];

        for (field, mutate) in context_inputs {
            let mut context = sample_context();
            mutate(&mut context);

            assert_ne!(
                synthesis_digest(&context, &sample_segment(), BASE_TAKE),
                baseline,
                "changing `{field}` must change the synthesis key"
            );
        }

        // Segment fields that reach the key: `spoken_text` and `style` from
        // ADR-0001 §12.5, and `speaker` for the reason `segment_digest` gives.
        let segment_inputs: [SegmentMutation; 3] = [
            ("speaker", |segment| {
                segment.speaker = "tom".to_owned();
            }),
            ("spoken_text", |segment| {
                segment.spoken_text = "A cache stores reusable work".to_owned();
            }),
            ("style", |segment| {
                segment.style = DeliveryStyle::Emphatic;
            }),
        ];

        for (field, mutate) in segment_inputs {
            let mut segment = sample_segment();
            mutate(&mut segment);

            assert_ne!(
                synthesis_digest(&sample_context(), &segment, BASE_TAKE),
                baseline,
                "changing `{field}` must change the synthesis key"
            );
        }

        // The take is not a segment field, but ADR-0001 §12.2 makes it an
        // input: a retake must not collide with the base key it was taken from.
        assert_ne!(
            synthesis_digest(&sample_context(), &sample_segment(), BASE_TAKE + 1),
            baseline,
            "changing the take must change the synthesis key"
        );

        // ADR-0001 §12.5: "It excludes display-only fields." A change to any of
        // these must reuse the cached audio, or every review edit will
        // re-synthesize a lesson that sounds exactly the same.
        let excluded: [SegmentMutation; 6] = [
            ("id", |segment| {
                segment.id = "seg-9999".to_owned();
            }),
            ("role", |segment| {
                segment.role = SegmentRole::Recap;
            }),
            ("source_refs", |segment| {
                segment.source_refs = vec!["block-002".to_owned()];
            }),
            ("display_text", |segment| {
                segment.display_text = "A cache stores work you can reuse.".to_owned();
            }),
            ("pause_after_ms", |segment| {
                segment.pause_after_ms = 500;
            }),
            ("review_status", |segment| {
                segment.review_status = ReviewStatus::NeedsReview;
            }),
        ];

        for (field, mutate) in excluded {
            let mut segment = sample_segment();
            mutate(&mut segment);

            assert_eq!(
                synthesis_digest(&sample_context(), &segment, BASE_TAKE),
                baseline,
                "changing `{field}` must not change the synthesis key"
            );
        }
    }

    #[test]
    fn t1_e1_two_speakers_reading_one_line_do_not_share_a_key() {
        // Clearing both resolved hashes isolates the speaker term. Otherwise,
        // distinct conditioning artifacts would keep the keys apart even if
        // `speaker` were removed from the identity.
        let mut context = sample_context();
        context.voice_conditioning_hashes.clear();
        let mut other_speaker = sample_segment();
        other_speaker.speaker = "tom".to_owned();

        assert_ne!(
            synthesis_digest(&context, &sample_segment(), BASE_TAKE),
            synthesis_digest(&context, &other_speaker, BASE_TAKE),
            "two speakers reading one line must not share cached audio"
        );
    }

    #[test]
    fn t1_e1_determinism_class_spelling_matches_its_serde_form() {
        // The identity writes `as_str` while a manifest writes the serde form.
        // An exhaustive match makes a new variant a compile error here rather
        // than a silently untested one.
        for class in [
            DeterminismClass::Reproducible,
            DeterminismClass::SeededNondeterministic,
        ] {
            let serialized =
                serde_json::to_string(&class).expect("a determinism class serializes as a string");

            assert_eq!(serialized, format!("\"{}\"", class.as_str()));
        }
    }

    #[test]
    fn t1_e1_worker_bundle_hashes_that_are_not_digests_are_rejected() {
        let too_short = "a".repeat(63);
        let uppercase = "A".repeat(64);
        let outside_hex = "g".repeat(64);

        for malformed in ["", "a", "日", &too_short, &uppercase, &outside_hex] {
            assert!(
                malformed.parse::<WorkerBundleHash>().is_err(),
                "`{malformed}` must not parse as a worker bundle hash"
            );
            // Deserialization is the boundary that matters: the hash arrives in
            // a worker frame and in a cached artifact's provenance.
            assert!(
                serde_json::from_value::<WorkerBundleHash>(serde_json::Value::String(
                    malformed.to_owned()
                ))
                .is_err(),
                "a recorded worker bundle hash `{malformed}` must not deserialize"
            );
        }
    }
}
