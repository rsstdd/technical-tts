use std::{ffi::OsString, path::Path, process::Command};

use serde::Deserialize;
use tempfile::Builder;

use crate::{BuildError, io_error, tools::ToolIdentity};

/// The audio codec every encoded output carries.
///
/// One definition for both ends of the agreement: `ffmpeg_arguments` encodes to
/// it and `probe_m4a` verifies it. Two literals could drift apart silently,
/// leaving the verification passing something the encoder no longer produces.
const REQUIRED_CODEC: &str = "aac";

/// The channel count every encoded output carries, on the same terms.
const REQUIRED_CHANNELS: u16 = 1;

/// The subset of an ffprobe response the pinned `-show_entries` selection asks
/// for.
///
/// Deliberately not `deny_unknown_fields`: this is another program's output,
/// not a contract this project defines, and a future ffprobe adding a field
/// must not fail a build. What bounds the shape is the pinned selection in
/// `ffprobe_arguments`.
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

#[derive(Clone, Debug)]
pub(crate) struct ToolExecution {
    pub arguments: Vec<String>,
}

pub(crate) fn export_m4a(
    ffmpeg: &ToolIdentity,
    master_wav: &Path,
    destination: &Path,
) -> Result<ToolExecution, BuildError> {
    let parent = destination
        .parent()
        .ok_or_else(|| BuildError::UnrootedDestination {
            path: destination.to_path_buf(),
        })?;
    let staged = Builder::new()
        .prefix("lesson-")
        .suffix(".m4a")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    let staged_path = staged.path().to_path_buf();
    drop(staged);

    let arguments = ffmpeg_arguments(master_wav, &staged_path);
    let output = Command::new(&ffmpeg.resolved_executable)
        .args(&arguments)
        .output()
        .map_err(|source| BuildError::StartFfmpeg {
            executable: ffmpeg.resolved_executable.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(BuildError::Ffmpeg {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    std::fs::rename(&staged_path, destination).map_err(|error| io_error(destination, error))?;
    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
    })
}

pub(crate) fn probe_m4a(ffprobe: &ToolIdentity, m4a: &Path) -> Result<ToolExecution, BuildError> {
    let arguments = ffprobe_arguments(m4a);
    let output = Command::new(&ffprobe.resolved_executable)
        .args(&arguments)
        .output()
        .map_err(|source| BuildError::InspectTool {
            tool: "ffprobe".to_owned(),
            executable: ffprobe.resolved_executable.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(BuildError::Ffprobe {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    interpret_probe(m4a, &output.stdout)?;

    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
    })
}

/// Reads one ffprobe response and decides whether it describes the stream this
/// build produces.
///
/// Split from `probe_m4a` so both outcomes are reachable without running a real
/// ffprobe: the failure modes are a response that cannot be read and a response
/// that reports the wrong stream, and only the first needs bytes a real tool
/// would never emit.
fn interpret_probe(m4a: &Path, response: &[u8]) -> Result<(), BuildError> {
    // Mapped explicitly rather than through `?`, because a probe this build cannot
    // read leaves the output unverified and must not surface as a generic JSON
    // failure naming no subsystem.
    let probe: ProbeResponse =
        serde_json::from_slice(response).map_err(|source| BuildError::UnreadableProbeResponse {
            path: m4a.to_path_buf(),
            source,
        })?;

    let stream = probe.streams.first();
    let codec = stream.and_then(|stream| stream.codec_name.clone());
    let channels = stream.and_then(|stream| stream.channels);
    if codec.as_deref() != Some(REQUIRED_CODEC) || channels != Some(REQUIRED_CHANNELS) {
        return Err(BuildError::UnexpectedEncodedStream {
            path: m4a.to_path_buf(),
            codec,
            channels,
            required_codec: REQUIRED_CODEC,
            required_channels: REQUIRED_CHANNELS,
        });
    }
    Ok(())
}

fn ffmpeg_arguments(master_wav: &Path, destination: &Path) -> Vec<OsString> {
    [
        OsString::from("-nostdin"),
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-y"),
        OsString::from("-i"),
        master_wav.as_os_str().to_owned(),
        OsString::from("-map_metadata"),
        OsString::from("-1"),
        OsString::from("-vn"),
        OsString::from("-ac"),
        OsString::from(REQUIRED_CHANNELS.to_string()),
        OsString::from("-channel_layout"),
        OsString::from("mono"),
        OsString::from("-c:a"),
        OsString::from(REQUIRED_CODEC),
        OsString::from("-b:a"),
        OsString::from("96k"),
        destination.as_os_str().to_owned(),
    ]
    .into()
}

fn ffprobe_arguments(m4a: &Path) -> Vec<OsString> {
    [
        OsString::from("-v"),
        OsString::from("error"),
        OsString::from("-select_streams"),
        OsString::from("a:0"),
        OsString::from("-show_entries"),
        OsString::from("stream=codec_name,channels"),
        OsString::from("-of"),
        OsString::from("json"),
        m4a.as_os_str().to_owned(),
    ]
    .into()
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

    // Only tool-free tests belong here. Anything requiring a real ffmpeg or ffprobe
    // lives in the testkit integration suite, so `cargo test -p study-tts-runtime`
    // stays runnable on a machine with neither binary installed.
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
            let error = interpret_probe(m4a, unreadable)
                .expect_err("an unreadable probe response must not verify an output");
            assert!(
                matches!(error, BuildError::UnreadableProbeResponse { .. }),
                "`{}` produced `{error}`",
                String::from_utf8_lossy(unreadable)
            );
        }

        // A readable response describing the wrong stream is the encoder failing
        // open, and the refusal quotes what was actually found.
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
            ("no audio stream", br#"{"streams":[]}"#.to_vec(), None, None),
        ] {
            let error = interpret_probe(m4a, &response)
                .expect_err("an unexpected stream must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::UnexpectedEncodedStream {
                        codec: ref found_codec,
                        channels: found_channels,
                        ..
                    } if found_codec.as_deref() == codec && found_channels == channels
                ),
                "{label} produced `{error}`"
            );
        }

        interpret_probe(m4a, br#"{"streams":[{"codec_name":"aac","channels":1}]}"#)
            .expect("one mono AAC stream is the stream this build produces");
    }

    #[test]
    fn t1_e0_ffmpeg_arguments_are_pinned_and_explicit() {
        let arguments = ffmpeg_arguments(Path::new("/input.wav"), Path::new("/output.m4a"));
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
        let arguments = ffprobe_arguments(Path::new("/output.m4a"));
        assert_eq!(
            arguments,
            vec![
                "-v",
                "error",
                "-select_streams",
                "a:0",
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
