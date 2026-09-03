//! Cross-process ownership for lesson jobs and cache keys.
//!
//! The validated `lesson_id` is the job identity. A Linux advisory lock is the
//! live-owner proof while an owner runs; the strict record beside it names
//! that owner by PID and `/proc` start identity, and is cleared on release.
//! So a record found on a *free* lock means the owner died holding it, and
//! ADR-0001 §12.3's rule applies: verify the owner is gone before taking over.
//! PID reuse is never mistaken for the same process because the start ticks
//! must agree too.

use std::{
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use study_tts_core::CacheKey;

use crate::{
    BuildError, DurableStateError, IoError,
    durable::{DurableFileSystem, read_bounded_bytes},
    io_error,
    job_events::{JobEvent, JobEventKind, append_event},
    managed, process,
};

/// Provisional lock-record version, deliberately not the E2 job schema.
const JOB_LOCK_SCHEMA_VERSION: &str = "0.1-skeleton-job-lock";
// Mirrored by `docs/architecture/WALKING-SKELETON.md` §Provisional resource
// ceilings so an untrusted ownership record is bounded before decoding.
const MAX_JOB_LOCK_BYTES: usize = 4 * 1024;

/// Maximum time a cache-key publication waits for another valid producer.
const CACHE_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Grace for lock-record creation and transient pre-exec descriptor
/// inheritance.
const JOB_LOCK_ACQUIRE_GRACE: Duration = Duration::from_millis(100);

/// Poll interval for a bounded cache-key lock wait.
const CACHE_LOCK_POLL: Duration = Duration::from_millis(10);

/// Poll interval for the short provisional job-lock grace.
const JOB_LOCK_POLL: Duration = Duration::from_millis(2);

/// A held lesson job lock.
#[derive(Debug)]
pub(crate) struct JobLock {
    file: File,
}

impl Drop for JobLock {
    fn drop(&mut self) {
        // Clear the owner record before the descriptor closes and releases
        // the advisory lock, so a record left behind means a crash rather than
        // a completed build. Best-effort by design: the lock itself is the
        // ownership proof, the record is audit, and if this truncation fails
        // the next owner takes the stale path, which verifies rather than
        // assumes.
        let _ = self.file.set_len(0);
    }
}

/// A held cache-key publication lock.
#[derive(Debug)]
pub(crate) struct CacheKeyLock {
    _file: File,
}

/// Strict metadata retained in the job lock file.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct JobLockRecord {
    schema_version: String,
    lesson_id: String,
    pid: u32,
    process_start: u64,
    created_unix_ms: u128,
}

/// Acquires the lesson lock and records this process's Linux identity.
///
/// A record left on a free lock is consulted before it is replaced, as
/// ADR-0001 §12.3 requires: ownership is taken only once the recorded owner
/// is verified gone.
///
/// # Errors
///
/// [`DurableStateError::LiveJobLock`] when another live process owns the
/// lesson — either holding the advisory lock, or named by a record on a free
/// lock and still running; [`DurableStateError::MalformedJobLock`] or
/// [`DurableStateError::IncompatibleJobLock`] when an existing record cannot
/// be trusted, in which case it is preserved; and
/// [`crate::IoError::FileSystem`] or [`crate::IoError::WriteJson`] when the
/// lock cannot be created, this process cannot be identified, or the record
/// cannot be persisted.
pub(crate) fn acquire_job_lock(
    filesystem: &dyn DurableFileSystem,
    job_dir: &Path,
    lesson_id: &str,
) -> Result<JobLock, BuildError> {
    let path = managed::leaf(job_dir, "build.lock")?;
    let file = open_lock_file(&path)?;
    acquire_job_file_lock(&file, &path)?;

    // Malformed and foreign records are refused before liveness is asked: a
    // record that cannot be read is not a record that may be assumed stale.
    let stale_owner = validate_existing_record(&path, lesson_id)?;
    if let Some(record) = &stale_owner
        && recorded_owner_is_live(record)?
    {
        return Err(DurableStateError::LiveJobLock {
            path,
            pid: Some(record.pid),
            process_start: Some(record.process_start),
        }
        .into());
    }
    let record = JobLockRecord {
        schema_version: JOB_LOCK_SCHEMA_VERSION.to_owned(),
        lesson_id: lesson_id.to_owned(),
        pid: std::process::id(),
        process_start: current_process_start(&path)?,
        created_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| io_error(&path, std::io::Error::other(error)))?
            .as_millis(),
    };
    let mut lock = JobLock { file };
    lock.file
        .set_len(0)
        .map_err(|error| io_error(&path, error))?;
    lock.file
        .seek(SeekFrom::Start(0))
        .map_err(|error| io_error(&path, error))?;
    serde_json::to_writer_pretty(&mut lock.file, &record).map_err(|source| IoError::WriteJson {
        path: path.clone(),
        source,
    })?;
    lock.file
        .write_all(b"\n")
        .map_err(|error| io_error(&path, error))?;
    lock.file
        .sync_all()
        .map_err(|error| io_error(&path, error))?;
    filesystem.sync_directory(job_dir)?;
    // Recorded after the new owner record is durable (ADR-0001 §12.3 step
    // 5), so the audit trail names a takeover that actually happened.
    if let Some(stale) = stale_owner {
        append_event(
            filesystem,
            job_dir,
            &JobEvent::new(
                lesson_id,
                None,
                JobEventKind::JobLockRecovered {
                    pid: stale.pid,
                    process_start: stale.process_start,
                },
            ),
        )?;
    }

    Ok(lock)
}

