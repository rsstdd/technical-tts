use serde::{Deserialize, Serialize};

/// Classification a nontrivial input receives before use.
///
/// Transcribed from `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification. The two
/// must agree, and changing either requires a policy amendment rather than an edit. The product
/// records classification and scope; it does not encode a universal legal conclusion about any
/// third-party material.
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
    /// Mirrors the serde representation above so a refusal quotes what the author actually
    /// wrote. The exhaustive match makes a new variant a compile error rather than a silent
    /// fallback string, and `t3_e0_classification_spellings_match_their_serde_representation`
    /// proves the two agree.
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
    /// `EvaluationOnly`, `RightsReviewRequired`, and `Prohibited` do not: an unresolved or
    /// restricted classification blocks publish per `docs/governance/ROUTING-TABLES.md`
    /// ("Missing rights classification → External publication blocked").
    pub fn permits_production_release(self) -> bool {
        !matches!(
            self,
            Self::EvaluationOnly | Self::RightsReviewRequired | Self::Prohibited
        )
    }

    /// Whether this classification permits private preview rendering.
    ///
    /// Only `Prohibited` is excluded. An unresolved classification restricts use to the
    /// permitted private scope per `docs/governance/ROUTING-TABLES.md`; it does not block
    /// private preview.
    ///
    /// Deliberately unenforced at E0-S2: `BuildRequest` carries no source classification, so
    /// nothing in the preview path can consult this yet. It states the policy row now so the
    /// release gate and the preview gate cannot drift apart later; the preview-scope story
    /// wires it in. Do not read the preview path as classification-gated today.
    pub fn permits_private_preview(self) -> bool {
        !matches!(self, Self::Prohibited)
    }
}

/// One classified source named by a production manifest.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRightsDeclaration {
    /// Identifier of the classified source.
    pub source_id: String,
    /// The classification recorded for the source.
    pub classification: SourceClassification,
    /// The rights record under `evidence/rights/<record-id>/` backing the classification.
    pub rights_record_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(
                serde_json::to_value(classification).expect("unit variant serializes"),
                serde_json::Value::String(classification.as_str().to_owned()),
                "`{classification:?}` spelling drifted from its serde representation"
            );
        }
    }

    #[test]
    fn t3_e0_only_resolved_classifications_permit_production_release() {
        for classification in ALL_CLASSIFICATIONS {
            // Expected values are a table read off `RIGHTS-DATA-ARTIFACT-POLICY.md`
            // §Classification, not a re-derivation of the implementation: repeating the
            // implementation's own `matches!` here would pass for any policy, including a wrong
            // one. The exhaustive match also makes a ninth variant a compile error in this test
            // rather than an untested one.
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
