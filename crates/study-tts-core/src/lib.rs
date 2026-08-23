//! Lesson domain types, deterministic render planning, and the rights and
//! release records that gate what may be synthesized or published.
//!
//! This crate owns every value that is parsed from a record on disk, so an
//! unknown or malformed value is refused here rather than somewhere downstream
//! that can only compare it.

mod digest;
mod lesson;
mod plan;
mod release;
mod rights;
mod voice;

pub use digest::is_blake3_hex;
pub use lesson::{Lesson, LessonError, LessonSegment, ReviewStatus};
pub use plan::{
    CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, CacheKey, MalformedCacheKey, PlanHash,
    PlannedSegment, RenderPlan,
};
pub use release::{REQUIRED_PRODUCTION_GATES, ReleaseClaim, ReleaseError, ReleaseStatus};
pub use rights::{SourceClassification, SourceRightsDeclaration};
pub use voice::{
    ConsentStatus, RightsDecision, VoiceConsent, VoiceError, VoiceProfile, VoiceUse,
    validate_profile_for_use,
};
