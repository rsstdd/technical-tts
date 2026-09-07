//! Encoding the master WAV to M4A and MP3 through FFmpeg, and verifying the
//! master and both exports with ffprobe.
//!
//! The codec, channel count, and stream count each artifact carries are named
//! once and used by both ends: the encode maps them and the probe verifies
//! them, so the verification cannot drift into passing something the encoder
//! no longer writes. Both exports are encoded from the master and never from
//! each other, which [`EncodedFormat`] makes a property of the type rather than
//! of a call site remembering to pass the right path.
//!
//! `ProbeResponse` is the one deserialization boundary in this workspace
//! without `deny_unknown_fields`. It parses a diagnostic tool's output rather
//! than a format this project defines, and the exception is argued and tested
//! beside the type itself.

use std::{ffi::OsString, path::Path, process::Command};

use serde::{Deserialize, Serialize};
use study_tts_core::{CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE, ToolProfileHash};
use tempfile::Builder;

use crate::{
    BuildError, ManagedPathError, ToolError, ToolInvocation, ToolOperation, audio_error, io_error,
    process::{self, CommandRunError, FFMPEG_ENCODE_POLICY, FFPROBE_POLICY, VERSION_PROBE_POLICY},
    tools::ToolIdentity,
};

/// The audio codec the M4A export carries.
///
/// One definition for both ends of the agreement: the FFmpeg profile encodes
/// to it and [`PackagedAudio::codec`] verifies it. Two literals could drift
/// apart silently, leaving verification passing something the encoder no longer
/// produces.
const M4A_CODEC: &str = "aac";

/// The audio codec the MP3 export carries, on the same terms.
///
/// The encoder is `libmp3lame` and the codec ffprobe reports for what it writes
/// is `mp3`; the two are different names for the two ends of one agreement and
/// neither can be derived from the other.
const MP3_CODEC: &str = "mp3";

/// The FFmpeg encoder that produces [`MP3_CODEC`].
///
/// Named separately because this is the token `preflight_encoder` looks for in
/// `ffmpeg -encoders`: an FFmpeg built without it encodes no MP3, and a build
/// that discovered that after synthesis would have wasted the whole render.
pub(crate) const MP3_ENCODER: &str = "libmp3lame";

/// The sample format the canonical master carries, as ffprobe names it.
///
/// `cache` and `assembly` already hold the master to
/// [`study_tts_core::CANONICAL_SAMPLE_FORMAT`]; this is the same requirement
/// stated in ffprobe's vocabulary, so the structural validation of the master
/// is performed by a decoder this build did not write.
const MASTER_WAV_CODEC: &str = "pcm_f32le";

/// The channel count every encoded output carries, on the same terms.
///
/// Derived from [`CANONICAL_CHANNELS`] rather than repeating its value: the
/// export carries the master's channel layout, so the two cannot drift apart.
const REQUIRED_CHANNELS: u16 = CANONICAL_CHANNELS;

/// The channel layout FFmpeg is told to write, which must describe
/// [`REQUIRED_CHANNELS`].
///
/// FFmpeg names layouts rather than counting them, so this cannot be derived
/// from the count the way `-ac` is. The assertion is what keeps the two from
/// drifting: a changed canonical channel count becomes a compile error here
/// rather than an `-ac 2 -channel_layout mono` contradiction FFmpeg would be
/// left to resolve on its own.
const REQUIRED_CHANNEL_LAYOUT: &str = "mono";
const _: () = assert!(REQUIRED_CHANNELS == 1);

/// Streams every encoded output carries.
///
/// The encode maps one audio stream and strips video and metadata, so anything
/// else in the container did not come from this build.
const REQUIRED_STREAMS: usize = 1;

const INPUT_PATH_ARGUMENT: &str = "{input_path}";
const OUTPUT_PATH_ARGUMENT: &str = "{output_path}";

/// Placeholder for the measurements the first loudness pass produced.
///
/// Substituted *inside* an argument rather than replacing a whole one, which is
/// the only substring placeholder here and is deliberate: FFmpeg takes a filter
/// graph as a single `-af` argument, so the targets and the measurements share
/// one token. Keeping the targets literal in the profile is what puts them in
/// the hashed identity; keeping the measurements behind this placeholder is
/// what keeps them out of it. They are this lesson's data, not a setting, and a
/// profile digest that moved per lesson would make package reuse impossible.
const MEASURED_LOUDNESS_ARGUMENT: &str = "{measured_loudness}";

/// Provisional integrated-loudness target for the master, in LUFS.
///
/// `ADR-0001-D012` under `docs/adr/deviations/` authorizes this value and names
/// this constant; ADR-0003's calibration table owns the frozen one.
///
/// **Not a delivery reference.** It is bounded by the voice, not chosen for a
/// listener: `owner-fallback-v1`'s conditioning reference is itself about
/// -39 LUFS, Chatterbox clones level from its reference, and the resulting
/// master peaks around -10 dBTP. Reaching an ordinary -16 LUFS would need more
/// gain than [`PROVISIONAL_TRUE_PEAK_CEILING_DBTP`] leaves, and gain cannot buy
/// it because gain preserves crest factor. D012's 2026-09-06 amendment records
/// the measurements. A louder reference is the actual remedy and belongs to
/// ADR-0003's per-voice calibration.
const PROVISIONAL_LOUDNESS_TARGET_LUFS: &str = "-27.0";

/// Provisional true-peak ceiling for the master, in dBTP.
///
/// Authorized by `ADR-0001-D012` on the same terms. The headroom is for the
/// lossy encodes specifically: `lesson.m4a` and `lesson.mp3` derive from this
/// master, and inter-sample peaks inaudible in float PCM clip once encoded.
const PROVISIONAL_TRUE_PEAK_CEILING_DBTP: &str = "-1.0";

/// Loudness-range target, in LU.
///
/// FFmpeg's own default, verified against `ffmpeg -h filter=loudnorm` rather
/// than recalled: `LRA ... (from 1 to 50) (default 7)`. Kept rather than
/// chosen, which is why it is the one loudness value `ADR-0001-D012` does not
/// authorize and does not need to — the record covers the two targets this
/// build picks, and a range this build invented would be a third uncalibrated
/// number with no better provenance than the filter's own.
const PROVISIONAL_LOUDNESS_RANGE_LU: &str = "7.0";

/// Which of the two passes a filter graph is being assembled for.
///
/// A closed pair rather than a string fragment the caller appends. The settings
/// are colon-separated, so a caller composing its own tail has to remember a
/// separator the compiler cannot check — `"linear=true"` without one silently
/// yields `linear=trueprint_format=json`, a filter FFmpeg only rejects at run
/// time. [`loudnorm_filter`] joins the parts instead, which makes that
/// unrepresentable rather than documented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoudnessPass {
    /// Measure the input and write no audio.
    Measure,
    /// Normalize against what [`LoudnessPass::Measure`] reported.
    Apply,
}

