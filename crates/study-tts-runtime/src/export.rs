use std::{
    path::{Path, PathBuf},
    process::Command,
};

use tempfile::Builder;

use crate::{BuildError, io_error};

pub(crate) fn export_m4a(
    ffmpeg_executable: &Path,
    master_wav: &Path,
    destination: &Path,
) -> Result<(), BuildError> {
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

    let output = Command::new(ffmpeg_executable)
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(master_wav)
        .args([
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
        ])
        .arg(&staged_path)
        .output()
        .map_err(|source| BuildError::StartFfmpeg {
            executable: PathBuf::from(ffmpeg_executable),
            source,
        })?;
    if !output.status.success() {
        return Err(BuildError::Ffmpeg {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    std::fs::rename(&staged_path, destination).map_err(|error| io_error(destination, error))?;
    Ok(())
}
