//! The values this project's records are made of, and the rules that decide
//! whether one is valid.
//!
//! Two things live here, and the rest of the workspace rests on both:
//!
//! - **Domain records.** A lesson, a takes file, a voice record, a rights
//!   declaration, a release claim — each owns the validation rule that refuses
//!   unusable data before it reaches a durable decision.
//! - **Identity.** ADR-0001 §12.5 derives the synthesis cache key, the plan
//!   hash, and a separate verification key from named input lists, over the
//!   canonical byte form in [`canonical_bytes`]. The lists live here because a
//!   second copy of one would be a second answer to "is this the same audio".
//!
//! Deliberately absent: I/O, processes, and the filesystem. Nothing in this
//! crate opens a file, spawns a worker, or names a path — `study-tts-runtime`
//! does that, and parses its own on-disk records (`manifest.json`, a cache
//! artifact, a worker bundle manifest) into the types above. That split is what
//! lets every rule here be tested without a temporary directory or an external
//! binary.
//!
//! ADR-0001 governs the architecture. Where a rule is policy rather than
//! architecture, the governing document under `docs/governance/` is named by
//! the module that implements it, and names that module in return.

mod canonical;
mod contract;
mod digest;
mod identity;
mod job;
mod language;
mod lesson;
mod plan;
mod release;
mod rights;
mod schema;
mod takes;
mod tool;
mod verification;
mod voice;

pub use canonical::{CanonicalValue, canonical_bytes, canonical_digest};
pub use contract::{
    ContractChange, ContractDescriptor, ContractId, ContractVersion, ContractVersionError,
    SuccessorCompatibility,
};
pub use digest::{BLAKE3_HEX_PATTERN, is_blake3_hex};
pub use identity::{
    CACHE_SCHEMA_VERSION, DeterminismClass, MAX_REVISION_BYTES, MalformedModelArtifactsHash,
    MalformedRevision, MalformedWorkerBundleHash, ModelArtifactsHash, Revision,
    SYNTHESIS_IDENTITY_VERSION, SynthesisContext, WorkerBundleHash,
};
pub use job::{
    AbandonedAttempt, JOB_SCHEMA_VERSION, JobDocument, JobState, JobStateError, LessonDigest,
    MalformedLessonDigest, MalformedManifestDigest, ManifestDigest, SegmentStatus,
    SelectedPackageIdentity,
};
pub use language::{LanguageTag, MAX_LANGUAGE_TAG_BYTES, MalformedLanguageTag};
pub use lesson::{
    AuthoredLesson, DeliveryStyle, LESSON_SCHEMA_STEM, LESSON_SCHEMA_VERSION, LessonDiagnostic,
    LessonError, LessonSegment, LessonSource, MAX_LESSON_JSON_BYTES, MAX_LESSON_SEGMENTS,
    MalformedSourceContentHash, ReviewStatus, SegmentRole, SourceContentHash, SpeakerDeclaration,
    ValidatedLesson, validate_lesson_id, validate_segment_id,
};
pub use plan::{
    BASE_TAKE, CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT,
    CANONICAL_SAMPLE_RATE, CacheKey, MalformedCacheKey, MalformedPlanHash, PLAN_SCHEMA_STEM,
    PLAN_SCHEMA_VERSION, PlanError, PlanHash, PlannedSegment, RenderPlan,
};
pub use release::{REQUIRED_PRODUCTION_GATES, ReleaseClaim, ReleaseError, ReleaseStatus};
pub use rights::{SourceClassification, SourceRightsDeclaration};
pub use schema::{
    SCHEMA_URI_BASE, SCHEMA_VERSION_PATTERN, SchemaCompatibility, SchemaVersion,
    SchemaVersionError, schema_file_name, schema_uri,
};
pub use takes::{
    MAX_TAKES_JSON_BYTES, SelectedTake, TAKES_SCHEMA_STEM, TAKES_SCHEMA_VERSION, TakesDocument,
    TakesError, ValidatedTakes,
};
pub use tool::{MalformedToolProfileHash, ToolProfileHash};
pub use verification::{
    AsrConversionIdentity, AsrStackIdentity, AudioDigest, MalformedAudioDigest,
    MalformedVerificationKey, MalformedVerificationProfileHash, VERIFICATION_IDENTITY_VERSION,
    VERIFICATION_SCHEMA_STEM, VERIFICATION_SCHEMA_VERSION, VerificationContext,
    VerificationIdentityRecord, VerificationKey, VerificationProfileHash, VerificationRecordError,
    VerificationSubject,
};
pub use voice::{
    ConsentStatus, MalformedVoiceConditioningHash, MalformedVoiceProfileHash, RightsDecision,
    VoiceConditioningHash, VoiceConsent, VoiceError, VoiceProfile, VoiceProfileHash, VoiceUse,
    validate_profile_for_use,
};