/// The loudness filter graph, with the shared targets written once.
///
/// Assembled rather than written out as a literal, unlike the encode profiles:
/// there the two lists differ and each is read against the manifest it
/// produces, whereas here the two passes must carry *identical* targets or the
/// second pass normalizes to a level the first never measured against. A
/// duplicated literal is exactly how those two would drift apart.
fn loudnorm_filter(pass: LoudnessPass) -> String {
    let mut settings = vec![
        format!("I={PROVISIONAL_LOUDNESS_TARGET_LUFS}"),
        format!("TP={PROVISIONAL_TRUE_PEAK_CEILING_DBTP}"),
        format!("LRA={PROVISIONAL_LOUDNESS_RANGE_LU}"),
    ];
    // Exhaustive rather than a comparison against one variant: a third pass
    // added later would otherwise take the measurement arm silently, which is
    // the class of mistake this enum exists to prevent.
    match pass {
        LoudnessPass::Measure => {}
        LoudnessPass::Apply => {
            settings.push(MEASURED_LOUDNESS_ARGUMENT.to_owned());
            settings.push("linear=true".to_owned());
        }
    }
    settings.push("print_format=json".to_owned());
    format!("loudnorm={}", settings.join(":"))
}

/// The measurement pass: analyze the master and write no audio.
///
/// `-f null -` because this pass exists for the JSON it prints to stderr; an
/// output file here would be a dynamically normalized master written before
/// anything has checked whether linear normalization is possible.
///
/// `-nostats` rather than the `-loglevel error` the encode profiles carry.
/// The filter prints its report at FFmpeg's info level, so silencing that level
/// would silence the report this pass exists to produce. Progress statistics
/// are the part that can be dropped, and dropping them is what keeps a long
/// lesson inside the standard-error ceiling `FFMPEG_ENCODE_POLICY` sets —
/// otherwise a five-minute render reports [`ToolError::ToolOutputOverflow`]
/// for having talked too much about its own progress.
fn loudnorm_measure_arguments() -> Vec<String> {
    vec![
        "-nostdin".to_owned(),
        "-hide_banner".to_owned(),
        "-nostats".to_owned(),
        "-i".to_owned(),
        INPUT_PATH_ARGUMENT.to_owned(),
        "-af".to_owned(),
        loudnorm_filter(LoudnessPass::Measure),
        "-f".to_owned(),
        "null".to_owned(),
        "-".to_owned(),
    ]
}

/// The application pass: normalize the master against what pass one measured.
///
/// `linear=true` requests a single gain change over the whole master rather
/// than a moving one. FFmpeg falls back to dynamic normalization when the
/// measurements make linear impossible, and reports which it used;
/// [`NormalizationOutcome`] is where that report is checked, because a master
/// whose gain moves under the audio is one whose segments no longer read as one
/// recording.
///
/// The output is written at the canonical master format explicitly. The filter
/// chain would otherwise resample to a default FFmpeg chooses, and a master
/// that changed rate or sample format here would be refused downstream by the
/// same canonical checks that admitted it.
///
/// `-nostats` for the reason [`loudnorm_measure_arguments`] gives: this pass
/// reports too, and it runs over the same audio for longer.
fn loudnorm_apply_arguments() -> Vec<String> {
    vec![
        "-nostdin".to_owned(),
        "-hide_banner".to_owned(),
        "-nostats".to_owned(),
        "-y".to_owned(),
        "-i".to_owned(),
        INPUT_PATH_ARGUMENT.to_owned(),
        "-af".to_owned(),
        loudnorm_filter(LoudnessPass::Apply),
        "-ar".to_owned(),
        CANONICAL_SAMPLE_RATE.to_string(),
        "-ac".to_owned(),
        REQUIRED_CHANNELS.to_string(),
        "-c:a".to_owned(),
        MASTER_WAV_CODEC.to_owned(),
        OUTPUT_PATH_ARGUMENT.to_owned(),
    ]
}

const FFMPEG_M4A_ARGUMENT_PROFILE: &[&str] = &[
    "-nostdin",
    "-hide_banner",
    "-loglevel",
    "error",
    "-y",
    "-i",
    INPUT_PATH_ARGUMENT,
    "-map_metadata",
    "-1",
    "-vn",
    "-ac",
    "1",
    "-channel_layout",
    REQUIRED_CHANNEL_LAYOUT,
    "-c:a",
    M4A_CODEC,
    "-b:a",
    "96k",
    OUTPUT_PATH_ARGUMENT,
];

/// The MP3 encode, which is the M4A profile with its codec and bitrate
/// swapped.
///
/// Deliberately not derived from [`FFMPEG_M4A_ARGUMENT_PROFILE`] by editing a
/// copy at run time: both lists are hashed into the tool-profile identities the
/// manifest records and package reuse compares, so each one is written out to
/// be read against the manifest it produces.
///
/// The codec and bitrate are **provisional**. ADR-0003 is `Proposed; awaiting
/// calibration` and records "MP3 codec arguments" as `Pending`, so this build
/// chooses a value no *audio* profile states, under the bounded permission in
/// `docs/adr/deviations/ADR-0001-D009-provisional-mp3-profile.md`, which is
/// approved, expires when ADR-0003 is accepted, and names this constant in
/// return.
const FFMPEG_MP3_ARGUMENT_PROFILE: &[&str] = &[
    "-nostdin",
    "-hide_banner",
    "-loglevel",
    "error",
    "-y",
    "-i",
    INPUT_PATH_ARGUMENT,
    "-map_metadata",
    "-1",
    "-vn",
    "-ac",
    "1",
    "-channel_layout",
    REQUIRED_CHANNEL_LAYOUT,
    "-c:a",
    MP3_ENCODER,
    "-b:a",
    "128k",
    OUTPUT_PATH_ARGUMENT,
];

/// The encoder inventory probe, which carries no path at all.
///
/// Preflight rather than lazy discovery, on the terms `tools.rs` already sets:
/// an FFmpeg with no MP3 encoder must be refused before this build synthesizes
/// anything, not after it has rendered every segment.
const FFMPEG_ENCODERS_ARGUMENT_PROFILE: &[&str] = &["-nostdin", "-hide_banner", "-encoders"];

const FFPROBE_ARGUMENT_PROFILE: &[&str] = &[
    "-v",
    "error",
    "-show_entries",
    "stream=codec_name,channels",
    "-of",
    "json",
    INPUT_PATH_ARGUMENT,
];

/// The subset of an ffprobe response the pinned `-show_entries` selection asks
/// for.
///
/// The one deserialization boundary here without `deny_unknown_fields`, which
/// `.claude/skills/rust-review/SKILL.md` requires of every other one. It is an
/// exception on the terms `PRINCIPLES.md` sets for them: narrow, explained
/// beside the suppression, and covered by a test.
///
/// Strictness here would guard nothing and break something. ffprobe already
/// emits an empty `programs` array under this selection, and the version used
/// is whichever one the operator has installed — `tools.rs` records it rather
/// than pinning it — so a release that adds a section would stop every build
/// with `UnreadableProbeResponse`, reporting a sound artifact as unverifiable.
/// The unknown field the rule exists to catch is one carrying meaning its
/// author intended and this program ignored; an extra ffprobe section is a
/// tool describing itself to no one in particular.
///
/// What makes the leniency safe is that it can only refuse, never accept.
/// Every field below is absent-or-wrong rather than defaulted, so a renamed or
/// withdrawn one reaches a refusal instead of a pass:
/// `t1_e0_probe_leniency_cannot_accept_an_unverified_stream` holds that, and
/// `t4_e0_ffprobe_rejects_non_aac_input` runs a real ffprobe so the accepted
/// shape is observed rather than assumed.
#[derive(Debug, Deserialize)]
struct ProbeResponse {
    #[serde(default)]
    streams: Vec<ProbeStream>,
}

