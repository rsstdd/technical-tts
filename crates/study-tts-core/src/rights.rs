//! Rights classification of source material, and what each classification
//! permits.
//!
//! Transcribed from `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
//! §Classification. Every permission question is answered by exhaustive match
//! rather than by a default, so a classification added later cannot inherit
//! permission it was never granted.
//!
//! The product records classification and scope. It does not encode a
//! universal legal conclusion about any third-party material.

use serde::{Deserialize, Serialize};

/// Classification a nontrivial input receives before use.
///
/// Transcribed from `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
/// §Classification. The two must agree, and changing either requires a policy
/// amendment rather than an edit. The product records classification and scope;
/// it does not encode a universal legal conclusion about any third-party
/// material.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceClassification {
    /// Written or recorded by the project owner.
    OwnerAuthored,
    /// Material in the public domain.
    PublicDomain,
    /// Material under a permissive license.
    PermissivelyLicensed,
    /// Material under a commercial or private license.
    CommerciallyOrPrivatelyLicensed,
    /// A voice reference backed by a consent record.
    ConsentedVoiceReference,
    /// Material usable for evaluation only, never in a release.
    EvaluationOnly,
    /// Material whose rights have not yet been reviewed.
    RightsReviewRequired,
    /// Material that must not be used.
    Prohibited,
}

impl SourceClassification {
    /// The `snake_case` spelling this classification carries in a manifest.
    ///
    /// Mirrors the serde representation above so a refusal quotes what the
    /// author actually wrote. The exhaustive match makes a new variant a
    /// compile error rather than a silent fallback string, and
    /// `t3_e0_classification_spellings_match_their_serde_representation` proves
    /// the two agree.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OwnerAuthored => "owner_authored",
            Self::PublicDomain => "public_domain",
            Self::PermissivelyLicensed => "permissively_licensed",
            Self::CommerciallyOrPrivatelyLicensed => "commercially_or_privately_licensed",
            Self::ConsentedVoiceReference => "consented_voice_reference",
            Self::EvaluationOnly => "evaluation_only",
            Self::RightsReviewRequired => "rights_review_required",
            Self::Prohibited => "prohibited",
        }
    }

    /// Whether this classification permits use inside a production release.
    ///
    /// Exhaustive rather than a negative `matches!`, because this gates a real
    /// refusal in `study_tts_runtime::pipeline`: a ninth classification must
    /// be a compile error here rather than a variant that inherits release
    /// permission by falling through a default.
    ///
    /// The three refusals are rows a document states.
    /// `docs/governance/ROUTING-TABLES.md` §Failure routing blocks external
    /// publication for a missing rights classification, which is
    /// [`Self::RightsReviewRequired`]; `evaluation-only` and `prohibited` are
    /// refusals carried by their own names in
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification.
    pub fn permits_production_release(self) -> bool {
        match self {
            Self::OwnerAuthored
            | Self::PublicDomain
            | Self::PermissivelyLicensed
            | Self::CommerciallyOrPrivatelyLicensed
            | Self::ConsentedVoiceReference => true,
            Self::EvaluationOnly | Self::RightsReviewRequired | Self::Prohibited => false,
        }
    }

    /// Whether this classification permits private preview rendering.
    ///
    /// Exhaustive for the reason [`Self::permits_production_release`] is. Only
    /// [`Self::Prohibited`] is excluded: `docs/governance/ROUTING-TABLES.md`
    /// §Failure routing restricts an unresolved classification to the
    /// permitted private scope rather than blocking it, and
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification keeps
    /// private use and external distribution as separate permissions.
    ///
    /// Deliberately unenforced at E0-S2: `BuildRequest` carries no source
    /// classification, so nothing in the preview path can consult this yet. It
    /// states the policy row now so the release gate and the preview gate
    /// cannot drift apart later; the preview-scope story wires it in. Do not
    /// read the preview path as classification-gated today.
    pub fn permits_private_preview(self) -> bool {
        match self {
            Self::OwnerAuthored
            | Self::PublicDomain
            | Self::PermissivelyLicensed
            | Self::CommerciallyOrPrivatelyLicensed
            | Self::ConsentedVoiceReference
            | Self::EvaluationOnly
            | Self::RightsReviewRequired => true,
            Self::Prohibited => false,
        }
    }
}

/// One classified source named by a production manifest.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "snake_case")]
pub struct SourceRightsDeclaration {
    /// Identifier of the classified source.
    pub source_id: String,
    /// The classification recorded for the source.
    pub classification: SourceClassification,
    /// The rights record under `evidence/rights/<record-id>/` backing the
    /// classification.
    pub rights_record_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every classification, so a spelling cannot go untested. The exhaustive
    /// matches inside the tests are what make a new variant a compile error;
    /// this array only says which values to run them over.
    const ALL_CLASSIFICATIONS: [SourceClassification; 8] = [
        SourceClassification::OwnerAuthored,
        SourceClassification::PublicDomain,
        SourceClassification::PermissivelyLicensed,
        SourceClassification::CommerciallyOrPrivatelyLicensed,
        SourceClassification::ConsentedVoiceReference,
        SourceClassification::EvaluationOnly,
        SourceClassification::RightsReviewRequired,
        SourceClassification::Prohibited,
    ];

