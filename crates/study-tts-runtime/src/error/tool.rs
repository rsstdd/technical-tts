//! External-tool discovery, execution, probing, and stream-validation failures.

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Captured output stream owned by an external tool invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolOutputStream {
    /// Bytes written to the tool's standard output pipe.
    Stdout,
    /// Bytes written to the tool's standard error pipe.
    Stderr,
}

/// External-tool operation being supervised.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolOperation {
    /// Discover and record an executable version.
    VersionProbe,
    /// Encode the canonical master as M4A.
    M4aEncode,
    /// Validate an encoded M4A artifact.
    M4aValidation,
    /// Run one persistent speech worker for a lifetime of requests.
    WorkerSession,
}

impl fmt::Display for ToolOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VersionProbe => formatter.write_str("version probe"),
            Self::M4aEncode => formatter.write_str("M4A encode"),
            Self::M4aValidation => formatter.write_str("M4A validation"),
            Self::WorkerSession => formatter.write_str("worker session"),
        }
    }
}

/// Identifies the tool, operation, and subject under supervision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ToolInvocation {
    tool: String,
    operation: ToolOperation,
    subject: PathBuf,
}

impl ToolInvocation {
    /// Creates the exact context attached to an internally launched command.
    pub(crate) fn new(tool: &str, operation: ToolOperation, subject: &Path) -> Self {
        Self {
            tool: tool.to_owned(),
            operation,
            subject: subject.to_path_buf(),
        }
    }

    /// Returns the external tool name.
    pub fn tool(&self) -> &str {
        &self.tool
    }

    /// Returns the typed operation being performed.
    pub const fn operation(&self) -> ToolOperation {
        self.operation
    }

    /// Returns the artifact or executable the operation concerns.
    pub fn subject(&self) -> &Path {
        &self.subject
    }
}

impl fmt::Display for ToolInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} for `{}`",
            self.tool,
            self.operation,
            self.subject.display(),
        )
    }
}

impl fmt::Display for ToolOutputStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdout => formatter.write_str("stdout"),
            Self::Stderr => formatter.write_str("stderr"),
        }
    }
}

/// Why an FFmpeg-family tool operation or encoded stream was refused.
#[derive(Debug, Error)]
pub enum ToolError {
    /// A supervised tool did not finish within its fixed deadline.
    #[error("{invocation} exceeded its {timeout_ms} ms execution deadline")]
    ToolTimedOut {
        /// Exact operation that exceeded its deadline.
        invocation: ToolInvocation,
        /// Deadline enforced, in milliseconds.
        timeout_ms: u64,
    },

    /// A supervised tool emitted more bytes than its capture envelope permits.
    #[error("{invocation} {stream} exceeded the {limit_bytes}-byte capture limit")]
    ToolOutputOverflow {
        /// Exact operation whose output was refused.
        invocation: ToolInvocation,
        /// Stream that crossed its independent ceiling.
        stream: ToolOutputStream,
        /// Maximum number of bytes permitted for that stream.
        limit_bytes: usize,
    },

    /// A configured output pipe was unexpectedly absent after launch.
    #[error("{invocation} started without its {stream} capture pipe")]
    ToolPipeUnavailable {
        /// Exact operation whose pipe was absent.
        invocation: ToolInvocation,
        /// Missing output stream.
        stream: ToolOutputStream,
    },

    /// An output pipe could not be made nonblocking before capture.
    #[error("could not configure {invocation} {stream} capture: {source}")]
    ToolCaptureConfigurationFailed {
        /// Exact operation whose pipe could not be configured.
        invocation: ToolInvocation,
        /// Output stream being configured.
        stream: ToolOutputStream,
        /// What pipe configuration reported.
        source: io::Error,
    },

    /// A dedicated output-capture thread could not be started.
    #[error("could not start {invocation} {stream} capture: {source}")]
    ToolCaptureStartFailed {
        /// Exact operation whose capture thread could not start.
        invocation: ToolInvocation,
        /// Output stream whose thread could not start.
        stream: ToolOutputStream,
        /// What thread creation reported.
        source: io::Error,
    },

