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
    // Callers pass validated identifiers, but this helper is generic over its
    // component and the two checks fail independently.
    if !is_single_normal_component(component) {
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
/// The file need not exist; what is rejected is a name that is not one ordinary
/// element, and a link or a directory occupying it. A link here is not merely a
/// write that escapes: the build reads these files back and trusts them, so
/// following one feeds it bytes from outside the workspace and lets a planted
/// entry pass as a cache hit.
///
/// Nothing is created. The caller writes through a staged file and a rename,
/// which replaces the name rather than following it.
pub(crate) fn leaf(directory: &Path, name: &str) -> Result<PathBuf, BuildError> {
    // Before the join rather than after it. Joining an absolute name discards
    // `directory` outright, so a path built first and inspected second is
    // inspected in the wrong place: the link and directory checks below would
    // be reporting on somewhere else entirely.
    if !is_single_normal_component(name) {
        return Err(escape(directory.join(name), directory));
    }

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

/// True when `value` names exactly one ordinary path element.
///
/// Shared by both helpers because both promise containment and neither can keep
/// it once a name has been joined: `Path::join` lets an absolute name replace
/// the root it was given, and a `..` element walks out of it.
///
/// Comparing the element against the whole value is what rejects the spellings
/// `components` would otherwise normalize away — `./name`, `name/`, `name/.` —
/// which name the same file by a route this crate did not choose.
fn is_single_normal_component(value: &str) -> bool {
    let path = Path::new(value);
    let mut components = path.components();

    matches!(
        components.next(),
        Some(Component::Normal(component)) if component == path.as_os_str()
    ) && components.next().is_none()
}

fn escape(path: PathBuf, root: &Path) -> BuildError {
    BuildError::ManagedPathEscape {
        path,
        root: root.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both helpers promise containment and both begin by refusing a name that
    // is not one ordinary element. Held without a filesystem because the
    // refusal precedes every call either helper makes to one, which is the
    // property being asserted.
    #[test]
    fn t1_e0_managed_names_must_be_one_ordinary_path_element() {
        for accepted in ["artifact.json", ".staged", "lesson-1"] {
            assert!(
                is_single_normal_component(accepted),
                "`{accepted}` names one ordinary path element"
            );
        }

        // `..` and an absolute name are the two that escape. The rest reach a
        // file by a spelling this crate did not choose, including the forms
        // `components` normalizes away if the element alone is inspected.
        for rejected in [
            "",
            ".",
            "..",
            "../escape",
            "/absolute",
            "nested/file",
            "./name",
            "name/",
        ] {
            assert!(
                !is_single_normal_component(rejected),
                "`{rejected}` does not name one ordinary path element"
            );

            let managed = Path::new("/managed");
            for (helper, resolved) in [
                ("leaf", leaf(managed, rejected)),
                ("subdirectory", subdirectory(managed, rejected)),
            ] {
                let error = resolved.expect_err("a name that is not one element must not resolve");
                assert!(
                    matches!(error, BuildError::ManagedPathEscape { .. }),
                    "{helper} resolved `{rejected}` to `{error}`"
                );
            }
        }
    }
}
