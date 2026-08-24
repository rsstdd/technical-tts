//! Every way a build or publication can be refused.
//!
//! Split from the crate root so that navigating the pipeline does not mean
//! scrolling past every failure it can report. Nothing is re-shaped in the
//! move: `BuildError` stays one variant per violated invariant, and the two
//! fault enums stay nested inside the variants that supply their remedy.

use std::{io, path::PathBuf};

use study_tts_core::{CacheKey, ReleaseStatus};
use thiserror::Error;

use crate::SynthesisError;

/// Why a build or publication was refused.
///
/// One variant per violated invariant, so a test can assert the exact failure
/// and an operator is told which control stopped them. Every refusal names the
/// artifact, the invariant, and the remedy owner from
/// `docs/governance/ROUTING-TABLES.md`.
#[derive(Debug, Error)]
pub enum BuildError {
    /// An input the build was told to read is not readable.
    #[error("could not read `{path}`: {source}")]
    ReadFile {
        /// The file that could not be read.
        path: PathBuf,
        /// What the filesystem reported.
        source: io::Error,
    },

    /// The lesson document was refused; the inner error names which invariant.
    #[error(transparent)]
    Lesson(#[from] study_tts_core::LessonError),

    /// A voice record was refused; the inner error names which invariant.
    #[error(transparent)]
    Voice(#[from] study_tts_core::VoiceError),

    /// A record the voice policy requires is absent, so profile load fails
    /// closed.
    ///
    /// Remedy routing per `docs/governance/ROUTING-TABLES.md` ("Voice
    /// consent/checksum mismatch → Refuse profile load → Project owner →
    /// Blocked"): the owner resolves the record; the profile is never deleted
    /// or repaired automatically.
    ///
    /// All four records of the ADR-0001 §12.1 layout share this variant —
    /// `profile.json`, `consent.json`, `reference.wav`, `conditionals.pt` —
    /// because they are one class of refusal: a record the policy requires is
    /// absent. Each deserves the same remedy-bearing message, not a bare IO
    /// error naming neither the policy nor a person.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is missing; \
         profile load fails closed and the project owner must supply the record before use"
    )]
    MissingVoiceRecord {
        /// The profile directory the record was expected in.
        profile_dir: PathBuf,
        /// Which required record is absent.
        record: &'static str,
    },

