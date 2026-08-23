mod assembly;
mod cache;
mod export;
mod manifest;
mod pipeline;
mod synthesis;
mod tools;
mod voice_gate;

pub use pipeline::{
    BuildRequest, BuildResult, build_preview, publish, validate_encoded_output,
    validate_production_manifest,
};
pub use synthesis::{SegmentSynthesizer, SynthesisError, SynthesisReport};

use std::{
    io,
    path::{Path, PathBuf},
};

use study_tts_core::CacheKey;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("could not read `{path}`: {source}")]
    ReadFile { path: PathBuf, source: io::Error },

    #[error(transparent)]
    Lesson(#[from] study_tts_core::LessonError),

    #[error(transparent)]
    Voice(#[from] study_tts_core::VoiceError),

    // Remedy routing per `docs/governance/ROUTING-TABLES.md` ("Voice consent/checksum mismatch
    // → Refuse profile load → Project owner → Blocked"): the owner resolves the record; the
    // profile is never deleted or repaired automatically.
    //
    // `profile.json` and `consent.json` share this variant because they are the same class of
    // refusal — a record the policy requires is absent — and an absent profile record deserves
    // the same remedy-bearing message as an absent consent record, not a bare IO error.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is missing; \
         profile load fails closed and the project owner must supply the record before use"
    )]
    MissingVoiceRecord {
        profile_dir: PathBuf,
        record: &'static str,
    },

    #[error(
        "voice profile at `{profile_dir}` is refused: `{path}` does not match its recorded \
         checksum; do not use this profile until the project owner re-verifies it against its \
         rights record"
    )]
    VoiceChecksumMismatch { profile_dir: PathBuf, path: PathBuf },

    #[error(
        "production release is refused: source `{source_id}` has unresolved rights \
         classification `{classification}`; the project owner must resolve the classification in \
         its rights record before publication"
    )]
    UnresolvedContentRights {
        source_id: String,
        classification: String,
    },

    /// A manifest's rights section does not parse — an unknown classification, a missing field,
    /// or the wrong shape.
    ///
    /// Distinct from the `Json` catch-all below: on the publication path the section is known
    /// and worth naming, and an operator reading "JSON operation failed" would not learn that
    /// their manifest declared a classification outside the recorded vocabulary.
    #[error(
        "production release is refused: the `{section}` manifest section is not a valid rights \
         declaration ({source}); the project owner must correct the manifest before publication"
    )]
    InvalidRightsDeclaration {
        section: &'static str,
        source: serde_json::Error,
    },

    #[error("filesystem operation failed for `{path}`: {source}")]
    FileSystem { path: PathBuf, source: io::Error },

    #[error("audio operation failed for `{path}`: {source}")]
    AudioAt { path: PathBuf, source: hound::Error },

    #[error(transparent)]
    Synthesis(#[from] SynthesisError),

    /// A published cache entry cannot be used.
    ///
    /// Remedy routing per `docs/governance/ROUTING-TABLES.md` ("State or checksum corruption →
    /// Refuse overwrite; run reconciliation → Runtime → Blocked"). E0-S0 has no reconciliation —
    /// E2-S1 adds it — so deletion is the safe recovery action ADR-0001 §4.2 requires, because
    /// the segment regenerates from the plan on the next build.
    ///
    /// The entry directory and the remedy are stated once here rather than at each rejection
    /// site; `CacheEntryFault` carries which invariant was violated, so a test asserts the exact
    /// fault instead of matching a substring.
    ///
    /// The fault is boxed because it is by far the largest thing `BuildError` carries, and every
    /// fallible function in this crate pays for the widest variant on its success path too.
    #[error(
        "cache entry for segment `{segment_id}` is unusable: {fault}; delete `{}` to regenerate \
         this segment",
        entry_dir.display()
    )]
    UnusableCacheEntry {
        entry_dir: PathBuf,
        segment_id: String,
        fault: Box<CacheEntryFault>,
    },

    /// Audio that is not a published cache entry: freshly synthesized output, or a staged master.
    ///
    /// Routing: "Invalid or over-range audio → Quarantine unique attempt; bounded retry →
    /// Audio/runtime → Blocked for segment". Deliberately carries no deletion remedy — the
    /// staged file is discarded on drop, so there is nothing published to remove, and advising a
    /// deletion would name a path that no longer exists.
    #[error("`{path}` is not usable lesson audio: {fault}")]
    UnusableAudio { path: PathBuf, fault: AudioFault },

    /// The worker's report disagrees with the file it wrote.
    ///
    /// Routing: "Worker protocol or containment failure → Terminate worker tree; preserve
    /// diagnostics → Worker/runtime → Blocked". Separate from `UnusableAudio` because the audio
    /// is canonical: what failed is the worker's account of it, and no retry of the same worker
    /// build fixes that.
    #[error(
        "synthesizer reported {reported_sample_rate} Hz, {reported_channels} channels, and \
         {reported_frames} frames for segment `{segment_id}` but wrote a WAV with \
         {written_sample_rate} Hz, {written_channels} channel, and {written_frames} frames; the \
         worker is misreporting its own output and must be corrected before this build is rerun"
    )]
    SynthesizerReportMismatch {
        segment_id: String,
        reported_sample_rate: u32,
        reported_channels: u16,
        reported_frames: u32,
        written_sample_rate: u32,
        written_channels: u16,
        written_frames: u32,
    },

    #[error(
        "the pause of {pause_after_ms} ms after segment `{segment_id}` overflows the frame count \
         this build can assemble; shorten the pause in the lesson"
    )]
    PauseFrameOverflow {
        segment_id: String,
        pause_after_ms: u32,
    },

    #[error(
        "the planned lesson exceeds the frame count this build can assemble; split the lesson \
         into shorter lessons"
    )]
    PlannedLengthOverflow,

    #[error(
        "assembling `{destination}` exceeded the frame count this build can track; split the \
         lesson into shorter lessons"
    )]
    AssembledLengthOverflow { destination: PathBuf },

    /// The master's length disagrees with the length its validated cache metadata implies.
    ///
    /// Redundant while every per-segment check passes, and retained because it is the invariant
    /// the manifest and every downstream duration derive from. Routing: "State or checksum
    /// corruption → Refuse overwrite; run reconciliation → Runtime → Blocked".
    #[error(
        "assembled master `{destination}` contains {assembled} frames but the plan requires \
         {expected}; the runtime owner must reconcile the cache before this lesson is rebuilt"
    )]
    AssembledLengthMismatch {
        destination: PathBuf,
        assembled: u64,
        expected: u64,
    },

    /// Every output this crate writes is staged in its destination's parent and renamed into
    /// place, so a destination with no parent cannot be written atomically at all.
    #[error(
        "cannot write `{path}` atomically because it has no parent directory; supply an output \
         path with a directory component"
    )]
    UnrootedDestination { path: PathBuf },

    #[error("FFmpeg failed with status {status}: {stderr}")]
    Ffmpeg { status: String, stderr: String },

    #[error("could not start FFmpeg `{executable}`: {source}")]
    StartFfmpeg {
        executable: PathBuf,
        source: io::Error,
    },

    #[error("required tool {tool} was not found or is not executable at `{requested}`")]
    MissingTool { tool: String, requested: PathBuf },

    #[error("could not inspect {tool} at `{executable}`: {source}")]
    InspectTool {
        tool: String,
        executable: PathBuf,
        source: io::Error,
    },

    #[error("{tool} version probe failed with status {status}: {stderr}")]
    ToolProbeFailed {
        tool: String,
        status: String,
        stderr: String,
    },

    #[error("ffprobe failed with status {status}: {stderr}")]
    Ffprobe { status: String, stderr: String },

    #[error("encoded output failed structural validation: {0}")]
    InvalidEncodedOutput(String),

    #[error("managed path `{path}` resolves outside `{root}`")]
    ManagedPathEscape { path: PathBuf, root: PathBuf },

    #[error("production publication is refused: {reason}")]
    PublicationRefused { reason: String },

    #[error("manifest version `{version}` is not a production manifest")]
    UnsupportedProductionManifest { version: String },

    #[error("could not write JSON to `{path}`: {source}")]
    WriteJson {
        path: PathBuf,
        source: serde_json::Error,
    },

    /// Catch-all for `?` on a `serde_json` call with no useful context to add. Prefer a variant
    /// that carries the path or the subsystem; this exists so a future call site cannot silently
    /// inherit an unrelated error message the way the former `Manifest` variant did.
    #[error("JSON operation failed: {0}")]
    Json(#[from] serde_json::Error),
}

