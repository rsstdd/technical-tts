//! Resolution and identification of the external binaries a build shells out
//! to.
//!
//! Preflight rather than lazy discovery: a build that would fail for a missing
//! encoder says so before it synthesizes anything, and the manifest records
//! the binary that actually ran rather than the one that was requested.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::BuildError;

/// Which external binary a build actually used, for the manifest to record.
#[derive(Clone, Debug)]
pub(crate) struct ToolIdentity {
    /// Absolute path the request resolved to, after `PATH` lookup.
    pub resolved_executable: PathBuf,
    /// First line the tool reports for `-version`.
    pub version: String,
}

/// Resolves an external tool and records its identity, before any work runs.
///
/// Preflight rather than lazy discovery: a build that would fail for a missing
/// encoder must say so before it synthesizes anything, and the manifest must
/// name the binary that actually ran rather than the one that was asked for.
///
/// `-version` is the flag both FFmpeg and ffprobe answer; a tool that does not
/// is not one this function can identify.
///
/// # Errors
///
/// [`BuildError::MissingTool`] when the request resolves to nothing executable,
/// [`BuildError::InspectTool`] when the binary exists but cannot be launched,
/// and [`BuildError::ToolProbeFailed`] when it runs but reports no version this
/// build can record — an unsuccessful exit or empty output alike, since a
/// manifest that names no version cannot say what produced the build.
pub(crate) fn inspect(tool: &str, requested: &Path) -> Result<ToolIdentity, BuildError> {
    let resolved_executable =
        resolve_executable(requested).ok_or_else(|| BuildError::MissingTool {
            tool: tool.to_owned(),
            requested: requested.to_path_buf(),
        })?;
    let output = Command::new(&resolved_executable)
        .arg("-version")
        .output()
        .map_err(|source| BuildError::InspectTool {
            tool: tool.to_owned(),
            executable: resolved_executable.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(BuildError::ToolProbeFailed {
            tool: tool.to_owned(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if version.is_empty() {
        return Err(BuildError::ToolProbeFailed {
            tool: tool.to_owned(),
            status: output.status.to_string(),
            stderr: "version output was empty".to_owned(),
        });
    }

    Ok(ToolIdentity {
        resolved_executable,
        version,
    })
}

/// Finds the binary a request names, searching `PATH` only for a bare name.
///
/// A request carrying any path separator is taken literally, so an operator who
/// names an exact binary gets that one rather than whichever `PATH` prefers.
fn resolve_executable(requested: &Path) -> Option<PathBuf> {
    if requested.components().count() > 1 {
        return executable_file(requested);
    }

    let search_path = env::var_os("PATH")?;
    env::split_paths(&search_path)
        .map(|directory| directory.join(requested))
        .find_map(|candidate| executable_file(&candidate))
}

/// Accepts a candidate only if it is a file this process could execute.
fn executable_file(candidate: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(candidate).ok()?;
    if !metadata.is_file() {
        return None;
    }

    // ADR-0001 targets WSL2, so the executable bit is the meaningful check.
    // Other platforms fall back to "is a file", which is what `Command` would
    // discover anyway, one step later and with a worse message.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }

    fs::canonicalize(candidate).ok()
}
