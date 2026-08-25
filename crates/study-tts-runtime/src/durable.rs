//! Durable file and directory publication on the qualified Linux filesystem.
//!
//! This module owns the ordering required by ADR-0001 §12.3: file contents
//! become durable before a rename makes them authoritative, and the containing
//! directory is synchronized after every authoritative rename. The trait is a
//! narrow crash-injection seam; domain code still decides what a transaction
//! means and which records are authoritative.

use std::{
    fs::{self, File},
    path::Path,
};

#[cfg(test)]
use std::sync::Mutex;

use serde::Serialize;
use tempfile::Builder;

use crate::{BuildError, IoError, ManagedPathError, io_error};

/// Result of publishing a staged path without replacing an existing winner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RenameOutcome {
    /// The staged path became the destination.
    Published,
    /// Another complete path already owns the destination name.
    DestinationExists,
}

/// Filesystem operations whose ordering carries crash-durability guarantees.
pub(crate) trait DurableFileSystem: std::fmt::Debug {
    /// Synchronizes one regular file, including its metadata.
    fn sync_file(&self, path: &Path) -> Result<(), BuildError>;

    /// Synchronizes one directory after an entry change.
    fn sync_directory(&self, path: &Path) -> Result<(), BuildError>;

    /// Renames `staged` to `destination` only when no destination exists.
    fn rename_noreplace(
        &self,
        staged: &Path,
        destination: &Path,
    ) -> Result<RenameOutcome, BuildError>;

    /// Atomically replaces `destination` with the already-synchronized file.
    fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError>;
}

/// Linux implementation of the durable publication operations.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct OsDurableFileSystem;

impl DurableFileSystem for OsDurableFileSystem {
    fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
        File::open(path)
            .and_then(|file| file.sync_all())
            .map_err(|error| io_error(path, error))
    }

    fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
        File::open(path)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error(path, error))
    }

    #[cfg(target_os = "linux")]
    fn rename_noreplace(
        &self,
        staged: &Path,
        destination: &Path,
    ) -> Result<RenameOutcome, BuildError> {
        use rustix::fs::{CWD, RenameFlags, renameat_with};

        match renameat_with(CWD, staged, CWD, destination, RenameFlags::NOREPLACE) {
            Ok(()) => Ok(RenameOutcome::Published),
            Err(rustix::io::Errno::EXIST) => Ok(RenameOutcome::DestinationExists),
            Err(error) => Err(io_error(destination, error.into())),
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn rename_noreplace(
        &self,
        _staged: &Path,
        destination: &Path,
    ) -> Result<RenameOutcome, BuildError> {
        Err(io_error(
            destination,
            std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "atomic no-replace rename is supported only on Linux",
            ),
        ))
    }

    fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError> {
        fs::rename(staged, destination).map_err(|error| io_error(destination, error))
    }
}

/// Filesystem test seam that records durability operations before delegation.
#[cfg(test)]
#[derive(Debug, Default)]
pub(crate) struct TracingFileSystem {
    inner: OsDurableFileSystem,
    /// Recorded operations in call order.
    pub(crate) events: Mutex<Vec<String>>,
}

#[cfg(test)]
impl DurableFileSystem for TracingFileSystem {
    fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
        self.events
            .lock()
            .expect("trace lock")
            .push(format!("file:{}", path.display()));
        self.inner.sync_file(path)
    }

    fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
        self.events
            .lock()
            .expect("trace lock")
            .push(format!("directory:{}", path.display()));
        self.inner.sync_directory(path)
    }

    fn rename_noreplace(
        &self,
        staged: &Path,
        destination: &Path,
    ) -> Result<RenameOutcome, BuildError> {
        self.events
            .lock()
            .expect("trace lock")
            .push(format!("rename:{}", destination.display()));
        self.inner.rename_noreplace(staged, destination)
    }

    fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError> {
        self.events
            .lock()
            .expect("trace lock")
            .push(format!("replace:{}", destination.display()));
        self.inner.replace_file(staged, destination)
    }
}

/// Writes strict JSON through file sync, atomic replacement, and directory
/// sync.
///
/// # Errors
///
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent,
/// [`IoError::WriteJson`] when serialization fails, or [`IoError::FileSystem`]
/// when staging, synchronization, or replacement fails.
pub(crate) fn write_json_atomically<T: Serialize>(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    value: &T,
) -> Result<(), BuildError> {
    let parent = parent_of(destination)?;
    let mut staged = Builder::new()
        .prefix("json-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    serde_json::to_writer_pretty(staged.as_file_mut(), value).map_err(|source| {
        IoError::WriteJson {
            path: destination.to_path_buf(),
            source,
        }
    })?;
    filesystem.sync_file(staged.path())?;
    let staged_path = staged
        .into_temp_path()
        .keep()
        .map_err(|error| io_error(&error.path, error.error))?;
    filesystem.replace_file(&staged_path, destination)?;
    filesystem.sync_directory(parent)
}

