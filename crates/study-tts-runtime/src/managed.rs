//! Resolution of every path this crate creates beneath a root it owns.
//!
//! Centralized because containment is only as good as its least careful call
//! site. Building a path lexically and handing it to `create_dir_all` looks
//! contained — the components are validated, so no `..` or separator can appear
//! — but a symlink planted at any level is followed, and the build then reads
//! and writes outside the workspace it was given.

use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use crate::{BuildError, io_error};

/// Creates `root/component` and proves it stays beneath `root`.
///
/// `root` is always canonical: the workspace is canonicalized by the caller,
/// and each returned path is canonical and becomes the `root` of the next call.
/// Only the final component is therefore unresolved, and it is inspected before
/// anything is created, because `create_dir_all` follows a symlinked leaf and
/// would create the target outside the workspace even though the containment
/// check afterwards rejects the result.
///
/// A window remains between the inspection and the creation. Closing it
/// requires directory-relative `openat` operations and a new dependency, which
/// belongs to the E5-S4 containment story. For a single-user local tool the
/// attacker would already need write access to the workspace, so the
/// check-then-verify pair is proportionate here.
pub(crate) fn subdirectory(root: &Path, component: &str) -> Result<PathBuf, BuildError> {
    // Reject anything that is not a single ordinary path element. Callers pass
    // validated identifiers, but this helper is generic over its component and
    // the two checks fail independently.
    let mut parts = Path::new(component).components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(escape(root.join(component), root));
    }

    let candidate = root.join(component);
    match fs::symlink_metadata(&candidate) {
        // `symlink_metadata` reports the link's own type, so `is_symlink`
        // catches a leaf that would otherwise be followed. The `is_dir` clause
        // rejects a regular file occupying the managed name; that is an
        // obstruction rather than an escape, and it shares this variant only
        // until E5-S4 introduces a dedicated one.
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(escape(candidate, root));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&candidate, error)),
    }

    fs::create_dir_all(&candidate).map_err(|error| io_error(&candidate, error))?;

    // Defence in depth: catches a link planted between the inspection and the
    // creation.
    let resolved = fs::canonicalize(&candidate).map_err(|error| io_error(&candidate, error))?;
    if !resolved.starts_with(root) {
        return Err(escape(resolved, root));
    }
    Ok(resolved)
}

/// Resolves one file inside an already-resolved managed directory.
///
/// The file need not exist; what is rejected is a link or a directory occupying
/// the name. A link here is not merely a write that escapes: the build reads
/// these files back and trusts them, so following one feeds it bytes from
/// outside the workspace and lets a planted entry pass as a cache hit.
///
/// Nothing is created. The caller writes through a staged file and a rename,
/// which replaces the name rather than following it.
pub(crate) fn leaf(directory: &Path, name: &str) -> Result<PathBuf, BuildError> {
    let candidate = directory.join(name);
    match fs::symlink_metadata(&candidate) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(escape(candidate, directory))
        }
        Ok(_) => Ok(candidate),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(candidate),
        Err(error) => Err(io_error(&candidate, error)),
    }
}

fn escape(path: PathBuf, root: &Path) -> BuildError {
    BuildError::ManagedPathEscape {
        path,
        root: root.to_path_buf(),
    }
}
