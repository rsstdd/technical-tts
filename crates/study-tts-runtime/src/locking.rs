//! Cross-process ownership for provisional lesson jobs and cache keys.
//!
//! The validated `lesson_id` is the E0 job identity until E2 introduces the
//! approved versioned job ID. Linux advisory locks provide the live-owner
//! proof; strict records preserve PID and `/proc` start identity for diagnosis
//! and stale-owner audit without treating PID reuse as the same process.

use std::{
    fs::{self, File, OpenOptions},
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use study_tts_core::CacheKey;

use crate::{
    BuildError, DurableStateError, IoError, durable::DurableFileSystem, io_error, managed,
};

/// Provisional lock-record version, deliberately not the E2 job schema.
const JOB_LOCK_SCHEMA_VERSION: &str = "0.1-skeleton-job-lock";

/// Maximum time a cache-key publication waits for another valid producer.
const CACHE_LOCK_TIMEOUT: Duration = Duration::from_secs(30);

/// Grace for lock-record creation and transient pre-exec descriptor
/// inheritance.
const JOB_LOCK_ACQUIRE_GRACE: Duration = Duration::from_millis(100);

/// Poll interval for a bounded cache-key lock wait.
const CACHE_LOCK_POLL: Duration = Duration::from_millis(10);

/// Poll interval for the short provisional job-lock grace.
const JOB_LOCK_POLL: Duration = Duration::from_millis(2);

/// A held provisional lesson job lock.
#[derive(Debug)]
pub(crate) struct JobLock {
    _file: File,
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

/// Acquires the provisional lesson lock and records its Linux process identity.
///
/// # Errors
///
/// [`DurableStateError::LiveJobLock`] when another build owns the lesson,
/// [`DurableStateError::MalformedJobLock`] or
/// [`DurableStateError::IncompatibleJobLock`] when an existing record cannot
/// be trusted, and [`crate::IoError::FileSystem`] or
/// [`crate::IoError::WriteJson`] when the lock cannot be created or persisted.
pub(crate) fn acquire_job_lock(
    filesystem: &dyn DurableFileSystem,
    job_dir: &Path,
    lesson_id: &str,
) -> Result<JobLock, BuildError> {
    let path = managed::leaf(job_dir, "build.lock")?;
    let mut file = open_lock_file(&path)?;
    acquire_job_file_lock(&file, &path)?;

    validate_existing_record(&path, &file, lesson_id)?;
    let record = JobLockRecord {
        schema_version: JOB_LOCK_SCHEMA_VERSION.to_owned(),
        lesson_id: lesson_id.to_owned(),
        pid: std::process::id(),
        process_start: process_start_identity(std::process::id())?,
        created_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| io_error(&path, std::io::Error::other(error)))?
            .as_millis(),
    };
    file.set_len(0).map_err(|error| io_error(&path, error))?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| io_error(&path, error))?;
    serde_json::to_writer_pretty(&mut file, &record).map_err(|source| IoError::WriteJson {
        path: path.clone(),
        source,
    })?;
    file.write_all(b"\n")
        .map_err(|error| io_error(&path, error))?;
    file.sync_all().map_err(|error| io_error(&path, error))?;
    filesystem.sync_directory(job_dir)?;

    Ok(JobLock { _file: file })
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
                let record = read_job_lock_record(path)?;
                return Err(DurableStateError::LiveJobLock {
                    path: path.to_path_buf(),
                    pid: record.pid,
                    process_start: record.process_start,
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

fn validate_existing_record(path: &Path, file: &File, lesson_id: &str) -> Result<(), BuildError> {
    if file
        .metadata()
        .map_err(|error| io_error(path, error))?
        .len()
        == 0
    {
        return Ok(());
    }
    let record = read_job_lock_record(path)?;
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
    Ok(())
}

fn read_job_lock_record(path: &Path) -> Result<JobLockRecord, BuildError> {
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    serde_json::from_slice(&bytes).map_err(|source| {
        DurableStateError::MalformedJobLock {
            path: path.to_path_buf(),
            source,
        }
        .into()
    })
}

fn process_start_identity(pid: u32) -> Result<u64, BuildError> {
    let path = PathBuf::from(format!("/proc/{pid}/stat"));
    let stat = fs::read_to_string(&path).map_err(|error| io_error(&path, error))?;
    parse_process_start(&stat).ok_or_else(|| {
        io_error(
            &path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Linux process stat has no start-time field",
            ),
        )
    })
}

fn parse_process_start(stat: &str) -> Option<u64> {
    let command_end = stat.rfind(") ")?;
    // Field 3 follows the command. Start time is field 22, so it is the
    // twentieth token in the suffix beginning at field 3.
    stat[command_end + 2..]
        .split_ascii_whitespace()
        .nth(19)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::durable::OsDurableFileSystem;

    #[test]
    fn t1_e0_linux_process_start_parser_ignores_spaces_and_parentheses_in_command() {
        let mut fields = vec!["S".to_owned()];
        fields.extend((4..=21).map(|field| field.to_string()));
        fields.push("987654".to_owned());
        let stat = format!("7 (command with ) spaces) {}", fields.join(" "));

        assert_eq!(parse_process_start(&stat), Some(987_654));
    }

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
}
