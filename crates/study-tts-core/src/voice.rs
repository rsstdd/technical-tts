//! Voice profile and consent records, and the gate that decides whether a
//! profile may serve a particular use.
//!
//! Everything here fails closed per
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load fails
//! closed"): a profile is usable only while its consent is granted, its rights
//! decision is approved, the requested use is inside the recorded scope, and
//! the recorded checksums agree.
//!
//! This module handles only the records that describe a voice. It is IO-free;
//! reading `reference.wav` and `conditionals.pt` and checking them against the
//! recorded digests belongs to the runtime.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::is_blake3_hex;

/// Schema version this module accepts for `profile.json` and `consent.json`.
///
/// A record declaring anything else is refused rather than read on the guess
/// that its fields still mean what this build expects.
const VOICE_SCHEMA_VERSION: &str = "0.1-voice";

/// Consent status recorded for a voice reference.
///
/// Field set per ADR-0001 §15.3: a profile is usable only while consent is
/// `granted`; anything else refuses profile load per
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load fails
/// closed").
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsentStatus {
    /// Consent is recorded and in force.
    Granted,
    /// Consent has been requested but not yet recorded.
    Pending,
    /// Consent was withdrawn; new use is disabled immediately.
    Revoked,
}

impl ConsentStatus {
    /// The `snake_case` spelling this status carries in `consent.json`.
    ///
    /// Mirrors the serde representation above so a refusal message quotes what
    /// the author actually wrote in the record. The exhaustive match makes a
    /// new variant a compile error rather than a silent fallback string, and
    /// `t3_e0_record_state_spellings_match_their_serde_representation` proves
    /// the two agree.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Pending => "pending",
            Self::Revoked => "revoked",
        }
    }
}

/// Decision recorded for a rights record.
///
/// Mirrors the Decision checkboxes of
/// `docs/templates/RIGHTS-RECORD-TEMPLATE.md`. The two must agree, and changing
/// either requires a template amendment rather than an edit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RightsDecision {
    /// Approved for the recorded scope.
    Approved,
    /// Usable only under recorded restrictions.
    Restricted,
    /// A rights review is outstanding.
    ReviewRequired,
    /// The artifact must not be used.
    Prohibited,
}

impl RightsDecision {
    /// The `snake_case` spelling this decision carries in `profile.json`.
    ///
    /// Mirrors the serde representation above, on the same terms as
    /// [`ConsentStatus::as_str`].
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Restricted => "restricted",
            Self::ReviewRequired => "review_required",
            Self::Prohibited => "prohibited",
        }
    }
}

/// The use a caller asks a voice profile to serve, and the vocabulary a consent
/// record's `permitted_use` scope is written in.
///
/// A dedicated value object rather than a bare string, on both sides: scope
/// compared by ad-hoc string equality at each call site is how scope stops
/// being enforced, and a scope *recorded* as a bare string lets an unrecognized
/// use into the record, where it can never match a request and so is silently
/// unenforceable. Deserializing this enum makes an unknown value a parse error
/// at the record boundary instead.
///
/// The vocabulary is closed here rather than in a governance document: ADR-0001
/// §15.3 requires a permitted-use scope without enumerating one, and the
/// human-readable scope lives in the rights record under
/// `evidence/rights/<record-id>/`. Widening the machine-checked scope means
/// adding a variant and the call site that requests it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceUse {
    /// Private lesson or preview rendering.
    PrivateSynthesis,
    /// Model, hardware, or voice qualification runs that never reach a lesson.
    VoiceQualification,
}

impl VoiceUse {
    /// The `snake_case` spelling this use carries in a `permitted_use` entry.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrivateSynthesis => "private_synthesis",
            Self::VoiceQualification => "voice_qualification",
        }
    }
}

/// The consent record a voice profile directory carries as `consent.json`.
///
/// Fields transcribed from ADR-0001 §15.3: ownership or subject-consent
/// declaration, permitted-use scope, reference-audio checksum, creation date,
/// and consent status.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceConsent {
    /// Version of the consent record layout; must be `0.1-voice`.
    pub schema_version: String,
    /// Ownership declaration or documented subject consent for the reference
    /// recording.
    pub declaration: String,
    /// The uses the consent permits; an unrecognized use is a parse error, not
    /// an empty scope.
    pub permitted_use: Vec<VoiceUse>,
    /// BLAKE3 checksum of `reference.wav` as recorded at consent time.
    pub reference_wav_blake3: String,
    /// Date the consent record was created.
    pub created: String,
    /// Whether the consent is granted, pending, or revoked.
    pub consent_status: ConsentStatus,
    /// The rights record under `evidence/rights/<record-id>/` backing this
    /// consent.
    pub rights_record_id: String,
}