/// One audio stream as ffprobe reports it. Both fields are optional because an
/// absent field and a wrong value are different findings.
#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_name: Option<String>,
    channels: Option<u16>,
}

/// What a tool was actually told to do, for the manifest to record.
///
/// The arguments as they were passed, not as they were composed: a manifest
/// that records an intended command line rather than the executed one cannot
/// be used to reproduce a build.
#[derive(Clone, Debug)]
pub(crate) struct ToolExecution {
    /// The argument list the tool was invoked with, in order.
    pub arguments: Vec<String>,
    /// Digest of the path-normalized argument profile that produced the list.
    pub argument_profile_blake3: ToolProfileHash,
}

/// Path-normalized FFmpeg and ffprobe argument identities for one build.
///
/// One ffprobe profile covers all three validations because the probe asks the
/// same question of every artifact and differs only in the path it is handed;
/// the encodes differ in their arguments, so each carries its own identity.
#[derive(Clone, Debug)]
pub(crate) struct ExportProfiles {
    /// M4A encoding argument identity.
    pub ffmpeg_m4a: ToolProfile,
    /// MP3 encoding argument identity.
    pub ffmpeg_mp3: ToolProfile,
    /// Encoder-inventory preflight argument identity.
    pub ffmpeg_encoders: ToolProfile,
    /// Loudness measurement-pass argument identity.
    pub ffmpeg_loudnorm_measure: ToolProfile,
    /// Loudness application-pass argument identity.
    pub ffmpeg_loudnorm_apply: ToolProfile,
    /// Probe argument identity.
    pub ffprobe: ToolProfile,
}

impl ExportProfiles {
    /// Every argument identity this build can record, in a stable order.
    ///
    /// Package reuse compares this set rather than a per-tool pair, and
    /// `preview::transaction_identity` hashes it: adding a format changes the
    /// set, so a package written before that format existed stops matching and
    /// is rebuilt rather than silently reused as complete.
    ///
    /// Destructured rather than read field by field, and with no `..`: a
    /// profile added to this struct then fails to compile here instead of
    /// being quietly absent from both the reuse comparison and the transaction
    /// identity, which is the one failure this list exists to prevent.
    pub(crate) fn identities(&self) -> [&ToolProfileHash; 6] {
        let Self {
            ffmpeg_m4a,
            ffmpeg_mp3,
            ffmpeg_encoders,
            ffmpeg_loudnorm_measure,
            ffmpeg_loudnorm_apply,
            ffprobe,
        } = self;
        [
            ffmpeg_m4a.identity(),
            ffmpeg_mp3.identity(),
            ffmpeg_encoders.identity(),
            ffmpeg_loudnorm_measure.identity(),
            ffmpeg_loudnorm_apply.identity(),
            ffprobe.identity(),
        ]
    }
}

/// One audio artifact this build both produces and structurally validates.
///
/// The master is in this enum and not in [`EncodedFormat`] because Rust writes
/// it and FFmpeg does not: the two enums exist so that "encode the master WAV"
/// cannot be spelled at all, rather than being spelled and then refused.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PackagedAudio {
    /// The canonical lossless master `assembly` writes.
    MasterWav,
    /// The M4A derived from the master.
    M4a,
    /// The MP3 derived from the master.
    Mp3,
}

impl PackagedAudio {
    /// The codec ffprobe must report for this artifact.
    pub(crate) fn codec(self) -> &'static str {
        match self {
            Self::MasterWav => MASTER_WAV_CODEC,
            Self::M4a => M4A_CODEC,
            Self::Mp3 => MP3_CODEC,
        }
    }

    /// Which validation a supervision failure should name.
    fn validation(self) -> ToolOperation {
        match self {
            Self::MasterWav => ToolOperation::MasterWavValidation,
            Self::M4a => ToolOperation::M4aValidation,
            Self::Mp3 => ToolOperation::Mp3Validation,
        }
    }
}

/// One lossy format derived independently from the canonical master.
///
/// ADR-0001 §13.5 requires both exports to come from the master rather than
/// from each other, and `encode` taking the master path is what makes a
/// lossy-to-lossy chain unspellable here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EncodedFormat {
    /// The default listening file.
    M4a,
    /// The compatibility output.
    Mp3,
}

impl EncodedFormat {
    /// The artifact this format becomes once encoded.
    pub(crate) fn packaged(self) -> PackagedAudio {
        match self {
            Self::M4a => PackagedAudio::M4a,
            Self::Mp3 => PackagedAudio::Mp3,
        }
    }

    /// The argument identity that produces this format.
    fn profile(self, profiles: &ExportProfiles) -> &ToolProfile {
        match self {
            Self::M4a => &profiles.ffmpeg_m4a,
            Self::Mp3 => &profiles.ffmpeg_mp3,
        }
    }

    /// Which encode a supervision failure should name.
    fn encode(self) -> ToolOperation {
        match self {
            Self::M4a => ToolOperation::M4aEncode,
            Self::Mp3 => ToolOperation::Mp3Encode,
        }
    }

    /// The suffix the staged file carries while FFmpeg is writing it.
    ///
    /// FFmpeg selects its muxer from the output extension, so a staged file
    /// named without one would be encoded as something else entirely.
    fn staging_suffix(self) -> &'static str {
        match self {
            Self::M4a => ".m4a",
            Self::Mp3 => ".mp3",
        }
    }
}

/// One tool's normalized argument sequence and deterministic identity.
#[derive(Clone, Debug)]
pub(crate) struct ToolProfile {
    normalized_arguments: Vec<String>,
    identity: ToolProfileHash,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct ProfileIdentity<'a> {
    identity_version: &'static str,
    tool: &'a str,
    normalized_arguments: &'a [String],
}

impl ToolProfile {
    /// Derives an identity from a path-normalized argument sequence.
    pub(crate) fn new(tool: &str, arguments: &[&str]) -> Self {
        Self::owned(
            tool,
            arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        )
    }

    /// [`ToolProfile::new`] for a sequence assembled rather than written out.
    ///
    /// The loudness profiles share their targets through
    /// [`loudnorm_filter`], so they arrive owned; hashing them through the same
    /// function is what keeps one identity rule for every profile.
    pub(crate) fn owned(tool: &str, normalized_arguments: Vec<String>) -> Self {
        let bytes = serde_json::to_vec(&ProfileIdentity {
            identity_version: "0.1-skeleton-tool-profile",
            tool,
            normalized_arguments: &normalized_arguments,
        })
        .expect("tool profiles contain only infallibly serializable values");
        let identity = ToolProfileHash::from(blake3::hash(&bytes));
        Self {
            normalized_arguments,
            identity,
        }
    }

    /// Returns the deterministic path-normalized argument identity.
    pub(crate) fn identity(&self) -> &ToolProfileHash {
        &self.identity
    }
}

/// Returns the command profiles that define this build's export behavior.
pub(crate) fn export_profiles() -> ExportProfiles {
    ExportProfiles {
        ffmpeg_m4a: ToolProfile::new("ffmpeg", FFMPEG_M4A_ARGUMENT_PROFILE),
        ffmpeg_mp3: ToolProfile::new("ffmpeg", FFMPEG_MP3_ARGUMENT_PROFILE),
        ffmpeg_encoders: ToolProfile::new("ffmpeg", FFMPEG_ENCODERS_ARGUMENT_PROFILE),
        ffmpeg_loudnorm_measure: ToolProfile::owned("ffmpeg", loudnorm_measure_arguments()),
        ffmpeg_loudnorm_apply: ToolProfile::owned("ffmpeg", loudnorm_apply_arguments()),
        ffprobe: ToolProfile::new("ffprobe", FFPROBE_ARGUMENT_PROFILE),
    }
}