/// Which invariant a published cache entry violated.
///
/// Nested inside [`BuildError::UnusableCacheEntry`] rather than flattened into `BuildError`: the
/// entry directory and the deletion remedy are the same for every one of these, and stating them
/// once keeps the remedy from drifting between rejection sites. What differs is the invariant,
/// and that is exactly what this enum names.
#[derive(Debug, Error)]
pub enum CacheEntryFault {
    #[error("`{path}` could not be parsed ({source})")]
    UnparseableArtifact {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error(
        "the artifact declares schema `{schema_version}`, {sample_rate} Hz, {channels} channels, \
         and format `{sample_format}` but this build requires schema \
         `{required_schema_version}`, {required_sample_rate} Hz, {required_channels} channel, and \
         `{required_sample_format}`"
    )]
    IncompatibleArtifact {
        schema_version: String,
        sample_rate: u32,
        channels: u16,
        sample_format: String,
        required_schema_version: &'static str,
        required_sample_rate: u32,
        required_channels: u16,
        required_sample_format: &'static str,
    },

    /// The entry belongs to a different synthesis identity, so its audio is not this segment's.
    #[error("the artifact records cache key `{recorded}` but the plan requires `{required}`")]
    CacheKeyMismatch {
        recorded: CacheKey,
        required: CacheKey,
    },

