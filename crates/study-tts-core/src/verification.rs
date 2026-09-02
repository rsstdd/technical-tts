//! The identity of an ASR verification result, kept separate from synthesis.
//!
//! ADR-0001 §12.5 gives ASR evidence "a separate verification key" and states
//! the consequence plainly: "Changing any verification input reruns
//! verification without regenerating speech or invoking Chatterbox." That
//! separation is the whole reason this module exists rather than more fields on
//! [`crate::SynthesisContext`] — a decoder tweak or a threshold change must
//! cost one re-transcription, not a re-render of the lesson.
//!
//! The inverse also holds: no verification input may reach a synthesis key, or
//! re-tuning ASR would silently invalidate cached audio. It holds by
//! construction — a [`crate::SynthesisContext`] cannot see anything defined
//! here — and no runtime assertion can observe that, so what guards it is a
//! compile-time gate: the test named for that property destructures both
//! context types without `..`, so adding a field to either stops compiling and
//! puts the disjointness question in front of a human.
//!
//! **No ASR runs yet.** `whisper-rs` arrives with E4, so every stack, decoder,
//! and threshold value below is supplied by the caller as a recorded identity
//! string. This module defines and hashes the contract; E4-S1 supplies real
//! values to it. Nothing here pretends to transcribe anything.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::{blake3_newtype, json_schema_as_string};
use crate::{CanonicalValue, canonical_digest};

/// Version of the verification-key definition itself.
///
/// The lever that reruns every verification when the input list below changes,
/// exactly as [`crate::SYNTHESIS_IDENTITY_VERSION`] is for synthesis. The two
/// move independently; that independence is the contract.
pub const VERIFICATION_IDENTITY_VERSION: &str = "e1-s1-v1";

/// BLAKE3 digest of a cached audio artifact.
///
/// A value object for the reason [`crate::CacheKey`] is one: it is compared
/// against a checksum recomputed from a file, and a digest typed as a string is
/// one a caller can set to anything. Parsing at the boundary is also what lets
/// a tampered record be reported as *malformed* rather than as a mismatch,
/// which is a different message to a different person.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct AudioDigest(String);

impl AudioDigest {
    /// The digest as it is written into a manifest and a takes file.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(AudioDigest, MalformedAudioDigest);

/// Remedy routing: a checksum is recomputed from the artifact it describes, so
/// the message names reverification rather than editing the recorded value. The
/// cached audio is never deleted on this path:
/// `docs/governance/ROUTING-TABLES.md` makes an accepted artifact a prune root.
#[derive(Debug, Error)]
#[error(
    "audio digest `{0}` is not a BLAKE3 digest in lowercase hexadecimal; recompute it from the \
     cached artifact rather than editing the recorded value, and preserve the artifact itself"
)]
pub struct MalformedAudioDigest(String);

json_schema_as_string!(
    AudioDigest,
    "AudioDigest",
    "BLAKE3 over a cached audio artifact's bytes, as 64 lowercase \
     hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// BLAKE3 digest of one approved profile a verification was scored against.
///
/// One type for the expected-pattern profile, the comparison normalizer, and
/// the threshold profile because the three share a remedy exactly: each is a
/// governed record whose digest is recomputed from that record, and the field
/// name in [`VerificationContext`] says which record. Three types would be
/// three copies of one refusal.
///
/// A value object rather than a `String` for a reason this boundary makes
/// sharper than most: these digests are hashed into
/// [`VerificationContext::key_for`], so a malformed one produced a perfectly
/// well-formed [`VerificationKey`]. The record then passed
/// [`VerificationIdentityRecord::validate`] — its key really is what its inputs
/// derive — while naming a profile nothing could have been scored against.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct VerificationProfileHash(String);

impl VerificationProfileHash {
    /// The digest as it is written into a verification record.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(VerificationProfileHash, MalformedVerificationProfileHash);

/// Remedy routing: the profile is a governed record, so the message names
/// recomputing the digest from that record and never editing the recorded
/// value or removing the profile.
#[derive(Debug, Error)]
#[error(
    "verification profile hash `{0}` is not a BLAKE3 digest in lowercase hexadecimal; recompute \
     it from the approved profile record it names rather than editing the recorded value, and \
     refer the profile to the verification owner rather than removing it"
)]
pub struct MalformedVerificationProfileHash(String);

