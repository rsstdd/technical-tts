//! Published-cache refusals and their context-free inner faults.

use std::path::PathBuf;

use study_tts_core::CacheKey;
use thiserror::Error;

use super::{AudioFault, RemedyAdvice, RemedyOwner};

/// Why a published cache entry cannot be used.
#[derive(Debug, Error)]
pub enum CacheError {
    /// A published cache entry violated an acceptance invariant.
    ///
    /// The inner fault remains boxed because it is the largest error payload.
    /// The supported-target measurement recorded in
    /// `docs/architecture/WALKING-SKELETON.md` keeps [`crate::BuildError`] at
    /// 80 bytes without allocating on successful results.
    ///
    /// The routing table calls for reconciliation without overwrite. The E0
    /// durability foundation preserves the entry for the runtime owner rather
    /// than telling an operator to delete authoritative content-addressed data.
    #[error(
        "cache entry for segment `{segment_id}` at `{}` is unusable: {fault}; preserve it for \
         runtime reconciliation",
        entry_dir.display()
    )]
    UnusableCacheEntry {
        /// The entry directory runtime reconciliation must inspect.
        entry_dir: PathBuf,
        /// The segment the entry belongs to.
        segment_id: String,
        /// Which invariant the entry violated.
        fault: Box<CacheEntryFault>,
    },
}

impl CacheError {
    /// Returns the governed recovery advice for this cache refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::UnusableCacheEntry { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the unusable cache entry and run runtime reconciliation",
                Some("State or checksum corruption"),
            )),
        }
    }
}

/// Which invariant a published cache entry violated.
///
/// This inner fault carries no entry path or remedy. [`CacheError`] owns that
/// outer context so every rejection of an entry gives the same recovery
/// instruction without duplicating it at each validation site.
#[derive(Debug, Error)]
pub enum CacheEntryFault {
    /// The artifact is not readable as the record this build writes.
    #[error("`{path}` could not be parsed ({source})")]
    UnparseableArtifact {
        /// The artifact that could not be parsed.
        path: PathBuf,
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// The artifact parses but describes audio this build cannot consume.
    #[error(
        "the artifact declares schema `{schema_version}`, {sample_rate} Hz, {channels} channels, \
         and format `{sample_format}` but this build requires schema \
         `{required_schema_version}`, {required_sample_rate} Hz, {required_channels} channels, \
         and `{required_sample_format}`"
    )]
    IncompatibleArtifact {
        /// Schema the artifact declares.
        schema_version: String,
        /// Sample rate the artifact declares, in hertz.
        sample_rate: u32,
        /// Channel count the artifact declares.
        channels: u16,
        /// Sample format the artifact declares.
        sample_format: String,
        /// Schema this build requires.
        required_schema_version: &'static str,
        /// Sample rate this build requires, in hertz.
        required_sample_rate: u32,
        /// Channel count this build requires.
        required_channels: u16,
        /// Sample format this build requires.
        required_sample_format: &'static str,
    },

    /// The entry belongs to a different synthesis identity.
    #[error("the artifact records cache key `{recorded}` but the plan requires `{required}`")]
    CacheKeyMismatch {
        /// The identity the artifact records.
        recorded: CacheKey,
        /// The identity the current plan requires.
        required: CacheKey,
    },

    /// The audio length disagrees with its artifact record.
    #[error("the audio holds {found} frames but the artifact declares {declared}")]
    FrameCountMismatch {
        /// Frames the audio actually holds.
        found: u64,
        /// Frames the artifact declares.
        declared: u32,
    },

    /// The artifact's recorded audio digest is malformed.
    #[error(
        "the artifact records `{recorded}` as the audio digest, which is not a lowercase BLAKE3 \
         hex digest and so could never match the audio"
    )]
    MalformedRecordedDigest {
        /// The value the artifact records where a digest belongs.
        recorded: String,
    },

    /// The current audio checksum disagrees with its artifact record.
    #[error("the audio checksum `{found}` does not match the artifact record `{declared}`")]
    ChecksumMismatch {
        /// Digest computed from the audio now.
        found: String,
        /// Digest the artifact records.
        declared: String,
    },

    /// The entry's audio failed canonical-audio validation.
    #[error("{0}")]
    Audio(#[from] AudioFault),
}