    /// Reading one nonblocking output pipe failed.
    #[error("could not read {invocation} {stream}: {source}")]
    ToolCaptureReadFailed {
        /// Exact operation whose output could not be read.
        invocation: ToolInvocation,
        /// Output stream whose read failed.
        stream: ToolOutputStream,
        /// What the pipe read reported.
        source: io::Error,
    },

    /// Capture workers disconnected before reporting both streams.
    #[error("{invocation} capture channel closed before both streams completed")]
    ToolCaptureChannelClosed {
        /// Exact operation whose capture channel closed.
        invocation: ToolInvocation,
    },

    /// A capture worker panicked instead of returning its stream result.
    #[error("{invocation} {stream} capture thread panicked")]
    ToolCaptureThreadPanicked {
        /// Exact operation whose capture worker panicked.
        invocation: ToolInvocation,
        /// Output stream owned by the panicked worker.
        stream: ToolOutputStream,
    },

    /// Capture workers did not stop within the bounded cleanup window.
    #[error("{invocation} capture did not stop within {timeout_ms} ms")]
    ToolCaptureShutdownTimedOut {
        /// Exact operation whose capture cleanup timed out.
        invocation: ToolInvocation,
        /// Cleanup deadline enforced, in milliseconds.
        timeout_ms: u64,
    },

    /// Completed capture state omitted bytes for one stream.
    #[error("{invocation} {stream} capture completed without bytes")]
    ToolCaptureIncomplete {
        /// Exact operation whose capture state was incomplete.
        invocation: ToolInvocation,
        /// Output stream missing its byte record.
        stream: ToolOutputStream,
    },

    /// Cleanup failed after an earlier supervision failure was established.
    #[error("{primary}; cleanup also failed: {cleanup}")]
    ToolCleanupFailed {
        /// Failure that caused supervision or an earlier cleanup step to stop.
        #[source]
        primary: Box<ToolError>,
        /// Later failure encountered while containing or reaping resources.
        cleanup: Box<ToolError>,
    },

    /// The direct child process could not be inspected without reaping it.
    #[error("could not inspect the child running {invocation}: {source}")]
    ToolChildInspectionFailed {
        /// Exact operation whose child state could not be inspected.
        invocation: ToolInvocation,
        /// What child inspection reported.
        source: io::Error,
    },

    /// The owned process group could not be signalled for termination.
    #[error("could not signal the process group running {invocation}: {source}")]
    ToolTerminationSignalFailed {
        /// Exact operation whose process group could not be signalled.
        invocation: ToolInvocation,
        /// What process-group signalling reported.
        source: io::Error,
    },

    /// Linux containment could not inspect process-tree descendants.
    #[error("could not inspect escaped descendants of {invocation}: {source}")]
    ToolContainmentInspectionFailed {
        /// Exact operation whose descendants could not be inspected.
        invocation: ToolInvocation,
        /// What `/proc` inspection reported.
        source: io::Error,
    },

    /// An escaped process proven to descend from the child could not be
    /// signalled.
    #[error("could not signal escaped process {pid} from {invocation}: {source}")]
    ToolContainmentSignalFailed {
        /// Exact operation whose escaped process could not be signalled.
        invocation: ToolInvocation,
        /// Process previously observed in the owned child tree.
        pid: i32,
        /// What direct process signalling reported.
        source: io::Error,
    },

    /// The direct child could not be reaped after its exit was observed.
    #[error("could not reap the child running {invocation}: {source}")]
    ToolChildReapFailed {
        /// Exact operation whose direct child could not be reaped.
        invocation: ToolInvocation,
        /// What child reaping reported.
        source: io::Error,
    },

    /// Cleanup did not observe the process group and tracked descendants
    /// disappear.
    #[error("{invocation} cleanup did not finish within {timeout_ms} ms")]
    ToolTerminationTimedOut {
        /// Exact operation whose cleanup timed out.
        invocation: ToolInvocation,
        /// Cleanup deadline enforced, in milliseconds.
        timeout_ms: u64,
    },

