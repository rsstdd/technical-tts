//! Published-cache refusals and their context-free inner faults.

use std::{io, path::PathBuf};

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

    /// A directory inside the cache tree is not named by a cache key.
    ///
    /// Refused rather than skipped, because the two safe readings disagree:
    /// treating it as absent would hide it from reconciliation, and treating it
    /// as an entry would offer something this build never wrote to a prune
    /// command that will one day delete what it is offered.
    #[error(
        "`{entry_dir}` is inside the synthesis cache but is not named by a cache key; the cache \
         is content addressed, so the runtime owner must run reconciliation and move the \
         directory to quarantine rather than let a retention report reason about it"
    )]
    UnrecognizedCacheEntry {
        /// The directory runtime reconciliation must inspect.
        entry_dir: PathBuf,
    },

    /// A worker left a file in the staging transaction beside its audio.
    ///
    /// The stage *becomes* the published entry — ADR-0001 §12.6 renames it into
    /// place — so anything left in it is published inside a cache entry that
    /// claims to hold one segment's speech. Refused rather than swept, because
    /// a file nobody expected is a worker doing something nobody described, and
    /// deleting the evidence is the wrong half of that to automate.
    #[error(
        "the worker left `{unexpected}` in the staging transaction for segment `{segment_id}` \
         beside the audio it was assigned; the attempt is quarantined for a person to read"
    )]
    UncontainedStagedFile {
        /// The segment whose transaction was refused.
        segment_id: String,
        /// Name of the first unexpected entry found in the stage.
        unexpected: String,
    },

    /// A package request omitted or added validated cache artifacts.
    #[error(
        "package request supplied {found} cached artifacts for a plan with {required} segments; \
         preserve the cache and route the request builder to runtime reconciliation"
    )]
    PackageArtifactCountMismatch {
        /// Number of artifacts the package request supplied.
        found: usize,
        /// Number of artifacts the render plan requires.
        required: usize,
    },

    /// A package-position artifact belongs to a different planned segment.
    #[error(
        "package {mismatch}; preserve the cache and route the request builder to runtime \
         reconciliation"
    )]
    PackageArtifactPlanMismatch {
        /// Compared artifact and plan fields at the mismatching position.
        mismatch: Box<PackageArtifactMismatch>,
    },
}

/// Compared package artifact and plan fields at one mismatching position.
#[derive(Debug, Error)]
#[error(
    "artifact {position} records segment `{recorded_segment_id}`, cache key \
     `{recorded_cache_key}`, and pause {recorded_pause_after_ms} ms but the plan requires segment \
     `{required_segment_id}`, cache key `{required_cache_key}`, and pause \
     {required_pause_after_ms} ms"
)]
pub struct PackageArtifactMismatch {
    /// Zero-based package position whose artifact and plan disagree.
    pub position: usize,
    /// Segment identity carried by the artifact.
    pub recorded_segment_id: String,
    /// Synthesis identity carried by the artifact.
    pub recorded_cache_key: CacheKey,
    /// Pause carried by the artifact.
    pub recorded_pause_after_ms: u32,
    /// Segment identity required by the plan.
    pub required_segment_id: String,
    /// Synthesis identity required by the plan.
    pub required_cache_key: CacheKey,
    /// Pause required by the plan.
    pub required_pause_after_ms: u32,
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
            Self::UncontainedStagedFile { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "read the quarantined attempt and correct the worker that staged an unexpected \
                 file",
                Some("Worker protocol or containment failure"),
            )),
            Self::UnrecognizedCacheEntry { .. }
            | Self::PackageArtifactCountMismatch { .. }
            | Self::PackageArtifactPlanMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the cache and run runtime reconciliation",
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
    /// The artifact file cannot be read.
    #[error("`{path}` could not be read ({source})")]
    UnreadableArtifact {
        /// Artifact path that could not be read.
        path: PathBuf,
        /// What the filesystem reported.
        source: io::Error,
    },

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

    /// The artifact's recorded provenance does not derive the key it is filed
    /// under.
    ///
    /// Separate from [`CacheEntryFault::CacheKeyMismatch`], which compares the
    /// recorded key with the plan's. That comparison says nothing about the
    /// inputs recorded beside it: an edited `model_revision`, `language`, or
    /// worker-bundle hash leaves both keys agreeing while the audit record
    /// describes synthesis that never happened.
    ///
    /// The whole key is recomputed rather than the fields compared one by one,
    /// so an input added to [`study_tts_core::SynthesisContext`] later is
    /// covered without this check being edited.
    #[error(
        "the artifact is filed under cache key `{recorded}` but its recorded provenance derives \
         `{derived}`, so the inputs it names are not the inputs that produced this audio"
    )]
    ProvenanceKeyMismatch {
        /// The identity the entry is published under.
        recorded: CacheKey,
        /// The identity the recorded provenance actually derives.
        derived: CacheKey,
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

    /// The artifact declares conditioning ADR-0001 does not permit.
    ///
    /// This is the fixed ADR-0001 §13.4 ceiling. Audio-derived feasibility is
    /// checked separately by [`Self::ConditioningInconsistentWithAudio`].
    #[error(
        "the artifact declares {field} of {declared} samples, beyond the {permitted} \
         ({permitted_milliseconds} ms) ADR-0001 §13.4 permits"
    )]
    ConditioningOutsideRatifiedGeometry {
        /// Which recorded count is out of range.
        field: &'static str,
        /// The value the artifact declares.
        declared: u32,
        /// The most ADR-0001 §13.4 permits there.
        permitted: u32,
        /// The same limit as the duration ADR-0001 §13.4 states.
        permitted_milliseconds: u32,
    },

    /// The recorded ramp geometry cannot describe the cached audio.
    #[error(
        "the artifact declares {field} of {declared} samples, but the cached audio permits \
         {minimum} through {maximum} samples"
    )]
    ConditioningInconsistentWithAudio {
        /// Which recorded ramp contradicts the audio.
        field: &'static str,
        /// The value the artifact declares.
        declared: u32,
        /// The smallest ramp consistent with the audio.
        minimum: u32,
        /// The largest ramp consistent with the audio.
        maximum: u32,
    },

    /// The entry was conditioned under a calibration this build does not apply.
    ///
    /// The recorded counts describe a silence threshold, and a threshold that
    /// has moved describes different audio. Refusing here is what makes
    /// `ADR-0001-D007` expire cleanly: every entry conditioned against the
    /// provisional value is refused once an accepted ADR-0003 gives this build
    /// a frozen one, rather than being reused under a reading nobody checked.
    #[error(
        "the artifact records conditioning calibrated as {recorded}, but this build conditions \
         against a {required} threshold"
    )]
    ConditionedUnderAnotherCalibration {
        /// The calibration the entry records.
        recorded: &'static str,
        /// The calibration this build applies.
        required: &'static str,
    },

    /// The entry's audio failed canonical-audio validation.
    #[error("{0}")]
    Audio(#[from] AudioFault),
}
