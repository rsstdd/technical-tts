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

use crate::{BuildError, ManagedPathError, io_error};

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
///
/// # Errors
///
/// [`ManagedPathError::InvalidManagedName`] when `component` is not one
/// ordinary path element, before anything is created.
/// [`ManagedPathError::ManagedPathEscape`] when a symlink or a non-directory
/// occupies the name, or when the created directory resolves outside `root`.
/// Otherwise [`crate::IoError::FileSystem`] carries what the filesystem
/// reported.
pub(crate) fn subdirectory(root: &Path, component: &str) -> Result<PathBuf, BuildError> {
    let candidate = directory_candidate(root, component)?;
    if candidate.is_dir() {
        return canonicalize_contained_directory(root, &candidate);
    }
    match fs::create_dir(&candidate) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // Another valid local producer may win this exact creation race.
            // Revalidate its occupant so a link or file cannot borrow that
            // benign race handling.
            let _ = directory_candidate(root, component)?;
        }
        Err(error) => return Err(io_error(&candidate, error)),
    }

    // Defence in depth: catches a link planted between the inspection and the
    // creation.
    canonicalize_contained_directory(root, &candidate)
}

fn canonicalize_contained_directory(root: &Path, candidate: &Path) -> Result<PathBuf, BuildError> {
    let resolved = fs::canonicalize(candidate).map_err(|error| io_error(candidate, error))?;
    if !resolved.starts_with(root) {
        return Err(escape(resolved, root));
    }
    Ok(resolved)
}

/// Resolves a managed child-directory name without creating it.
///
/// An existing directory is accepted; a missing one is returned lexically so
/// a caller can use it as an atomic rename destination. Links and non-directory
/// occupants are refused before publication.
///
/// # Errors
///
/// [`ManagedPathError::InvalidManagedName`] when `component` is not one path
/// element, [`ManagedPathError::ManagedPathEscape`] when an existing occupant
/// is not a real directory, otherwise [`crate::IoError::FileSystem`].
pub(crate) fn directory_candidate(root: &Path, component: &str) -> Result<PathBuf, BuildError> {
    validate_managed_name(root, component)?;

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
    Ok(candidate)
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
///
/// # Errors
///
/// [`ManagedPathError::InvalidManagedName`] when `name` is not one ordinary
/// path element. [`ManagedPathError::ManagedPathEscape`] when a symlink or a
/// directory occupies the name. Otherwise [`crate::IoError::FileSystem`]
/// carries what the filesystem reported.
pub(crate) fn leaf(directory: &Path, name: &str) -> Result<PathBuf, BuildError> {
    validate_managed_name(directory, name)?;

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

/// Refuses a managed name that is not exactly one ordinary path element.
///
/// Runs before either helper joins anything, because `Path::join` lets an
/// absolute name replace the root it was given: a path built first and
/// inspected second is inspected somewhere else entirely, and the link and
/// directory checks would be reporting on the wrong file.
///
/// # Errors
///
/// [`ManagedPathError::InvalidManagedName`], and never
/// [`ManagedPathError::ManagedPathEscape`]. Most of what this refuses — `""`,
/// `"./name"`, `"name/"` — names a file inside the root by a route this crate
/// did not choose, so calling it an escape would report an attack that did not
/// happen. `".."` and an absolute name would escape, but are refused here on
/// their spelling before any path exists to contain.
fn validate_managed_name(root: &Path, name: &str) -> Result<(), BuildError> {
    if is_single_normal_component(name) {
        return Ok(());
    }

    Err(ManagedPathError::InvalidManagedName {
        name: name.to_owned(),
        root: root.to_path_buf(),
    }
    .into())
}

/// True when `value` names exactly one ordinary path element.
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

/// Reports a path that resolved outside the root it was confined to.
///
/// Reached only once the lexical contract has held, so every caller has a real
/// path or a planted link rather than a name that was merely spelled wrong.
fn escape(path: PathBuf, root: &Path) -> BuildError {
    ManagedPathError::ManagedPathEscape {
        path,
        root: root.to_path_buf(),
    }
    .into()
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

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

        // `..` and an absolute name are the two that could escape; the rest
        // reach a file inside the root by a spelling this crate did not choose,
        // including the forms `components` normalizes away if the element alone
        // is inspected. All of them fail the same lexical contract, so all of
        // them must arrive as that failure rather than as a containment
        // breach that did not happen.
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
                    matches!(
                        error,
                        BuildError::ManagedPath(ManagedPathError::InvalidManagedName { .. })
                    ),
                    "{helper} resolved `{rejected}` to `{error}`"
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_existing_managed_directory_must_resolve_beneath_root() {
        let workspace = TempDir::new().expect("create managed-path workspace");
        let outside = TempDir::new().expect("create outside directory");
        let linked_root = workspace.path().join("linked-root");
        symlink(outside.path(), &linked_root).expect("link managed root outside");
        fs::create_dir(outside.path().join("existing")).expect("create existing directory");

        let error = subdirectory(&linked_root, "existing")
            .expect_err("resolved directory outside the supplied root must be rejected");

        assert!(matches!(
            error,
            BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
        ));
    }
}