/// Synchronizes a complete staged directory before its publication rename.
///
/// # Errors
///
/// [`IoError::FileSystem`] when a named file or the directory cannot be
/// synchronized.
pub(crate) fn sync_directory_transaction(
    filesystem: &dyn DurableFileSystem,
    directory: &Path,
    files: &[&Path],
) -> Result<(), BuildError> {
    for file in files {
        filesystem.sync_file(file)?;
    }
    filesystem.sync_directory(directory)
}

/// Publishes a synchronized sibling directory without replacing a winner.
///
/// # Errors
///
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent,
/// or [`IoError::FileSystem`] when the rename or parent synchronization fails.
pub(crate) fn publish_directory_noreplace(
    filesystem: &dyn DurableFileSystem,
    staged: &Path,
    destination: &Path,
) -> Result<RenameOutcome, BuildError> {
    let parent = parent_of(destination)?;
    let outcome = filesystem.rename_noreplace(staged, destination)?;
    if outcome == RenameOutcome::Published {
        filesystem.sync_directory(parent)?;
    }
    Ok(outcome)
}

fn parent_of(path: &Path) -> Result<&Path, BuildError> {
    path.parent().ok_or_else(|| {
        ManagedPathError::UnrootedDestination {
            path: path.to_path_buf(),
        }
        .into()
    })
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use tempfile::TempDir;

    use super::*;

    #[derive(Serialize)]
    struct Record {
        value: u8,
    }

    #[derive(Debug, Default)]
    struct FailingReplacementFileSystem {
        inner: OsDurableFileSystem,
    }

    impl DurableFileSystem for FailingReplacementFileSystem {
        fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_file(path)
        }

        fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_directory(path)
        }

        fn rename_noreplace(
            &self,
            staged: &Path,
            destination: &Path,
        ) -> Result<RenameOutcome, BuildError> {
            self.inner.rename_noreplace(staged, destination)
        }

        fn replace_file(&self, _staged: &Path, destination: &Path) -> Result<(), BuildError> {
            Err(io_error(
                destination,
                std::io::Error::other("injected replacement interruption"),
            ))
        }
    }

    #[test]
    fn t4_e0_durable_json_replacement_flushes_file_then_rename_then_parent() {
        let root = TempDir::new().expect("create durable workspace");
        let destination = root.path().join("record.json");
        let filesystem = TracingFileSystem::default();

        write_json_atomically(&filesystem, &destination, &Record { value: 7 })
            .expect("write durable JSON");

        let events = filesystem.events.lock().expect("trace lock");
        assert_eq!(events.len(), 3);
        assert!(events[0].starts_with("file:"));
        assert_eq!(events[1], format!("replace:{}", destination.display()));
        assert_eq!(events[2], format!("directory:{}", root.path().display()));
    }

    #[test]
    fn t4_e0_directory_publication_flushes_files_before_rename_and_parent() {
        let root = TempDir::new().expect("create durable workspace");
        let staged = root.path().join("staged");
        let destination = root.path().join("published");
        fs::create_dir(&staged).expect("create stage");
        let first = staged.join("audio.wav");
        let second = staged.join("artifact.json");
        fs::write(&first, b"audio").expect("write first file");
        fs::write(&second, b"artifact").expect("write second file");
        let filesystem = TracingFileSystem::default();

        sync_directory_transaction(&filesystem, &staged, &[&first, &second])
            .expect("sync transaction");
        publish_directory_noreplace(&filesystem, &staged, &destination)
            .expect("publish transaction");

        let events = filesystem.events.lock().expect("trace lock");
        assert_eq!(events[0], format!("file:{}", first.display()));
        assert_eq!(events[1], format!("file:{}", second.display()));
        assert_eq!(events[2], format!("directory:{}", staged.display()));
        assert_eq!(events[3], format!("rename:{}", destination.display()));
        assert_eq!(events[4], format!("directory:{}", root.path().display()));
    }

    #[test]
    fn t4_e0_interrupted_json_replacement_preserves_prior_authoritative_record() {
        let root = TempDir::new().expect("create durable workspace");
        let destination = root.path().join("current.json");
        fs::write(&destination, b"prior-current").expect("write prior record");

        let error = write_json_atomically(
            &FailingReplacementFileSystem::default(),
            &destination,
            &Record { value: 9 },
        )
        .expect_err("injected replacement must fail");

        assert!(matches!(error, BuildError::Io(IoError::FileSystem { .. })));
        assert_eq!(
            fs::read(&destination).expect("read prior record"),
            b"prior-current"
        );
        let retained = fs::read_dir(root.path())
            .expect("read durable workspace")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path != &destination)
            .collect::<Vec<_>>();
        assert_eq!(retained.len(), 1);
        assert!(retained[0].is_file());
    }
}