/// Encodes the master WAV to one lossy format, returning what FFmpeg was told
/// to do.
///
/// Encoded to a staged path beside the destination and renamed on success, so a
/// failed encode never leaves a partial export for a later step to find. The
/// staging guard is held across the encode rather than released once it has
/// reserved a name: FFmpeg opens its output container before it can discover
/// it cannot finish, so a failure after that point leaves a partial file that
/// only the guard's drop removes. Previews are what an operator listens
/// through, and one that accumulates a partial export per failed encode stops
/// being a record of what the build produced.
///
/// A process killed outright still leaves the staged file because no drop runs.
/// The next preview reconciliation quarantines its incomplete package stage.
///
/// # Errors
///
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent;
/// [`ToolError::StartFfmpeg`] when the binary cannot be launched;
/// [`ToolError::ToolTimedOut`] when the encode deadline expires;
/// [`ToolError::ToolOutputOverflow`] when either output stream exceeds its
/// ceiling; [`ToolError::ToolPipeUnavailable`],
/// [`ToolError::ToolCaptureConfigurationFailed`],
/// [`ToolError::ToolCaptureStartFailed`],
/// [`ToolError::ToolCaptureReadFailed`],
/// [`ToolError::ToolCaptureChannelClosed`],
/// [`ToolError::ToolCaptureThreadPanicked`],
/// [`ToolError::ToolCaptureShutdownTimedOut`],
/// [`ToolError::ToolCaptureIncomplete`],
/// [`ToolError::ToolCleanupFailed`],
/// [`ToolError::ToolChildInspectionFailed`],
/// [`ToolError::ToolTerminationSignalFailed`],
/// [`ToolError::ToolContainmentInspectionFailed`],
/// [`ToolError::ToolContainmentSignalFailed`],
/// [`ToolError::ToolChildReapFailed`],
/// [`ToolError::ToolTerminationTimedOut`],
/// [`ToolError::ToolReaperStartFailed`], or
/// [`ToolError::ToolCaptureReaperStartFailed`] when the named supervision
/// invariant fails;
/// [`ToolError::Ffmpeg`] carrying the status and stderr when it runs and fails;
/// otherwise [`crate::IoError::FileSystem`].
pub(crate) fn encode(
    ffmpeg: &ToolIdentity,
    profiles: &ExportProfiles,
    format: EncodedFormat,
    master_wav: &Path,
    destination: &Path,
) -> Result<ToolExecution, BuildError> {
    let profile = format.profile(profiles);
    let parent = destination
        .parent()
        .ok_or_else(|| ManagedPathError::UnrootedDestination {
            path: destination.to_path_buf(),
        })?;
    // The handle is closed but the path is kept, because FFmpeg writes the
    // file itself rather than through a handle this process holds. Dropping the
    // whole `NamedTempFile` here would take the cleanup with it.
    let staged = Builder::new()
        .prefix("lesson-")
        .suffix(format.staging_suffix())
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?
        .into_temp_path();

    let arguments = materialize_arguments(profile, master_wav, Some(&staged));
    let mut command = Command::new(&ffmpeg.resolved_executable);
    command.args(&arguments);
    let invocation = ToolInvocation::new("FFmpeg", format.encode(), destination);
    let output =
        process::run(invocation, command, FFMPEG_ENCODE_POLICY).map_err(|error| match error {
            CommandRunError::Start(source) => ToolError::StartFfmpeg {
                executable: ffmpeg.resolved_executable.clone(),
                source,
            },
            CommandRunError::Supervision(error) => error,
        })?;
    if !output.status.success() {
        return Err(ToolError::Ffmpeg {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .into());
    }
    staged
        .persist(destination)
        .map_err(|error| io_error(destination, error.error))?;
    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
        argument_profile_blake3: profile.identity().clone(),
    })
}

/// Verifies an encoded output against what this build claims to produce.
///
/// Counts the streams rather than sampling the first one: a second stream is
/// invisible to a probe that asks only about stream zero, and stream zero is
/// the one this build writes correctly.
///
/// # Errors
///
/// [`ToolError::InspectTool`] when ffprobe cannot be launched;
/// [`ToolError::ToolTimedOut`] when its deadline expires;
/// [`ToolError::ToolOutputOverflow`] when either output stream exceeds its
/// ceiling; [`ToolError::ToolPipeUnavailable`],
/// [`ToolError::ToolCaptureConfigurationFailed`],
/// [`ToolError::ToolCaptureStartFailed`],
/// [`ToolError::ToolCaptureReadFailed`],
/// [`ToolError::ToolCaptureChannelClosed`],
/// [`ToolError::ToolCaptureThreadPanicked`],
/// [`ToolError::ToolCaptureShutdownTimedOut`],
/// [`ToolError::ToolCaptureIncomplete`],
/// [`ToolError::ToolCleanupFailed`],
/// [`ToolError::ToolChildInspectionFailed`],
/// [`ToolError::ToolTerminationSignalFailed`],
/// [`ToolError::ToolContainmentInspectionFailed`],
/// [`ToolError::ToolContainmentSignalFailed`],
/// [`ToolError::ToolChildReapFailed`],
/// [`ToolError::ToolTerminationTimedOut`],
/// [`ToolError::ToolReaperStartFailed`], or
/// [`ToolError::ToolCaptureReaperStartFailed`] when the named supervision
/// invariant fails;
/// [`ToolError::Ffprobe`] when it runs and fails;
/// [`ToolError::UnreadableProbeResponse`] when its output cannot be parsed;
/// [`ToolError::UnexpectedEncodedStreamCount`] when the stream count differs;
/// or [`ToolError::UnexpectedEncodedStream`] when the codec or channel count is
/// not the one this build encodes to.
pub(crate) fn probe(
    ffprobe: &ToolIdentity,
    profile: &ToolProfile,
    artifact: PackagedAudio,
    path: &Path,
) -> Result<ToolExecution, BuildError> {
    let arguments = materialize_arguments(profile, path, None);
    let mut command = Command::new(&ffprobe.resolved_executable);
    command.args(&arguments);
    let invocation = ToolInvocation::new("ffprobe", artifact.validation(), path);
    let output =
        process::run(invocation, command, FFPROBE_POLICY).map_err(|error| match error {
            CommandRunError::Start(source) => ToolError::InspectTool {
                tool: "ffprobe".to_owned(),
                executable: ffprobe.resolved_executable.clone(),
                source,
            },
            CommandRunError::Supervision(error) => error,
        })?;
    if !output.status.success() {
        return Err(ToolError::Ffprobe {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .into());
    }
    interpret_probe(artifact, path, &output.stdout)?;

    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
        argument_profile_blake3: profile.identity().clone(),
    })
}

/// What FFmpeg reports when it normalized with a single gain change.
///
/// The one value [`normalize_master`] accepts. Compared as a whole string
/// rather than by prefix so `linear_and_something` could never pass.
const LINEAR_NORMALIZATION: &str = "linear";