fn acquire_job_file_lock(file: &File, path: &Path) -> Result<(), BuildError> {
    let started = Instant::now();
    loop {
        match try_lock_exclusive(file) {
            Ok(()) => return Ok(()),
            Err(error) if is_contended(error) && started.elapsed() < JOB_LOCK_ACQUIRE_GRACE => {
                thread::sleep(JOB_LOCK_POLL);
            }
            Err(error) if is_contended(error) => {
                // The holder may be mid-release, having cleared its record
                // but not yet closed the descriptor; the lock is still live,
                // only the identity is unknown.
                let record = read_optional_job_lock_record(path)?;
                return Err(DurableStateError::LiveJobLock {
                    path: path.to_path_buf(),
                    pid: record.as_ref().map(|record| record.pid),
                    process_start: record.as_ref().map(|record| record.process_start),
                }
                .into());
            }
            Err(error) => return Err(io_error(path, error.into())),
        }
    }
}

/// Acquires a bounded lock for one content-addressed cache key.
///
/// # Errors
///
/// [`DurableStateError::CacheLockTimeout`] when the owner does not release the
/// key within the bound, otherwise [`crate::IoError::FileSystem`].
pub(crate) fn acquire_cache_key_lock(
    cache_root: &Path,
    cache_key: &CacheKey,
) -> Result<CacheKeyLock, BuildError> {
    let locks = managed::subdirectory(cache_root, "locks")?;
    let shard = managed::subdirectory(&locks, &cache_key.as_str()[..2])?;
    let path = managed::leaf(&shard, &format!("{}.lock", cache_key.as_str()))?;
    let file = open_lock_file(&path)?;
    let started = Instant::now();

    loop {
        match try_lock_exclusive(&file) {
            Ok(()) => return Ok(CacheKeyLock { _file: file }),
            Err(error) if is_contended(error) && started.elapsed() < CACHE_LOCK_TIMEOUT => {
                thread::sleep(CACHE_LOCK_POLL);
            }
            Err(error) if is_contended(error) => {
                return Err(DurableStateError::CacheLockTimeout {
                    path,
                    cache_key: cache_key.clone(),
                    timeout_ms: CACHE_LOCK_TIMEOUT.as_millis() as u64,
                }
                .into());
            }
            Err(error) => return Err(io_error(&path, error.into())),
        }
    }
}

