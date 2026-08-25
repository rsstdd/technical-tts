//! External-tool discovery, execution, probing, and stream-validation failures.

use std::{io, path::PathBuf};

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why an FFmpeg-family tool operation or encoded stream was refused.
#[derive(Debug, Error)]
pub enum ToolError {
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
    /// Returns governed recovery advice when encoded-output validation owns
    /// the refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::UnreadableProbeResponse { .. }
            | Self::UnexpectedEncodedStreamCount { .. }
            | Self::UnexpectedEncodedStream { .. } => Some(RemedyAdvice::new(
                RemedyOwner::AudioRuntime,
                "reconcile the encode settings with output verification",
                Some("Invalid or over-range audio"),
            )),
            Self::Ffmpeg { .. }
            | Self::StartFfmpeg { .. }
            | Self::MissingTool { .. }
            | Self::InspectTool { .. }
            | Self::ToolProbeFailed { .. }
            | Self::Ffprobe { .. } => None,
        }
    }
}