    /// A background child reaper could not be started after cleanup timed out.
    #[error("could not start the fallback child reaper for {invocation}: {source}")]
    ToolReaperStartFailed {
        /// Exact operation whose child could not be handed to a reaper.
        invocation: ToolInvocation,
        /// What reaper thread creation reported.
        source: io::Error,
    },

    /// A background capture-thread reaper could not be started.
    #[error("could not start the fallback capture reaper for {invocation}: {source}")]
    ToolCaptureReaperStartFailed {
        /// Exact operation whose capture workers could not be handed off.
        invocation: ToolInvocation,
        /// What reaper thread creation reported.
        source: io::Error,
    },

    /// FFmpeg ran and exited non-zero.
    #[error("FFmpeg failed with status {status}: {stderr}")]
    Ffmpeg {
        /// Exit status FFmpeg reported.
        status: String,
        /// What FFmpeg wrote to standard error, trimmed.
        stderr: String,
    },

    /// FFmpeg could not be launched.
    #[error("could not start FFmpeg `{executable}`: {source}")]
    StartFfmpeg {
        /// The executable that could not be started.
        executable: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },

    /// Preflight could not resolve a required external tool.
    #[error("required tool {tool} was not found or is not executable at `{requested}`")]
    MissingTool {
        /// Which tool is required.
        tool: String,
        /// The path the build was told to use.
        requested: PathBuf,
    },

    /// A required tool exists but could not be inspected.
    #[error("could not inspect {tool} at `{executable}`: {source}")]
    InspectTool {
        /// Which tool was being inspected.
        tool: String,
        /// The resolved executable.
        executable: PathBuf,
        /// What the operating system reported.
        source: io::Error,
    },

    /// A tool ran but would not report its version.
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

    /// ffprobe returned a response this build could not parse.
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

    /// The output holds a number of streams other than the required count.
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

    /// The probe describes something other than the stream this build produces.
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
        /// Codec ffprobe reported, absent if there is no audio stream.
        codec: Option<String>,
        /// Channel count ffprobe reported, absent on the same terms.
        channels: Option<u16>,
        /// Codec this build encodes to.
        required_codec: &'static str,
        /// Channel count this build encodes to.
        required_channels: u16,
    },
}

impl ToolError {
    /// Returns governed recovery advice for routed tool and lifecycle failures.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::UnreadableProbeResponse { .. }
            | Self::UnexpectedEncodedStreamCount { .. }
            | Self::UnexpectedEncodedStream { .. } => Some(RemedyAdvice::new(
                RemedyOwner::AudioRuntime,
                "reconcile the encode settings with output verification",
                Some("Invalid or over-range audio"),
            )),
            Self::ToolCleanupFailed { .. }
            | Self::ToolChildInspectionFailed { .. }
            | Self::ToolTerminationSignalFailed { .. }
            | Self::ToolContainmentInspectionFailed { .. }
            | Self::ToolContainmentSignalFailed { .. }
            | Self::ToolChildReapFailed { .. }
            | Self::ToolTerminationTimedOut { .. }
            | Self::ToolReaperStartFailed { .. }
            | Self::ToolCaptureReaperStartFailed { .. } => Some(RemedyAdvice::new(
                RemedyOwner::WorkerRuntime,
                "preserve diagnostics and correct the external-tool containment lifecycle",
                Some("Worker protocol or containment failure"),
            )),
            Self::Ffmpeg { .. }
            | Self::ToolTimedOut { .. }
            | Self::ToolOutputOverflow { .. }
            | Self::ToolPipeUnavailable { .. }
            | Self::ToolCaptureConfigurationFailed { .. }
            | Self::ToolCaptureStartFailed { .. }
            | Self::ToolCaptureReadFailed { .. }
            | Self::ToolCaptureChannelClosed { .. }
            | Self::ToolCaptureThreadPanicked { .. }
            | Self::ToolCaptureShutdownTimedOut { .. }
            | Self::ToolCaptureIncomplete { .. }
            | Self::StartFfmpeg { .. }
            | Self::MissingTool { .. }
            | Self::InspectTool { .. }
            | Self::ToolProbeFailed { .. }
            | Self::Ffprobe { .. } => None,
        }
    }
}
