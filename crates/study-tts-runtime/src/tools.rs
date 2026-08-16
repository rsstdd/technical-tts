use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::BuildError;

#[derive(Clone, Debug)]
pub(crate) struct ToolIdentity {
    pub resolved_executable: PathBuf,
    pub version: String,
}

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

fn resolve_executable(requested: &Path) -> Option<PathBuf> {
    if requested.components().count() > 1 {
        return executable_file(requested);
    }

    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(requested))
        .find_map(|candidate| executable_file(&candidate))
}

fn executable_file(candidate: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(candidate).ok()?;
    if !metadata.is_file() {
        return None;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }

    fs::canonicalize(candidate).ok()
}
