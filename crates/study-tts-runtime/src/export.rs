//! Encoding the master WAV to M4A through FFmpeg, and verifying the result
//! with ffprobe.
//!
//! The codec, channel count, and stream count this build produces are named
//! once and used by both ends: the encode maps them and the probe verifies
//! them, so the verification cannot drift into passing something the encoder
//! no longer writes.
//!
//! `ProbeResponse` is the one deserialization boundary in this workspace
//! without `deny_unknown_fields`. It parses a diagnostic tool's output rather
//! than a format this project defines, and the exception is argued and tested
//! beside the type itself.

use std::{ffi::OsString, path::Path, process::Command};

use serde::Deserialize;
use study_tts_core::CANONICAL_CHANNELS;
use tempfile::Builder;

use crate::{BuildError, io_error, tools::ToolIdentity};

/// The audio codec every encoded output carries.
///
/// One definition for both ends of the agreement: `ffmpeg_arguments` encodes to
/// it and `probe_m4a` verifies it. Two literals could drift apart silently,
/// leaving the verification passing something the encoder no longer produces.
const REQUIRED_CODEC: &str = "aac";

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
}

/// Encodes the master WAV to M4A, returning what FFmpeg was told to do.
///
/// Encoded to a staged path beside the destination and renamed on success, so
/// a failed encode never leaves a partial `.m4a` for a later step to find. The
/// staging guard is held across the encode rather than released once it has
/// reserved a name: FFmpeg opens its output container before it can discover
/// it cannot finish, so a failure after that point leaves a partial file that
/// only the guard's drop removes. Previews are what an operator listens
/// through, and one that accumulates a partial `.m4a` per failed encode stops
/// being a record of what the build produced.
///
/// A process killed outright still leaves the staged file, because no drop
/// runs. That is a crash-recovery sweep rather than an encode concern, and it
/// belongs with the reconciliation E2-S1 introduces.
///
/// # Errors
///
/// [`BuildError::UnrootedDestination`] when `destination` has no parent;
/// [`BuildError::StartFfmpeg`] when the binary cannot be launched;
/// [`BuildError::Ffmpeg`] carrying the status and stderr when it runs and
/// fails; otherwise [`BuildError::FileSystem`].
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
    // The handle is closed but the path is kept, because FFmpeg writes the
    // file itself rather than through a handle this process holds. Dropping the
    // whole `NamedTempFile` here would take the cleanup with it.
    let staged = Builder::new()
        .prefix("lesson-")
        .suffix(".m4a")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?
        .into_temp_path();

    let arguments = ffmpeg_arguments(master_wav, &staged);
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
    staged
        .persist(destination)
        .map_err(|error| io_error(destination, error.error))?;
    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
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
/// [`BuildError::InspectTool`] when ffprobe cannot be launched;
/// [`BuildError::Ffprobe`] when it runs and fails;
/// [`BuildError::UnreadableProbeResponse`] when its output cannot be parsed;
/// [`BuildError::UnexpectedEncodedStream`] when the codec, channel count, or
/// stream count is not the one this build encodes to.
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
    // Mapped explicitly rather than through `?`, because a probe this build
    // cannot read leaves the output unverified and must not surface as a
    // generic JSON failure naming no subsystem.
    let probe: ProbeResponse =
        serde_json::from_slice(response).map_err(|source| BuildError::UnreadableProbeResponse {
            path: m4a.to_path_buf(),
            source,
        })?;

    // Counted before any stream is described. Picking one out of several and
    // reporting on it is what let a second stream pass unnoticed: the first was
    // the stream this build writes, and nothing looked at the rest.
    if probe.streams.len() != REQUIRED_STREAMS {
        return Err(BuildError::UnexpectedEncodedStreamCount {
            path: m4a.to_path_buf(),
            found: probe.streams.len(),
            required: REQUIRED_STREAMS,
        });
    }

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

/// The encode this build performs, stated as one list.
///
/// Every flag is load-bearing. `-nostdin` keeps a prompt from hanging an
/// offline render; `-map_metadata -1` and `-vn` strip anything that did not
/// come from the master, so the container holds exactly the single stream
/// `probe_m4a` verifies; the codec, channel count, and channel layout come from
/// the constants that verification also reads, so the two cannot drift.
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
        OsString::from(REQUIRED_CHANNEL_LAYOUT),
        OsString::from("-c:a"),
        OsString::from(REQUIRED_CODEC),
        OsString::from("-b:a"),
        OsString::from("96k"),
        destination.as_os_str().to_owned(),
    ]
    .into()
}

/// Asks for every stream in the container, not just the first audio one.
///
/// `-select_streams a:0` reported a single stream for a file holding two, so a
/// second stream was invisible to the check. Dropping the selection entirely
/// also surfaces video and data streams, which `a` would still have hidden.
fn ffprobe_arguments(m4a: &Path) -> Vec<OsString> {
    [
        OsString::from("-v"),
        OsString::from("error"),
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

        let error = export_m4a(&identity, Path::new("/master.wav"), &destination)
            .expect_err("an encoder that exits non-zero must not report success");
        assert!(
            matches!(error, BuildError::Ffmpeg { .. }),
            "a failing encoder produced `{error}`"
        );

        // Nothing the encode staged may outlive it. A leftover is not merely
        // untidy: previews are what an operator listens through, and a
        // directory that accumulates one partial `.m4a` per failed encode
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
            let error = interpret_probe(m4a, unreadable)
                .expect_err("an unreadable probe response must not verify an output");
            assert!(
                matches!(error, BuildError::UnreadableProbeResponse { .. }),
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
            let error = interpret_probe(m4a, &response)
                .expect_err("a wrong stream count must not verify an output");
            assert!(
                matches!(
                    error,
                    BuildError::UnexpectedEncodedStreamCount { found: reported, required: 1, .. }
                        if reported == found
                ),
                "{label} produced `{error}`"
            );
        }

        interpret_probe(m4a, br#"{"streams":[{"codec_name":"aac","channels":1}]}"#)
            .expect("one mono AAC stream is the stream this build produces");
    }

    // Bounds the `deny_unknown_fields` exception recorded on `ProbeResponse`:
    // the parser may ignore what ffprobe adds, but nothing it fails to read
    // may become an acceptance.
    #[test]
    fn t1_e0_probe_leniency_cannot_accept_an_unverified_stream() {
        let m4a = Path::new("/lesson.m4a");
        let response_for =
            |stream: &str| format!(r#"{{"programs":[],"streams":[{stream}]}}"#).into_bytes();

        // The shape a real ffprobe 6.1 emits under the pinned selection. The
        // `programs` array it volunteers is the field strictness would reject.
        interpret_probe(m4a, &response_for(r#"{"codec_name":"aac","channels":1}"#))
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
            let error = interpret_probe(m4a, &response_for(stream))
                .expect_err("an unconfirmed stream must not verify an output");
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

        // The container itself is the one rename with nothing to describe, and
        // `#[serde(default)]` is what would otherwise make it look like a file
        // holding no streams rather than a response this build cannot read.
        let error = interpret_probe(m4a, br#"{"programs":[],"stream":[]}"#)
            .expect_err("a response with no readable streams must not verify an output");
        assert!(
            matches!(
                error,
                BuildError::UnexpectedEncodedStreamCount { found: 0, .. }
            ),
            "a renamed streams array produced `{error}`"
        );
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