fn open_lock_file(path: &Path) -> Result<File, BuildError> {
    OpenOptions::new()
        .create(true)
        .read(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(|error| io_error(path, error))
}

#[cfg(unix)]
fn try_lock_exclusive(file: &File) -> rustix::io::Result<()> {
    rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
}

#[cfg(not(unix))]
fn try_lock_exclusive(_file: &File) -> rustix::io::Result<()> {
    Err(rustix::io::Errno::NOSYS)
}

fn is_contended(error: rustix::io::Errno) -> bool {
    error == rustix::io::Errno::WOULDBLOCK || error == rustix::io::Errno::AGAIN
}

/// Returns the record a previous owner left, or `None` after a clean release.
fn validate_existing_record(
    path: &Path,
    lesson_id: &str,
) -> Result<Option<JobLockRecord>, BuildError> {
    let Some(record) = read_optional_job_lock_record(path)? else {
        return Ok(None);
    };
    if record.schema_version != JOB_LOCK_SCHEMA_VERSION || record.lesson_id != lesson_id {
        return Err(DurableStateError::IncompatibleJobLock {
            path: path.to_path_buf(),
            schema_version: record.schema_version,
            lesson_id: record.lesson_id,
            required_schema_version: JOB_LOCK_SCHEMA_VERSION,
            required_lesson_id: lesson_id.to_owned(),
        }
        .into());
    }
    Ok(Some(record))
}

/// Reads the lock record; a zero-length file is a released lock, not a
/// malformed one.
fn read_optional_job_lock_record(path: &Path) -> Result<Option<JobLockRecord>, BuildError> {
    let bytes = read_bounded_bytes(path, MAX_JOB_LOCK_BYTES)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    serde_json::from_slice(&bytes).map(Some).map_err(|source| {
        DurableStateError::MalformedJobLock {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

#[cfg(target_os = "linux")]
fn recorded_owner_is_live(record: &JobLockRecord) -> Result<bool, BuildError> {
    // A PID that does not fit the kernel's type names no process that can
    // exist, so its owner is verifiably gone.
    Ok(i32::try_from(record.pid)
        .is_ok_and(|pid| process::process_identity_is_live(pid, record.process_start)))
}

#[cfg(target_os = "linux")]
fn current_process_start(path: &Path) -> Result<u64, BuildError> {
    i32::try_from(std::process::id())
        .ok()
        .and_then(process::read_process_record)
        .map(|record| record.start_time_ticks)
        .ok_or_else(|| unsupported_process_identity(path))
}

// Job ownership needs `/proc`, so off Linux nothing can be recorded or
// verified and acquisition refuses rather than guessing. ADR-0001 §12.3 scopes
// recovery guarantees to the qualified WSL2 Linux filesystem in any case.
#[cfg(not(target_os = "linux"))]
fn recorded_owner_is_live(_record: &JobLockRecord) -> Result<bool, BuildError> {
    Err(unsupported_process_identity(Path::new("/proc")))
}

#[cfg(not(target_os = "linux"))]
fn current_process_start(path: &Path) -> Result<u64, BuildError> {
    Err(unsupported_process_identity(path))
}

fn unsupported_process_identity(path: &Path) -> BuildError {
    io_error(
        path,
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Linux process identity is unavailable, so job ownership cannot be recorded",
        ),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::durable::OsDurableFileSystem;

    #[test]
    fn t4_e0_live_job_lock_is_refused_and_released_owner_is_recoverable() {
        let root = TempDir::new().expect("create lock workspace");
        let filesystem = OsDurableFileSystem;
        let first = acquire_job_lock(&filesystem, root.path(), "lesson").expect("first owner");

        let error = acquire_job_lock(&filesystem, root.path(), "lesson")
            .expect_err("a live owner must be refused");
        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::LiveJobLock { .. })
        ));

        drop(first);
        acquire_job_lock(&filesystem, root.path(), "lesson")
            .expect("a released owner record is recoverable");
    }

    /// A record naming a process that is live right now and is not this one.
    ///
    /// The parent of the test process fits: it is alive for as long as this
    /// test runs, it is never this process, and reading it needs no spawn and
    /// no assumption about `/proc/1`. Linux-only for the same reason
    /// `recorded_owner_is_live` is: `/proc` is where the identity comes from,
    /// and off Linux acquisition refuses before liveness is ever asked.
    #[cfg(target_os = "linux")]
    fn record_naming_live_parent(lesson_id: &str) -> (JobLockRecord, u32) {
        let own_pid = i32::try_from(std::process::id()).expect("the test PID fits");
        let parent = process::read_process_record(own_pid)
            .expect("a running test has a /proc entry")
            .parent_pid;
        let parent_record =
            process::read_process_record(parent).expect("the parent of a running test is live");
        let parent_pid = u32::try_from(parent).expect("a Linux PID is non-negative");
        let record = JobLockRecord {
            schema_version: JOB_LOCK_SCHEMA_VERSION.to_owned(),
            lesson_id: lesson_id.to_owned(),
            pid: parent_pid,
            process_start: parent_record.start_time_ticks,
            created_unix_ms: 0,
        };
        (record, parent_pid)
    }

    fn write_record(path: &Path, record: &JobLockRecord) -> Vec<u8> {
        let bytes = serde_json::to_vec_pretty(record).expect("serialize lock record");
        fs::write(path, &bytes).expect("write lock record");
        bytes
    }

    #[test]
    fn t4_e2_a_released_job_lock_leaves_no_owner_record() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");

        let lock = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson").expect("owner");
        assert!(
            fs::metadata(&path).expect("held lock record").len() > 0,
            "a held lock records its owner"
        );
        drop(lock);

        assert_eq!(
            fs::metadata(&path).expect("released lock record").len(),
            0,
            "a released lock leaves no owner record, so a record present on a free lock means \
             the owner died"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e2_live_lock_is_refused() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");
        let (record, parent_pid) = record_naming_live_parent("lesson");
        let bytes = write_record(&path, &record);

        let error = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson")
            .expect_err("a record naming a live owner must be refused even when the lock is free");

        assert!(
            matches!(
                error,
                BuildError::DurableState(ref state)
                    if matches!(
                        **state,
                        DurableStateError::LiveJobLock { pid: Some(pid), .. } if pid == parent_pid
                    )
            ),
            "{error}"
        );
        assert_eq!(fs::read(&path).expect("record remains"), bytes);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e2_verified_stale_lock_is_recoverable() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");
        let (mut record, parent_pid) = record_naming_live_parent("lesson");
        // The same PID with a start time no process can have: the recorded
        // owner is verifiably gone, whatever process holds that PID now.
        record.process_start = u64::MAX;
        write_record(&path, &record);

        let _lock = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson")
            .expect("a verified stale owner is recoverable");

        let rewritten: JobLockRecord =
            serde_json::from_slice(&fs::read(&path).expect("rewritten record"))
                .expect("the rewritten record parses");
        assert_eq!(rewritten.pid, std::process::id());
        assert_ne!(rewritten.pid, parent_pid);
        let events = crate::job_events::read_events(root.path()).expect("the event log parses");
        assert_eq!(
            events.iter().map(|event| &event.kind).collect::<Vec<_>>(),
            [&JobEventKind::JobLockRecovered {
                pid: parent_pid,
                process_start: u64::MAX,
            }],
            "the takeover is recorded, naming the owner that was verified gone"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e2_failed_stale_takeover_leaves_no_live_owner_record() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");
        let (mut record, _) = record_naming_live_parent("lesson");
        record.process_start = u64::MAX;
        write_record(&path, &record);
        fs::write(root.path().join("events.ndjson"), b"{torn").expect("write torn event log");

        let error = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson")
            .expect_err("the torn event log prevents takeover completion");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::MalformedJobEventLog { .. })
        ));
        assert_eq!(
            fs::metadata(path)
                .expect("lock record remains addressable")
                .len(),
            0,
            "a failed takeover must not leave a free lock naming this live process"
        );
    }

    #[test]
    fn t4_e0_malformed_released_job_lock_is_not_overwritten() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");
        fs::write(&path, b"{broken").expect("write malformed record");

        let error = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson")
            .expect_err("malformed authoritative lock must be refused");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::MalformedJobLock { .. })
        ));
        assert_eq!(fs::read(path).expect("record remains"), b"{broken");
    }

    #[test]
    fn t4_e2_job_lock_record_size_is_bounded_before_decoding() {
        let root = TempDir::new().expect("create lock workspace");
        let path = root.path().join("build.lock");
        let bytes = vec![b' '; MAX_JOB_LOCK_BYTES + 1];
        fs::write(&path, &bytes).expect("write oversized lock record");

        let error = acquire_job_lock(&OsDurableFileSystem, root.path(), "lesson")
            .expect_err("an oversized lock record must be refused");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(
                    **state,
                    DurableStateError::DurableRecordTooLarge {
                        max_bytes: MAX_JOB_LOCK_BYTES,
                        ..
                    }
                )
        ));
        assert_eq!(fs::read(path).expect("lock record remains"), bytes);
    }
}