json_schema_as_string!(
    VerificationProfileHash,
    "VerificationProfileHash",
    "BLAKE3 over an approved profile a verification result was scored \
     against, as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

/// The pinned ASR stack a verification result was produced by.
///
/// ADR-0001 §17 requires the whole native stack to be pinned, not just the
/// model: `whisper-rs` binds `whisper.cpp`, whose build features change decoder
/// behavior, so a result produced under different features is a different
/// result even for identical audio.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AsrStackIdentity {
    /// `whisper-rs` crate version.
    pub whisper_rs_version: String,
    /// `whisper-rs-sys` crate version.
    pub whisper_rs_sys_version: String,
    /// Revision of the bound `whisper.cpp` source.
    pub whisper_cpp_revision: String,
    /// Identity of the ASR model weights.
    pub model_identity: String,
    /// Compilation features the native stack was built with.
    ///
    /// A set, so the identity does not depend on the order a build system
    /// happened to list them in.
    pub compilation_features: BTreeSet<String>,
    /// Device the decoder executed on.
    pub execution_device: String,
}

/// How audio was converted before it reached the decoder.
///
/// ADR-0001 §12.5 names "FFmpeg version and effective arguments" specifically:
/// resampling to the decoder's rate is lossy, so a conversion change can move a
/// transcript without anything else having changed.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct AsrConversionIdentity {
    /// FFmpeg version string that performed the conversion.
    pub ffmpeg_version: String,
    /// The effective argument vector, in the order it was passed.
    ///
    /// Ordered rather than a set: argument order changes FFmpeg's behavior.
    pub arguments: Vec<String>,
}

/// Every verification-affecting input that is not the audio itself.
///
/// Deliberately holds nothing that appears in [`crate::SynthesisContext`].
/// `t2_e1_a_verification_input_never_changes_the_synthesis_key` is what keeps
/// that true as both types grow.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct VerificationContext {
    /// The pinned ASR stack.
    pub stack: AsrStackIdentity,
    /// The audio conversion applied before decoding.
    pub conversion: AsrConversionIdentity,
    /// Every decoder parameter, by name, in its exact configured spelling.
    ///
    /// Spellings rather than parsed numbers, for the reason given on
    /// [`crate::SynthesisContext::generation_parameters`]: these are floating
    /// point, and over-invalidating is the safe direction.
    pub decoder_parameters: BTreeMap<String, String>,
    /// Thread count the decoder ran with, which can change its output.
    pub thread_count: u16,
    /// Hash of the approved expected-ASR pattern profile.
    pub expected_pattern_profile_hash: VerificationProfileHash,
    /// Hash of the comparison normalizer applied before scoring.
    pub comparison_normalizer_hash: VerificationProfileHash,
    /// Hash of the threshold profile a finding is scored against.
    pub threshold_profile_hash: VerificationProfileHash,
}

/// The audio and text one verification result is about.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct VerificationSubject {
    /// Checksum of the cached audio being verified.
    pub audio_blake3: AudioDigest,
    /// The exact text that audio is supposed to speak.
    pub spoken_text: String,
}

/// The identity of one ASR verification result.
///
/// Produced only by [`VerificationContext::key_for`], so a key cannot be
/// *derived* without the inputs that define it having been hashed. It is also
/// parseable, because [`VerificationIdentityRecord`] records it on disk and
/// must be read back — and a recorded key that is not a digest at all has to be
/// reported as malformed rather than as a mismatch, which is a different
/// message to a different person. Reading one back proves nothing on its own;
/// [`VerificationIdentityRecord::validate`] re-derives it from the record's own
/// inputs and refuses a record whose key its inputs do not produce.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct VerificationKey(String);