/// Where the normalized master is written before it replaces the assembled one.
///
/// A fixed name rather than a randomized temporary, and that is a provenance
/// decision rather than a style one. The executed arguments are recorded in
/// `manifest.json`, and the manifest's digest is what names the package
/// directory — so a random component here would make two builds of identical
/// input produce different package identities. Recording the destination
/// instead would be worse: it would record a path FFmpeg was never given.
///
/// Safe as a constant because it lives in the package transaction's own stage
/// directory, which `preview::start_transaction` creates fresh and quarantines
/// if one is already there, so exactly one normalization ever writes it.
///
/// The encode path still stages under randomized names; issue #82 owns that,
/// and until it lands the manifest is not yet reproducible as a whole.
///
/// The stem only. The extension is given to `Builder` as a suffix rather than
/// carried here, so it stays last in the filename even if the random component
/// ever returns — FFmpeg infers the output container from the extension, and a
/// name with the extension in its middle is one FFmpeg refuses to write at all.
const NORMALIZED_MASTER_STAGING_STEM: &str = "lesson-normalized";

/// What FFmpeg's loudness filter reported about one pass.
///
/// The second deserialization boundary in this workspace without
/// `deny_unknown_fields`, and it does not inherit [`ProbeResponse`]'s
/// exemption — it argues its own, on the same terms. This is a diagnostic
/// summary printed by a tool whose version `tools.rs` records rather than pins,
/// not a format this project defines; a release that adds a field would
/// otherwise stop every build while reporting a master that normalized
/// correctly.
///
/// The leniency can only refuse, never accept. Every field is
/// absent-or-wrong rather than defaulted, so a renamed or withdrawn one reaches
/// [`ToolError::UnreadableLoudnessReport`] instead of a pass — and an absent
/// `normalization_type` in particular cannot become a silent `linear`.
///
/// FFmpeg prints these numbers as JSON *strings*, not numbers, and they are
/// kept as strings deliberately: they are carried back into the second pass's
/// arguments verbatim, so parsing them to `f64` and formatting them again would
/// introduce a rounding step between what was measured and what is normalized
/// against, for no reader that needs the numeric value.
#[derive(Debug, Deserialize)]
struct LoudnessReport {
    input_i: String,
    input_tp: String,
    input_lra: String,
    input_thresh: String,
    target_offset: String,
    normalization_type: String,
}

impl LoudnessReport {
    /// This measurement, as the filter settings the second pass consumes.
    ///
    /// No leading or trailing separator: [`loudnorm_filter`] joins the settings
    /// it assembles, so a separator here would double one of its.
    fn measured_arguments(&self) -> String {
        let Self {
            input_i,
            input_tp,
            input_lra,
            input_thresh,
            target_offset,
            normalization_type: _,
        } = self;
        format!(
            "measured_I={input_i}:measured_TP={input_tp}:measured_LRA={input_lra}:\
             measured_thresh={input_thresh}:offset={target_offset}"
        )
    }
}

/// Reads the JSON summary FFmpeg's loudness filter prints to standard error.
///
/// The summary is the last brace-delimited span of the stream: `-hide_banner`
/// suppresses the build description, the object is flat, and the filter prints
/// it after the encode finishes. Scanning from the end rather than the start is
/// what keeps a warning FFmpeg emitted earlier from being parsed as the report.
///
/// # Errors
///
/// [`ToolError::UnreadableLoudnessReport`] when no such span exists or it does
/// not carry every field, which routes the audio owner to the pinned filter
/// arguments rather than to the audio.
fn loudness_report(stderr: &[u8], operation: ToolOperation) -> Result<LoudnessReport, ToolError> {
    let text = String::from_utf8_lossy(stderr);
    text.rfind('{')
        .zip(text.rfind('}'))
        .filter(|(start, end)| start < end)
        .and_then(|(start, end)| text.get(start..=end))
        .and_then(|json| serde_json::from_str(json).ok())
        .ok_or(ToolError::UnreadableLoudnessReport { operation })
}

/// Normalizes the assembled master in place, in two passes.
///
/// ADR-0001 §13.3 requires final-package loudness normalization, and §13.5 has
/// both lossy outputs derive independently from the master — so the master is
/// what is normalized, and the encodes inherit one loudness decision rather
/// than making three. This runs after assembly and before either encode.
///
/// The references are provisional under `ADR-0001-D012`; see
/// [`PROVISIONAL_LOUDNESS_TARGET_LUFS`].
///
/// Normalized through a staged file beside the master and renamed over it,
/// rather than in place: FFmpeg cannot read and write one path, and a failure
/// partway through an in-place rewrite would leave a master that is neither the
/// assembled audio nor the normalized audio.
///
/// # Errors
///
/// [`ToolError::StartFfmpeg`] when the binary cannot be launched; the same
/// supervision variants [`encode`] documents; [`ToolError::Ffmpeg`] when either
/// pass exits non-zero; [`ToolError::UnreadableLoudnessReport`] when either
/// pass prints no readable summary; [`ToolError::LoudnessNotLinear`] when
/// FFmpeg fell back to a moving gain, which routes to the human-review owner
/// per `docs/governance/ROUTING-TABLES.md`;
/// [`ToolError::LoudnessChangedLength`] when the returned master is a different
/// length from the assembled one; otherwise [`crate::IoError::FileSystem`].
pub(crate) fn normalize_master(
    ffmpeg: &ToolIdentity,
    profiles: &ExportProfiles,
    master_wav: &Path,
    expected_frames: u64,
) -> Result<Vec<ToolExecution>, BuildError> {
    let measure_arguments =
        materialize_arguments(&profiles.ffmpeg_loudnorm_measure, master_wav, None);
    let measured = run_loudness_pass(
        ffmpeg,
        &measure_arguments,
        ToolOperation::LoudnessMeasure,
        master_wav,
    )?;

    let parent = master_wav
        .parent()
        .ok_or_else(|| ManagedPathError::UnrootedDestination {
            path: master_wav.to_path_buf(),
        })?;
    // Through `Builder` with its randomness switched off rather than a bare
    // path, so the file keeps both properties `encode` relies on. `tempfile`
    // creates it `0600` — which is where a package's file mode comes from, as
    // `package_port::PACKAGE_FILE_MODE` records — and the guard removes a
    // partial normalization on every early return below, because FFmpeg opens
    // its output before it can discover it cannot finish.
    let staged = Builder::new()
        .prefix(NORMALIZED_MASTER_STAGING_STEM)
        .suffix(".wav")
        .rand_bytes(0)
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?
        .into_temp_path();
    let apply_arguments = materialize_measured_arguments(
        &profiles.ffmpeg_loudnorm_apply,
        master_wav,
        &staged,
        &measured.measured_arguments(),
    );
    let applied = run_loudness_pass(
        ffmpeg,
        &apply_arguments,
        ToolOperation::LoudnessNormalize,
        master_wav,
    )?;
    if applied.normalization_type != LINEAR_NORMALIZATION {
        return Err(ToolError::LoudnessNotLinear {
            normalization_type: applied.normalization_type,
        }
        .into());
    }
    // Checked before the rename, so a master that changed length never becomes
    // the published one. The count is passed in rather than measured from the
    // input again: the caller already holds the assembled timeline's total, and
    // that is the number the manifest, captions, and chapters were written
    // from, so it is the one this has to agree with.
    let produced = u64::from(
        hound::WavReader::open(&staged)
            .map_err(|error| audio_error(&staged, error))?
            .duration(),
    );
    if produced != expected_frames {
        return Err(ToolError::LoudnessChangedLength {
            expected: expected_frames,
            produced,
        }
        .into());
    }
    staged
        .persist(master_wav)
        .map_err(|error| io_error(master_wav, error.error))?;

    Ok(vec![
        ToolExecution {
            arguments: display_arguments(&measure_arguments),
            argument_profile_blake3: profiles.ffmpeg_loudnorm_measure.identity().clone(),
        },
        ToolExecution {
            arguments: display_arguments(&apply_arguments),
            argument_profile_blake3: profiles.ffmpeg_loudnorm_apply.identity().clone(),
        },
    ])
}

