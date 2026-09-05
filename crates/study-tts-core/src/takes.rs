//! Versioned take-selection documents required by ADR-0001 §12.2.
//!
//! Two invariant classes live here, and they are deliberately separate.
//! [`TakesDocument::validate`] checks what holds without a plan — version,
//! identity spellings, one selection per segment, and the two cross-field
//! rules relating a take to its key. [`ValidatedTakes::reconcile_with_plan`]
//! checks what needs one: §12.2's refusal of a selection whose synthesis base
//! key no longer matches the current plan.
//!
//! Both are pure. Comparing a selection with the *cache* is the runtime's,
//! because resolving an entry is filesystem work this crate does not do.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    CacheKey, SchemaVersion, SchemaVersionError,
    lesson::{MAX_LESSON_SEGMENTS, validate_lesson_id, validate_segment_id},
    plan::RenderPlan,
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

/// Whether a build's take selection was recorded by a reviewer or generated.
///
/// ADR-0001 §12.2 makes take zero the synthesis default but requires "an
/// explicit versioned takes file even when every selection remains zero" for a
/// production release. Carrying that provenance as a value rather than
/// re-deriving it later is what stops a generated selection reaching a
/// production path by being passed along until nobody remembers where it came
/// from — the shape `study_tts_runtime::SilenceThreshold` already uses for the
/// same reason.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum TakeSelectionSource {
    /// No takes document was present, so every segment was planned at take
    /// zero. Permitted for a private preview and refused for production.
    Implicit,
    /// A validated takes document selected every segment.
    Explicit,
}

impl TakeSelectionSource {
    /// This provenance's name, in the spelling a plan and manifest record.
    ///
    /// Pinned to the serde representation by
    /// `t1_e2_take_selection_source_spelling_matches_its_serde_form`, so a
    /// refusal quotes what a reader will find in the file rather than a second
    /// spelling that drifted from it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Implicit => "implicit",
            Self::Explicit => "explicit",
        }
    }

    /// Confirms this selection may back a production release.
    ///
    /// # Errors
    ///
    /// [`ImplicitTakeSelection`] for [`TakeSelectionSource::Implicit`]. This is
    /// ADR-0001 §12.2's production rule made mechanical, and the gate
    /// `explicit_take_selection` in
    /// `docs/governance/RELEASE-PROFILES.md` §3 is what it lets a build claim.
    pub const fn production(&self) -> Result<(), ImplicitTakeSelection> {
        match self {
            Self::Explicit => Ok(()),
            Self::Implicit => Err(ImplicitTakeSelection),
        }
    }
}