    /// A record the voice policy requires is present under its name, but the
    /// name holds something other than a regular file.
    ///
    /// Distinct from [`BuildError::MissingVoiceRecord`] because the record is
    /// there: reporting it absent would send the owner looking for a file they
    /// would find. Distinct from [`BuildError::ManagedPathEscape`] because that
    /// variant names the root a build confined itself to, and a profile
    /// directory is operator-supplied input this crate neither creates nor
    /// canonicalizes, so it can make no such claim.
    ///
    /// The refusal is what keeps the checksum gate meaningful. `reference.wav`
    /// and `conditionals.pt` are read and hashed through the same name, so a
    /// link would supply both the bytes and the digest from one file outside
    /// the directory and the gate would agree with itself, admitting audio no
    /// consent record covers. A FIFO or device node is refused by the same
    /// check, because hashing one never returns.
    ///
    /// Remedy routing per `docs/governance/ROUTING-TABLES.md` ("Voice
    /// consent/checksum mismatch → Refuse profile load → Project owner →
    /// Blocked"): only the owner can say what the record should contain, and
    /// nothing here is repaired or removed automatically.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is not a regular \
         file; profile load fails closed and the project owner must supply the record itself \
         before use"
    )]
    VoiceRecordNotRegularFile {
        /// The profile directory the record was expected in.
        profile_dir: PathBuf,
        /// Which required record is not a regular file.
        record: &'static str,
    },

    /// A profile file no longer hashes to what its record says, so the record
    /// no longer describes it. Routing: "Voice consent/checksum mismatch →
    /// Project owner → Blocked".
    #[error(
        "voice profile at `{profile_dir}` is refused: `{path}` does not match its recorded \
         checksum; do not use this profile until the project owner re-verifies it against its \
         rights record"
    )]
    VoiceChecksumMismatch {
        /// The profile directory whose record no longer holds.
        profile_dir: PathBuf,
        /// The file whose contents disagree with the record.
        path: PathBuf,
    },

    /// A source reached publication without a resolved rights classification.
    #[error(
        "production release is refused: source `{source_id}` has unresolved rights \
         classification `{classification}`; the project owner must resolve the classification in \
         its rights record before publication"
    )]
    UnresolvedContentRights {
        /// The source whose classification is unresolved.
        source_id: String,
        /// The classification it currently carries.
        classification: String,
    },

    /// A manifest's rights section does not parse — an unknown classification,
    /// a missing field, or the wrong shape.
    ///
    /// One of several parse failures that each name their own artifact, rather
    /// than one shared JSON variant: an operator reading "JSON operation
    /// failed" would not learn that their manifest declared a classification
    /// outside the recorded vocabulary.
    #[error(
        "production release is refused: the `{section}` manifest section is not a valid rights \
         declaration ({source}); the project owner must correct the manifest before publication"
    )]
    InvalidRightsDeclaration {
        /// Which manifest section failed to parse.
        section: &'static str,
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// A filesystem operation the build performs itself failed.
    #[error("filesystem operation failed for `{path}`: {source}")]
    FileSystem {
        /// The path being operated on.
        path: PathBuf,
        /// What the filesystem reported.
        source: io::Error,
    },

    /// An audio read or write failed. `hound::Error` wraps `io::Error`, which
    /// carries no filename, so the path is attached here or the message names
    /// nothing.
    #[error("audio operation failed for `{path}`: {source}")]
    AudioAt {
        /// The audio file being read or written.
        path: PathBuf,
        /// What the WAV layer reported.
        source: hound::Error,
    },

    /// The synthesizer refused or failed; the inner error carries what it
    /// reported.
    #[error(transparent)]
    Synthesis(#[from] SynthesisError),

    /// A published cache entry cannot be used.
    ///
    /// Remedy routing per `docs/governance/ROUTING-TABLES.md` ("State or
    /// checksum corruption → Refuse overwrite; run reconciliation → Runtime →
    /// Blocked"). E0-S0 has no reconciliation — E2-S1 adds it — so deletion is
    /// the safe recovery action ADR-0001 §4.2 requires, because the segment
    /// regenerates from the plan on the next build.
    ///
    /// The entry directory and the remedy are stated once here rather than at
    /// each rejection site; `CacheEntryFault` carries which invariant was
    /// violated, so a test asserts the exact fault instead of matching a
    /// substring.
    ///
    /// The fault is boxed because it is by far the largest thing `BuildError`
    /// carries, and every fallible function in this crate pays for the widest
    /// variant on its success path too.
    #[error(
        "cache entry for segment `{segment_id}` is unusable: {fault}; delete `{}` to regenerate \
         this segment",
        entry_dir.display()
    )]
    UnusableCacheEntry {
        /// The entry directory to delete in order to regenerate the segment.
        entry_dir: PathBuf,
        /// The segment the entry belongs to.
        segment_id: String,
        /// Which invariant the entry violated.
        fault: Box<CacheEntryFault>,
    },

    /// Audio that is not a published cache entry: freshly synthesized output,
    /// or a staged master.
    ///
    /// Routing: "Invalid or over-range audio → Quarantine unique attempt;
    /// bounded retry → Audio/runtime → Blocked for segment". Deliberately
    /// carries no deletion remedy — the staged file is discarded on drop, so
    /// there is nothing published to remove, and advising a deletion would name
    /// a path that no longer exists.
    #[error("`{path}` is not usable lesson audio: {fault}")]
    UnusableAudio {
        /// The audio file that failed validation.
        path: PathBuf,
        /// Which audio property failed.
        fault: AudioFault,
    },

    /// The worker's report disagrees with the file it wrote.
    ///
    /// Routing: "Worker protocol or containment failure → Terminate worker
    /// tree; preserve diagnostics → Worker/runtime → Blocked". Separate from
    /// `UnusableAudio` because the audio is canonical: what failed is the
    /// worker's account of it, and no retry of the same worker build fixes
    /// that.
    #[error(
        "synthesizer reported {reported_sample_rate} Hz, {reported_channels} channels, and \
         {reported_frames} frames for segment `{segment_id}` but wrote a WAV with \
         {written_sample_rate} Hz, {written_channels} channels, and {written_frames} frames; the \
         worker is misreporting its own output and must be corrected before this build is rerun"
    )]
    SynthesizerReportMismatch {
        /// The segment whose synthesis was misreported.
        segment_id: String,
        /// Sample rate the worker claimed.
        reported_sample_rate: u32,
        /// Channel count the worker claimed.
        reported_channels: u16,
        /// Frame count the worker claimed.
        reported_frames: u32,
        /// Sample rate the file actually carries.
        written_sample_rate: u32,
        /// Channel count the file actually carries.
        written_channels: u16,
        /// Frame count the file actually carries.
        written_frames: u32,
    },

    /// A segment's trailing pause is too long to express as a frame count.
    ///
    /// A guard rather than a condition a caller should expect: at the canonical
    /// 24 kHz no `u32` pause can overflow the frame count, and `pause_frames`
    /// in `assembly` records the arithmetic and what would have to change for
    /// this to become reachable.
    #[error(
        "the pause of {pause_after_ms} ms after segment `{segment_id}` overflows the frame count \
         this build can assemble; shorten the pause in the lesson"
    )]
    PauseFrameOverflow {
        /// The segment carrying the pause.
        segment_id: String,
        /// The pause as the lesson declares it, in milliseconds.
        pause_after_ms: u32,
    },

    /// The planned lesson is longer than the build can represent, before any
    /// audio was written.
    ///
    /// No plan reaches it: overflow needs 171,798,691 segments with every
    /// field at its `u32` maximum, and four billion under the pause cap
    /// `study_tts_core` enforces. Kept because those are properties of the
    /// current field widths rather than of the lesson format; `expected_frames`
    /// in `assembly` records the arithmetic.
    #[error(
        "the planned lesson exceeds the frame count this build can assemble; split the lesson \
         into shorter lessons"
    )]
    PlannedLengthOverflow,

    /// The master grew past what the build can count while it was being
    /// written.
    ///
    /// No input reaches either of the two checks that raise it: one counts a
    /// single WAV, whose length a data chunk declares in 32 bits, and the other
    /// re-sums totals the pre-pass already summed without overflow. Both are
    /// checked so the write loop stands on its own arithmetic.
    #[error(
        "assembling `{destination}` exceeded the frame count this build can track; split the \
         lesson into shorter lessons"
    )]
    AssembledLengthOverflow {
        /// The master being assembled.
        destination: PathBuf,
    },

    /// The master's length disagrees with the length its validated cache
    /// metadata implies.
    ///
    /// Redundant while every per-segment check passes, and retained because it
    /// is the invariant the manifest and every downstream duration derive from.
    /// Routing: "State or checksum corruption → Refuse overwrite; run
    /// reconciliation → Runtime → Blocked".
    #[error(
        "assembled master `{destination}` contains {assembled} frames but the plan requires \
         {expected}; the runtime owner must reconcile the cache before this lesson is rebuilt"
    )]
    AssembledLengthMismatch {
        /// The master whose length disagrees with the plan.
        destination: PathBuf,
        /// Frames actually written.
        assembled: u64,
        /// Frames the validated cache metadata implies.
        expected: u64,
    },

    /// Every output this crate writes is staged in its destination's parent and
    /// renamed into place, so a destination with no parent cannot be written
    /// atomically at all.
    #[error(
        "cannot write `{path}` atomically because it has no parent directory; supply an output \
         path with a directory component"
    )]
    UnrootedDestination {
        /// The destination that has no parent directory to stage into.
        path: PathBuf,
    },

    /// FFmpeg ran and exited non-zero.
    #[error("FFmpeg failed with status {status}: {stderr}")]
    Ffmpeg {
        /// Exit status FFmpeg reported.
        status: String,
        /// What FFmpeg wrote to standard error, trimmed.
        stderr: String,
    },

    /// FFmpeg could not be launched at all, which is distinct from its running
    /// and failing.
    #[error("could not start FFmpeg `{executable}`: {source}")]
    StartFfmpeg {
        /// The executable that could not be started.
        executable: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },

    /// Preflight could not resolve a required external tool, before any work
    /// began.
    #[error("required tool {tool} was not found or is not executable at `{requested}`")]
    MissingTool {
        /// Which tool is required.
        tool: String,
        /// The path the build was told to use.
        requested: PathBuf,
    },

    /// A required tool exists but could not be inspected, so its identity
    /// cannot be recorded.
    #[error("could not inspect {tool} at `{executable}`: {source}")]
    InspectTool {
        /// Which tool was being inspected.
        tool: String,
        /// The resolved executable.
        executable: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },

    /// A tool ran but would not report its version, so the manifest cannot
    /// record what was used.
    #[error("{tool} version probe failed with status {status}: {stderr}")]
    ToolProbeFailed {
        /// Which tool was probed.
        tool: String,
        /// Exit status the probe returned.
        status: String,
        /// What the probe wrote to standard error.
        stderr: String,
    },

    /// ffprobe ran and exited non-zero while validating an encoded output.
    #[error("ffprobe failed with status {status}: {stderr}")]
    Ffprobe {
        /// Exit status ffprobe reported.
        status: String,
        /// What ffprobe wrote to standard error, trimmed.
        stderr: String,
    },

    /// ffprobe exited successfully but its response could not be read.
    ///
    /// Distinct from `UnexpectedEncodedStream`: nothing is known about the
    /// output yet, so the encode is unverified rather than wrong, and the fault
    /// is in the probe or its version rather than in the file.
    #[error(
        "encoded output `{path}` is unverified: ffprobe returned a response this build could \
         not read ({source}); the audio owner must reconcile the ffprobe version with the \
         pinned probe arguments"
    )]
    UnreadableProbeResponse {
        /// The output that could not be verified.
        path: PathBuf,
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// The output holds a number of streams other than the one this build
    /// writes.
    ///
    /// Distinct from `UnexpectedEncodedStream`, which describes a stream
    /// that is present and wrong. Here the count itself is wrong, and a
    /// per-stream description would have to pick one arbitrarily — which is
    /// exactly how a second stream went unnoticed while the first one looked
    /// correct.
    #[error(
        "encoded output `{path}` holds {found} streams, not {required}; the encode settings \
         and this verification must agree before the output is used"
    )]
    UnexpectedEncodedStreamCount {
        /// The output that failed verification.
        path: PathBuf,
        /// Streams ffprobe reported.
        found: usize,
        /// Streams this build writes.
        required: usize,
    },

    /// The probe was read and describes something other than the stream this
    /// build produces.
    ///
    /// Routing: "Invalid or over-range audio → Audio/runtime → Blocked for
    /// segment". The encode settings and this check are two ends of one
    /// agreement, so a mismatch means one of them moved.
    #[error(
        "encoded output `{path}` is not the stream this build produces: ffprobe reports codec \
         `{}` with `{}` channels, not {required_channels}-channel `{required_codec}`; the \
         encode settings and this verification must agree before the output is used",
        codec.as_deref().unwrap_or("none"),
        channels.map_or_else(|| "none".to_owned(), |count| count.to_string())
    )]
    UnexpectedEncodedStream {
        /// The output that failed verification.
        path: PathBuf,
        /// Codec ffprobe reported, absent if the output declares no audio
        /// stream.
        codec: Option<String>,
        /// Channel count ffprobe reported, absent on the same terms.
        channels: Option<u16>,
        /// Codec this build encodes to.
        required_codec: &'static str,
        /// Channel count this build encodes to.
        required_channels: u16,
    },

    /// A name the build offered a managed helper is not exactly one ordinary
    /// path element, so nothing is joined, inspected, or created.
    ///
    /// Distinct from [`BuildError::ManagedPathEscape`] because the two are
    /// different faults. Most of what this catches — `""`, `"./name"`,
    /// `"name/"` — reaches a file this crate did not choose to name without
    /// ever leaving the managed root; `".."` and an absolute name would leave
    /// it, but are refused lexically here rather than by a containment check.
    /// Reporting either as an escape tells an operator their workspace was
    /// attacked when the fault is a caller passing a spelling the contract does
    /// not allow.
    ///
    /// Routing per `docs/governance/ROUTING-TABLES.md` ("Worker protocol or
    /// containment failure → Worker/runtime → Blocked"): the runtime owner
    /// corrects the caller. Nothing on disk is touched, so there is nothing for
    /// an operator to repair.
    #[error(
        "managed name `{name}` beneath `{root}` is not one ordinary path element; the build \
         created nothing, and the runtime owner must correct the caller that supplied it"
    )]
    InvalidManagedName {
        /// The name that was refused.
        name: String,
        /// The managed directory the name would have been joined beneath.
        root: PathBuf,
    },

    /// A path the build derived would leave the root it is confined to, so
    /// nothing is written.
    ///
    /// Reserved for a path or a link that can actually escape: a resolved path
    /// that is no longer beneath its root, or a symlink occupying a managed
    /// name. A name that is merely spelled wrong is
    /// [`BuildError::InvalidManagedName`].
    #[error("managed path `{path}` resolves outside `{root}`")]
    ManagedPathEscape {
        /// The path that resolved outside its root.
        path: PathBuf,
        /// The root the build is confined to.
        root: PathBuf,
    },

    /// A release profile refused the claim made on it.
    ///
    /// Transparent because `ReleaseError` already names the profile rule that
    /// refused, and restating it here would be a second copy of a policy the
    /// core crate owns.
    #[error(transparent)]
    Release(#[from] study_tts_core::ReleaseError),

    /// A manifest was offered for publication under a version this build cannot
    /// evaluate.
    #[error("manifest version `{version}` is not a production manifest")]
    UnsupportedProductionManifest {
        /// The version the manifest declares.
        version: String,
    },

    /// The manifest is not JSON, or not the shape its declared version
    /// requires.
    ///
    /// Named for the manifest rather than for JSON, because an operator reading
    /// "JSON operation failed" would not learn that publication refused their
    /// manifest. An unknown top-level field lands here too, because a field
    /// this build cannot evaluate is one it must not publish past.
    #[error(
        "production release is refused: the manifest is not a valid production manifest \
         ({source}); the project owner must correct the manifest before publication"
    )]
    MalformedProductionManifest {
        /// What the parser reported.
        source: serde_json::Error,
    },

    /// The manifest asks to be published while declaring itself something
    /// other than a production release.
    ///
    /// Separate from `ProductionGatesUnavailable`, which is where a manifest
    /// that does claim production release lands once its rights preconditions
    /// pass. A preview manifest never made the claim, so reporting missing
    /// gates would name work nobody asked for. The status is what `manifest.rs`
    /// writes as a typed `ReleaseStatus`, and only a build that earned the
    /// production profile may carry it.
    #[error(
        "production release is refused: the manifest declares release status `{}`, not \
         `production_release`; the project owner must publish the manifest of a build that \
         earned the production profile rather than restate the status",
        declared.as_str()
    )]
    ManifestNotProductionRelease {
        /// The status the manifest declares.
        declared: ReleaseStatus,
    },

    /// The manifest names no classified source at all.
    ///
    /// Separate from `UnresolvedContentRights`, which is a source whose
    /// classification was recorded and does not permit release. Declaring
    /// nothing is not the same as declaring something unresolved, and the
    /// remedies differ.
    #[error(
        "production release is refused: the manifest declares no `content_rights` \
         classification for its sources; the project owner must classify every source in its \
         rights record before publication"
    )]
    MissingContentRightsDeclaration,

    /// The manifest names no voice profile at all.
    ///
    /// Separate from `ProfileNotApproved`, which is a profile whose rights
    /// decision was recorded and does not permit release. Declaring no voice is
    /// not the same as declaring an unapproved one, and the remedies differ, so
    /// this is the `content_rights` refusal above applied to the other half of
    /// the same rule: `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
    /// ("Generated release") requires source *and* voice record IDs, and every
    /// artifact this project releases is spoken by at least one voice.
    #[error(
        "production release is refused: the manifest declares no `voice_profiles` for the \
         voices it was rendered with; the project owner must declare each voice profile and \
         its rights record before publication"
    )]
    MissingVoiceProfileDeclaration,

    /// A declaration names an identifier but leaves it blank.
    ///
    /// Distinct from an absent field, which serde refuses outright: a blank
    /// identifier parses and then traces to no record, so it would satisfy a
    /// rights gate while naming nothing.
    #[error(
        "production release is refused: the `{section}` manifest section declares an empty \
         `{field}`; the project owner must name it before publication"
    )]
    EmptyManifestIdentifier {
        /// The manifest section holding the declaration.
        section: &'static str,
        /// The identifier field left blank.
        field: &'static str,
    },

    /// The manifest satisfied every rights precondition this build can check,
    /// and the release gates that would decide the rest do not exist yet.
    ///
    /// Separate from `Release`, which refuses a claim on its profile. A
    /// manifest that passed its rights checks is not reported the same way as
    /// an artifact that never earned the profile. Reaching this is the closest
    /// a manifest can currently come to acceptance.
    #[error(
        "production release is refused: manifest acceptance is unavailable before the \
         production gates of `docs/governance/RELEASE-PROFILES.md` §3 are implemented"
    )]
    ProductionGatesUnavailable,

    /// A record could not be serialized to its destination.
    #[error("could not write JSON to `{path}`: {source}")]
    WriteJson {
        /// The record being written.
        path: PathBuf,
        /// What the serializer reported.
        source: serde_json::Error,
    },
}