/// Runs one loudness pass and returns what it reported.
fn run_loudness_pass(
    ffmpeg: &ToolIdentity,
    arguments: &[OsString],
    operation: ToolOperation,
    master_wav: &Path,
) -> Result<LoudnessReport, BuildError> {
    let mut command = Command::new(&ffmpeg.resolved_executable);
    command.args(arguments);
    let invocation = ToolInvocation::new("FFmpeg", operation, master_wav);
    let output =
        process::run(invocation, command, FFMPEG_ENCODE_POLICY).map_err(|error| match error {
            CommandRunError::Start(source) => ToolError::StartFfmpeg {
                executable: ffmpeg.resolved_executable.clone(),
                source,
            },
            CommandRunError::Supervision(error) => error,
        })?;
    if !output.status.success() {
        return Err(ToolError::Ffmpeg {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .into());
    }
    Ok(loudness_report(&output.stderr, operation)?)
}

/// Refuses an FFmpeg that cannot encode [`MP3_ENCODER`], before any work runs.
///
/// Asks the binary rather than trusting the platform: FFmpeg is routinely
/// packaged without `libmp3lame`, and `tools::inspect` reports only the first
/// line of `-version`, which says nothing about which encoders were compiled
/// in. Running here, inside package preflight, is what keeps a missing encoder
/// from being discovered after a full render has already been synthesized.
///
/// # Errors
///
/// [`ToolError::StartFfmpeg`] when the binary cannot be launched; the same
/// supervision variants [`encode`] documents when a named supervision invariant
/// fails; [`ToolError::Ffmpeg`] when the inventory probe itself exits non-zero;
/// and [`ToolError::MissingEncoder`] when it succeeds and the encoder is not
/// listed, which routes the operator to their FFmpeg build.
pub(crate) fn preflight_encoder(
    ffmpeg: &ToolIdentity,
    profile: &ToolProfile,
    encoder: &'static str,
) -> Result<ToolExecution, BuildError> {
    let arguments = materialize_arguments(profile, Path::new(""), None);
    let mut command = Command::new(&ffmpeg.resolved_executable);
    command.args(&arguments);
    let invocation = ToolInvocation::new(
        "FFmpeg",
        ToolOperation::EncoderProbe,
        &ffmpeg.resolved_executable,
    );
    let output =
        process::run(invocation, command, VERSION_PROBE_POLICY).map_err(|error| match error {
            CommandRunError::Start(source) => ToolError::StartFfmpeg {
                executable: ffmpeg.resolved_executable.clone(),
                source,
            },
            CommandRunError::Supervision(error) => error,
        })?;
    if !output.status.success() {
        return Err(ToolError::Ffmpeg {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        }
        .into());
    }
    if !lists_encoder(&output.stdout, encoder) {
        return Err(ToolError::MissingEncoder {
            executable: ffmpeg.resolved_executable.clone(),
            encoder,
        }
        .into());
    }

    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
        argument_profile_blake3: profile.identity().clone(),
    })
}

/// Whether an `ffmpeg -encoders` listing offers `encoder` by that exact name.
///
/// Each inventory line is `<capability flags> <name> <description>`, so the
/// name is the second whitespace-separated token. Matched positionally rather
/// than by searching the line, because every encoder's description repeats
/// words that appear in other encoders' names — a substring search finds
/// `libmp3lame` in the description of a binary that cannot encode MP3 at all.
fn lists_encoder(listing: &[u8], encoder: &str) -> bool {
    String::from_utf8_lossy(listing)
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(encoder))
}

/// Reads one ffprobe response and decides whether it describes the stream this
/// build produces.
///
/// Split from [`probe`] so both outcomes are reachable without running a real
/// ffprobe: the failure modes are a response that cannot be read and a response
/// that reports the wrong stream, and only the first needs bytes a real tool
/// would never emit.
fn interpret_probe(
    artifact: PackagedAudio,
    path: &Path,
    response: &[u8],
) -> Result<(), BuildError> {
    // Mapped explicitly rather than through `?`, because a probe this build
    // cannot read leaves the output unverified and must not surface as a
    // generic JSON failure naming no subsystem.
    let probe: ProbeResponse =
        serde_json::from_slice(response).map_err(|source| ToolError::UnreadableProbeResponse {
            path: path.to_path_buf(),
            source,
        })?;

    // Counted before any stream is described. Picking one out of several and
    // reporting on it is what let a second stream pass unnoticed: the first was
    // the stream this build writes, and nothing looked at the rest.
    if probe.streams.len() != REQUIRED_STREAMS {
        return Err(ToolError::UnexpectedEncodedStreamCount {
            path: path.to_path_buf(),
            found: probe.streams.len(),
            required: REQUIRED_STREAMS,
        }
        .into());
    }

    let stream = probe.streams.first();
    let codec = stream.and_then(|stream| stream.codec_name.clone());
    let channels = stream.and_then(|stream| stream.channels);
    if codec.as_deref() != Some(artifact.codec()) || channels != Some(REQUIRED_CHANNELS) {
        return Err(ToolError::UnexpectedEncodedStream {
            path: path.to_path_buf(),
            codec,
            channels,
            required_codec: artifact.codec(),
            required_channels: REQUIRED_CHANNELS,
        }
        .into());
    }
    Ok(())
}

/// Substitutes the path placeholders in one profile, in order.
///
/// Every flag in the encode profiles is load-bearing. `-nostdin` keeps a prompt
/// from hanging an offline render; `-map_metadata -1` and `-vn` strip anything
/// that did not come from the master, so the container holds exactly the single
/// stream [`probe`] verifies; the channel count and channel layout come from
/// the constants that verification also reads, so the two cannot drift.
///
/// A profile carrying neither placeholder — the encoder inventory — passes
/// through unchanged, so `input` is simply never consulted. Only `output` is an
/// `Option`, because only its absence is a real case: a probe has an input and
/// no output.
fn materialize_arguments(
    profile: &ToolProfile,
    input: &Path,
    output: Option<&Path>,
) -> Vec<OsString> {
    profile
        .normalized_arguments
        .iter()
        .map(|argument| match argument.as_str() {
            INPUT_PATH_ARGUMENT => input.as_os_str().to_owned(),
            OUTPUT_PATH_ARGUMENT => output
                .expect("only the FFmpeg profile contains an output placeholder")
                .as_os_str()
                .to_owned(),
            argument => OsString::from(argument),
        })
        .collect()
}