/// A generated take-zero selection was offered for a production release.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
#[error(
    "this build selected take zero for every segment because no takes document was present; \
     ADR-0001 §12.2 requires an explicit versioned takes file for a production release even when \
     every selection remains zero, so the reviewer must record and accept one before this build \
     is offered for production"
)]
pub struct ImplicitTakeSelection;

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
    ///
    /// Held to the same rule `crate::lesson` applies to that identity, through
    /// [`validate_lesson_id`]: a document naming a lesson no lesson file can
    /// carry records approvals for a lesson that cannot exist.
    #[schemars(schema_with = "crate::lesson::portable_id_json_schema")]
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
    /// The document records selections for a different lesson than the plan
    /// describes.
    #[error(
        "takes document records selections for lesson `{recorded}` but the plan derives lesson \
         `{plan}`; the reviewer must record the selections against the lesson being built rather \
         than apply one lesson's approvals to another"
    )]
    LessonMismatch {
        /// Lesson the document claims.
        recorded: String,
        /// Lesson the plan was derived from.
        plan: String,
    },
    /// The plan handed in for comparison already carries a selection.
    ///
    /// A caller error rather than a document one, and refused rather than
    /// assumed: every recorded synthesis base key would be compared against a
    /// retake's key, so every stale selection would pass.
    #[error(
        "segment `{segment_id}` of the plan offered for reconciliation is at take {take} rather \
         than take zero, so no selection can be compared against its synthesis base keys; this \
         build must reconcile against the plan it derived before applying any selection"
    )]
    PlanIsNotBaseTakes {
        /// First segment found already carrying a selection.
        segment_id: String,
        /// Take that segment already carries.
        take: u32,
    },
    /// The plan has a segment the document records no selection for.
    #[error(
        "the plan carries segment `{segment_id}` but the takes document records no selection for \
         it; ADR-0001 §12.2 requires an explicit selection for every segment, so the reviewer must \
         record one rather than leave the segment unapproved"
    )]
    UnselectedSegment {
        /// Segment the plan carries and the document does not select.
        segment_id: String,
    },
    /// The document selects a segment the plan does not carry.
    #[error(
        "the takes document selects segment `{segment_id}`, which the plan does not carry; the \
         reviewer must re-record the selections against the current lesson rather than approve a \
         segment that will not be rendered"
    )]
    UnplannedSelection {
        /// Segment the document selects and the plan does not carry.
        segment_id: String,
    },
    /// The recorded synthesis base key is no longer the one the plan derives.
    ///
    /// ADR-0001 §12.2's refusal: the segment's speech-affecting inputs moved
    /// since the selection was recorded, so the approval was given for
    /// different words.
    #[error(
        "segment `{segment_id}` records synthesis base key `{recorded}` but the current plan \
         derives `{derived}`; the segment's speech-affecting inputs changed since the selection \
         was recorded, so the reviewer must review the current take and re-record the selection \
         rather than edit the recorded key"
    )]
    StaleSynthesisBaseKey {
        /// Segment whose recorded base key is stale.
        segment_id: String,
        /// Base key the document recorded.
        recorded: CacheKey,
        /// Base key the current plan derives.
        derived: CacheKey,
    },
    /// The recorded cache key of a selected take is not the one that take
    /// derives.
    ///
    /// Distinct from [`TakesError::StaleSynthesisBaseKey`], which reports that
    /// the segment's *inputs* moved. This reports a document whose recorded
    /// key does not name the artifact its own take identifies, so the approval
    /// points at an entry this build would never render.
    #[error(
        "segment `{segment_id}` selects take {selected_take} and records cache key `{recorded}`, \
         but that take derives `{derived}`; the recorded key names an artifact this build would \
         not render, so the reviewer must re-record the selection from the cache entry that was \
         reviewed rather than edit the recorded key"
    )]
    SelectedCacheKeyMismatch {
        /// Segment whose recorded selected key is wrong.
        segment_id: String,
        /// Take the selection claims.
        selected_take: u32,
        /// Key the document recorded.
        recorded: CacheKey,
        /// Key that take actually derives.
        derived: CacheKey,
    },
    /// The plan offered for verification is not the one these selections
    /// reconciled against.
    ///
    /// A caller error rather than a document one, and refused rather than
    /// assumed: [`AppliedSelection::verify_selected_keys`] checks the segments
    /// the plan lists, so a plan missing a reconciled segment leaves that
    /// segment's recorded key unverified and an empty plan verifies nothing at
    /// all. The remedy is this build's, not the reviewer's, which is why the
    /// message names no document.
    #[error(
        "segment `{segment_id}` is described by only one of the reconciled selections and the \
         plan offered to verify them, so the recorded selected keys cannot all be checked; this \
         build must verify against the plan it derived from the same lesson at these takes"
    )]
    SelectedPlanMismatch {
        /// First segment either side carries alone.
        segment_id: String,
    },
    /// The audio a selection approved is not the audio its cache entry holds.
    ///
    /// ADR-0001 §12.2: "Byte-identical reconstruction additionally requires
    /// the referenced cached artifact or an archived segment bundle; rerunning
    /// a nondeterministic model from the same synthesis request is not a
    /// byte-reconstruction guarantee." Refused rather than silently substituted
    /// for that reason: the approval was given for audio a reviewer listened
    /// to, and re-synthesized audio under the same key is different audio.
    #[error(
        "segment `{segment_id}` approved audio `{recorded}` but cache entry `{cache_key}` holds \
         `{actual}`; the approved artifact is no longer available and re-synthesis does not \
         reproduce it, so the reviewer must review the current take and re-record the selection"
    )]
    ApprovedAudioMismatch {
        /// Segment whose approved audio is absent.
        segment_id: String,
        /// Cache entry the selection names.
        cache_key: CacheKey,
        /// Digest the selection approved.
        recorded: AudioDigest,
        /// Digest the resolved entry holds.
        actual: AudioDigest,
    },
}

