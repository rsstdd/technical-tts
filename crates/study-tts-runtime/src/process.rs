//! Bounded supervision for external child processes.
//!
//! This module owns deadlines, concurrent pipe capture, and process-tree
//! cleanup. Callers retain launch and nonzero-exit error ownership so their
//! established distinctions remain intact.

use std::{
    io::{self, Read},
    process::{Child, Command, ExitStatus, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::fs;

#[cfg(unix)]
use std::os::fd::AsFd;

use thiserror::Error;

use crate::{ToolError, ToolInvocation, ToolOutputStream};

/// Bytes captured independently from each external-tool output stream.
///
/// This provisional ceiling mirrors
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
const TOOL_OUTPUT_LIMIT_BYTES: usize = 1024 * 1024;

/// Version discovery deadline mirrored by
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
pub(crate) const VERSION_PROBE_POLICY: CommandPolicy = CommandPolicy::new(Duration::from_secs(5));

/// Worker-environment integrity deadline mirrored by
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
pub(crate) const WORKER_ENVIRONMENT_PROBE_POLICY: CommandPolicy =
    CommandPolicy::new(Duration::from_secs(2 * 60));

/// Encoded-output deadline mirrored by
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
pub(crate) const FFPROBE_POLICY: CommandPolicy = CommandPolicy::new(Duration::from_secs(30));

/// Encode deadline mirrored by
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings.
pub(crate) const FFMPEG_ENCODE_POLICY: CommandPolicy =
    CommandPolicy::new(Duration::from_secs(30 * 60));

/// Keeps timeout and overflow response prompt without busy-spinning.
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Allows ordinary EOF events to arrive before retained pipes imply a child.
const DESCENDANT_PIPE_GRACE: Duration = Duration::from_millis(100);

/// Bounds pipe shutdown and Unix process-group disappearance after a kill.
const TERMINATION_OBSERVATION_GRACE: Duration = Duration::from_secs(1);

/// Deadline and independent pipe ceilings for one command execution.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CommandPolicy {
    deadline: Duration,
    stdout_limit: usize,
    stderr_limit: usize,
}

impl CommandPolicy {
    const fn new(deadline: Duration) -> Self {
        Self {
            deadline,
            stdout_limit: TOOL_OUTPUT_LIMIT_BYTES,
            stderr_limit: TOOL_OUTPUT_LIMIT_BYTES,
        }
    }
}

/// Bounded output and exit status from a supervised child process.
#[derive(Debug)]
pub(crate) struct CommandOutput {
    /// Exit status reported by the direct child.
    pub status: ExitStatus,
    /// Bytes captured from standard output.
    pub stdout: Vec<u8>,
    /// Bytes captured from standard error.
    pub stderr: Vec<u8>,
}

/// A launch failure kept separate from failures after supervision begins.
#[derive(Debug, Error)]
pub(crate) enum CommandRunError {
    /// The direct child could not be launched.
    #[error("could not start command: {0}")]
    Start(#[source] io::Error),
    /// A running child violated or escaped its supervision policy.
    #[error(transparent)]
    Supervision(#[from] ToolError),
}

#[derive(Debug)]
enum CaptureEvent {
    Complete {
        stream: ToolOutputStream,
        bytes: Vec<u8>,
    },
    Overflow {
        stream: ToolOutputStream,
        limit_bytes: usize,
    },
    Failed {
        stream: ToolOutputStream,
        source: io::Error,
    },
    Cancelled {
        stream: ToolOutputStream,
    },
}

#[derive(Debug, Default)]
struct CaptureState {
    stdout: Option<Vec<u8>>,
    stderr: Option<Vec<u8>>,
    stdout_done: bool,
    stderr_done: bool,
}

impl CaptureState {
    fn apply(&mut self, invocation: &ToolInvocation, event: CaptureEvent) -> Option<ToolError> {
        match event {
            CaptureEvent::Complete { stream, bytes } => {
                self.mark_complete(stream, bytes);
                None
            }
            CaptureEvent::Overflow {
                stream,
                limit_bytes,
            } => {
                self.mark_done(stream);
                Some(ToolError::ToolOutputOverflow {
                    invocation: invocation.clone(),
                    stream,
                    limit_bytes,
                })
            }
            CaptureEvent::Failed { stream, source } => {
                self.mark_done(stream);
                Some(ToolError::ToolCaptureReadFailed {
                    invocation: invocation.clone(),
                    stream,
                    source,
                })
            }
            CaptureEvent::Cancelled { stream } => {
                self.mark_done(stream);
                None
            }
        }
    }

    fn mark_complete(&mut self, stream: ToolOutputStream, bytes: Vec<u8>) {
        match stream {
            ToolOutputStream::Stdout => {
                self.stdout = Some(bytes);
                self.stdout_done = true;
            }
            ToolOutputStream::Stderr => {
                self.stderr = Some(bytes);
                self.stderr_done = true;
            }
        }
    }

    fn mark_done(&mut self, stream: ToolOutputStream) {
        match stream {
            ToolOutputStream::Stdout => self.stdout_done = true,
            ToolOutputStream::Stderr => self.stderr_done = true,
        }
    }

    const fn is_done(&self) -> bool {
        self.stdout_done && self.stderr_done
    }

    fn into_output(
        self,
        invocation: &ToolInvocation,
        status: ExitStatus,
    ) -> Result<CommandOutput, ToolError> {
        let stdout = self
            .stdout
            .ok_or_else(|| ToolError::ToolCaptureIncomplete {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stdout,
            })?;
        let stderr = self
            .stderr
            .ok_or_else(|| ToolError::ToolCaptureIncomplete {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stderr,
            })?;
        Ok(CommandOutput {
            status,
            stdout,
            stderr,
        })
    }
}

#[derive(Debug)]
struct CaptureWorker {
    stream: ToolOutputStream,
    handle: JoinHandle<()>,
}

#[derive(Debug)]
struct SupervisedChild {
    invocation: ToolInvocation,
    child: Child,
    ownership: ProcessOwnership,
    receiver: Receiver<CaptureEvent>,
    cancel: Arc<AtomicBool>,
    capture_workers: Vec<CaptureWorker>,
}

#[cfg(target_os = "linux")]
#[derive(Debug, Default)]
struct ProcessOwnership {
    root_pid: Option<i32>,
    descendants: Vec<OwnedProcess>,
}

// `/proc` proves ancestry while the pidfd binds later signals to that exact
// process, even after reparenting or numeric PID reuse.
#[cfg(target_os = "linux")]
#[derive(Debug)]
struct OwnedProcess {
    pid: i32,
    start_time_ticks: u64,
    pidfd: rustix::fd::OwnedFd,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug)]
struct ProcessRecord {
    pid: i32,
    parent_pid: i32,
    start_time_ticks: u64,
}

#[cfg(not(target_os = "linux"))]
#[derive(Debug, Default)]
struct ProcessOwnership;

/// Runs one command with concurrent bounded capture and process-tree cleanup.
///
/// # Errors
///
/// [`CommandRunError::Start`] when the child cannot be launched. Supervision
/// returns the distinct [`ToolError`] variant for the violated execution,
/// capture, containment, or reaping invariant.
pub(crate) fn run(
    invocation: ToolInvocation,
    command: Command,
    policy: CommandPolicy,
) -> Result<CommandOutput, CommandRunError> {
    run_with_capture_spawner(invocation, command, policy, spawn_capture_pipe)
}

fn run_with_capture_spawner<F>(
    invocation: ToolInvocation,
    mut command: Command,
    policy: CommandPolicy,
    mut spawn_capture: F,
) -> Result<CommandOutput, CommandRunError>
where
    F: FnMut(
        Box<dyn Read + Send>,
        ToolOutputStream,
        usize,
        Sender<CaptureEvent>,
        Arc<AtomicBool>,
    ) -> io::Result<JoinHandle<()>>,
{
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_process_group(&mut command);

    let started = Instant::now();
    let mut child = command.spawn().map_err(CommandRunError::Start)?;
    let ownership = match ProcessOwnership::for_child(&child) {
        Ok(ownership) => ownership,
        Err(source) => {
            let error = ToolError::ToolContainmentInspectionFailed {
                invocation: invocation.clone(),
                source,
            };
            return Err(cleanup_setup_failure(
                invocation,
                child,
                ProcessOwnership::default(),
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
                error,
            ));
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let error = ToolError::ToolPipeUnavailable {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stdout,
            };
            return Err(cleanup_setup_failure(
                invocation,
                child,
                ownership,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
                error,
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            let error = ToolError::ToolPipeUnavailable {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stderr,
            };
            return Err(cleanup_setup_failure(
                invocation,
                child,
                ownership,
                Vec::new(),
                Arc::new(AtomicBool::new(false)),
                error,
            ));
        }
    };

    if let Err(source) = configure_capture_pipe(&stdout) {
        let error = ToolError::ToolCaptureConfigurationFailed {
            invocation: invocation.clone(),
            stream: ToolOutputStream::Stdout,
            source,
        };
        return Err(cleanup_setup_failure(
            invocation,
            child,
            ownership,
            Vec::new(),
            Arc::new(AtomicBool::new(false)),
            error,
        ));
    }
    if let Err(source) = configure_capture_pipe(&stderr) {
        let error = ToolError::ToolCaptureConfigurationFailed {
            invocation: invocation.clone(),
            stream: ToolOutputStream::Stderr,
            source,
        };
        return Err(cleanup_setup_failure(
            invocation,
            child,
            ownership,
            Vec::new(),
            Arc::new(AtomicBool::new(false)),
            error,
        ));
    }

    let (sender, receiver) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let stdout_thread = match spawn_capture(
        Box::new(stdout),
        ToolOutputStream::Stdout,
        policy.stdout_limit,
        sender.clone(),
        Arc::clone(&cancel),
    ) {
        Ok(thread) => thread,
        Err(source) => {
            let error = ToolError::ToolCaptureStartFailed {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stdout,
                source,
            };
            return Err(cleanup_setup_failure(
                invocation,
                child,
                ownership,
                Vec::new(),
                cancel,
                error,
            ));
        }
    };
    let mut capture_workers = vec![CaptureWorker {
        stream: ToolOutputStream::Stdout,
        handle: stdout_thread,
    }];
    let stderr_thread = match spawn_capture(
        Box::new(stderr),
        ToolOutputStream::Stderr,
        policy.stderr_limit,
        sender,
        Arc::clone(&cancel),
    ) {
        Ok(thread) => thread,
        Err(source) => {
            let error = ToolError::ToolCaptureStartFailed {
                invocation: invocation.clone(),
                stream: ToolOutputStream::Stderr,
                source,
            };
            return Err(cleanup_setup_failure(
                invocation,
                child,
                ownership,
                capture_workers,
                cancel,
                error,
            ));
        }
    };
    capture_workers.push(CaptureWorker {
        stream: ToolOutputStream::Stderr,
        handle: stderr_thread,
    });
    supervise(
        SupervisedChild {
            invocation,
            child,
            ownership,
            receiver,
            cancel,
            capture_workers,
        },
        policy,
        started,
    )
}

fn supervise(
    mut supervised: SupervisedChild,
    policy: CommandPolicy,
    started: Instant,
) -> Result<CommandOutput, CommandRunError> {
    let mut child_exited = false;
    let mut child_exited_at = None;
    let mut capture = CaptureState::default();

    loop {
        if let Err(source) = supervised.ownership.refresh() {
            let error = ToolError::ToolContainmentInspectionFailed {
                invocation: supervised.invocation.clone(),
                source,
            };
            return finish(supervised, Some(error), capture);
        }
        while let Ok(event) = supervised.receiver.try_recv() {
            if let Some(error) = capture.apply(&supervised.invocation, event) {
                return finish(supervised, Some(error), capture);
            }
        }

        if !child_exited {
            match child_has_exited(&mut supervised.child) {
                Ok(true) => {
                    child_exited = true;
                    child_exited_at = Some(Instant::now());
                }
                Ok(false) => {}
                Err(source) => {
                    let error = ToolError::ToolChildInspectionFailed {
                        invocation: supervised.invocation.clone(),
                        source,
                    };
                    return finish(supervised, Some(error), capture);
                }
            }
        }

        if started.elapsed() >= policy.deadline {
            let timeout_ms = u64::try_from(policy.deadline.as_millis()).unwrap_or(u64::MAX);
            let error = ToolError::ToolTimedOut {
                invocation: supervised.invocation.clone(),
                timeout_ms,
            };
            return finish(supervised, Some(error), capture);
        }

        if child_exited && capture.is_done() {
            return finish(supervised, None, capture);
        }

        if child_exited_at.is_some_and(|exited| exited.elapsed() >= DESCENDANT_PIPE_GRACE) {
            return finish(supervised, None, capture);
        }

        let wait = next_wait(started, policy.deadline, child_exited_at);
        match supervised.receiver.recv_timeout(wait) {
            Ok(event) => {
                if let Some(error) = capture.apply(&supervised.invocation, event) {
                    return finish(supervised, Some(error), capture);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) if capture.is_done() => {
                thread::sleep(wait);
            }
            Err(RecvTimeoutError::Disconnected) => {
                let error = ToolError::ToolCaptureChannelClosed {
                    invocation: supervised.invocation.clone(),
                };
                return finish(supervised, Some(error), capture);
            }
        }
    }
}

fn next_wait(started: Instant, deadline: Duration, child_exited_at: Option<Instant>) -> Duration {
    let mut wait = PROCESS_POLL_INTERVAL.min(deadline.saturating_sub(started.elapsed()));
    if let Some(exited) = child_exited_at {
        wait = wait.min(DESCENDANT_PIPE_GRACE.saturating_sub(exited.elapsed()));
    }
    wait
}

fn finish(
    supervised: SupervisedChild,
    original_error: Option<ToolError>,
    mut capture: CaptureState,
) -> Result<CommandOutput, CommandRunError> {
    let SupervisedChild {
        invocation,
        child,
        ownership,
        receiver,
        cancel,
        capture_workers,
    } = supervised;
    let termination = terminate(child, &invocation, ownership);
    let cancel_immediately = original_error.is_some() || termination.is_err();
    let capture_cleanup = finish_capture(
        &invocation,
        &receiver,
        &mut capture,
        cancel,
        capture_workers,
        cancel_immediately,
    );

    let mut failure = original_error;
    let status = match termination {
        Ok(status) => Some(status),
        Err(error) => {
            preserve_failure(&mut failure, error);
            None
        }
    };
    if let Err(error) = capture_cleanup {
        preserve_failure(&mut failure, error);
    }
    if let Some(error) = failure {
        return Err(error.into());
    }
    match status {
        Some(status) => capture.into_output(&invocation, status).map_err(Into::into),
        None => Err(ToolError::ToolChildInspectionFailed {
            invocation,
            source: io::Error::other("cleanup completed without an exit status or failure"),
        }
        .into()),
    }
}

fn cleanup_setup_failure(
    invocation: ToolInvocation,
    child: Child,
    ownership: ProcessOwnership,
    capture_workers: Vec<CaptureWorker>,
    cancel: Arc<AtomicBool>,
    error: ToolError,
) -> CommandRunError {
    let termination = terminate(child, &invocation, ownership);
    cancel.store(true, Ordering::Release);
    let capture_cleanup = finish_capture_workers(&invocation, capture_workers);
    let mut failure = Some(error);
    if let Err(error) = termination {
        preserve_failure(&mut failure, error);
    }
    if let Err(error) = capture_cleanup {
        preserve_failure(&mut failure, error);
    }
    match failure {
        Some(failure) => failure,
        None => ToolError::ToolChildInspectionFailed {
            invocation,
            source: io::Error::other("setup cleanup completed without its primary failure"),
        },
    }
    .into()
}

fn preserve_failure(failure: &mut Option<ToolError>, later: ToolError) {
    *failure = Some(match failure.take() {
        Some(primary) => ToolError::ToolCleanupFailed {
            primary: Box::new(primary),
            cleanup: Box::new(later),
        },
        None => later,
    });
}

fn finish_capture(
    invocation: &ToolInvocation,
    receiver: &Receiver<CaptureEvent>,
    capture: &mut CaptureState,
    cancel: Arc<AtomicBool>,
    capture_workers: Vec<CaptureWorker>,
    cancel_immediately: bool,
) -> Result<(), ToolError> {
    if cancel_immediately {
        cancel.store(true, Ordering::Release);
    }
    let deadline = Instant::now() + TERMINATION_OBSERVATION_GRACE;
    let mut first_error = None;
    while !capture.is_done() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            cancel.store(true, Ordering::Release);
            if first_error.is_none() {
                first_error = Some(ToolError::ToolCaptureShutdownTimedOut {
                    invocation: invocation.clone(),
                    timeout_ms: duration_ms(TERMINATION_OBSERVATION_GRACE),
                });
            }
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok(event) => {
                if first_error.is_none() {
                    first_error = capture.apply(invocation, event);
                } else {
                    let _ = capture.apply(invocation, event);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                cancel.store(true, Ordering::Release);
                if first_error.is_none() {
                    first_error = Some(ToolError::ToolCaptureShutdownTimedOut {
                        invocation: invocation.clone(),
                        timeout_ms: duration_ms(TERMINATION_OBSERVATION_GRACE),
                    });
                }
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                cancel.store(true, Ordering::Release);
                if first_error.is_none() {
                    first_error = Some(ToolError::ToolCaptureChannelClosed {
                        invocation: invocation.clone(),
                    });
                }
                break;
            }
        }
    }
    if let Err(error) = finish_capture_workers(invocation, capture_workers) {
        preserve_failure(&mut first_error, error);
    }
    first_error.map_or(Ok(()), Err)
}

fn finish_capture_workers(
    invocation: &ToolInvocation,
    capture_workers: Vec<CaptureWorker>,
) -> Result<(), ToolError> {
    let deadline = Instant::now() + TERMINATION_OBSERVATION_GRACE;
    while capture_workers
        .iter()
        .any(|worker| !worker.handle.is_finished())
    {
        if Instant::now() >= deadline {
            let mut failure = Some(ToolError::ToolCaptureShutdownTimedOut {
                invocation: invocation.clone(),
                timeout_ms: duration_ms(TERMINATION_OBSERVATION_GRACE),
            });
            if let Err(error) = handoff_capture_reaper(invocation, capture_workers) {
                preserve_failure(&mut failure, error);
            }
            return match failure {
                Some(error) => Err(error),
                None => Ok(()),
            };
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }

    let mut first_error = None;
    for worker in capture_workers {
        if worker.handle.join().is_err() && first_error.is_none() {
            first_error = Some(ToolError::ToolCaptureThreadPanicked {
                invocation: invocation.clone(),
                stream: worker.stream,
            });
        }
    }
    first_error.map_or(Ok(()), Err)
}

fn spawn_capture_pipe(
    mut pipe: Box<dyn Read + Send>,
    stream: ToolOutputStream,
    limit_bytes: usize,
    sender: Sender<CaptureEvent>,
    cancel: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name(format!("study-tts-{stream}-capture"))
        .spawn(move || {
            let mut captured = Vec::with_capacity(limit_bytes.min(8 * 1024));
            let mut chunk = [0_u8; 8 * 1024];
            loop {
                if cancel.load(Ordering::Acquire) {
                    let _ = sender.send(CaptureEvent::Cancelled { stream });
                    return;
                }
                match pipe.read(&mut chunk) {
                    Ok(0) => {
                        let _ = sender.send(CaptureEvent::Complete {
                            stream,
                            bytes: captured,
                        });
                        return;
                    }
                    Ok(read) if read > limit_bytes.saturating_sub(captured.len()) => {
                        let _ = sender.send(CaptureEvent::Overflow {
                            stream,
                            limit_bytes,
                        });
                        return;
                    }
                    Ok(read) => captured.extend_from_slice(&chunk[..read]),
                    Err(source) if source.kind() == io::ErrorKind::Interrupted => {}
                    Err(source) if source.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(PROCESS_POLL_INTERVAL);
                    }
                    Err(source) => {
                        let _ = sender.send(CaptureEvent::Failed { stream, source });
                        return;
                    }
                }
            }
        })
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn handoff_capture_reaper(
    invocation: &ToolInvocation,
    capture_workers: Vec<CaptureWorker>,
) -> Result<(), ToolError> {
    thread::Builder::new()
        .name("study-tts-capture-reaper".to_owned())
        .spawn(move || {
            for worker in capture_workers {
                let _ = worker.handle.join();
            }
        })
        .map(|_| ())
        .map_err(|source| ToolError::ToolCaptureReaperStartFailed {
            invocation: invocation.clone(),
            source,
        })
}

#[cfg(unix)]
fn configure_capture_pipe(pipe: &impl AsFd) -> io::Result<()> {
    use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};

    let flags = fcntl_getfl(pipe)?;
    fcntl_setfl(pipe, flags | OFlags::NONBLOCK)?;
    Ok(())
}

#[cfg(not(unix))]
fn configure_capture_pipe<T>(_pipe: &T) -> io::Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
impl ProcessOwnership {
    fn for_child(child: &Child) -> io::Result<Self> {
        let root_pid =
            i32::try_from(child.id()).map_err(|_| io::Error::other("child PID overflow"))?;
        Ok(Self {
            root_pid: Some(root_pid),
            descendants: Vec::new(),
        })
    }

    fn refresh(&mut self) -> io::Result<()> {
        use rustix::process::{Pid, PidfdFlags, pidfd_open};

        let Some(root_pid) = self.root_pid else {
            return Ok(());
        };
        let mut owned_parents = vec![root_pid];
        for process in &self.descendants {
            if process_identity_is_live(process.pid, process.start_time_ticks) {
                owned_parents.push(process.pid);
            }
        }

        let mut parent_index = 0;
        while let Some(parent_pid) = owned_parents.get(parent_index).copied() {
            parent_index += 1;
            for pid in child_process_ids(parent_pid)? {
                if owned_parents.contains(&pid) {
                    continue;
                }
                let Some(process) = read_process_record(pid) else {
                    continue;
                };
                if !owned_parents.contains(&process.parent_pid) {
                    continue;
                }
                let Some(pid) = Pid::from_raw(pid) else {
                    continue;
                };
                let pidfd = match pidfd_open(pid, PidfdFlags::empty()) {
                    Ok(pidfd) => pidfd,
                    Err(rustix::io::Errno::SRCH) => continue,
                    Err(source) => return Err(source.into()),
                };
                if read_process_record(process.pid).is_some_and(|current| {
                    current.start_time_ticks == process.start_time_ticks
                        && owned_parents.contains(&current.parent_pid)
                }) {
                    self.descendants.push(OwnedProcess {
                        pid: process.pid,
                        start_time_ticks: process.start_time_ticks,
                        pidfd,
                    });
                    owned_parents.push(process.pid);
                }
            }
        }
        Ok(())
    }

    fn signal_descendants(&self, invocation: &ToolInvocation) -> Result<(), ToolError> {
        use rustix::process::{Signal, pidfd_send_signal};

        let mut failure = None;
        for process in &self.descendants {
            match pidfd_send_signal(&process.pidfd, Signal::KILL) {
                Ok(()) | Err(rustix::io::Errno::SRCH) => {}
                Err(source) => {
                    preserve_failure(
                        &mut failure,
                        ToolError::ToolContainmentSignalFailed {
                            invocation: invocation.clone(),
                            pid: process.pid,
                            source: source.into(),
                        },
                    );
                }
            }
        }
        failure.map_or(Ok(()), Err)
    }

    fn has_live_descendants(&self) -> io::Result<bool> {
        Ok(self
            .descendants
            .iter()
            .any(|process| process_identity_is_live(process.pid, process.start_time_ticks)))
    }
}

#[cfg(target_os = "linux")]
fn child_process_ids(parent_pid: i32) -> io::Result<Vec<i32>> {
    let task_path = format!("/proc/{parent_pid}/task");
    let tasks = match fs::read_dir(task_path) {
        Ok(tasks) => tasks,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(source),
    };
    let mut children = Vec::new();
    for task in tasks {
        let Ok(task) = task else { continue };
        let child_path = task.path().join("children");
        let child_list = match fs::read_to_string(child_path) {
            Ok(child_list) => child_list,
            Err(source) if source.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => return Err(source),
        };
        for pid in child_list.split_ascii_whitespace() {
            if let Ok(pid) = pid.parse() {
                children.push(pid);
            }
        }
    }
    Ok(children)
}

#[cfg(target_os = "linux")]
fn read_process_record(pid: i32) -> Option<ProcessRecord> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields = stat
        .rsplit_once(") ")?
        .1
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    Some(ProcessRecord {
        pid,
        parent_pid: fields.get(1)?.parse().ok()?,
        start_time_ticks: fields.get(19)?.parse().ok()?,
    })
}

#[cfg(target_os = "linux")]
fn process_identity_is_live(pid: i32, start_time_ticks: u64) -> bool {
    read_process_record(pid).is_some_and(|process| process.start_time_ticks == start_time_ticks)
}

#[cfg(not(target_os = "linux"))]
impl ProcessOwnership {
    fn for_child(_child: &Child) -> io::Result<Self> {
        Ok(Self)
    }

    fn refresh(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
fn child_has_exited(child: &mut Child) -> io::Result<bool> {
    use rustix::process::{WaitId, WaitIdOptions, waitid};

    let pid = child_pid(child)?;
    // The waitable leader keeps its numeric process-group identity reserved
    // until `terminate` signals the owned group and then calls `Child::wait`.
    let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
    waitid(WaitId::Pid(pid), options)
        .map(|status| status.is_some())
        .map_err(Into::into)
}

#[cfg(not(unix))]
fn child_has_exited(child: &mut Child) -> io::Result<bool> {
    child.try_wait().map(|status| status.is_some())
}

#[cfg(unix)]
fn configure_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn child_pid(child: &Child) -> io::Result<rustix::process::Pid> {
    use rustix::process::Pid;

    let raw_pid = i32::try_from(child.id()).map_err(|_| io::Error::other("child PID overflow"))?;
    Pid::from_raw(raw_pid).ok_or_else(|| io::Error::other("child PID was zero"))
}

fn terminate(
    mut child: Child,
    invocation: &ToolInvocation,
    mut ownership: ProcessOwnership,
) -> Result<ExitStatus, ToolError> {
    let deadline = Instant::now() + TERMINATION_OBSERVATION_GRACE;
    let direct_pid = child_pid_for_diagnostics(&child).map_err(|source| {
        ToolError::ToolChildInspectionFailed {
            invocation: invocation.clone(),
            source,
        }
    })?;
    let mut failure = signal_owned_processes(&mut child, invocation, &mut ownership).err();

    if let Err(error) = wait_for_child_exit(&mut child, invocation, deadline) {
        preserve_failure(&mut failure, error);
        if let Err(error) = handoff_child_reaper(invocation, child) {
            preserve_failure(&mut failure, error);
        }
        return match failure {
            Some(error) => Err(error),
            None => Err(ToolError::ToolChildInspectionFailed {
                invocation: invocation.clone(),
                source: io::Error::other("failed cleanup lost its diagnostic context"),
            }),
        };
    }
    let status = match child.wait() {
        Ok(status) => Some(status),
        Err(source) => {
            preserve_failure(
                &mut failure,
                ToolError::ToolChildReapFailed {
                    invocation: invocation.clone(),
                    source,
                },
            );
            None
        }
    };
    if let Err(error) = wait_for_containment(invocation, &ownership, direct_pid, deadline) {
        preserve_failure(&mut failure, error);
    }

    if let Some(error) = failure {
        return Err(error);
    }
    match status {
        Some(status) => Ok(status),
        None => Err(ToolError::ToolChildReapFailed {
            invocation: invocation.clone(),
            source: io::Error::other("child cleanup completed without an exit status"),
        }),
    }
}

fn wait_for_child_exit(
    child: &mut Child,
    invocation: &ToolInvocation,
    deadline: Instant,
) -> Result<(), ToolError> {
    loop {
        match child_has_exited(child) {
            Ok(true) => return Ok(()),
            Ok(false) if Instant::now() >= deadline => {
                return Err(ToolError::ToolTerminationTimedOut {
                    invocation: invocation.clone(),
                    timeout_ms: duration_ms(TERMINATION_OBSERVATION_GRACE),
                });
            }
            Ok(false) => thread::sleep(PROCESS_POLL_INTERVAL),
            Err(source) => {
                return Err(ToolError::ToolChildInspectionFailed {
                    invocation: invocation.clone(),
                    source,
                });
            }
        }
    }
}

fn handoff_child_reaper(invocation: &ToolInvocation, mut child: Child) -> Result<(), ToolError> {
    thread::Builder::new()
        .name("study-tts-child-reaper".to_owned())
        .spawn(move || {
            let _ = child.wait();
        })
        .map(|_| ())
        .map_err(|source| ToolError::ToolReaperStartFailed {
            invocation: invocation.clone(),
            source,
        })
}

#[cfg(unix)]
fn child_pid_for_diagnostics(child: &Child) -> io::Result<i32> {
    child_pid(child).map(|pid| pid.as_raw_nonzero().get())
}

#[cfg(not(unix))]
fn child_pid_for_diagnostics(child: &Child) -> io::Result<i32> {
    i32::try_from(child.id()).map_err(|_| io::Error::other("child PID overflow"))
}

#[cfg(unix)]
fn signal_owned_processes(
    child: &mut Child,
    invocation: &ToolInvocation,
    ownership: &mut ProcessOwnership,
) -> Result<(), ToolError> {
    use rustix::process::{Signal, kill_process_group};

    let mut failure = None;
    #[cfg(not(target_os = "linux"))]
    let _ = ownership;
    #[cfg(target_os = "linux")]
    if let Err(source) = ownership.refresh() {
        preserve_failure(
            &mut failure,
            ToolError::ToolContainmentInspectionFailed {
                invocation: invocation.clone(),
                source,
            },
        );
    }
    match child_pid(child) {
        Ok(pid) => match kill_process_group(pid, Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(source) => {
                let _ = child.kill();
                preserve_failure(
                    &mut failure,
                    ToolError::ToolTerminationSignalFailed {
                        invocation: invocation.clone(),
                        source: source.into(),
                    },
                );
            }
        },
        Err(source) => {
            let _ = child.kill();
            preserve_failure(
                &mut failure,
                ToolError::ToolChildInspectionFailed {
                    invocation: invocation.clone(),
                    source,
                },
            );
        }
    }

    #[cfg(target_os = "linux")]
    if let Err(error) = ownership.signal_descendants(invocation) {
        preserve_failure(&mut failure, error);
    }

    failure.map_or(Ok(()), Err)
}

#[cfg(not(unix))]
fn signal_owned_processes(
    child: &mut Child,
    invocation: &ToolInvocation,
    _ownership: &mut ProcessOwnership,
) -> Result<(), ToolError> {
    child
        .kill()
        .or_else(|source| {
            if source.kind() == io::ErrorKind::InvalidInput {
                Ok(())
            } else {
                Err(source)
            }
        })
        .map_err(|source| ToolError::ToolTerminationSignalFailed {
            invocation: invocation.clone(),
            source,
        })
}

#[cfg(unix)]
fn wait_for_containment(
    invocation: &ToolInvocation,
    ownership: &ProcessOwnership,
    direct_pid: i32,
    deadline: Instant,
) -> Result<(), ToolError> {
    use rustix::process::{Pid, test_kill_process_group};

    let pid = Pid::from_raw(direct_pid).ok_or_else(|| ToolError::ToolChildInspectionFailed {
        invocation: invocation.clone(),
        source: io::Error::other("child PID was zero"),
    })?;
    loop {
        let group_exists = match test_kill_process_group(pid) {
            Ok(()) => true,
            Err(rustix::io::Errno::SRCH) => false,
            Err(source) => {
                return Err(ToolError::ToolContainmentInspectionFailed {
                    invocation: invocation.clone(),
                    source: source.into(),
                });
            }
        };

        #[cfg(target_os = "linux")]
        let descendants_are_live = ownership.has_live_descendants().map_err(|source| {
            ToolError::ToolContainmentInspectionFailed {
                invocation: invocation.clone(),
                source,
            }
        })?;
        #[cfg(not(target_os = "linux"))]
        let descendants_are_live = false;

        if !group_exists && !descendants_are_live {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(ToolError::ToolTerminationTimedOut {
                invocation: invocation.clone(),
                timeout_ms: duration_ms(TERMINATION_OBSERVATION_GRACE),
            });
        }
        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

#[cfg(not(unix))]
fn wait_for_containment(
    _invocation: &ToolInvocation,
    _ownership: &ProcessOwnership,
    _direct_pid: i32,
    _deadline: Instant,
) -> Result<(), ToolError> {
    Ok(())
}

// These process-executing T4 tests remain colocated as a proportionate
// exception to `docs/testing/TEST-STRATEGY.md`: injected deadlines and capture
// startup failures use private policy seams that must not become production API
// solely to move the harness into `study-tts-testkit`.
#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Write},
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::Ordering,
        thread,
        time::{Duration, Instant},
    };

    use tempfile::TempDir;

    use super::{
        CaptureEvent, CommandPolicy, CommandRunError, FFMPEG_ENCODE_POLICY, FFPROBE_POLICY,
        VERSION_PROBE_POLICY, WORKER_ENVIRONMENT_PROBE_POLICY, run, run_with_capture_spawner,
        spawn_capture_pipe,
    };
    use crate::{ToolError, ToolInvocation, ToolOperation, ToolOutputStream};

    #[cfg(unix)]
    fn stand_in(directory: &Path, name: &str, body: &str) -> PathBuf {
        let path = directory.join(name);
        let mut script = fs::File::create(&path).expect("create stand-in executable");
        writeln!(script, "#!/bin/sh\n{body}").expect("write stand-in executable");
        script.sync_all().expect("flush stand-in executable");
        drop(script);
        path
    }

    #[cfg(unix)]
    fn stand_in_command(script: impl AsRef<Path>) -> Command {
        let mut command = Command::new("/bin/sh");
        command.arg(script.as_ref());
        command
    }

    fn policy(deadline: Duration, output_limit: usize) -> CommandPolicy {
        CommandPolicy {
            deadline,
            stdout_limit: output_limit,
            stderr_limit: output_limit,
        }
    }

    fn invocation() -> ToolInvocation {
        ToolInvocation::new(
            "stand-in",
            ToolOperation::VersionProbe,
            Path::new("stand-in"),
        )
    }

    fn encode_invocation() -> ToolInvocation {
        ToolInvocation::new(
            "FFmpeg",
            ToolOperation::M4aEncode,
            Path::new("preview/lesson.m4a"),
        )
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_bounded_command_drains_stdout_and_stderr_without_deadlock() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(
            directory.path(),
            "dual-output",
            concat!(
                "i=0\n",
                "while [ \"$i\" -lt 2000 ]; do\n",
                "  printf 'stdout-line\\n'\n",
                "  printf 'stderr-line\\n' >&2\n",
                "  i=$((i + 1))\n",
                "done",
            ),
        );

        let output = run(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_secs(2), 64 * 1024),
        )
        .expect("both bounded streams must be drained");

        assert!(output.status.success());
        assert_eq!(output.stdout.len(), 24_000);
        assert_eq!(output.stderr.len(), 24_000);
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_closed_capture_pipes_can_precede_a_successful_exit() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(
            directory.path(),
            "early-pipe-close",
            "exec 1>&- 2>&-\nsleep 0.1\nexit 0",
        );

        let output = run(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_secs(1), 1024),
        )
        .expect("closed capture pipes must not hide the later successful exit");

        assert!(output.status.success());
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn t1_e0_external_tool_supervision_policies_are_pinned() {
        for (policy, deadline) in [
            (VERSION_PROBE_POLICY, Duration::from_secs(5)),
            (WORKER_ENVIRONMENT_PROBE_POLICY, Duration::from_secs(2 * 60)),
            (FFPROBE_POLICY, Duration::from_secs(30)),
            (FFMPEG_ENCODE_POLICY, Duration::from_secs(30 * 60)),
        ] {
            assert_eq!(policy.deadline, deadline);
            assert_eq!(policy.stdout_limit, 1024 * 1024);
            assert_eq!(policy.stderr_limit, 1024 * 1024);
        }
    }

    #[test]
    fn t1_e0_primary_failure_remains_typed_when_termination_also_fails() {
        let mut failure = Some(ToolError::ToolTimedOut {
            invocation: invocation(),
            timeout_ms: 20,
        });

        super::preserve_failure(
            &mut failure,
            ToolError::ToolTerminationSignalFailed {
                invocation: invocation(),
                source: io::Error::other("injected termination failure"),
            },
        );

        assert!(matches!(
            failure,
            Some(ToolError::ToolCleanupFailed { primary, cleanup })
                if matches!(*primary, ToolError::ToolTimedOut { timeout_ms: 20, .. })
                    && matches!(*cleanup, ToolError::ToolTerminationSignalFailed { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_exit_observation_keeps_process_group_leader_waitable() {
        use rustix::process::{WaitId, WaitIdOptions, waitid};

        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "immediate-exit", "exit 0");
        let mut command = stand_in_command(executable);
        super::configure_process_group(&mut command);
        let mut child = command.spawn().expect("start stand-in child");
        let observed = (0..200).any(|_| {
            if super::child_has_exited(&mut child).expect("observe child exit") {
                true
            } else {
                thread::sleep(Duration::from_millis(10));
                false
            }
        });
        assert!(observed, "stand-in child did not exit");

        let pid = super::child_pid(&child).expect("child has a valid PID");
        let options = WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT;
        let waitable = waitid(WaitId::Pid(pid), options)
            .expect("inspect waitable child")
            .is_some();
        if waitable {
            super::terminate(child, &invocation(), Default::default())
                .expect("clean up waitable child");
        }

        assert!(waitable, "exit observation reaped the process-group leader");
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_bounded_command_reports_the_stream_that_overflows() {
        let directory = TempDir::new().expect("create stand-in directory");
        let stdout = stand_in(
            directory.path(),
            "stdout-overflow",
            "printf '012345678901234567890123456789012'",
        );
        let stderr = stand_in(
            directory.path(),
            "stderr-overflow",
            "printf '012345678901234567890123456789012' >&2",
        );

        for (stream, executable) in [
            (ToolOutputStream::Stdout, stdout),
            (ToolOutputStream::Stderr, stderr),
        ] {
            let error = run(
                invocation(),
                stand_in_command(executable),
                policy(Duration::from_secs(2), 32),
            )
            .expect_err("one byte beyond the stream ceiling must fail");
            assert!(
                matches!(
                    error,
                    CommandRunError::Supervision(ToolError::ToolOutputOverflow {
                        stream: reported,
                        limit_bytes: 32,
                        ..
                    }) if reported == stream
                ),
                "{stream} overflow produced `{error}`"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_deadline_includes_capture_setup_and_precedes_success() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "immediate-success", "exit 0");
        let mut capture_attempt = 0;

        let error = run_with_capture_spawner(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_millis(20), 1024),
            |pipe, stream, limit_bytes, sender, cancel| {
                capture_attempt += 1;
                if capture_attempt == 1 {
                    thread::sleep(Duration::from_millis(100));
                }
                spawn_capture_pipe(pipe, stream, limit_bytes, sender, cancel)
            },
        )
        .expect_err("capture setup time must count against the command deadline");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolTimedOut { timeout_ms: 20, .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_timeout_preserves_capture_cleanup_failure() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "hang", "sleep 30");

        let error = run_with_capture_spawner(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_millis(20), 1024),
            |_pipe, stream, _limit_bytes, sender, cancel| {
                thread::Builder::new().spawn(move || {
                    while !cancel.load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    let _ = sender.send(CaptureEvent::Cancelled { stream });
                    panic!("injected capture cleanup panic");
                })
            },
        )
        .expect_err("timeout and cleanup failure must both be reported");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolCleanupFailed {
                primary,
                cleanup,
            }) if matches!(*primary, ToolError::ToolTimedOut { timeout_ms: 20, .. })
                && matches!(*cleanup, ToolError::ToolCaptureThreadPanicked { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_capture_error_precedes_capture_join_failure() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "hang", "sleep 30");

        let error = run_with_capture_spawner(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_secs(2), 32),
            |_pipe, stream, limit_bytes, sender, _cancel| {
                thread::Builder::new().spawn(move || match stream {
                    ToolOutputStream::Stdout => {
                        let _ = sender.send(CaptureEvent::Overflow {
                            stream,
                            limit_bytes,
                        });
                        panic!("injected capture join panic");
                    }
                    ToolOutputStream::Stderr => {
                        let _ = sender.send(CaptureEvent::Complete {
                            stream,
                            bytes: Vec::new(),
                        });
                    }
                })
            },
        )
        .expect_err("capture and join failures must both be reported");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolCleanupFailed {
                primary,
                cleanup,
            }) if matches!(*primary, ToolError::ToolOutputOverflow {
                stream: ToolOutputStream::Stdout,
                limit_bytes: 32,
                ..
            }) && matches!(*cleanup, ToolError::ToolCaptureThreadPanicked {
                stream: ToolOutputStream::Stdout,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_capture_thread_start_failure_terminates_and_reaps_child() {
        use rustix::process::{Pid, test_kill_process};

        let directory = TempDir::new().expect("create stand-in directory");
        let child_pid_path = directory.path().join("child.pid");
        let executable = stand_in(
            directory.path(),
            "capture-start-failure",
            "printf '%s\n' \"$$\" > \"$1\"\nsleep 30",
        );
        let mut command = stand_in_command(executable);
        command.arg(&child_pid_path);
        let mut capture_attempt = 0;
        let child_start_deadline = Instant::now() + Duration::from_secs(1);

        let error = run_with_capture_spawner(
            invocation(),
            command,
            policy(Duration::from_secs(2), 1024),
            |pipe, stream, limit_bytes, sender, cancel| {
                capture_attempt += 1;
                if capture_attempt == 2 {
                    while !child_pid_path.exists() && Instant::now() < child_start_deadline {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(io::Error::other("injected capture-thread start failure"))
                } else {
                    spawn_capture_pipe(pipe, stream, limit_bytes, sender, cancel)
                }
            },
        )
        .expect_err("capture-thread creation failure must be typed");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolCaptureStartFailed {
                stream: ToolOutputStream::Stderr,
                ..
            })
        ));
        let raw_pid: i32 = fs::read_to_string(&child_pid_path)
            .expect("stand-in must record its PID")
            .trim()
            .parse()
            .expect("child PID must be numeric");
        let pid = Pid::from_raw(raw_pid).expect("a spawned process has a nonzero PID");
        assert!(
            test_kill_process(pid).is_err(),
            "child {raw_pid} survived capture-thread setup failure"
        );
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_capture_setup_error_preserves_worker_cleanup_failure() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "hang", "sleep 30");
        let mut capture_attempt = 0;

        let error = run_with_capture_spawner(
            invocation(),
            stand_in_command(executable),
            policy(Duration::from_secs(2), 1024),
            |_pipe, stream, _limit_bytes, sender, cancel| {
                capture_attempt += 1;
                if capture_attempt == 2 {
                    Err(io::Error::other("injected capture-thread start failure"))
                } else {
                    thread::Builder::new().spawn(move || {
                        while !cancel.load(Ordering::Acquire) {
                            thread::yield_now();
                        }
                        let _ = sender.send(CaptureEvent::Cancelled { stream });
                        panic!("injected setup cleanup panic");
                    })
                }
            },
        )
        .expect_err("setup and cleanup failures must both be reported");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolCleanupFailed {
                primary,
                cleanup,
            }) if matches!(*primary, ToolError::ToolCaptureStartFailed {
                stream: ToolOutputStream::Stderr,
                ..
            }) && matches!(*cleanup, ToolError::ToolCaptureThreadPanicked {
                stream: ToolOutputStream::Stdout,
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_bounded_command_times_out_with_an_injected_policy() {
        let directory = TempDir::new().expect("create stand-in directory");
        let executable = stand_in(directory.path(), "hang", "sleep 30");
        let started = Instant::now();

        let error = run(
            encode_invocation(),
            stand_in_command(executable),
            policy(Duration::from_millis(100), 1024),
        )
        .expect_err("a hanging child must time out");

        assert!(
            matches!(
                &error,
                CommandRunError::Supervision(ToolError::ToolTimedOut {
                    invocation,
                    timeout_ms: 100,
                    ..
                }) if invocation.operation() == ToolOperation::M4aEncode
                    && invocation.subject() == Path::new("preview/lesson.m4a")
            ),
            "timeout produced `{error}`"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e0_escaped_descendant_parent_helper() {
        use std::os::unix::process::CommandExt;

        let Some(pid_path) = std::env::var_os("STUDY_TTS_ESCAPE_DESCENDANT_PID") else {
            return;
        };
        let executable = std::env::current_exe().expect("resolve process-test executable");
        let mut command = Command::new(executable);
        command
            .arg("--exact")
            .arg("process::tests::t4_e0_escaped_descendant_leaf_helper")
            .arg("--nocapture")
            .env("STUDY_TTS_ESCAPE_DESCENDANT_LEAF", "1")
            .process_group(0);
        if std::env::var_os("STUDY_TTS_CLOSE_DESCENDANT_PIPES").is_some() {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let mut descendant = command.spawn().expect("start escaped descendant helper");
        fs::write(pid_path, descendant.id().to_string()).expect("record escaped descendant PID");
        thread::sleep(Duration::from_secs(30));
        let _ = descendant.wait();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e0_escaped_descendant_leaf_helper() {
        if std::env::var_os("STUDY_TTS_ESCAPE_DESCENDANT_LEAF").is_some() {
            thread::sleep(Duration::from_secs(30));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e0_timeout_terminates_escaped_descendant_retaining_capture_pipes() {
        use rustix::process::{Pid, test_kill_process};

        let directory = TempDir::new().expect("create escaped-descendant test directory");
        let descendant_pid_path = directory.path().join("descendant.pid");
        let executable = std::env::current_exe().expect("resolve process-test executable");
        let mut command = Command::new(executable);
        command
            .arg("--exact")
            .arg("process::tests::t4_e0_escaped_descendant_parent_helper")
            .arg("--nocapture")
            .env("STUDY_TTS_ESCAPE_DESCENDANT_PID", &descendant_pid_path);
        let started = Instant::now();

        let error = run(
            invocation(),
            command,
            policy(Duration::from_millis(250), 16 * 1024),
        )
        .expect_err("an escaped descendant retaining capture pipes must time out");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolTimedOut { .. })
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
        let raw_pid: i32 = fs::read_to_string(&descendant_pid_path)
            .expect("helper must record the escaped descendant")
            .trim()
            .parse()
            .expect("escaped descendant PID must be numeric");
        let pid = Pid::from_raw(raw_pid).expect("a spawned process has a nonzero PID");
        assert!(
            test_kill_process(pid).is_err(),
            "escaped descendant {raw_pid} survived bounded cleanup"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn t4_e0_timeout_terminates_escaped_descendant_that_closes_capture_pipes() {
        use rustix::process::{Pid, Signal, kill_process, test_kill_process};

        let directory = TempDir::new().expect("create escaped-descendant test directory");
        let descendant_pid_path = directory.path().join("descendant.pid");
        let executable = std::env::current_exe().expect("resolve process-test executable");
        let mut command = Command::new(executable);
        command
            .arg("--exact")
            .arg("process::tests::t4_e0_escaped_descendant_parent_helper")
            .arg("--nocapture")
            .env("STUDY_TTS_ESCAPE_DESCENDANT_PID", &descendant_pid_path)
            .env("STUDY_TTS_CLOSE_DESCENDANT_PIPES", "1");

        let error = run(
            invocation(),
            command,
            policy(Duration::from_millis(250), 16 * 1024),
        )
        .expect_err("an escaped descendant closing capture pipes must time out");

        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolTimedOut { .. })
        ));
        let raw_pid: i32 = fs::read_to_string(&descendant_pid_path)
            .expect("helper must record the escaped descendant")
            .trim()
            .parse()
            .expect("escaped descendant PID must be numeric");
        let pid = Pid::from_raw(raw_pid).expect("a spawned process has a nonzero PID");
        let survived = test_kill_process(pid).is_ok();
        if survived {
            let _ = kill_process(pid, Signal::KILL);
        }
        assert!(
            !survived,
            "escaped descendant {raw_pid} survived bounded cleanup"
        );
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_timeout_terminates_and_reaps_the_process_group() {
        use rustix::process::{Pid, test_kill_process};

        let directory = TempDir::new().expect("create stand-in directory");
        let descendant_pid_path = directory.path().join("descendant.pid");
        let executable = stand_in(
            directory.path(),
            "descendant-hang",
            concat!(
                "sleep 30 &\n",
                "printf '%s\\n' \"$!\" > \"$1\"\n",
                "sleep 30",
            ),
        );
        let mut command = stand_in_command(executable);
        command.arg(&descendant_pid_path);

        let error = run(
            invocation(),
            command,
            policy(Duration::from_millis(150), 1024),
        )
        .expect_err("the process group must time out");
        assert!(matches!(
            error,
            CommandRunError::Supervision(ToolError::ToolTimedOut { .. })
        ));

        let raw_pid: i32 = fs::read_to_string(&descendant_pid_path)
            .expect("stand-in must record its descendant")
            .trim()
            .parse()
            .expect("descendant PID must be numeric");
        let pid = Pid::from_raw(raw_pid).expect("a spawned process has a nonzero PID");
        let reaped = (0..200).any(|_| {
            if test_kill_process(pid).is_err() {
                true
            } else {
                thread::sleep(Duration::from_millis(10));
                false
            }
        });
        assert!(
            reaped,
            "descendant {raw_pid} survived process-group cleanup"
        );
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_successful_child_terminates_and_reaps_lingering_descendants() {
        use rustix::process::{Pid, test_kill_process};

        let directory = TempDir::new().expect("create stand-in directory");
        let descendant_pid_path = directory.path().join("descendant.pid");
        let executable = stand_in(
            directory.path(),
            "lingering-descendant",
            concat!(
                "sleep 30 >/dev/null 2>&1 &\n",
                "printf '%s\\n' \"$!\" > \"$1\"\n",
                "exit 0",
            ),
        );
        let mut command = stand_in_command(executable);
        command.arg(&descendant_pid_path);
        let started = Instant::now();

        let output = run(invocation(), command, policy(Duration::from_secs(2), 1024))
            .expect("a successful direct child must not wait on a lingering descendant");

        assert!(output.status.success());
        assert!(started.elapsed() < Duration::from_secs(1));
        let raw_pid: i32 = fs::read_to_string(&descendant_pid_path)
            .expect("stand-in must record its descendant")
            .trim()
            .parse()
            .expect("descendant PID must be numeric");
        let pid = Pid::from_raw(raw_pid).expect("a spawned process has a nonzero PID");
        let reaped = (0..200).any(|_| {
            if test_kill_process(pid).is_err() {
                true
            } else {
                thread::sleep(Duration::from_millis(10));
                false
            }
        });
        assert!(
            reaped,
            "descendant {raw_pid} survived successful-child cleanup"
        );
    }
}