/// [`materialize_arguments`] with the measurement placeholder resolved too.
///
/// Layered on that function rather than replacing it, so path substitution
/// keeps one implementation: the loudness pass differs only in carrying a
/// second placeholder, and a parallel copy is how the two would stop agreeing
/// about what a path argument looks like.
fn materialize_measured_arguments(
    profile: &ToolProfile,
    input: &Path,
    output: &Path,
    measured: &str,
) -> Vec<OsString> {
    materialize_arguments(profile, input, Some(output))
        .into_iter()
        .map(|argument| match argument.to_str() {
            Some(text) if text.contains(MEASURED_LOUDNESS_ARGUMENT) => {
                OsString::from(text.replace(MEASURED_LOUDNESS_ARGUMENT, measured))
            }
            _ => argument,
        })
        .collect()
}

/// Non-UTF-8 arguments are rendered lossily. Authoritative non-UTF-8 path
/// representation in provenance records is deferred and recorded in
/// `docs/architecture/WALKING-SKELETON.md`.
fn display_arguments(arguments: &[OsString]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // No test here runs a real ffmpeg or ffprobe; anything that does lives in
    // the testkit integration suite, so `cargo test -p study-tts-runtime` stays
    // runnable on a machine with neither binary installed. The failing-encoder
    // test below stands one in rather than reaching for the real thing.
    //
    // Unix-gated on the same terms as the executable-bit check in `tools.rs`:
    // the stand-in has to be marked executable, and ADR-0001 targets WSL2.
    #[cfg(unix)]
    #[test]
    fn t1_e0_a_failed_encode_leaves_no_staged_file_behind() {
        use std::{fs, os::unix::fs::PermissionsExt};

        use tempfile::TempDir;

        // The failure that matters is the one where the encoder runs, creates
        // its output, and then exits non-zero: FFmpeg opens the container
        // before it can discover it cannot finish, and an interrupted encode
        // leaves the same partial file. A stand-in encoder reproduces exactly
        // that shape without needing FFmpeg installed — pointing at a binary
        // that never starts would leave nothing behind either way, and so
        // could not fail if the guard were dropped.
        let directory = TempDir::new().expect("create a directory to encode into");
        let encoder = directory.path().join("failing-encoder");
        // POSIX `sh`, so this runs under dash as well as bash: the loop leaves
        // `output` holding the last argument, which is the path this build
        // told the encoder to write. The receipt is what keeps the assertions
        // below from passing because the encoder did nothing at all.
        let script = concat!(
            "#!/bin/sh\n",
            "for output; do :; done\n",
            ": > \"$output\"\n",
            ": > \"$(dirname \"$output\")/ran\"\n",
            "exit 1\n",
        );
        fs::write(&encoder, script).expect("write the stand-in encoder");
        fs::set_permissions(&encoder, fs::Permissions::from_mode(0o755))
            .expect("make the stand-in encoder executable");

        let identity = ToolIdentity {
            resolved_executable: encoder,
            version: "failing-encoder-v1".to_owned(),
        };
        let destination = directory.path().join("lesson.m4a");

        let profiles = export_profiles();
        let error = encode(
            &identity,
            &profiles,
            EncodedFormat::M4a,
            Path::new("/master.wav"),
            &destination,
        )
        .expect_err("an encoder that exits non-zero must not report success");
        assert!(
            matches!(error, BuildError::Tool(ToolError::Ffmpeg { .. })),
            "a failing encoder produced `{error}`"
        );

        // Nothing the encode staged may outlive it. A leftover is not merely
        // untidy: previews are what an operator listens through, and a
        // directory that accumulates one partial export per failed encode
        // stops being a record of what the build produced.
        assert!(
            directory.path().join("ran").is_file(),
            "the stand-in encoder never ran, so nothing was staged to clean up"
        );
        let leftovers: Vec<_> = fs::read_dir(directory.path())
            .expect("read the directory back")
            .map(|entry| entry.expect("read a directory entry").file_name())
            .filter(|name| name != "failing-encoder" && name != "ran")
            .collect();
        assert!(
            leftovers.is_empty(),
            "a failed encode left {leftovers:?} behind"
        );
    }

    #[test]
    fn t1_e0_unreadable_and_unexpected_probes_are_reported_separately() {
        let m4a = Path::new("/lesson.m4a");

        // A response this build cannot read says nothing about the file, so the
        // output is unverified rather than known-wrong, and the fault lies with
        // the probe rather than the encode.
        for unreadable in [
            &b"{ not json"[..],
            b"",
            br#"{"streams":"none"}"#,
            br#"{"streams":[{"codec_name":"aac","channels":"two"}]}"#,
        ] {
            let error = interpret_probe(PackagedAudio::M4a, m4a, unreadable)
                .expect_err("an unreadable probe response must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::Tool(ToolError::UnreadableProbeResponse { .. })
                ),
                "`{}` produced `{error}`",
                String::from_utf8_lossy(unreadable)
            );
        }

        // A readable response describing the wrong stream is the encoder
        // failing open, and the refusal quotes what was actually found.
        for (label, response, codec, channels) in [
            (
                "wrong codec",
                br#"{"streams":[{"codec_name":"pcm_f32le","channels":1}]}"#.to_vec(),
                Some("pcm_f32le"),
                Some(1),
            ),
            (
                "wrong channel count",
                br#"{"streams":[{"codec_name":"aac","channels":2}]}"#.to_vec(),
                Some("aac"),
                Some(2),
            ),
        ] {
            let error = interpret_probe(PackagedAudio::M4a, m4a, &response)
                .expect_err("an unexpected stream must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::Tool(ToolError::UnexpectedEncodedStream {
                        codec: ref found_codec,
                        channels: found_channels,
                        ..
                    }) if found_codec.as_deref() == codec && found_channels == channels
                ),
                "{label} produced `{error}`"
            );
        }

        // A wrong number of streams is counted, not described: with several,
        // any description has to pick one, and picking the first is what let a
        // second stream pass unnoticed.
        const MONO_AAC: &str = r#"{"codec_name":"aac","channels":1}"#;
        const VIDEO: &str = r#"{"codec_name":"h264"}"#;
        let probe_of = |streams: &str| format!(r#"{{"streams":[{streams}]}}"#).into_bytes();

        for (label, response, found) in [
            ("no streams at all", probe_of(""), 0),
            (
                "a second audio stream",
                probe_of(&format!("{MONO_AAC},{MONO_AAC}")),
                2,
            ),
            (
                "an extra video stream",
                probe_of(&format!("{MONO_AAC},{VIDEO}")),
                2,
            ),
        ] {
            let error = interpret_probe(PackagedAudio::M4a, m4a, &response)
                .expect_err("a wrong stream count must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::Tool(ToolError::UnexpectedEncodedStreamCount {
                        found: reported,
                        required: 1,
                        ..
                    })
                        if reported == found
                ),
                "{label} produced `{error}`"
            );
        }

        interpret_probe(
            PackagedAudio::M4a,
            m4a,
            br#"{"streams":[{"codec_name":"aac","channels":1}]}"#,
        )
        .expect("one mono AAC stream is the stream this build produces");
    }

    // Bounds the `deny_unknown_fields` exception recorded on `ProbeResponse`:
    // the parser may ignore what ffprobe adds, but nothing it fails to read
    // may become an acceptance.
    #[test]
    fn t1_e2_loudness_leniency_cannot_accept_an_unreported_normalization() {
        // The counterpart of the probe-leniency test above, for the other
        // lenient boundary. `LoudnessReport` omits
        // `deny_unknown_fields` so an FFmpeg release that adds a field does not
        // stop every build, and the argument for that is only sound while the
        // leniency can *refuse* but never *accept*. Every case below is a
        // report this build must not read as a successful linear result.
        // Every measurement the second pass needs, so each case below differs
        // from a readable report in exactly one way.
        const MEASURED: &str = concat!(
            r#""input_i":"-27.85","input_tp":"-9.61","input_lra":"5.20","#,
            r#""input_thresh":"-38.10","target_offset":"0.02""#
        );
        let no_type = format!("{{{MEASURED}}}");
        let renamed = format!(r#"{{{MEASURED},"normalisation_type":"linear"}}"#);
        let refused = [
            ("no report at all", "size=N/A time=00:00:14.96"),
            ("a report missing normalization_type", no_type.as_str()),
            ("a renamed normalization_type", renamed.as_str()),
            (
                "a report missing a measurement",
                r#"{"input_i":"-27.85","normalization_type":"linear"}"#,
            ),
            ("a closing brace before its opening one", "} {"),
        ];

        for (case, stderr) in refused {
            let refusal = loudness_report(stderr.as_bytes(), ToolOperation::LoudnessNormalize)
                .expect_err(case);

            assert!(
                matches!(refusal, ToolError::UnreadableLoudnessReport { .. }),
                "{case}: refused as {refusal:?} rather than an unreadable report"
            );
        }
    }

    #[test]
    fn t1_e2_a_complete_report_is_read_and_carries_its_normalization() {
        // The other half: leniency that refused everything would also satisfy
        // the test above, so the accepted shape is pinned beside the refusals.
        // Carries the filter tag FFmpeg prints before the object, and an
        // `output_i` this build does not read, because both are present in
        // real output and neither may prevent a read.
        const STDERR: &str = concat!(
            r#"[Parsed_loudnorm_0 @ 0x1] {"input_i":"-27.85","#,
            r#""input_tp":"-9.61","input_lra":"5.20","input_thresh":"-38.10","#,
            r#""output_i":"-16.02","target_offset":"0.02","#,
            r#""normalization_type":"linear"}"#
        );

        let report = loudness_report(STDERR.as_bytes(), ToolOperation::LoudnessNormalize)
            .expect("a complete report is readable");

        assert_eq!(report.normalization_type, LINEAR_NORMALIZATION);
        assert_eq!(
            report.measured_arguments(),
            concat!(
                "measured_I=-27.85:measured_TP=-9.61:measured_LRA=5.20:",
                "measured_thresh=-38.10:offset=0.02"
            )
        );
    }

    #[test]
    fn t1_e2_the_loudness_filter_carries_the_deviation_values() {
        // Read against `ADR-0001-D012`'s constant table, not against the
        // assembler: this string is hashed into the argument-profile identity
        // that names every package directory, so an edit here moves package
        // identity for every lesson.
        assert_eq!(
            loudnorm_filter(LoudnessPass::Measure),
            "loudnorm=I=-27.0:TP=-1.0:LRA=7.0:print_format=json"
        );
        assert_eq!(
            loudnorm_filter(LoudnessPass::Apply),
            "loudnorm=I=-27.0:TP=-1.0:LRA=7.0:{measured_loudness}:linear=true:print_format=json"
        );
    }

    #[test]
    fn t1_e0_probe_leniency_cannot_accept_an_unverified_stream() {
        let m4a = Path::new("/lesson.m4a");
        let response_for =
            |stream: &str| format!(r#"{{"programs":[],"streams":[{stream}]}}"#).into_bytes();

        // The shape a real ffprobe 6.1 emits under the pinned selection. The
        // `programs` array it volunteers is the field strictness would reject.
        interpret_probe(
            PackagedAudio::M4a,
            m4a,
            &response_for(r#"{"codec_name":"aac","channels":1}"#),
        )
        .expect("the response a real ffprobe emits must be accepted");

        // Each field read here is absent-or-wrong, never defaulted, so a
        // spelling this build no longer recognizes is reported as a stream it
        // could not confirm rather than passed as one it did.
        for (label, stream, codec, channels) in [
            (
                "codec_name renamed",
                r#"{"codec":"aac","channels":1}"#,
                None,
                Some(1),
            ),
            (
                "channels renamed",
                r#"{"codec_name":"aac","channel_count":1}"#,
                Some("aac"),
                None,
            ),
            (
                "a wrong stream padded with fields this build ignores",
                r#"{"codec_name":"mp3","channels":2,"profile":"HE-AAC"}"#,
                Some("mp3"),
                Some(2),
            ),
        ] {
            let error = interpret_probe(PackagedAudio::M4a, m4a, &response_for(stream))
                .expect_err("an unconfirmed stream must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::Tool(ToolError::UnexpectedEncodedStream {
                        codec: ref found_codec,
                        channels: found_channels,
                        ..
                    }) if found_codec.as_deref() == codec && found_channels == channels
                ),
                "{label} produced `{error}`"
            );
        }

        // The container itself is the one rename with nothing to describe, and
        // `#[serde(default)]` is what would otherwise make it look like a file
        // holding no streams rather than a response this build cannot read.
        let error = interpret_probe(PackagedAudio::M4a, m4a, br#"{"programs":[],"stream":[]}"#)
            .expect_err("a response with no readable streams must not verify an output");
        assert!(
            matches!(
                error,
                BuildError::Tool(ToolError::UnexpectedEncodedStreamCount { found: 0, .. })
            ),
            "a renamed streams array produced `{error}`"
        );
    }

    /// Each artifact is held to the codec its own section assigns it.
    ///
    /// The T4 suite proves these against real files, but it needs FFmpeg and
    /// ffprobe to do it, and this claim needs neither: swapping two arms of
    /// `PackagedAudio::codec` would leave every artifact validated against
    /// another artifact's codec, and that is readable here.
    ///
    /// Matched exhaustively rather than listed, so a fourth artifact is a
    /// compile error in this test rather than an untested one.
    #[test]
    fn t1_e1_each_packaged_artifact_is_held_to_its_own_codec() {
        for artifact in [
            PackagedAudio::MasterWav,
            PackagedAudio::M4a,
            PackagedAudio::Mp3,
        ] {
            let expected = match artifact {
                // The canonical master ADR-0001 §13.1 defines, as ffprobe
                // spells it; §13.5's two exports.
                PackagedAudio::MasterWav => "pcm_f32le",
                PackagedAudio::M4a => "aac",
                PackagedAudio::Mp3 => "mp3",
            };

            assert_eq!(artifact.codec(), expected, "{artifact:?}");
        }
    }

    #[test]
    fn t1_e0_ffmpeg_arguments_are_pinned_and_explicit() {
        let profiles = export_profiles();
        let arguments = materialize_arguments(
            &profiles.ffmpeg_m4a,
            Path::new("/input.wav"),
            Some(Path::new("/output.m4a")),
        );
        assert_eq!(
            arguments,
            vec![
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-i",
                "/input.wav",
                "-map_metadata",
                "-1",
                "-vn",
                "-ac",
                "1",
                "-channel_layout",
                "mono",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "/output.m4a",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn t1_e0_ffprobe_arguments_are_pinned_and_explicit() {
        let profiles = export_profiles();
        let arguments = materialize_arguments(&profiles.ffprobe, Path::new("/output.m4a"), None);
        assert_eq!(
            arguments,
            vec![
                "-v",
                "error",
                "-show_entries",
                "stream=codec_name,channels",
                "-of",
                "json",
                "/output.m4a",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
    }
}