/// The takes a reconciled document selects, keyed by segment identity.
///
/// Produced only by [`ValidatedTakes::reconcile_with_plan`], so holding one is
/// evidence that every selection was compared against the plan it will be
/// applied to.
#[derive(Clone, Debug)]
pub struct AppliedSelection {
    selected: BTreeMap<String, SelectedTake>,
}

impl AppliedSelection {
    /// Every reconciled selection, keyed by segment identity.
    pub fn selections(&self) -> impl Iterator<Item = (&str, &SelectedTake)> {
        self.selected
            .iter()
            .map(|(segment_id, selection)| (segment_id.as_str(), selection))
    }

    /// The recorded selection for one segment, if the document selects it.
    #[must_use]
    pub fn selected(&self, segment_id: &str) -> Option<&SelectedTake> {
        self.selected.get(segment_id)
    }

    /// Checks that each recorded selected key is the one its take derives.
    ///
    /// [`ValidatedTakes::reconcile_with_plan`] compares recorded base keys with
    /// a plan at take zero, which proves the segment's inputs have not moved.
    /// It cannot check `selected_cache_key`, because the plan it is given has
    /// not applied the selection yet. `selected` is that plan — the one derived
    /// at these takes — and this is the second half of the comparison.
    ///
    /// # Errors
    ///
    /// [`TakesError::SelectedPlanMismatch`] when `selected` does not describe
    /// exactly the reconciled segments, and
    /// [`TakesError::SelectedCacheKeyMismatch`] when a recorded selected cache
    /// key is not the key its own take derives.
    pub fn verify_selected_keys(&self, selected: &RenderPlan) -> Result<(), TakesError> {
        // The caller's precondition is checked alongside the document's, in
        // both directions, because this gate reads only the segments `selected`
        // lists: a plan short of a reconciled segment would leave that
        // segment's recorded key unverified and still report success, so the
        // completeness of the check would belong to the argument rather than to
        // the selection. `reconcile_with_plan` makes `self.selected` name
        // exactly the base plan's segments, which is what the two loops compare
        // against.
        for segment in &selected.segments {
            let recorded =
                self.selected
                    .get(&segment.id)
                    .ok_or_else(|| TakesError::SelectedPlanMismatch {
                        segment_id: segment.id.clone(),
                    })?;
            if recorded.selected_cache_key != segment.cache_key {
                return Err(TakesError::SelectedCacheKeyMismatch {
                    segment_id: segment.id.clone(),
                    selected_take: recorded.selected_take,
                    recorded: recorded.selected_cache_key.clone(),
                    derived: segment.cache_key.clone(),
                });
            }
        }

        let verified: BTreeSet<&str> = selected
            .segments
            .iter()
            .map(|segment| segment.id.as_str())
            .collect();
        for segment_id in self.selected.keys() {
            if !verified.contains(segment_id.as_str()) {
                return Err(TakesError::SelectedPlanMismatch {
                    segment_id: segment_id.clone(),
                });
            }
        }
        Ok(())
    }
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

