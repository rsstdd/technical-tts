//! Durable file and directory publication on the qualified Linux filesystem.
//!
//! This module owns the ordering required by ADR-0001 §12.3: file contents
//! become durable before a rename makes them authoritative, and the containing
//! directory is synchronized after every authoritative rename. The trait is a
//! narrow crash-injection seam; domain code still decides what a transaction
//! means and which records are authoritative.

use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

#[cfg(test)]
use std::sync::Mutex;

use serde::Serialize;
use tempfile::Builder;

use crate::{BuildError, DurableStateError, IoError, ManagedPathError, io_error};

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
    replace_atomically(filesystem, destination, |staged| {
        serde_json::to_writer_pretty(staged, value).map_err(|source| {
            IoError::WriteJson {
                path: destination.to_path_buf(),
                source,
            }
            .into()
        })
    })
}

/// Writes exact bytes through the same sync, replace, sync sequence.
///
/// For a document this build validated as bytes and retains as bytes — the
/// lesson beside `job.json` — so what resume reads back is what was checked,
/// not a re-serialization of it.
///
/// # Errors
///
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent,
/// or [`IoError::FileSystem`] when staging, synchronization, or replacement
/// fails.
pub(crate) fn write_bytes_atomically(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), BuildError> {
    replace_atomically(filesystem, destination, |staged| {
        staged
            .write_all(bytes)
            .map_err(|error| io_error(destination, error))
    })
}

/// Reads a durable record without allocating beyond its boundary's ceiling.
///
/// # Errors
///
/// [`DurableStateError::DurableRecordTooLarge`] when the file advertises or
/// yields more than `max_bytes`, or [`IoError::FileSystem`] when it cannot be
/// opened, inspected, or read.
pub(crate) fn read_bounded_bytes(path: &Path, max_bytes: usize) -> Result<Vec<u8>, BuildError> {
    let file = File::open(path).map_err(|error| io_error(path, error))?;
    let advertised = file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .len();
    if advertised > max_bytes as u64 {
        return Err(DurableStateError::DurableRecordTooLarge {
            path: path.to_path_buf(),
            max_bytes,
        }
        .into());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(advertised).unwrap_or(max_bytes));
    file.take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(path, error))?;
    if bytes.len() > max_bytes {
        return Err(DurableStateError::DurableRecordTooLarge {
            path: path.to_path_buf(),
            max_bytes,
        }
        .into());
    }
    Ok(bytes)
}

