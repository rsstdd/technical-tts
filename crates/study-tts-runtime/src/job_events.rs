//! Append-only job diagnostics, written only after the state they describe
//! is durable.
//!
//! ADR-0001 §12.3 step 5: "append a diagnostic event after the authoritative
//! state is durable." This module owns the file (`jobs/<job-id>/events.ndjson`,
//! §12.1) and the append; the *ordering* is owned by every caller, which
//! appends only once its durable write has returned `Ok`. Nothing here is
//! reachable from `durable::write_json_atomically`, so a line can never
//! describe a transition a crash discarded.
//!
//! Deliberately an internal diagnostic record, as `preview`'s journal is:
//! DELIVERY-PLAN E2-S4 owns structured observability, per-segment metrics, and
//! whether this becomes a published schema. Per `AGENTS.md` §Security and
//! data an event carries identifiers, states, and hashes — never spoken text,
//! source text, or a voice-reference path.

use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use study_tts_core::JobState;

use crate::{
    BuildError, DurableStateError, IoError, durable::DurableFileSystem, io_error, managed,
};

/// Version of the event line. Listed in
/// `docs/architecture/G1-FREEZE-CHARTER.md` §Deliberately not frozen; E2-S4
/// decides whether it becomes a contract.
pub(crate) const JOB_EVENT_SCHEMA_VERSION: &str = "e2.job-event.0.1";

const EVENT_LOG_NAME: &str = "events.ndjson";
// Mirrored by `docs/architecture/WALKING-SKELETON.md` §Provisional resource
// ceilings; the first bounds disk growth and the second bounds one message.
const MAX_JOB_EVENT_LINE_BYTES: usize = 4 * 1024;
const MAX_JOB_EVENT_LOG_BYTES: usize = 8 * 1024 * 1024;

/// One line of `events.ndjson`.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct JobEvent {
    schema_version: String,
    job_id: String,
    /// Absent for events raised before the job document is read, such as a
    /// lock takeover.
    build_attempt: Option<u32>,
    /// The fact this event records.
    pub(crate) kind: JobEventKind,
    at_unix_ms: u128,
}

/// What durable fact an event records.
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "kind")]
pub(crate) enum JobEventKind {
    /// `job.json` was atomically replaced and now records this state.
    StateDurable { state: JobState },
    /// A lock whose recorded owner was verified gone was taken over.
    JobLockRecovered { pid: u32, process_start: u64 },
}

impl JobEvent {
    /// An event about `job_id`, stamped now.
    pub(crate) fn new(job_id: &str, build_attempt: Option<u32>, kind: JobEventKind) -> Self {
        Self {
            schema_version: JOB_EVENT_SCHEMA_VERSION.to_owned(),
            job_id: job_id.to_owned(),
            build_attempt,
            kind,
            at_unix_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis())
                .unwrap_or_default(),
        }
    }
}

/// Appends one event line and makes it durable.
///
/// Call only after the state the event describes has been made durable; this
/// function cannot know, so the caller's ordering is the guarantee.
///
/// # Errors
///
/// [`DurableStateError::MalformedJobEventLog`] when an existing line is
/// malformed, foreign, or partial, [`DurableStateError::DurableRecordTooLarge`]
/// or [`DurableStateError::JobEventLineTooLarge`] when a configured ceiling is
/// exceeded, in which case the log is preserved; otherwise
/// [`crate::IoError::FileSystem`] or [`crate::IoError::WriteJson`].
pub(crate) fn append_event(
    filesystem: &dyn DurableFileSystem,
    job_dir: &Path,
    event: &JobEvent,
) -> Result<(), BuildError> {
    let path = managed::leaf(job_dir, EVENT_LOG_NAME)?;
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(&path)
        .map_err(|error| io_error(&path, error))?;
    let length = file
        .metadata()
        .map_err(|error| io_error(&path, error))?
        .len();
    validate_event_file(&mut file, &path, length, &event.job_id)?;

    let mut line = serde_json::to_vec(event).map_err(|source| IoError::WriteJson {
        path: path.clone(),
        source,
    })?;
    line.push(b'\n');
    if line.len() > MAX_JOB_EVENT_LINE_BYTES {
        return Err(event_line_too_large(&path));
    }
    if length.saturating_add(line.len() as u64) > MAX_JOB_EVENT_LOG_BYTES as u64 {
        return Err(record_too_large(&path, MAX_JOB_EVENT_LOG_BYTES));
    }
    // The job lock keeps there being only one writer, so a completed append is
    // always one whole line.
    file.write_all(&line)
        .map_err(|error| io_error(&path, error))?;
    file.sync_data().map_err(|error| io_error(&path, error))?;
    if length == 0 {
        // `sync_data` covers the bytes and the size, not the directory entry
        // of a file that did not exist a moment ago.
        filesystem.sync_directory(job_dir)?;
    }
    Ok(())
}