/// Which invariant a published cache entry violated.
///
/// Nested inside [`BuildError::UnusableCacheEntry`] rather than flattened into
/// `BuildError`: the entry directory and the deletion remedy are the same for
/// every one of these, and stating them once keeps the remedy from drifting
/// between rejection sites. What differs is the invariant, and that is exactly
/// what this enum names.
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

    /// The entry belongs to a different synthesis identity, so its audio is not
    /// this segment's.
    #[error("the artifact records cache key `{recorded}` but the plan requires `{required}`")]
    CacheKeyMismatch {
        /// The identity the artifact records.
        recorded: CacheKey,
        /// The identity the current plan requires.
        required: CacheKey,
    },

    /// Raised both when the entry is loaded and when the master is assembled
    /// from it, because a truncated entry and a truncated read are the same
    /// violated invariant.
    #[error("the audio holds {found} frames but the artifact declares {declared}")]
    FrameCountMismatch {
        /// Frames the audio actually holds.
        found: u64,
        /// Frames the artifact declares.
        declared: u32,
    },

    /// Distinct from `ChecksumMismatch` on purpose: a malformed record reported
    /// as a mismatch tells the operator their audio was tampered with when the
    /// artifact is what broke.
    #[error(
        "the artifact records `{recorded}` as the audio digest, which is not a lowercase BLAKE3 \
         hex digest and so could never match the audio"
    )]
    MalformedRecordedDigest {
        /// The value the artifact records where a digest belongs.
        recorded: String,
    },

    /// The audio hashes to something other than what the artifact records.
    #[error("the audio checksum `{found}` does not match the artifact record `{declared}`")]
    ChecksumMismatch {
        /// Digest computed from the audio now.
        found: String,
        /// Digest the artifact records.
        declared: String,
    },

    /// The entry's audio failed validation; the inner fault names which
    /// property.
    #[error("{0}")]
    Audio(#[from] AudioFault),
}