/// The identity record a voice profile directory carries as `profile.json`.
///
/// Identity per ADR-0001 §5.2: the conditional hash and extractor identity are
/// the synthesis identity; the reference hash is provenance.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceProfile {
    /// Version of the profile record layout; must be `0.1-voice`.
    pub schema_version: String,
    /// Identifier of the voice profile, e.g. `owner-fallback-v1`.
    pub profile_id: String,
    /// BLAKE3 checksum of `reference.wav` (provenance).
    pub reference_wav_blake3: String,
    /// BLAKE3 checksum of `conditionals.pt` (synthesis identity).
    pub conditionals_blake3: String,
    /// Identity of the extractor that produced the conditionals.
    pub extractor_identity: String,
    /// The rights-record decision recorded for this profile.
    pub approval: RightsDecision,
}

/// Why a voice record was refused.
#[derive(Debug, Error)]
pub enum VoiceError {
    /// The record bytes are not the expected JSON shape.
    #[error("voice record JSON is invalid: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// The record declares a layout version this build does not accept.
    #[error("unsupported voice record schema version `{0}`")]
    UnsupportedSchema(String),
    /// A required record field is empty.
    #[error("voice record field `{0}` must not be empty")]
    MissingField(&'static str),
    /// A recorded checksum is not a BLAKE3 digest, so it could never match a
    /// computed one.
    ///
    /// Caught at parse time so a malformed record is reported as malformed.
    /// Without this, an uppercase or truncated digest reaches the runtime
    /// comparison and is reported as a checksum *mismatch* — telling the owner
    /// their file was tampered with when the record was simply mistyped.
    #[error(
        "voice record field `{field}` is not a lowercase BLAKE3 hex digest: `{value}`; correct \
         the record rather than the artifact"
    )]
    MalformedChecksum {
        /// The field holding the malformed digest.
        field: &'static str,
        /// The value as recorded.
        value: String,
    },
    /// The consent record is not in the `granted` state.
    #[error(
        "voice profile `{profile_id}` is refused: consent status is `{status}`, not `granted`; \
         profile load fails closed and the project owner must resolve the consent record before \
         use"
    )]
    ConsentNotGranted {
        /// The refused profile.
        profile_id: String,
        /// The recorded consent status.
        status: String,
    },
    /// The rights-record decision for the profile is not `approved`.
    #[error(
        "voice profile `{profile_id}` is refused: rights decision is `{decision}`, not \
         `approved`; the profile enters neither preview nor production until the project owner \
         records approval"
    )]
    ProfileNotApproved {
        /// The refused profile.
        profile_id: String,
        /// The recorded rights decision.
        decision: String,
    },
    /// The requested use is outside the scope the consent record permits.
    #[error(
        "voice profile `{profile_id}` is refused: the consent record permits `{permitted}`, not \
         the requested `{requested}`; the project owner must record consent for this use before \
         it proceeds"
    )]
    ConsentScopeExcluded {
        /// The refused profile.
        profile_id: String,
        /// The use that was requested.
        requested: &'static str,
        /// The uses the consent record permits, as recorded.
        permitted: String,
    },
    /// The profile and consent records disagree about the reference-audio
    /// checksum.
    #[error(
        "voice profile `{profile_id}` is refused: profile.json and consent.json record \
         different reference-audio checksums; the project owner must re-verify the profile \
         against its rights record before use"
    )]
    ConsentChecksumDisagreement {
        /// The refused profile.
        profile_id: String,
    },
}

impl VoiceProfile {
    /// Parses and validates a `profile.json` record.
    ///
    /// # Errors
    ///
    /// [`VoiceError::InvalidJson`] when the bytes are not this record's shape,
    /// otherwise whichever variant [`VoiceProfile::validate`] returns.
    pub fn from_json(bytes: &[u8]) -> Result<Self, VoiceError> {
        let profile: Self = serde_json::from_slice(bytes)?;
        profile.validate()?;
        Ok(profile)
    }