/// Refuses an untrusted event log before authoritative state is changed.
pub(crate) fn validate_event_log(job_dir: &Path, job_id: &str) -> Result<(), BuildError> {
    let path = managed::leaf(job_dir, EVENT_LOG_NAME)?;
    let mut file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error(&path, error)),
    };
    let length = file
        .metadata()
        .map_err(|error| io_error(&path, error))?
        .len();
    validate_event_file(&mut file, &path, length, job_id)
}

/// Parses every existing line before an append or authoritative job load.
fn validate_event_file(
    file: &mut std::fs::File,
    path: &Path,
    length: u64,
    job_id: &str,
) -> Result<(), BuildError> {
    if length == 0 {
        return Ok(());
    }
    if length > MAX_JOB_EVENT_LOG_BYTES as u64 {
        return Err(record_too_large(path, MAX_JOB_EVENT_LOG_BYTES));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|error| io_error(path, error))?;
    let mut reader = BufReader::new(file);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = (&mut reader)
            .take(MAX_JOB_EVENT_LINE_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|error| io_error(path, error))?;
        if read == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n') {
            return Err(malformed_log(path));
        }
        if line.len() > MAX_JOB_EVENT_LINE_BYTES {
            return Err(event_line_too_large(path));
        }
        let event: JobEvent =
            serde_json::from_slice(&line[..line.len() - 1]).map_err(|_| malformed_log(path))?;
        if event.schema_version != JOB_EVENT_SCHEMA_VERSION || event.job_id != job_id {
            return Err(malformed_log(path));
        }
    }
}

fn malformed_log(path: &Path) -> BuildError {
    DurableStateError::MalformedJobEventLog {
        path: path.to_path_buf(),
    }
    .into()
}

fn event_line_too_large(path: &Path) -> BuildError {
    DurableStateError::JobEventLineTooLarge {
        path: path.to_path_buf(),
        max_bytes: MAX_JOB_EVENT_LINE_BYTES,
    }
    .into()
}

fn record_too_large(path: &Path, max_bytes: usize) -> BuildError {
    DurableStateError::DurableRecordTooLarge {
        path: path.to_path_buf(),
        max_bytes,
    }
    .into()
}