fn replace_atomically(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    write: impl FnOnce(&mut File) -> Result<(), BuildError>,
) -> Result<(), BuildError> {
    let parent = parent_of(destination)?;
    let mut staged = Builder::new()
        .prefix("json-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    write(staged.as_file_mut())?;
    filesystem.sync_file(staged.path())?;
    let staged_path = staged
        .into_temp_path()
        .keep()
        .map_err(|error| io_error(&error.path, error.error))?;
    filesystem.replace_file(&staged_path, destination)?;
    filesystem.sync_directory(parent)
}

/// Writes bytes through file sync and an atomic rename that claims an unused
/// name.
///
/// The caller receives the outcome rather than an error for a taken
/// destination, because only the caller knows whether losing the race is a
/// refusal or a no-op. `study-tts lesson new` treats it as a refusal;
/// [`crate::authoring::scaffold_lesson`] is the sentence that says so.
///
/// Bytes rather than a `Serialize` value, unlike [`write_json_atomically`], so
/// a caller can validate the exact bytes it is about to publish. Serializing
/// twice would leave the document that was checked and the document that was
/// written as two artifacts nothing holds together.
///
/// # Errors
///
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent,
/// or [`IoError::FileSystem`] when staging, writing, synchronization, or the
/// rename fails.
pub(crate) fn write_bytes_noreplace(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    bytes: &[u8],
) -> Result<RenameOutcome, BuildError> {
    let parent = parent_of(destination)?;
    // The published mode comes from the staged temporary, which `tempfile`
    // creates `0600`, and the rename carries it to the destination.
    // `t4_e1_a_scaffold_is_published_owner_readable_only` is what holds it, so
    // a change to plain creation here fails rather than quietly widening the
    // mode on a file holding an author's own work.
    let mut staged = Builder::new()
        .prefix("authored-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    staged
        .as_file_mut()
        .write_all(bytes)
        .map_err(|error| io_error(destination, error))?;
    filesystem.sync_file(staged.path())?;
    let staged_path = staged.into_temp_path();

    let outcome = filesystem.rename_noreplace(&staged_path, destination)?;
    if outcome == RenameOutcome::Published {
        filesystem.sync_directory(parent)?;
    }
    Ok(outcome)
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

/// The directory a destination is staged in.
///
/// `Path::parent` answers an empty path for a bare file name, which names the
/// current directory rather than a missing one. Staging into `""` fails with
/// `ENOENT`, so without this an author writing `--out lesson.json` would be
/// refused a destination that is perfectly writable.
fn parent_of(path: &Path) -> Result<&Path, BuildError> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(ManagedPathError::UnrootedDestination {
            path: path.to_path_buf(),
        }
        .into()),
    }
}

/// Crash-injection filesystems shared by the tests of every module that
/// publishes through this one. Test-only and crate-private on purpose: the
/// seam exists to prove ordering, not to become API.
#[cfg(test)]
pub(crate) mod fault {
    use std::path::Path;

    use super::{DurableFileSystem, OsDurableFileSystem, RenameOutcome};
    use crate::{BuildError, io_error};

    /// Fails the authoritative `replace_file`, after staging and file sync.
    #[derive(Debug, Default)]
    pub(crate) struct FailingReplacementFileSystem {
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
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use tempfile::TempDir;

    use super::{fault::FailingReplacementFileSystem, *};

    #[derive(Serialize)]
    struct Record {
        value: u8,
    }

    #[derive(Debug, Default)]
    struct FailingRenameFileSystem {
        inner: OsDurableFileSystem,
    }

    impl DurableFileSystem for FailingRenameFileSystem {
        fn sync_file(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_file(path)
        }

        fn sync_directory(&self, path: &Path) -> Result<(), BuildError> {
            self.inner.sync_directory(path)
        }

        fn rename_noreplace(
            &self,
            _staged: &Path,
            destination: &Path,
        ) -> Result<RenameOutcome, BuildError> {
            Err(io_error(
                destination,
                std::io::Error::other("injected rename interruption"),
            ))
        }

        fn replace_file(&self, staged: &Path, destination: &Path) -> Result<(), BuildError> {
            self.inner.replace_file(staged, destination)
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
    fn t4_e1_durable_byte_publication_flushes_file_then_rename_then_parent() {
        let root = TempDir::new().expect("create durable workspace");
        let destination = root.path().join("lesson.json");
        let filesystem = TracingFileSystem::default();

        let outcome = write_bytes_noreplace(&filesystem, &destination, b"authored")
            .expect("publish durable bytes");

        assert_eq!(outcome, RenameOutcome::Published);
        let events = filesystem.events.lock().expect("trace lock");
        assert_eq!(events.len(), 3);
        assert!(events[0].starts_with("file:"));
        assert_eq!(events[1], format!("rename:{}", destination.display()));
        assert_eq!(events[2], format!("directory:{}", root.path().display()));
    }

    #[test]
    fn t4_e1_a_taken_destination_keeps_its_bytes_and_leaves_no_staged_file() {
        let root = TempDir::new().expect("create durable workspace");
        let destination = root.path().join("lesson.json");
        fs::write(&destination, b"the author's own bytes").expect("write prior document");
        let filesystem = TracingFileSystem::default();

        let outcome = write_bytes_noreplace(&filesystem, &destination, b"replacement")
            .expect("a taken destination is an outcome, not an error");

        assert_eq!(outcome, RenameOutcome::DestinationExists);
        assert_eq!(
            fs::read(&destination).expect("read prior document"),
            b"the author's own bytes"
        );
        assert_eq!(
            fs::read_dir(root.path())
                .expect("read durable workspace")
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .collect::<Vec<_>>(),
            [destination],
            "a refused publication leaves no staged sibling behind"
        );
        // The parent is not synchronized, because nothing about it changed.
        let events = filesystem.events.lock().expect("trace lock");
        assert_eq!(events.len(), 2);
        assert!(events[0].starts_with("file:"));
        assert!(events[1].starts_with("rename:"));
    }

    #[test]
    fn t4_e1_a_failed_byte_publication_leaves_no_destination_or_staged_file() {
        let root = TempDir::new().expect("create durable workspace");
        let destination = root.path().join("lesson.json");

        let error = write_bytes_noreplace(
            &FailingRenameFileSystem::default(),
            &destination,
            b"authored",
        )
        .expect_err("injected rename must fail");

        let BuildError::Io(IoError::FileSystem { path, source }) = error else {
            panic!("rename failure used the wrong error class: {error}");
        };
        assert_eq!(path, destination);
        assert_eq!(source.kind(), std::io::ErrorKind::Other);
        assert!(
            !destination.exists(),
            "a failed rename created its destination"
        );
        let staged = fs::read_dir(root.path())
            .expect("read durable workspace")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter(|name| {
                let name = name.to_string_lossy();
                name.starts_with("authored-") && name.ends_with(".tmp")
            })
            .collect::<Vec<_>>();
        assert!(
            staged.is_empty(),
            "a failed rename left a staged sibling behind"
        );
    }

    /// A bare file name resolves to the current directory, not to `""`.
    ///
    /// The one case no caller inside a workspace root can reach, and the one
    /// an author typing `--out lesson.json` reaches first: `tempfile_in("")`
    /// fails with `ENOENT`, so before this the refusal named a destination that
    /// was perfectly writable. Asserted on `parent_of` rather than by
    /// publishing a relative path, because the current directory is
    /// process-wide state and these tests run concurrently.
    #[test]
    fn t1_e1_a_destination_with_no_directory_component_stages_in_the_current_directory() {
        assert_eq!(
            parent_of(Path::new("lesson.json")).expect("a bare file name has somewhere to stage"),
            Path::new("."),
        );
        assert_eq!(
            parent_of(Path::new("lessons/lesson.json")).expect("a relative path keeps its parent"),
            Path::new("lessons"),
        );
        assert!(
            matches!(
                parent_of(Path::new("/")),
                Err(BuildError::ManagedPath(
                    ManagedPathError::UnrootedDestination { .. }
                ))
            ),
            "a path with no parent at all is still refused"
        );
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