    /// Compares these selections with the plan they will be applied to.
    ///
    /// ADR-0001 §12.2 gives this check the plan
    /// [`TakesDocument::validate`] does not have. `base` must be the plan this
    /// build derived, every segment of which is at [`crate::BASE_TAKE`]:
    /// a planned segment's cache key is that segment's *synthesis base key*
    /// only while no selection has been applied, so reconciling against an
    /// already-selected plan would compare recorded base keys with retake keys
    /// and pass every stale selection.
    ///
    /// # Errors
    ///
    /// [`TakesError::LessonMismatch`] when the document records another
    /// lesson, [`TakesError::PlanIsNotBaseTakes`] when `base` already carries a
    /// selection, [`TakesError::UnselectedSegment`] when the plan carries a
    /// segment the document does not select,
    /// [`TakesError::StaleSynthesisBaseKey`] when a recorded base key is no
    /// longer the one the plan derives, and [`TakesError::UnplannedSelection`]
    /// when the document selects a segment the plan does not carry.
    pub fn reconcile_with_plan(&self, base: &RenderPlan) -> Result<AppliedSelection, TakesError> {
        if self.authored.lesson_id != base.lesson_id {
            return Err(TakesError::LessonMismatch {
                recorded: self.authored.lesson_id.clone(),
                plan: base.lesson_id.clone(),
            });
        }
        // The caller's precondition is checked before the document's, because
        // a wrong plan makes every verdict about the document unreliable.
        let planned: BTreeSet<&str> = base
            .segments
            .iter()
            .map(|segment| {
                if segment.take == crate::BASE_TAKE {
                    return Ok(segment.id.as_str());
                }
                Err(TakesError::PlanIsNotBaseTakes {
                    segment_id: segment.id.clone(),
                    take: segment.take,
                })
            })
            .collect::<Result<_, _>>()?;

        let selected: BTreeMap<String, SelectedTake> = self
            .authored
            .selections
            .iter()
            .map(|selection| (selection.segment_id.clone(), selection.clone()))
            .collect();

        for segment in &base.segments {
            let selection =
                selected
                    .get(&segment.id)
                    .ok_or_else(|| TakesError::UnselectedSegment {
                        segment_id: segment.id.clone(),
                    })?;
            if selection.synthesis_base_key != segment.cache_key {
                return Err(TakesError::StaleSynthesisBaseKey {
                    segment_id: segment.id.clone(),
                    recorded: selection.synthesis_base_key.clone(),
                    derived: segment.cache_key.clone(),
                });
            }
        }

        for segment_id in selected.keys() {
            if !planned.contains(segment_id.as_str()) {
                return Err(TakesError::UnplannedSelection {
                    segment_id: segment_id.clone(),
                });
            }
        }

        Ok(AppliedSelection { selected })
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
    use crate::{ValidatedLesson, identity::sample_context};

    /// The plan every reconciliation case below is compared against.
    fn base_plan() -> RenderPlan {
        let lesson = ValidatedLesson::from_json(
            "fixtures/lessons/e0-s0-two-segment.json",
            include_bytes!("../../../fixtures/lessons/e0-s0-two-segment.json"),
        )
        .expect("fixture should be valid");
        RenderPlan::for_lesson(&lesson, &sample_context())
            .expect("the sample context resolves both fixture speakers")
    }

    /// The document a reviewer would record having approved every take zero.
    fn selection_for(plan: &RenderPlan) -> TakesDocument {
        TakesDocument {
            schema: None,
            schema_version: TAKES_SCHEMA_VERSION.to_string(),
            lesson_id: plan.lesson_id.clone(),
            selections: plan
                .segments
                .iter()
                .map(|segment| SelectedTake {
                    segment_id: segment.id.clone(),
                    synthesis_base_key: segment.cache_key.clone(),
                    selected_take: crate::BASE_TAKE,
                    selected_cache_key: segment.cache_key.clone(),
                    audio_blake3: blake3::hash(segment.id.as_bytes()).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn t1_e2_stale_synthesis_base_key_is_rejected() {
        let plan = base_plan();
        let applied = selection_for(&plan)
            .validate()
            .expect("a document derived from the plan is internally valid")
            .reconcile_with_plan(&plan)
            .expect("a selection recorded against this plan reconciles with it");

        assert_eq!(
            applied
                .selections()
                .map(|(segment_id, selection)| (segment_id, selection.selected_take))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                ("seg-0001", crate::BASE_TAKE),
                ("seg-0002", crate::BASE_TAKE)
            ]),
        );

        // Each row is one violated invariant, the edit that violates it, and
        // the refusal ADR-0001 §12.2 requires for it.
        type Spoil = fn(&mut TakesDocument, &mut RenderPlan);
        type Expected = fn(&TakesError) -> bool;
        const CASES: [(&str, Spoil, Expected); 5] = [
            (
                "selections recorded for another lesson",
                |document, _| document.lesson_id = "e1-s4-three-segment".to_owned(),
                |error| {
                    matches!(
                        error,
                        TakesError::LessonMismatch { recorded, plan }
                            if recorded == "e1-s4-three-segment"
                                && plan == "e0-s0-walking-skeleton"
                    )
                },
            ),
            (
                "a plan that already carries a selection",
                |_, plan| plan.segments[1].take = crate::BASE_TAKE + 1,
                |error| {
                    matches!(
                        error,
                        TakesError::PlanIsNotBaseTakes { segment_id, take }
                            if segment_id == "seg-0002" && *take == crate::BASE_TAKE + 1
                    )
                },
            ),
            (
                "a segment the document leaves unapproved",
                |document, _| {
                    document.selections.retain(|s| s.segment_id != "seg-0002");
                },
                |error| {
                    matches!(
                        error,
                        TakesError::UnselectedSegment { segment_id } if segment_id == "seg-0002"
                    )
                },
            ),
            (
                "a selection for a segment the plan does not carry",
                |document, _| {
                    let mut extra = document.selections[0].clone();
                    extra.segment_id = "seg-0003".to_owned();
                    document.selections.push(extra);
                },
                |error| {
                    matches!(
                        error,
                        TakesError::UnplannedSelection { segment_id } if segment_id == "seg-0003"
                    )
                },
            ),
            (
                "a base key recorded before the segment's words changed",
                // Both keys move together, because that is the document a
                // reviewer actually recorded against the earlier plan; editing
                // only one would be refused by `validate` as a take-zero
                // mismatch and never reach this comparison.
                |document, _| {
                    document.selections[0].synthesis_base_key = key("c");
                    document.selections[0].selected_cache_key = key("c");
                },
                |error| {
                    matches!(
                        error,
                        TakesError::StaleSynthesisBaseKey { segment_id, recorded, derived }
                            if segment_id == "seg-0001"
                                && *recorded == key("c")
                                && *derived != key("c")
                    )
                },
            ),
        ];

        for (case, spoil, expected) in CASES {
            let mut document = selection_for(&plan);
            let mut spoiled = plan.clone();
            spoil(&mut document, &mut spoiled);

            let error = document
                .validate()
                .expect("every case must fail reconciliation, not document validation")
                .reconcile_with_plan(&spoiled)
                .expect_err("the spoiled selection must be refused");

            assert!(expected(&error), "{case}: {error}");
        }
    }

    /// The gate verifies the segments it reconciled, not the segments the
    /// caller happens to offer.
    ///
    /// `seg-0002` carries the same corruption in both halves. Catching it only
    /// while the plan lists the segment would make the refusal a property of
    /// the argument rather than of the recorded selection, which is the shape
    /// a vacuous gate takes: an empty plan would verify nothing and pass.
    #[test]
    fn t1_e2_selected_key_verification_refuses_a_plan_it_did_not_reconcile() {
        let base = base_plan();
        let applied = selection_for(&base)
            .validate()
            .expect("a document derived from the plan is internally valid")
            .reconcile_with_plan(&base)
            .expect("a selection recorded against this plan reconciles with it");

        let mut corrupt = base.clone();
        corrupt.segments[1].cache_key = key("c");

        assert!(
            matches!(
                applied.verify_selected_keys(&corrupt),
                Err(TakesError::SelectedCacheKeyMismatch { ref segment_id, .. })
                    if segment_id == "seg-0002"
            ),
            "a recorded key the plan does not derive must be refused",
        );

        corrupt.segments.retain(|segment| segment.id != "seg-0002");
        let error = applied
            .verify_selected_keys(&corrupt)
            .expect_err("a plan omitting a reconciled segment must be refused");

        assert!(
            matches!(
                &error,
                TakesError::SelectedPlanMismatch { segment_id } if segment_id == "seg-0002"
            ),
            "expected a mismatched-plan refusal, got {error}",
        );

        let mut foreign = base;
        foreign.segments[0].id = "seg-0003".to_owned();
        let error = applied
            .verify_selected_keys(&foreign)
            .expect_err("a plan carrying an unreconciled segment must be refused");

        assert!(
            matches!(
                &error,
                TakesError::SelectedPlanMismatch { segment_id } if segment_id == "seg-0003"
            ),
            "expected a mismatched-plan refusal, got {error}",
        );
    }

    #[test]
    fn t1_e2_take_selection_source_spelling_matches_its_serde_form() {
        // A plan and a manifest record the serde form; a refusal quotes
        // `name`. An exhaustive list makes a third provenance a compile error
        // here rather than a spelling nobody checked.
        for source in [TakeSelectionSource::Implicit, TakeSelectionSource::Explicit] {
            let serialized =
                serde_json::to_string(&source).expect("a take selection source serializes");

            assert_eq!(serialized, format!("\"{}\"", source.name()));
        }
    }

    #[test]
    fn t1_e2_an_implicit_take_selection_cannot_serve_a_production_release() {
        assert_eq!(
            TakeSelectionSource::Implicit.production(),
            Err(ImplicitTakeSelection)
        );
        assert_eq!(TakeSelectionSource::Explicit.production(), Ok(()));
    }

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