/// Parses every line of a job's event log, for tests and diagnostics.
#[cfg(test)]
pub(crate) fn read_events(job_dir: &Path) -> Result<Vec<JobEvent>, BuildError> {
    let path = job_dir.join(EVENT_LOG_NAME);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(&path).map_err(|error| io_error(&path, error))?;
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            serde_json::from_slice(line).map_err(|source| {
                io_error(
                    &path,
                    std::io::Error::new(std::io::ErrorKind::InvalidData, source),
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::durable::OsDurableFileSystem;

    fn event(kind: JobEventKind) -> JobEvent {
        JobEvent::new("lesson", Some(1), kind)
    }

    #[test]
    fn t4_e2_appended_events_are_one_line_each_in_order() {
        let root = TempDir::new().expect("create job directory");
        let first = event(JobEventKind::StateDurable {
            state: JobState::Planned,
        });
        let second = event(JobEventKind::StateDurable {
            state: JobState::Rendering,
        });

        append_event(&OsDurableFileSystem, root.path(), &first).expect("first append");
        append_event(&OsDurableFileSystem, root.path(), &second).expect("second append");

        let events = read_events(root.path()).expect("the log parses");
        assert_eq!(
            events.iter().map(|event| &event.kind).collect::<Vec<_>>(),
            [&first.kind, &second.kind]
        );
        let bytes = fs::read(root.path().join(EVENT_LOG_NAME)).expect("read log");
        assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 2);
    }

    #[test]
    fn t4_e2_a_partial_trailing_line_is_refused_and_preserved() {
        let root = TempDir::new().expect("create job directory");
        let path = root.path().join(EVENT_LOG_NAME);
        fs::write(&path, b"{\"schema_version\":\"e2.job-ev").expect("write torn line");

        let error = append_event(
            &OsDurableFileSystem,
            root.path(),
            &event(JobEventKind::StateDurable {
                state: JobState::Planned,
            }),
        )
        .expect_err("a torn log must not be appended to");

        assert!(matches!(
            error,
            BuildError::DurableState(ref state)
                if matches!(**state, DurableStateError::MalformedJobEventLog { .. })
        ));
        assert_eq!(
            fs::read(&path).expect("log remains"),
            b"{\"schema_version\":\"e2.job-ev"
        );
    }

    #[test]
    fn t4_e2_a_complete_malformed_event_line_is_refused_and_preserved() {
        let valid = event(JobEventKind::StateDurable {
            state: JobState::Planned,
        });
        let mut foreign = serde_json::to_value(&valid).expect("event serializes");
        foreign["job_id"] = serde_json::Value::String("another-job".to_owned());
        let mut unknown = serde_json::to_value(&valid).expect("event serializes");
        unknown["schema_version"] = serde_json::Value::String("future-version".to_owned());

        for mut bytes in [
            b"{not-json}".to_vec(),
            serde_json::to_vec(&foreign).expect("foreign event serializes"),
            serde_json::to_vec(&unknown).expect("unknown event serializes"),
        ] {
            bytes.push(b'\n');
            let root = TempDir::new().expect("create job directory");
            let path = root.path().join(EVENT_LOG_NAME);
            fs::write(&path, &bytes).expect("write untrusted complete line");

            let error = append_event(&OsDurableFileSystem, root.path(), &valid)
                .expect_err("an untrusted complete line must not be authoritative JSON");

            assert!(matches!(
                error,
                BuildError::DurableState(ref state)
                    if matches!(**state, DurableStateError::MalformedJobEventLog { .. })
            ));
            assert_eq!(fs::read(&path).expect("log remains"), bytes);
        }
    }

    #[test]
    fn t4_e2_event_log_limits_are_enforced_before_append() {
        let mut oversized_line = vec![b'x'; MAX_JOB_EVENT_LINE_BYTES];
        oversized_line.push(b'\n');
        let cases = [
            (oversized_line, false, MAX_JOB_EVENT_LINE_BYTES),
            (
                vec![b'x'; MAX_JOB_EVENT_LOG_BYTES + 1],
                true,
                MAX_JOB_EVENT_LOG_BYTES,
            ),
        ];

        for (bytes, whole_log, max_bytes) in cases {
            let root = TempDir::new().expect("create job directory");
            let path = root.path().join(EVENT_LOG_NAME);
            fs::write(&path, &bytes).expect("write oversized event data");

            let error = append_event(
                &OsDurableFileSystem,
                root.path(),
                &event(JobEventKind::StateDurable {
                    state: JobState::Planned,
                }),
            )
            .expect_err("oversized event data must be refused");

            assert!(
                matches!(
                    error,
                    BuildError::DurableState(ref state)
                        if if whole_log {
                            matches!(
                                **state,
                                DurableStateError::DurableRecordTooLarge {
                                    max_bytes: found,
                                    ..
                                } if found == max_bytes
                            )
                        } else {
                            matches!(
                                **state,
                                DurableStateError::JobEventLineTooLarge {
                                    max_bytes: found,
                                    ..
                                } if found == max_bytes
                            )
                        }
                ),
                "wrong limit error for {max_bytes} bytes: {error}"
            );
            assert_eq!(fs::read(path).expect("event log remains"), bytes);
        }
    }
}