    #[test]
    fn t3_e0_classification_vocabulary_round_trips_and_unknown_values_are_rejected() {
        for (json, expected) in [
            ("\"owner_authored\"", SourceClassification::OwnerAuthored),
            ("\"public_domain\"", SourceClassification::PublicDomain),
            (
                "\"permissively_licensed\"",
                SourceClassification::PermissivelyLicensed,
            ),
            (
                "\"commercially_or_privately_licensed\"",
                SourceClassification::CommerciallyOrPrivatelyLicensed,
            ),
            (
                "\"consented_voice_reference\"",
                SourceClassification::ConsentedVoiceReference,
            ),
            ("\"evaluation_only\"", SourceClassification::EvaluationOnly),
            (
                "\"rights_review_required\"",
                SourceClassification::RightsReviewRequired,
            ),
            ("\"prohibited\"", SourceClassification::Prohibited),
        ] {
            assert_eq!(
                serde_json::from_str::<SourceClassification>(json)
                    .expect("known classification must parse"),
                expected
            );
        }

        for unknown in [
            "\"licensed\"",
            "\"owner-authored\"",
            "\"OwnerAuthored\"",
            "\"public domain\"",
            "\"unclassified\"",
            "\"\"",
            "null",
            "0",
        ] {
            assert!(
                serde_json::from_str::<SourceClassification>(unknown).is_err(),
                "`{unknown}` must be rejected"
            );
        }
    }

    #[test]
    fn t3_e0_classification_spellings_match_their_serde_representation() {
        for classification in ALL_CLASSIFICATIONS {
            // Transcribed from `RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification
            // rather than read from `as_str`: a table that asked the
            // implementation what it spells would agree with any spelling,
            // including a wrong one. Asserting the serde form against `as_str`
            // alone only proves the two mechanisms move together, which they
            // would do through a rename of both.
            let spelling = match classification {
                SourceClassification::OwnerAuthored => "owner_authored",
                SourceClassification::PublicDomain => "public_domain",
                SourceClassification::PermissivelyLicensed => "permissively_licensed",
                SourceClassification::CommerciallyOrPrivatelyLicensed => {
                    "commercially_or_privately_licensed"
                }
                SourceClassification::ConsentedVoiceReference => "consented_voice_reference",
                SourceClassification::EvaluationOnly => "evaluation_only",
                SourceClassification::RightsReviewRequired => "rights_review_required",
                SourceClassification::Prohibited => "prohibited",
            };

            assert_eq!(
                serde_json::to_value(classification).expect("unit variant serializes"),
                serde_json::Value::String(spelling.to_owned()),
                "`{classification:?}` serde representation drifted"
            );
            assert_eq!(
                classification.as_str(),
                spelling,
                "`{classification:?}` spelling drifted from its serde representation"
            );
        }
    }

    #[test]
    fn t3_e0_classification_permissions_match_the_recorded_policy() {
        for classification in ALL_CLASSIFICATIONS {
            // A table rather than a re-derivation of the implementation:
            // repeating its own match here would pass for any policy,
            // including a wrong one. The exhaustive match also makes a ninth
            // variant a compile error in this test rather than an untested
            // one.
            //
            // No document enumerates the whole table, so each row is read off
            // what does state it. `ROUTING-TABLES.md` §Failure routing blocks
            // external publication for a missing rights classification while
            // restricting it to the permitted private scope, which is the
            // `RightsReviewRequired` row. `RIGHTS-DATA-ARTIFACT-POLICY.md`
            // §Classification names `evaluation-only` and `prohibited` and
            // keeps private use separate from external distribution, and its
            // §Required records blocks publish on unresolved external
            // distribution alone, which is the five permitted rows. Ratifying
            // the table itself takes a policy amendment.
            let (releasable, previewable) = match classification {
                SourceClassification::OwnerAuthored => (true, true),
                SourceClassification::PublicDomain => (true, true),
                SourceClassification::PermissivelyLicensed => (true, true),
                SourceClassification::CommerciallyOrPrivatelyLicensed => (true, true),
                SourceClassification::ConsentedVoiceReference => (true, true),
                SourceClassification::EvaluationOnly => (false, true),
                SourceClassification::RightsReviewRequired => (false, true),
                SourceClassification::Prohibited => (false, false),
            };

            assert_eq!(
                classification.permits_production_release(),
                releasable,
                "`{classification:?}` release permission must match the policy"
            );
            assert_eq!(
                classification.permits_private_preview(),
                previewable,
                "`{classification:?}` preview permission must match the policy"
            );
        }
    }
}