/// Why a WAV cannot serve as canonical lesson audio.
///
/// Carries no path and no remedy: the same validation runs on a staged file
/// that no operator can act on and on a published entry that they can, and only
/// the caller knows which. Attaching the remedy here is what previously
/// produced "delete this entry" for a file that was about to be discarded
/// anyway.
#[derive(Debug, Error)]
pub enum AudioFault {
    /// The file is not readable as WAV at all.
    #[error("it could not be read as WAV ({0})")]
    Unreadable(#[from] hound::Error),

    /// The stream is readable but is not the one canonical format.
    #[error(
        "the stream is {channels}-channel {sample_rate} Hz {bits_per_sample}-bit \
         {sample_format}, not canonical {required_channels}-channel \
         {required_sample_rate} Hz {required_bits_per_sample}-bit float"
    )]
    NonCanonical {
        /// Channel count the stream carries.
        channels: u16,
        /// Sample rate the stream carries, in hertz.
        sample_rate: u32,
        /// Bit depth the stream carries.
        bits_per_sample: u16,
        /// Whether the stream is integer or float.
        sample_format: &'static str,
        /// The one channel count this project accepts.
        required_channels: u16,
        /// The one sample rate this project accepts, in hertz.
        required_sample_rate: u32,
        /// The one bit depth this project accepts.
        required_bits_per_sample: u16,
    },

    /// A sample is infinite, NaN, or beyond full scale, which would clip on
    /// export.
    #[error("sample {index} is `{value}`, outside the finite range -1.0 to 1.0")]
    OutOfRangeSample {
        /// Zero-based frame the bad sample sits at.
        index: u32,
        /// The offending value.
        value: f32,
    },

    /// The file is a valid WAV holding no audio, which no segment may be.
    #[error("it contains no audio frames")]
    Empty,

    /// The file holds more frames than the frame counter can represent.
    ///
    /// Not reachable through a WAV: the ceiling is about 17 GB of sample data,
    /// four times what a data chunk can declare. Kept because the counter is
    /// the `u32` the artifact record carries, so a container without that cap
    /// would wrap here first.
    #[error("it holds more frames than this build can count")]
    FrameCountOverflow,
}

/// `io::Error` carries no filename, so every filesystem failure must be given
/// its path here or the message names nothing.
pub(crate) fn io_error(path: impl Into<PathBuf>, source: io::Error) -> BuildError {
    BuildError::FileSystem {
        path: path.into(),
        source,
    }
}

/// `hound::Error` wraps `io::Error`, which carries no filename, so every audio
/// failure must be given its path here or the message names nothing.
pub(crate) fn audio_error(path: impl Into<PathBuf>, source: hound::Error) -> BuildError {
    BuildError::AudioAt {
        path: path.into(),
        source,
    }
}