    /// Rejects a structurally complete record whose fields are absent or
    /// unsupported.
    ///
    /// # Errors
    ///
    /// [`VoiceError::UnsupportedSchema`] for a version this build cannot read,
    /// [`VoiceError::MissingField`] for a blank identity field, and
    /// [`VoiceError::MalformedChecksum`] for a recorded digest that is not one
    /// — reported as malformed rather than as a mismatch, so the owner is not
    /// told their file was tampered with.
    pub fn validate(&self) -> Result<(), VoiceError> {
        if self.schema_version != VOICE_SCHEMA_VERSION {
            return Err(VoiceError::UnsupportedSchema(self.schema_version.clone()));
        }
        require("profile_id", &self.profile_id)?;
        require("extractor_identity", &self.extractor_identity)?;
        require_blake3_hex("reference_wav_blake3", &self.reference_wav_blake3)?;
        require_blake3_hex("conditionals_blake3", &self.conditionals_blake3)?;
        Ok(())
    }
}

impl VoiceConsent {
    /// Parses and validates a `consent.json` record.
    ///
    /// # Errors
    ///
    /// [`VoiceError::InvalidJson`] when the bytes are not this record's shape,
    /// otherwise whichever variant [`VoiceConsent::validate`] returns.
    pub fn from_json(bytes: &[u8]) -> Result<Self, VoiceError> {
        let consent: Self = serde_json::from_slice(bytes)?;
        consent.validate()?;
        Ok(consent)
    }

    /// Rejects a structurally complete record whose fields are absent or
    /// unsupported.
    ///
    /// # Errors
    ///
    /// [`VoiceError::UnsupportedSchema`], [`VoiceError::MissingField`] — which
    /// an empty `permitted_use` also produces, because a record granting no
    /// scope grants nothing — and [`VoiceError::MalformedChecksum`], on the
    /// same terms as [`VoiceProfile::validate`].
    pub fn validate(&self) -> Result<(), VoiceError> {
        if self.schema_version != VOICE_SCHEMA_VERSION {
            return Err(VoiceError::UnsupportedSchema(self.schema_version.clone()));
        }
        require("declaration", &self.declaration)?;
        require("created", &self.created)?;
        require("rights_record_id", &self.rights_record_id)?;
        require_blake3_hex("reference_wav_blake3", &self.reference_wav_blake3)?;
        if self.permitted_use.is_empty() {
            return Err(VoiceError::MissingField("permitted_use"));
        }
        Ok(())
    }
}

/// Accepts a profile for `requested` use only with granted consent, a recorded
/// approval, a consent scope covering that use, and agreeing reference
/// checksums.
///
/// `requested` is not optional by design: a consent record's `permitted_use`
/// list is scope, and a gate that never receives the use it is gating cannot
/// enforce scope. Every caller states what it is about to do.
///
/// This gate is IO-free; verifying the on-disk `reference.wav` and
/// `conditionals.pt` bytes against the recorded checksums is the runtime's
/// responsibility.
///
/// # Errors
///
/// Structure first, so a malformed record is reported as malformed rather
/// than as a refused permission: whatever [`VoiceProfile::validate`] and
/// [`VoiceConsent::validate`] return. Then authorization, one variant per
/// refusal — [`VoiceError::ConsentNotGranted`],
/// [`VoiceError::ProfileNotApproved`], [`VoiceError::ConsentScopeExcluded`]
/// naming the requested use against the recorded scope, and
/// [`VoiceError::ConsentChecksumDisagreement`] when the two records no longer
/// describe the same reference audio.
pub fn validate_profile_for_use(
    profile: &VoiceProfile,
    consent: &VoiceConsent,
    requested: VoiceUse,
) -> Result<(), VoiceError> {
    // Both records are revalidated here even though `from_json` already did it,
    // because these types have public fields: a caller can build one directly,
    // and a gate that assumes its inputs were parsed does not uphold its own
    // contract. Structure is checked before authorization so a malformed record
    // is reported as malformed rather than as a refused permission, the same
    // distinction `MalformedChecksum` draws.
    profile.validate()?;
    consent.validate()?;

    if consent.consent_status != ConsentStatus::Granted {
        return Err(VoiceError::ConsentNotGranted {
            profile_id: profile.profile_id.clone(),
            status: consent.consent_status.as_str().to_owned(),
        });
    }
    if profile.approval != RightsDecision::Approved {
        return Err(VoiceError::ProfileNotApproved {
            profile_id: profile.profile_id.clone(),
            decision: profile.approval.as_str().to_owned(),
        });
    }
    if !permits(consent, requested) {
        return Err(VoiceError::ConsentScopeExcluded {
            profile_id: profile.profile_id.clone(),
            requested: requested.as_str(),
            permitted: recorded_scope(consent),
        });
    }
    if profile.reference_wav_blake3 != consent.reference_wav_blake3 {
        return Err(VoiceError::ConsentChecksumDisagreement {
            profile_id: profile.profile_id.clone(),
        });
    }
    Ok(())
}