    /// Raised both when the entry is loaded and when the master is assembled from it, because a
    /// truncated entry and a truncated read are the same violated invariant.
    #[error("the audio holds {found} frames but the artifact declares {declared}")]
    FrameCountMismatch { found: u64, declared: u32 },

    /// Distinct from `ChecksumMismatch` on purpose: a malformed record reported as a mismatch
    /// tells the operator their audio was tampered with when the artifact is what broke.
    #[error(
        "the artifact records `{recorded}` as the audio digest, which is not a lowercase BLAKE3 \
         hex digest and so could never match the audio"
    )]
    MalformedRecordedDigest { recorded: String },

    #[error("the audio checksum `{found}` does not match the artifact record `{declared}`")]
    ChecksumMismatch { found: String, declared: String },

    #[error("{0}")]
    Audio(#[from] AudioFault),
}

/// Why a WAV cannot serve as canonical lesson audio.
///
/// Carries no path and no remedy: the same validation runs on a staged file that no operator can
/// act on and on a published entry that they can, and only the caller knows which. Attaching the
/// remedy here is what previously produced "delete this entry" for a file that was about to be
/// discarded anyway.
#[derive(Debug, Error)]
pub enum AudioFault {
    #[error("it could not be read as WAV ({0})")]
    Unreadable(#[from] hound::Error),

    #[error(
        "the stream is {channels}-channel {sample_rate} Hz {bits_per_sample}-bit \
         {sample_format}, not canonical mono {required_sample_rate} Hz 32-bit float"
    )]
    NonCanonical {
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        sample_format: &'static str,
        required_sample_rate: u32,
    },

    #[error("sample {index} is `{value}`, outside the finite range -1.0 to 1.0")]
    OutOfRangeSample { index: u32, value: f32 },

    #[error("it contains no audio frames")]
    Empty,

    #[error("it holds more frames than this build can count")]
    FrameCountOverflow,
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    BuildError::FileSystem {
        path: path.into(),
        source,
    }
}

/// `hound::Error` wraps `io::Error`, which carries no filename, so every audio failure must be
/// given its path here or the message names nothing.
pub(crate) fn audio_error(path: impl Into<PathBuf>, source: hound::Error) -> BuildError {
    BuildError::AudioAt {
        path: path.into(),
        source,
    }
}

/// Directory holding one cache entry.
///
/// Exposed so integration tests can corrupt a specific entry without duplicating the sharding
/// scheme. Changing the shard width in `cache::entry_dir` updates the tests automatically.
///
/// Takes a parsed `CacheKey` rather than a string: this is the one cache path that crosses the
/// crate boundary, and a caller reading a key out of a manifest should be told the key is
/// malformed there rather than have it panic inside the shard slice.
#[doc(hidden)]
pub fn cache_entry_dir(cache_root: &Path, cache_key: &CacheKey) -> PathBuf {
    cache::entry_dir(cache_root, cache_key)
}