impl VerificationKey {
    /// The key as it is written into job state and a verification record.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(VerificationKey, MalformedVerificationKey);

/// Remedy routing: a verification key is derived from the record's own inputs,
/// so the message names re-deriving it rather than editing the recorded value.
#[derive(Debug, Error)]
#[error(
    "verification key `{0}` is not a BLAKE3 digest in lowercase hexadecimal; ADR-0001 §12.5 \
     derives it from the recorded verification inputs; re-derive it from those inputs rather \
     than editing the recorded value"
)]
pub struct MalformedVerificationKey(String);

json_schema_as_string!(
    VerificationKey,
    "VerificationKey",
    "BLAKE3 over the canonical serialization of every verification-affecting \
     input (ADR-0001 12.5), as 64 lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);

impl VerificationContext {
    /// Derives the verification identity for one cached segment.
    ///
    /// Every field named below is an ADR-0001 §12.5 verification-key input, and
    /// none of them is a synthesis-key input. Hashing goes through
    /// [`crate::canonical_digest`], so the result is stable across rebuilds.
    pub fn key_for(&self, subject: &VerificationSubject) -> VerificationKey {
        let compilation_features = CanonicalValue::array(
            self.stack
                .compilation_features
                .iter()
                .map(|feature| CanonicalValue::from(feature.clone())),
        );
        let conversion_arguments = CanonicalValue::array(
            self.conversion
                .arguments
                .iter()
                .map(|argument| CanonicalValue::from(argument.clone())),
        );
        let decoder_parameters = CanonicalValue::Object(
            self.decoder_parameters
                .iter()
                .map(|(name, value)| (name.clone(), CanonicalValue::from(value.clone())))
                .collect(),
        );

        canonical_digest(&CanonicalValue::object([
            ("identity_version", VERIFICATION_IDENTITY_VERSION.into()),
            ("audio_blake3", subject.audio_blake3.as_str().into()),
            ("spoken_text", subject.spoken_text.as_str().into()),
            (
                "whisper_rs_version",
                self.stack.whisper_rs_version.as_str().into(),
            ),
            (
                "whisper_rs_sys_version",
                self.stack.whisper_rs_sys_version.as_str().into(),
            ),
            (
                "whisper_cpp_revision",
                self.stack.whisper_cpp_revision.as_str().into(),
            ),
            (
                "asr_model_identity",
                self.stack.model_identity.as_str().into(),
            ),
            ("compilation_features", compilation_features),
            (
                "execution_device",
                self.stack.execution_device.as_str().into(),
            ),
            ("decoder_parameters", decoder_parameters),
            ("thread_count", self.thread_count.into()),
            (
                "conversion_ffmpeg_version",
                self.conversion.ffmpeg_version.as_str().into(),
            ),
            ("conversion_arguments", conversion_arguments),
            (
                "expected_pattern_profile_hash",
                self.expected_pattern_profile_hash.as_str().into(),
            ),
            (
                "comparison_normalizer_hash",
                self.comparison_normalizer_hash.as_str().into(),
            ),
            (
                "threshold_profile_hash",
                self.threshold_profile_hash.as_str().into(),
            ),
        ]))
        .into()
    }
}

/// Layout version this build publishes for a verification identity record.
pub const VERIFICATION_SCHEMA_VERSION: crate::SchemaVersion = crate::SchemaVersion::new(1, 0);

/// File-name stem of the published verification schema, per ADR-0001 §7.1.
pub const VERIFICATION_SCHEMA_STEM: &str = "verification";

/// The durable record of one verification identity.
///
/// **This is the identity, not yet the finding.** ADR-0001 §12.5 separates the
/// two deliberately — the identity says which inputs a verification was run
/// under, and re-running ASR under a new decoder must not re-run synthesis.
/// E1-S1 owns the identity, so that is what this document records; the
/// transcript, the expected-pattern comparison, and the scored findings arrive
/// with the ASR stack in E4-S1. They are additive optional fields, so they are
/// a compatible extension under
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` and will land as
/// `1.1` rather than as a second document.
///
/// Recording the identity now is what makes that later addition cheap: a
/// finding written against a key this build already produces can be matched to
/// the audio it was about, whereas a finding written beside no identity could
/// only ever be matched by re-deriving one.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct VerificationIdentityRecord {
    /// Published schema this document links to; absent is its declared default.
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    #[schemars(schema_with = "schema_link_json_schema")]
    pub schema: Option<String>,
    /// Schema this document claims, as authored text.
    #[schemars(schema_with = "schema_version_json_schema")]
    pub schema_version: String,
    /// The identity derived from `subject` and `context`.
    pub verification_key: VerificationKey,
    /// The audio and text this verification was about.
    pub subject: VerificationSubject,
    /// Every verification-affecting input that is not the audio itself.
    pub context: VerificationContext,
}

/// Why a verification identity record cannot be trusted.
#[derive(Debug, Error)]
pub enum VerificationRecordError {
    /// This build does not know that version and will not guess.
    #[error("verification schema version is unusable: {0}")]
    UnsupportedSchema(#[from] crate::SchemaVersionError),
    /// The record links to a schema other than the one for its version.
    #[error(
        "verification record links to schema `{declared}` but declares version `{version}`, \
         whose schema is `{expected}`; correct the link or the version"
    )]
    UnexpectedSchemaLink {
        /// Link the record carries.
        declared: String,
        /// Version the record declares.
        version: crate::SchemaVersion,
        /// Link that version requires.
        expected: String,
    },
    /// The recorded key is not the one the recorded inputs produce.
    ///
    /// The same class of failure the synthesis cache refuses at publication: a
    /// record naming a key its inputs do not derive describes a verification
    /// that was run under something other than what it says.
    #[error(
        "verification record declares key `{declared}` but its recorded inputs derive \
         `{derived}`; the verification owner must re-run verification rather than reconcile the \
         two by hand, because one of them describes work that did not happen"
    )]
    KeyDoesNotMatchInputs {
        /// Key the record declares.
        declared: VerificationKey,
        /// Key its own inputs derive.
        derived: VerificationKey,
    },
}

impl VerificationIdentityRecord {
    /// Records a verification identity, deriving the key rather than accepting
    /// one.
    ///
    /// Taking a key from a caller would let a record name a key its own inputs
    /// do not produce, which is the same failure the synthesis cache refuses at
    /// publication.
    pub fn new(subject: VerificationSubject, context: VerificationContext) -> Self {
        Self {
            schema: Some(crate::schema_uri(
                VERIFICATION_SCHEMA_STEM,
                VERIFICATION_SCHEMA_VERSION.major(),
            )),
            schema_version: VERIFICATION_SCHEMA_VERSION.to_string(),
            verification_key: context.key_for(&subject),
            subject,
            context,
        }
    }

    /// Checks a record read back from disk.
    ///
    /// # Errors
    ///
    /// [`VerificationRecordError::UnsupportedSchema`] for a version this build
    /// cannot read, [`VerificationRecordError::UnexpectedSchemaLink`] when the
    /// link names another schema, and
    /// [`VerificationRecordError::KeyDoesNotMatchInputs`] when the recorded key
    /// is not the one the recorded inputs derive.
    pub fn validate(&self) -> Result<(), VerificationRecordError> {
        let version: crate::SchemaVersion = self.schema_version.parse()?;
        version.accepted_by(VERIFICATION_SCHEMA_VERSION)?;
        if let Some(declared) = &self.schema {
            let expected = crate::schema_uri(VERIFICATION_SCHEMA_STEM, version.major());
            if declared != &expected {
                return Err(VerificationRecordError::UnexpectedSchemaLink {
                    declared: declared.clone(),
                    version,
                    expected,
                });
            }
        }
        let derived = self.context.key_for(&self.subject);
        if derived != self.verification_key {
            return Err(VerificationRecordError::KeyDoesNotMatchInputs {
                declared: self.verification_key.clone(),
                derived,
            });
        }
        Ok(())
    }
}

/// Publishes the versions of this document a build reads, rather than any
/// string.
///
/// The counterpart of the `accepted_by` check in
/// [`VerificationIdentityRecord::validate`], so an author's editor and this
/// build refuse the same recorded version.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::accepted_versions_json_schema(VERIFICATION_SCHEMA_VERSION)
}

/// Publishes the one link a document of this major may carry.
///
/// The published half of [`VerificationRecordError::UnexpectedSchemaLink`]:
/// without it the schema admits any string, and an author's editor stays green
/// on a link the build refuses.
fn schema_link_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    crate::schema::schema_link_json_schema(VERIFICATION_SCHEMA_STEM, VERIFICATION_SCHEMA_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        SynthesisContext,
        identity::{sample_context, sample_segment},
    };

    /// One named change to a verification input, for the sensitivity property.
    ///
    /// Reaches both halves of the input. The subject is an input too — the same
    /// decoder over different audio is a different result — and a mutation that
    /// could only touch the context left that half proved by hand, or not at
    /// all.
    type VerificationMutation = (
        &'static str,
        fn(&mut VerificationContext, &mut VerificationSubject),
    );

    /// A fully populated context for the properties below.
    ///
    /// Every field carries a value distinguishable from every other, so a
    /// mutation of one proves *that* field reaches the key rather than proving
    /// only that some field does.
    fn sample_verification_context() -> VerificationContext {
        VerificationContext {
            stack: AsrStackIdentity {
                whisper_rs_version: "0.16.0".to_owned(),
                whisper_rs_sys_version: "0.16.0".to_owned(),
                whisper_cpp_revision: "0123456789abcdef0123456789abcdef01234567".to_owned(),
                model_identity: "ggml-base.en".to_owned(),
                compilation_features: BTreeSet::from(["openblas".to_owned()]),
                execution_device: "cpu".to_owned(),
            },
            conversion: AsrConversionIdentity {
                ffmpeg_version: "6.1.1".to_owned(),
                arguments: vec!["-ar".to_owned(), "16000".to_owned()],
            },
            decoder_parameters: BTreeMap::from([("temperature".to_owned(), "0.0".to_owned())]),
            thread_count: 4,
            expected_pattern_profile_hash: "a".repeat(64).parse().expect("a digest of a parses"),
            comparison_normalizer_hash: "b".repeat(64).parse().expect("a digest of b parses"),
            threshold_profile_hash: "c".repeat(64).parse().expect("a digest of c parses"),
        }
    }

    fn sample_subject() -> VerificationSubject {
        VerificationSubject {
            audio_blake3: "d".repeat(64).parse().expect("a digest of d parses"),
            spoken_text: sample_segment().spoken_text,
        }
    }

    #[test]
    fn t2_e1_every_verification_input_changes_the_verification_key() {
        // Both halves destructured without `..`, so a new verification input on
        // either one is a compile error here until it is given a mutation
        // below. The subject was left out before, which is exactly where an
        // added field would have gone unproven while the property still read as
        // exhaustive.
        let VerificationContext {
            stack:
                AsrStackIdentity {
                    whisper_rs_version: _,
                    whisper_rs_sys_version: _,
                    whisper_cpp_revision: _,
                    model_identity: _,
                    compilation_features: _,
                    execution_device: _,
                },
            conversion:
                AsrConversionIdentity {
                    ffmpeg_version: _,
                    arguments: _,
                },
            decoder_parameters: _,
            thread_count: _,
            expected_pattern_profile_hash: _,
            comparison_normalizer_hash: _,
            threshold_profile_hash: _,
        } = sample_verification_context();
        let VerificationSubject {
            audio_blake3: _,
            spoken_text: _,
        } = sample_subject();

        let baseline = sample_verification_context().key_for(&sample_subject());

        let inputs: [VerificationMutation; 15] = [
            ("whisper_rs_version", |context, _| {
                context.stack.whisper_rs_version = "0.17.0".to_owned();
            }),
            ("whisper_rs_sys_version", |context, _| {
                context.stack.whisper_rs_sys_version = "0.17.0".to_owned();
            }),
            ("whisper_cpp_revision", |context, _| {
                context.stack.whisper_cpp_revision =
                    "fedcba9876543210fedcba9876543210fedcba98".to_owned();
            }),
            ("model_identity", |context, _| {
                context.stack.model_identity = "ggml-small.en".to_owned();
            }),
            ("compilation_features", |context, _| {
                context.stack.compilation_features.insert("cuda".to_owned());
            }),
            ("execution_device", |context, _| {
                context.stack.execution_device = "cuda:0".to_owned();
            }),
            ("ffmpeg_version", |context, _| {
                context.conversion.ffmpeg_version = "7.0.0".to_owned();
            }),
            ("conversion_arguments", |context, _| {
                context.conversion.arguments.push("-ac".to_owned());
            }),
            ("decoder_parameters", |context, _| {
                context
                    .decoder_parameters
                    .insert("temperature".to_owned(), "0.2".to_owned());
            }),
            ("thread_count", |context, _| {
                context.thread_count = 8;
            }),
            ("expected_pattern_profile_hash", |context, _| {
                context.expected_pattern_profile_hash =
                    "e".repeat(64).parse().expect("a digest of e parses");
            }),
            ("comparison_normalizer_hash", |context, _| {
                context.comparison_normalizer_hash =
                    "f".repeat(64).parse().expect("a digest of f parses");
            }),
            // The input a reviewer is most likely to retune, which is why
            // retuning it has to reverify rather than reuse a finding.
            ("threshold_profile_hash", |context, _| {
                context.threshold_profile_hash =
                    "0".repeat(64).parse().expect("a digest of zeros parses");
            }),
            ("audio_blake3", |_, subject| {
                subject.audio_blake3 = "9".repeat(64).parse().expect("a digest of nines parses");
            }),
            ("spoken_text", |_, subject| {
                subject.spoken_text = "Different words entirely.".to_owned();
            }),
        ];

        for (input, mutate) in inputs {
            let mut context = sample_verification_context();
            let mut subject = sample_subject();
            mutate(&mut context, &mut subject);

            assert_ne!(
                context.key_for(&subject),
                baseline,
                "changing `{input}` must change the verification key"
            );
        }
    }

    #[test]
    fn t2_e1_a_verification_input_never_changes_the_synthesis_key() {
        // ADR-0001 §12.5: "Changing any verification input reruns verification
        // without regenerating speech or invoking Chatterbox."
        //
        // No runtime assertion can observe this, and one here used to pretend
        // otherwise: it hashed `sample_context()` before and after retuning a
        // *verification* context and asserted the two were equal, which is one
        // expression compared with a second copy of itself. It could not fail,
        // including on the day the guarantee broke.
        //
        // What the guarantee actually needs is a human decision the day a field
        // is added to both types, so both are destructured here without `..`.
        // The addition stops compiling until somebody has looked at the two
        // lists side by side and confirmed they stay disjoint — which is the
        // only place that judgement can be made, and this is where it is
        // recorded.
        let SynthesisContext {
            worker_bundle_hash: _,
            model_repository: _,
            model_revision: _,
            tokenizer_revision: _,
            // Synthesis-side: it identifies the weights that produced the
            // audio. Nothing here re-runs when ASR changes, so the two lists
            // stay disjoint.
            model_artifacts_hash: _,
            language: _,
            determinism_class: _,
            seed: _,
            generation_parameters: _,
            voice_conditioning_hashes: _,
        } = sample_context();
        let VerificationContext {
            stack: _,
            conversion: _,
            decoder_parameters: _,
            thread_count: _,
            expected_pattern_profile_hash: _,
            comparison_normalizer_hash: _,
            threshold_profile_hash: _,
        } = sample_verification_context();

        // The half that can move, and the ADR sentence as one concrete case:
        // retuning the decoder reverifies.
        let mut retuned = sample_verification_context();
        retuned.thread_count = 16;
        retuned
            .decoder_parameters
            .insert("beam_size".to_owned(), "5".to_owned());
        retuned.threshold_profile_hash = "1".repeat(64).parse().expect("a digest of ones parses");

        assert_ne!(
            retuned.key_for(&sample_subject()),
            sample_verification_context().key_for(&sample_subject()),
            "the retuned decoder must produce a different verification key"
        );
    }
}
