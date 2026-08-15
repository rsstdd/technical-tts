use std::{ffi::OsString, path::Path, process::Command};

use serde_json::Value;
use tempfile::Builder;

use crate::{BuildError, io_error, tools::ToolIdentity};

#[derive(Clone, Debug)]
pub(crate) struct ToolExecution {
    pub arguments: Vec<String>,
}

pub(crate) fn export_m4a(
    ffmpeg: &ToolIdentity,
    master_wav: &Path,
    destination: &Path,
) -> Result<ToolExecution, BuildError> {
    let parent = destination.parent().ok_or_else(|| {
        BuildError::InvalidCache(format!(
            "`{}` has no parent directory",
            destination.display()
        ))
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
    let probe: Value = serde_json::from_slice(&output.stdout)?;
    let stream = probe["streams"]
        .as_array()
        .and_then(|streams| streams.first());
    if stream.and_then(|value| value["codec_name"].as_str()) != Some("aac")
        || stream.and_then(|value| value["channels"].as_u64()) != Some(1)
    {
        return Err(BuildError::InvalidEncodedOutput(
            "expected one mono AAC audio stream".to_owned(),
        ));
    }

    Ok(ToolExecution {
        arguments: display_arguments(&arguments),
    })
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
        OsString::from("1"),
        OsString::from("-channel_layout"),
        OsString::from("mono"),
        OsString::from("-c:a"),
        OsString::from("aac"),
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

fn display_arguments(arguments: &[OsString]) -> Vec<String> {
    arguments
        .iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