/// Whether the recorded consent scope covers the use being requested.
fn permits(consent: &VoiceConsent, requested: VoiceUse) -> bool {
    consent.permitted_use.contains(&requested)
}

/// The recorded scope as the record spells it, for a refusal the owner can
/// compare to the file.
fn recorded_scope(consent: &VoiceConsent) -> String {
    consent
        .permitted_use
        .iter()
        .map(|permitted| permitted.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Rejects a blank required field, naming it so the owner knows what to fill
/// in.
fn require(field: &'static str, value: &str) -> Result<(), VoiceError> {
    if value.trim().is_empty() {
        return Err(VoiceError::MissingField(field));
    }
    Ok(())
}

/// A recorded digest must be exactly the form `blake3::Hash::to_hex` produces,
/// because that is what the runtime compares it against byte for byte.
fn require_blake3_hex(field: &'static str, value: &str) -> Result<(), VoiceError> {
    require(field, value)?;
    if !is_blake3_hex(value) {
        return Err(VoiceError::MalformedChecksum {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::{Value, json};
    use std::fmt::Debug;

    /// Stand-in digests. Any well-formed value works: this module never hashes
    /// anything, and the runtime is what compares a recorded digest to a
    /// computed one. Both carry hex letters, so `to_uppercase` below actually
    /// changes them.
    const REFERENCE_DIGEST: &str =
        "afafafafafafafafafafafafafafafafafafafafafafafafafafafafafafafaf";
    const CONDITIONALS_DIGEST: &str =
        "bdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbdbd";

    fn profile_value() -> Value {
        json!({
            "schema_version": "0.1-voice",
            "profile_id": "synthetic-test-voice-v1",
            "reference_wav_blake3": REFERENCE_DIGEST,
            "conditionals_blake3": CONDITIONALS_DIGEST,
            "extractor_identity": "test-extractor-v1",
            "approval": "approved",
        })
    }

    fn consent_value() -> Value {
        json!({
            "schema_version": "0.1-voice",
            "declaration": "Owner-recorded reference with a permitted-use declaration.",
            "permitted_use": ["private_synthesis"],
            "reference_wav_blake3": REFERENCE_DIGEST,
            "created": "2026-08-23",
            "consent_status": "granted",
            "rights_record_id": "rights-voice-owner-fallback-v1",
        })
    }

    fn parse_profile(value: &Value) -> Result<VoiceProfile, VoiceError> {
        VoiceProfile::from_json(&serde_json::to_vec(value).expect("profile should serialize"))
    }

    fn parse_consent(value: &Value) -> Result<VoiceConsent, VoiceError> {
        VoiceConsent::from_json(&serde_json::to_vec(value).expect("consent should serialize"))
    }

    #[test]
    fn t1_e0_valid_voice_records_parse_and_pass_the_use_gate() {
        let profile = parse_profile(&profile_value()).expect("valid profile must parse");
        let consent = parse_consent(&consent_value()).expect("valid consent must parse");
        validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
            .expect("granted, approved, in-scope profile is usable");
    }

    /// One way to make a record structurally invalid: a label, and the edit
    /// that breaks the pair.
    type Mutation = (&'static str, fn(&mut VoiceProfile, &mut VoiceConsent));

    /// Records built field by field rather than parsed, which is what a caller
    /// with access to these public fields can do.
    fn constructed_records() -> (VoiceProfile, VoiceConsent) {
        (
            VoiceProfile {
                schema_version: VOICE_SCHEMA_VERSION.to_owned(),
                profile_id: "synthetic-test-voice-v1".to_owned(),
                reference_wav_blake3: REFERENCE_DIGEST.to_owned(),
                conditionals_blake3: CONDITIONALS_DIGEST.to_owned(),
                extractor_identity: "test-extractor-v1".to_owned(),
                approval: RightsDecision::Approved,
            },
            VoiceConsent {
                schema_version: VOICE_SCHEMA_VERSION.to_owned(),
                declaration: "Owner-recorded reference.".to_owned(),
                permitted_use: vec![VoiceUse::PrivateSynthesis],
                reference_wav_blake3: REFERENCE_DIGEST.to_owned(),
                created: "2026-08-23".to_owned(),
                consent_status: ConsentStatus::Granted,
                rights_record_id: "rights-voice-owner-fallback-v1".to_owned(),
            },
        )
    }

    #[test]
    fn t1_e0_structurally_invalid_records_are_refused_by_the_use_gate() {
        // `from_json` validates, so every earlier test reaches the gate with a
        // well-formed record. These fields are public, so a caller can skip
        // that path entirely — and a gate that trusts its inputs were parsed
        // would authorize an unsupported schema, an empty identifier, or two
        // malformed digests that happen to agree with each other.
        let mutations: [Mutation; 6] = [
            ("unsupported profile schema", |profile, _| {
                profile.schema_version = "9.9-future".to_owned();
            }),
            ("unsupported consent schema", |_, consent| {
                consent.schema_version = "9.9-future".to_owned();
            }),
            ("empty profile identifier", |profile, _| {
                profile.profile_id = String::new();
            }),
            ("empty rights record identifier", |_, consent| {
                consent.rights_record_id = "   ".to_owned();
            }),
            ("empty permitted-use scope", |_, consent| {
                consent.permitted_use.clear();
            }),
            // The trap the agreement check alone cannot catch: two digests that
            // are equal, so they agree, and malformed, so neither could ever
            // match the file it claims to describe.
            ("agreeing malformed digests", |profile, consent| {
                profile.reference_wav_blake3 = "not-a-digest".to_owned();
                consent.reference_wav_blake3 = "not-a-digest".to_owned();
            }),
        ];

        for (label, mutate) in mutations {
            let (mut profile, mut consent) = constructed_records();
            mutate(&mut profile, &mut consent);

            let error = validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
                .expect_err("a structurally invalid record must not be authorized");

            assert!(
                matches!(
                    error,
                    VoiceError::UnsupportedSchema(_)
                        | VoiceError::MissingField(_)
                        | VoiceError::MalformedChecksum { .. }
                ),
                "{label} was refused as `{error}` rather than as a structural fault"
            );
        }

        // The unmutated pair still passes, so the cases above fail for the
        // reason under test rather than because the fixture is unusable.
        let (profile, consent) = constructed_records();
        validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
            .expect("a well-formed constructed record is still authorized");
    }

    #[test]
    fn t1_e0_non_granted_consent_statuses_are_refused() {
        let profile = parse_profile(&profile_value()).expect("valid profile must parse");
        for status in ["pending", "revoked"] {
            let mut value = consent_value();
            value["consent_status"] = Value::String(status.to_owned());
            let consent = parse_consent(&value).expect("record with known status must parse");

            let error = validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
                .expect_err("non-granted consent must be refused");
            assert!(
                matches!(
                    error,
                    VoiceError::ConsentNotGranted { status: ref reported, .. }
                        if reported == status
                ),
                "consent status `{status}` produced `{error}`"
            );
        }
    }

    #[test]
    fn t1_e0_non_approved_rights_decisions_are_refused() {
        let consent = parse_consent(&consent_value()).expect("valid consent must parse");
        for decision in ["restricted", "review_required", "prohibited"] {
            let mut value = profile_value();
            value["approval"] = Value::String(decision.to_owned());
            let profile = parse_profile(&value).expect("record with known decision must parse");

            let error = validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
                .expect_err("non-approved profile must be refused");
            assert!(
                matches!(
                    error,
                    VoiceError::ProfileNotApproved { decision: ref reported, .. }
                        if reported == decision
                ),
                "rights decision `{decision}` produced `{error}`"
            );
        }
    }

    #[test]
    fn t1_e0_disagreeing_reference_checksums_are_refused() {
        let profile = parse_profile(&profile_value()).expect("valid profile must parse");
        let mut value = consent_value();
        // Well-formed, so it reaches the agreement check rather than the format
        // check.
        value["reference_wav_blake3"] = Value::String(CONDITIONALS_DIGEST.to_owned());
        let consent = parse_consent(&value).expect("valid consent must parse");

        assert!(matches!(
            validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis),
            Err(VoiceError::ConsentChecksumDisagreement { .. })
        ));
    }

    #[test]
    fn t1_e0_uses_outside_the_recorded_consent_scope_are_refused() {
        let profile = parse_profile(&profile_value()).expect("valid profile must parse");
        let mut value = consent_value();
        value["permitted_use"] = json!(["voice_qualification"]);
        let consent = parse_consent(&value).expect("valid consent must parse");

        let error = validate_profile_for_use(&profile, &consent, VoiceUse::PrivateSynthesis)
            .expect_err("a use outside the recorded scope must be refused");
        assert!(
            matches!(
                error,
                VoiceError::ConsentScopeExcluded { requested, ref permitted, .. }
                    if requested == "private_synthesis" && permitted == "voice_qualification"
            ),
            "out-of-scope use produced `{error}`"
        );

        // The same record permits the use it was actually recorded for.
        validate_profile_for_use(&profile, &consent, VoiceUse::VoiceQualification)
            .expect("the recorded scope must remain usable");
    }

    #[test]
    fn t1_e0_malformed_recorded_checksums_are_reported_as_malformed() {
        // Uppercase is the trap this guards: `blake3::Hash::to_hex` is
        // lowercase, so an uppercase digest would otherwise reach the runtime
        // and be reported as tampering.
        for malformed in [
            &REFERENCE_DIGEST.to_uppercase(),
            "1111",
            "11111111111111111111111111111111111111111111111111111111111111zz",
        ] {
            let mut value = profile_value();
            value["reference_wav_blake3"] = Value::String((*malformed).to_owned());
            assert!(
                matches!(
                    parse_profile(&value),
                    Err(VoiceError::MalformedChecksum { field, .. })
                        if field == "reference_wav_blake3"
                ),
                "`{malformed}` must be reported as a malformed checksum"
            );
        }
    }

    /// The spelling `consent.json` must carry for each consent status.
    ///
    /// Literal and independent of [`ConsentStatus::as_str`] on purpose: a table
    /// that asked the implementation what it spells would agree with any
    /// spelling, including a wrong one. The match is exhaustive, so a new
    /// variant is a compile error here rather than a variant no test covers —
    /// and the compiler lands the author beside the list the test iterates.
    fn expected_consent_status_spelling(status: ConsentStatus) -> &'static str {
        match status {
            ConsentStatus::Granted => "granted",
            ConsentStatus::Pending => "pending",
            ConsentStatus::Revoked => "revoked",
        }
    }

    /// The spelling `profile.json` must carry for each rights decision, on the
    /// same terms.
    fn expected_rights_decision_spelling(decision: RightsDecision) -> &'static str {
        match decision {
            RightsDecision::Approved => "approved",
            RightsDecision::Restricted => "restricted",
            RightsDecision::ReviewRequired => "review_required",
            RightsDecision::Prohibited => "prohibited",
        }
    }

    /// The spelling a `permitted_use` entry must carry for each use, on the
    /// same terms.
    fn expected_voice_use_spelling(requested: VoiceUse) -> &'static str {
        match requested {
            VoiceUse::PrivateSynthesis => "private_synthesis",
            VoiceUse::VoiceQualification => "voice_qualification",
        }
    }

    /// Asserts a record value is written as `expected` and read back from it.
    ///
    /// Both directions matter: writing the right string while accepting a
    /// different one would let a record spell a state one way and be gated as
    /// another.
    fn assert_serde_spelling<T>(value: T, expected: &'static str)
    where
        T: Copy + Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let recorded = Value::String(expected.to_owned());
        assert_eq!(
            serde_json::to_value(value).expect("unit variant serializes"),
            recorded,
            "`{value:?}` is not written as `{expected}`"
        );
        assert_eq!(
            serde_json::from_value::<T>(recorded).expect("the recorded spelling parses"),
            value,
            "`{expected}` does not parse back to `{value:?}`"
        );
    }

    #[test]
    fn t3_e0_record_state_spellings_match_their_serde_representation() {
        for status in [
            ConsentStatus::Granted,
            ConsentStatus::Pending,
            ConsentStatus::Revoked,
        ] {
            let expected = expected_consent_status_spelling(status);
            assert_serde_spelling(status, expected);
            assert_eq!(
                status.as_str(),
                expected,
                "`{status:?}` spelling drifted from its serde representation"
            );
        }
        for decision in [
            RightsDecision::Approved,
            RightsDecision::Restricted,
            RightsDecision::ReviewRequired,
            RightsDecision::Prohibited,
        ] {
            let expected = expected_rights_decision_spelling(decision);
            assert_serde_spelling(decision, expected);
            assert_eq!(
                decision.as_str(),
                expected,
                "`{decision:?}` spelling drifted from its serde representation"
            );
        }
        for requested in [VoiceUse::PrivateSynthesis, VoiceUse::VoiceQualification] {
            let expected = expected_voice_use_spelling(requested);
            assert_serde_spelling(requested, expected);
            assert_eq!(
                requested.as_str(),
                expected,
                "`{requested:?}` spelling drifted from its serde representation"
            );
        }
    }

    #[test]
    fn t3_e0_unknown_permitted_use_values_are_rejected() {
        // A recorded scope outside the vocabulary can never match a request, so
        // as a bare string it would sit in the record unenforced. The last case
        // is the one that matters: a record must not be able to widen its own
        // scope with a use this build cannot gate.
        for unknown in [
            json!(["commercial_distribution"]),
            json!(["PrivateSynthesis"]),
            json!(["private-synthesis"]),
            json!(["private_synthesis "]),
            json!([""]),
            json!([null]),
            json!(["private_synthesis", "commercial_distribution"]),
        ] {
            let mut value = consent_value();
            value["permitted_use"] = unknown.clone();
            assert!(
                matches!(parse_consent(&value), Err(VoiceError::InvalidJson(_))),
                "permitted_use `{unknown}` must be rejected at parse time"
            );
        }
    }

    #[test]
    fn t3_e0_unknown_consent_status_and_rights_decision_are_rejected() {
        for unknown in [
            "\"Granted\"",
            "\"granted \"",
            "\"approved\"",
            "\"withdrawn\"",
            "\"\"",
            "null",
        ] {
            assert!(
                serde_json::from_str::<ConsentStatus>(unknown).is_err(),
                "consent status `{unknown}` must be rejected"
            );
        }
        for unknown in ["\"Approved\"", "\"review-required\"", "\"granted\"", "null"] {
            assert!(
                serde_json::from_str::<RightsDecision>(unknown).is_err(),
                "rights decision `{unknown}` must be rejected"
            );
        }
    }

    #[test]
    fn t1_e0_absent_record_fields_have_distinct_missing_errors() {
        for field in [
            "profile_id",
            "reference_wav_blake3",
            "conditionals_blake3",
            "extractor_identity",
        ] {
            let mut value = profile_value();
            value[field] = Value::String("  ".to_owned());
            assert!(
                matches!(
                    parse_profile(&value),
                    Err(VoiceError::MissingField(name)) if name == field
                ),
                "profile field `{field}` must be reported as missing"
            );
        }

        for field in [
            "declaration",
            "reference_wav_blake3",
            "created",
            "rights_record_id",
        ] {
            let mut value = consent_value();
            value[field] = Value::String(String::new());
            assert!(
                matches!(
                    parse_consent(&value),
                    Err(VoiceError::MissingField(name)) if name == field
                ),
                "consent field `{field}` must be reported as missing"
            );
        }

        let mut value = consent_value();
        value["permitted_use"] = Value::Array(Vec::new());
        assert!(matches!(
            parse_consent(&value),
            Err(VoiceError::MissingField("permitted_use"))
        ));

        let mut value = consent_value();
        value["schema_version"] = Value::String("0.2-voice".to_owned());
        assert!(matches!(
            parse_consent(&value),
            Err(VoiceError::UnsupportedSchema(version)) if version == "0.2-voice"
        ));
    }
}
