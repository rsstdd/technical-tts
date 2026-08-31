//! One persistent worker child process, and the NDJSON conversation with it.
//!
//! ADR-0001 §10.1 runs the selected model in persistent children that load the
//! model once per lifetime and accept one request at a time. That shape is why
//! this exists rather than another caller of [`crate::process::run`]: that
//! function is one-shot — it spawns, drains both pipes to completion, and
//! waits — which would reload the model for every segment and defeat the one
//! property §10.1 asks for.
//!
//! What it does *not* re-implement is process-tree ownership. Spawning into its
//! own group, killing the group, and proving the tree is gone are
//! [`crate::process`]'s, shared rather than copied: a second, weaker copy of
//! containment is exactly the defect that boundary exists to prevent.
//!
//! `worker/study_tts_worker/protocol.py` is the other end of every rule here,
//! and `schemas/worker-protocol-v2.schema.json` is the published shape.

use std::collections::BTreeMap;
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::process::{ProcessOwnership, configure_process_group, terminate};
use crate::worker_protocol::{
    MAX_WORKER_FRAME_BYTES, MAX_WORKER_REQUEST_ID_BYTES, WORKER_PROTOCOL_VERSION,
    WorkerRequestFrame, WorkerResponseFrame, parse_worker_response,
};
use crate::{BackendError, ToolInvocation};

/// Diagnostics retained from the worker's standard error.
///
/// Bounded for the reason [`crate::process`] bounds its own capture: a backend
/// that logs per inference step would otherwise grow this without limit for the
/// life of the process, which is a memory bug rather than a diagnostic.
///
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
/// records it and names this constant.
const MAX_WORKER_STDERR_BYTES: usize = 1024 * 1024;

/// Responses buffered before the reader thread blocks.
///
/// One: the protocol is strictly one response per request (ADR-0001 §10.3), so
/// a second frame arriving before the first is read is the worker running ahead
/// of its own contract, and back-pressure is the honest response to it.
const RESPONSE_CHANNEL_DEPTH: usize = 1;

/// What the reader thread observed on the protocol stream.
enum ProtocolEvent {
    /// One complete frame, still to be parsed.
    Frame(Vec<u8>),
    /// A line exceeding [`MAX_WORKER_FRAME_BYTES`], refused before it was kept.
    Oversized,
    /// Bytes left on the stream at end of input with no newline to close them.
    ///
    /// Reported rather than dropped: a partial line is either a worker killed
    /// mid-frame or output that was never a frame at all, and discarding it
    /// silently is what let contamination past the last frame go unseen.
    /// Carries the length, not the bytes — ADR-0001 §16 keeps source text and
    /// voice paths out of a diagnostic, and this build cannot know which of
    /// those a partial line holds.
    Unterminated(usize),
    /// The stream could not be read.
    Unreadable(String),
}

/// A live worker child that answers one request at a time.
///
/// Not [`Clone`] and not shareable: it owns a process, and the executor holds
/// exactly one behind a mutex because ADR-0001 §10.1 gives each worker one
/// in-flight request.
#[derive(Debug)]
pub(crate) struct WorkerClient {
    invocation: ToolInvocation,
    /// `None` once terminated, so [`Drop`] cannot kill the tree twice.
    child: Option<Child>,
    ownership: ProcessOwnership,
    /// `None` once shut down, which also closes the worker's standard input.
    stdin: Option<ChildStdin>,
    responses: Option<Receiver<ProtocolEvent>>,
    stderr: Arc<Mutex<Vec<u8>>>,
    readers: Vec<JoinHandle<()>>,
    /// Exchanges this worker has been asked for, so no two carry one identity.
    ///
    /// ADR-0001 §10.3 requires a request identity unique per worker lifetime,
    /// and the caller's is deliberately not: `PlannedSegment::request_id` is a
    /// plan input, so a retake or a retried attempt re-asks for one segment
    /// under one identity. Counting here rather than at the caller is what lets
    /// both hold — this is the worker's lifetime, by definition.
    exchanges: u64,
    /// Set by any failure that leaves the stream's position unknown.
    ///
    /// A timed-out request has not been cancelled — the worker may still answer
    /// it — so the next response on the stream may belong to the previous
    /// request. Correlating that to a new request would attribute one segment's
    /// audio to another, so the client refuses instead and the caller restarts
    /// it. ADR-0001 §10.3 requires a restart after exactly this.
    poisoned: bool,
}

impl WorkerClient {
    /// Starts one worker child with its own process group and captured pipes.
    ///
    /// # Errors
    ///
    /// [`BackendError::Protocol`] when the child cannot be launched or its
    /// pipes cannot be taken, naming `startup` as the request.
    pub(crate) fn spawn(
        invocation: ToolInvocation,
        program: &std::path::Path,
        arguments: &[String],
        working_directory: &std::path::Path,
        environment: &BTreeMap<String, String>,
    ) -> Result<Self, BackendError> {
        let mut command = Command::new(program);
        // Discrete checked arguments, never a shell: `AGENTS.md` §Security.
        command.args(arguments);
        // Set rather than inherited. `python -m` resolves the entry module
        // against the working directory, so an inherited one makes whether the
        // worker starts at all depend on where the build was invoked from.
        command.current_dir(working_directory);
        // Cleared before anything is declared, so the child's environment is
        // exactly what this build chose. Overlaying onto an inherited
        // environment leaves `PYTHONPATH`, `PYTHONHOME`, `PYTHONSTARTUP`,
        // user-site configuration and `sitecustomize` able to change what the
        // interpreter imports, and proxy and certificate variables able to
        // change what it can reach — all of it *before* the worker applies the
        // offline settings in `_apply_offline_environment`. That window is what
        // makes an inherited environment a reproducibility and offline defect
        // rather than untidiness, and it is why the declared map now carries
        // the offline variables too rather than leaving them to the child.
        command.env_clear();
        for (name, value) in environment {
            command.env(name, value);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Its own group, so a kill reaches the backend's own children too.
        configure_process_group(&mut command);

        let mut child = command.spawn().map_err(|source| startup_failure(&source))?;
        let ownership = ProcessOwnership::for_child(&child).map_err(|source| {
            // The child is already running and cannot be owned, so it is killed
            // and reaped here: dropping `Child` detaches it, and a kill
            // without a wait leaves a zombie nothing ever collects.
            let _ = child.kill();
            let _ = child.wait();
            startup_failure(&source)
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| missing_pipe("standard input"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| missing_pipe("standard output"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| missing_pipe("standard error"))?;

        let (sender, responses) = sync_channel(RESPONSE_CHANNEL_DEPTH);
        let diagnostics = Arc::new(Mutex::new(Vec::new()));
        // Both pipes are drained on their own threads. Reading one to
        // completion before the other deadlocks as soon as the unread pipe
        // fills its buffer, and a backend that writes progress to stderr while
        // waiting to be read on stdout is exactly that case.
        let readers = vec![
            spawn_protocol_reader(stdout, sender),
            spawn_diagnostic_reader(stderr, Arc::clone(&diagnostics)),
        ];

        Ok(Self {
            invocation,
            child: Some(child),
            ownership,
            stdin: Some(stdin),
            responses: Some(responses),
            stderr: diagnostics,
            readers,
            exchanges: 0,
            poisoned: false,
        })
    }

    /// The identity the next exchange for `caller_request_id` goes out under.
    ///
    /// Prefixed rather than suffixed: every planned identity begins `e0-`, so a
    /// leading count cannot collide with another segment's identity, while a
    /// trailing one could match a segment whose name ends in the same digits.
    /// The caller's identity is kept inside it so a worker diagnostic still
    /// names the segment it belongs to.
    ///
    /// # Errors
    ///
    /// [`BackendError::Protocol`] when the composed identity would exceed
    /// [`MAX_WORKER_REQUEST_ID_BYTES`], refused here rather than at the
    /// worker's parser so the refusal names this build's composition.
    pub(crate) fn next_request_id(
        &mut self,
        caller_request_id: &str,
    ) -> Result<String, BackendError> {
        self.exchanges = self.exchanges.saturating_add(1);
        let composed = format!("{}-{caller_request_id}", self.exchanges);
        if composed.len() > MAX_WORKER_REQUEST_ID_BYTES {
            return Err(protocol_failure(
                caller_request_id,
                &format!(
                    "the request identity is {} bytes once made unique for this worker, past the \
                     {MAX_WORKER_REQUEST_ID_BYTES}-byte protocol ceiling",
                    composed.len()
                ),
            ));
        }
        Ok(composed)
    }

    /// Sends one request and returns the worker's answer to it.
    ///
    /// The deadline is the caller's, because a model load and one segment of
    /// speech are different waits: ADR-0001 §10.3 requires both to be bounded,
    /// not to be bounded alike.
    ///
    /// # Errors
    ///
    /// [`BackendError::Timeout`] when no answer arrives within `deadline`,
    /// [`BackendError::Execution`] when the worker answers with a failure
    /// frame, carrying the backend's own code, and [`BackendError::Protocol`]
    /// when the worker cannot be written to, answers with something this build
    /// cannot read, answers a different request, or has already left the stream
    /// in an unknown position.
    pub(crate) fn request(
        &mut self,
        request_id: &str,
        frame: &WorkerRequestFrame,
        deadline: Duration,
    ) -> Result<WorkerResponseFrame, BackendError> {
        if self.poisoned {
            return Err(protocol_failure(
                request_id,
                "the worker was left at an unknown position in the protocol stream by an earlier \
                 failure and must be restarted before it is asked for anything else",
            ));
        }
        // Poisoned for the whole exchange and cleared only on a correlated
        // answer: every path out between here and there leaves the stream
        // somewhere this client cannot describe.
        self.poisoned = true;

        let line = serde_json::to_vec(frame).map_err(|source| {
            protocol_failure(request_id, &format!("unencodable request: {source}"))
        })?;
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| protocol_failure(request_id, "the worker's standard input is closed"))?;
        stdin
            .write_all(&line)
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .map_err(|source| {
                protocol_failure(request_id, &format!("unwritable request: {source}"))
            })?;

        // The deadline bounds the whole exchange, not each frame. ADR-0001
        // §10.2 lets `progress` events precede a result, so a per-frame
        // deadline would let a worker emitting progress forever never time out
        // — which is the hang the deadline exists to catch.
        let expiry = Instant::now() + deadline;
        let responses = self.responses.as_ref().ok_or_else(|| {
            protocol_failure(
                request_id,
                "the worker's protocol response stream is closed",
            )
        })?;
        loop {
            let remaining = expiry.saturating_duration_since(Instant::now());
            let response = match responses.recv_timeout(remaining) {
                Ok(ProtocolEvent::Frame(bytes)) => parse_worker_response(&bytes)
                    .map_err(|source| protocol_failure(request_id, &source.to_string()))?,
                Ok(ProtocolEvent::Oversized) => {
                    return Err(protocol_failure(
                        request_id,
                        "the worker sent a frame larger than the protocol ceiling",
                    ));
                }
                Ok(ProtocolEvent::Unterminated(bytes)) => {
                    // A worker that died mid-frame, which is not the same
                    // refusal as one that exited between frames: the caller is
                    // told the stream stopped inside a message rather than that
                    // nothing answered.
                    return Err(protocol_failure(
                        request_id,
                        &format!("the worker's standard output ended inside a {bytes}-byte frame"),
                    ));
                }
                Ok(ProtocolEvent::Unreadable(message)) => {
                    return Err(protocol_failure(request_id, &message));
                }
                Err(RecvTimeoutError::Timeout) => {
                    // The tree goes now, not when this client happens to be
                    // dropped. ADR-0001 §10.3 requires the parent to terminate
                    // the full child process tree on a deadline, and a worker
                    // that missed one is still running: it holds a model
                    // resident and the staging directory open, and it may still
                    // write the audio nobody is waiting for any more. The
                    // client stays poisoned either way, so the caller is told
                    // to restart rather than handed a corpse that looks usable.
                    //
                    // Best effort by necessity — the timeout is what this
                    // exchange failed for, and a containment error on top of it
                    // would replace the diagnostic the caller needs with one
                    // about cleaning up after it.
                    let _ = self.shutdown();
                    return Err(BackendError::Timeout {
                        request_id: request_id.to_owned(),
                        timeout_ms: u64::try_from(deadline.as_millis()).unwrap_or(u64::MAX),
                    });
                }
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(protocol_failure(
                        request_id,
                        "the worker closed its protocol stream without answering",
                    ));
                }
            };

            // Correlation is the whole reason a request ID is on every frame
            // (ADR-0001 §10.3). An uncorrelated frame is not a late answer to
            // ignore: it means this client and the worker disagree about which
            // request is in flight, and one segment's audio could then be
            // published under another's key.
            if response.request_id() != request_id {
                return Err(protocol_failure(
                    request_id,
                    "the worker answered a different request",
                ));
            }
            if response.is_interim() {
                continue;
            }
            self.poisoned = false;
            // A failure frame is a terminal, correlated answer: the backend
            // refused, and it said with which of its own stable codes. Handing
            // it back as a frame would leave every caller to re-derive that,
            // and a caller that forgot would report a protocol fault for a
            // backend that was working correctly and saying no.
            if let WorkerResponseFrame::Failure {
                request_id,
                code,
                message,
                ..
            } = response
            {
                return Err(BackendError::Execution {
                    request_id,
                    code: code.as_str().to_owned(),
                    message,
                });
            }
            return Ok(response);
        }
    }

    /// Everything the worker has written to standard error so far.
    ///
    /// Diagnostics only. ADR-0001 §16 keeps source text and voice paths off
    /// this stream, and nothing here reaches an identity or a published file.
    pub(crate) fn diagnostics(&self) -> String {
        let captured = self
            .stderr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        String::from_utf8_lossy(&captured).into_owned()
    }

    /// Writes one `shutdown` frame, best effort.
    ///
    /// Nothing is read back. The response is drained by the reader thread like
    /// any other frame, and what this call is for is the worker's *exit*, which
    /// [`wait_for_voluntary_exit`] observes directly. Waiting for a reply here
    /// would add a second deadline to a path whose whole job is to end.
    fn ask_to_leave(&mut self) {
        // A poisoned stream is one this client cannot describe its position in,
        // so a frame written into it could be read as the tail of something
        // else. Killing is the honest option there.
        if self.poisoned {
            return;
        }
        let frame = WorkerRequestFrame::Shutdown {
            protocol_version: WORKER_PROTOCOL_VERSION.to_owned(),
            request_id: "shutdown".to_owned(),
        };
        let Ok(line) = serde_json::to_vec(&frame) else {
            return;
        };
        let Some(stdin) = self.stdin.as_mut() else {
            return;
        };
        let _ = stdin
            .write_all(&line)
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush());
    }

    /// Asks the worker to leave, then kills the process group and proves it
    /// gone.
    ///
    /// **What it reaches is the process group plus the descendants it
    /// enumerated, which is not the full tree ADR-0001 §10.3 requires.** The
    /// group kill reaches a descendant started after the enumeration below, and
    /// the recorded pidfds reach one that left the group before it — but a
    /// descendant that calls `setsid()` in between is in no group this build
    /// owns and appears in no `/proc` entry the exit left behind, so nothing
    /// can name it and `wait_for_containment` reports success without having
    /// seen it. `ADR-0001-D008` under `docs/adr/deviations/` is the
    /// owner-approved permission for that residual and names E5-S4 as where it
    /// closes; do not read the success below as a contained tree.
    ///
    /// Idempotent, so [`Drop`] after an explicit shutdown does nothing.
    ///
    /// # Errors
    ///
    /// [`BackendError::Protocol`] carrying what the containment boundary
    /// reported, when the tree cannot be signalled, reaped, or observed gone.
    pub(crate) fn shutdown(&mut self) -> Result<(), BackendError> {
        // Asked before it is killed. The protocol has carried a `shutdown`
        // frame since E1-S1 and nothing sent it: closing standard input and
        // going straight to terminating the group is `SIGKILL` on Unix, which
        // gives a worker no chance to release what it holds. ADR-0001 §17.7
        // asks for graceful shutdown, not only for containment.
        //
        // Best effort by design. A worker that is already gone, wedged, or left
        // at an unknown position in the stream cannot be asked anything, and
        // none of those is a reason to skip the containment below — which is
        // why this returns nothing and the kill still runs.
        // Enumerated before the worker is even asked to leave, which is the
        // only moment its children are certainly still nameable:
        // `/proc/<pid>/task/*/children` disappears with the process, so a
        // descendant not recorded by then is one nothing can find afterwards.
        // A failure to look is not a reason to skip the containment below.
        let inspection = self.ownership.refresh().err();
        self.ask_to_leave();
        // Dropped after the frame: a worker blocked reading its next frame sees
        // end of input and leaves on its own, so the kill below is a backstop
        // rather than the normal path.
        self.stdin = None;
        let Some(child) = self.child.take() else {
            return Ok(());
        };
        // One path, whether the worker took the invitation or not, because the
        // wait above observes the exit without reaping. The child is therefore
        // still waitable here, its PID is still allocated, and that PID is the
        // process group ID it was spawned as leader of — POSIX keeps a process
        // group ID unusable while the group still has a member, so the group
        // kill inside `terminate` cannot land on a stranger and does reach a
        // descendant started after the enumeration above, which holds no pidfd
        // and appears in no `/proc` entry the exit left behind.
        wait_for_voluntary_exit(&child);
        let ownership = std::mem::take(&mut self.ownership);
        let result = terminate(child, &self.invocation, ownership)
            .map(|_| ())
            .map_err(|source| protocol_failure("shutdown", &source.to_string()));
        // Drained *before* the readers are joined, never after. The response
        // channel holds one frame, because ADR-0001 §10.3 is one response per
        // request — so a worker that wrote anything past its last frame leaves
        // the protocol reader blocked on a full channel, and joining it first
        // is a deadlock rather than a wait.
        let epilogue = self.epilogue_was_only_the_shutdown_response();
        self.responses = None;
        // Joined after the pipes are closed by the child's exit, so neither
        // reader outlives the client and leaks a thread per worker.
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        // Containment first: a worker that left a process behind is the worse
        // failure of the two, and the epilogue is only about bytes.
        result?;
        if let Some(source) = inspection {
            return Err(protocol_failure("shutdown", &source.to_string()));
        }
        epilogue
    }

    /// Refuses anything the worker left on standard output past its last frame.
    ///
    /// Exactly one event is expected: the response to the `shutdown` frame
    /// [`ask_to_leave`](Self::ask_to_leave) sent and deliberately did not wait
    /// for. Anything else is contamination — a second frame, an unterminated
    /// tail, or a line past the ceiling — and ADR-0001 §17.7 requires standard
    /// output to carry protocol messages and nothing else. Nothing reads this
    /// channel after the last request, so unless it is drained here the last
    /// bytes on the stream are the ones no check ever sees.
    ///
    /// # Errors
    ///
    /// [`BackendError::Protocol`] naming what was left, for the tool owner per
    /// `docs/governance/ROUTING-TABLES.md`.
    fn epilogue_was_only_the_shutdown_response(&self) -> Result<(), BackendError> {
        let responses = self.responses.as_ref().ok_or_else(|| {
            protocol_failure(
                "shutdown",
                "the worker's protocol response stream is closed",
            )
        })?;
        let mut answered = false;
        loop {
            let event = match responses.recv_timeout(WORKER_SHUTDOWN_GRACE) {
                Ok(event) => event,
                // The reader owns the only sender, so a disconnect is it having
                // seen end of input: the stream is complete and this is the
                // whole of what the worker wrote.
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
                // The worker is gone by the time this runs, so its standard
                // output is still open only because something it started
                // inherited the pipe and outlived the containment above.
                Err(RecvTimeoutError::Timeout) => {
                    return Err(protocol_failure(
                        "shutdown",
                        "the worker's standard output stayed open after the worker was gone",
                    ));
                }
            };
            match event {
                ProtocolEvent::Frame(bytes) if !answered => {
                    let frame = parse_worker_response(&bytes)
                        .map_err(|source| protocol_failure("shutdown", &source.to_string()))?;
                    let WorkerResponseFrame::Shutdown { .. } = frame else {
                        return Err(protocol_failure(
                            "shutdown",
                            &format!(
                                "the worker answered the shutdown request with `{}`",
                                frame.event_name()
                            ),
                        ));
                    };
                    answered = true;
                }
                ProtocolEvent::Frame(_) => {
                    return Err(protocol_failure(
                        "shutdown",
                        "the worker wrote a further frame after answering the shutdown request",
                    ));
                }
                ProtocolEvent::Oversized => {
                    return Err(protocol_failure(
                        "shutdown",
                        "the worker wrote a line past the protocol ceiling after its last frame",
                    ));
                }
                ProtocolEvent::Unterminated(bytes) => {
                    return Err(protocol_failure(
                        "shutdown",
                        &format!(
                            "the worker left {bytes} bytes of unterminated output on standard \
                             output after its last frame"
                        ),
                    ));
                }
                ProtocolEvent::Unreadable(message) => {
                    return Err(protocol_failure("shutdown", &message));
                }
            }
        }
    }
}

/// How long a worker asked to leave is given to do it before it is killed.
///
/// Short because it is not a synthesis budget: the worker has already been told
/// to stop, and everything it might still be doing is bounded by the deadlines
/// on the exchange that started it. Long enough that a healthy worker finishing
/// its current write is not killed for being ordinary.
const WORKER_SHUTDOWN_GRACE: Duration = Duration::from_millis(500);

/// Waits out [`WORKER_SHUTDOWN_GRACE`] for a worker to exit on its own.
///
/// Reports nothing, because the caller contains the tree either way; all this
/// buys is that a worker which leaves politely is not killed for being slow.
/// Polls rather than blocking, so a worker that will not leave costs the grace
/// period rather than the rest of the build; the interval mirrors
/// [`crate::process`]'s, whose rule is that a wait never busy-spins.
///
/// **It must not reap.** `Child::try_wait` would, and a reaped child's PID is
/// released — with it the process group ID that equals it, since the child was
/// spawned as its own group leader. The group kill that follows would then be
/// aimed at a number the kernel is free to have given to a stranger, which is
/// why the containment used to fall back to recorded pidfds alone and miss any
/// descendant started after the last enumeration. `WNOWAIT` observes the exit
/// and leaves the child waitable, so the PID — and the group — stay reserved
/// until [`crate::process::terminate`] has signalled and reaped them.
#[cfg(unix)]
fn wait_for_voluntary_exit(child: &Child) {
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};

    let Some(pid) = i32::try_from(child.id()).ok().and_then(Pid::from_raw) else {
        return;
    };
    let deadline = Instant::now() + WORKER_SHUTDOWN_GRACE;
    loop {
        // Unobservable is not gone: an error stops the wait rather than the
        // containment, which proves the tree's state instead of asking.
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        ) {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => {}
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        thread::sleep(SHUTDOWN_POLL_INTERVAL.min(remaining));
    }
}

/// Waits out the grace period without asking, where asking would reap.
///
/// There is no reap-free exit check off Unix, and no process group for
/// [`crate::process::terminate`] to signal either, so the grace is simply spent
/// and the kill does the rest.
#[cfg(not(unix))]
fn wait_for_voluntary_exit(_child: &Child) {
    thread::sleep(WORKER_SHUTDOWN_GRACE);
}

/// How often [`wait_for_voluntary_exit`] asks, mirroring `process`'s interval.
#[cfg(unix)]
const SHUTDOWN_POLL_INTERVAL: Duration = Duration::from_millis(10);

impl Drop for WorkerClient {
    fn drop(&mut self) {
        // A child left running holds the staging directory open and keeps a
        // model resident. Best effort by necessity — `Drop` cannot report — so
        // callers that need the diagnostic call `shutdown` first.
        let _ = self.shutdown();
    }
}

/// Reads whole protocol frames, refusing an oversized one before keeping it.
///
/// The ceiling is enforced while reading rather than after: a hostile or broken
/// worker that never sends a newline would otherwise be handed memory until the
/// process dies, which is the denial of service the ceiling exists to stop.
/// `worker/study_tts_worker/protocol.py` `read_line` is the same rule at the
/// other end.
fn spawn_protocol_reader(stdout: ChildStdout, sender: SyncSender<ProtocolEvent>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut frame = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            match reader.read(&mut byte) {
                Ok(0) => {
                    if !frame.is_empty() {
                        let _ = sender.send(ProtocolEvent::Unterminated(frame.len()));
                    }
                    return;
                }
                Ok(_) if byte[0] == b'\n' => {
                    if sender
                        .send(ProtocolEvent::Frame(std::mem::take(&mut frame)))
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(_) => {
                    if frame.len() == MAX_WORKER_FRAME_BYTES {
                        let _ = sender.send(ProtocolEvent::Oversized);
                        return;
                    }
                    frame.push(byte[0]);
                }
                Err(source) => {
                    let _ = sender.send(ProtocolEvent::Unreadable(source.to_string()));
                    return;
                }
            }
        }
    })
}

/// Drains standard error into a bounded buffer, discarding the overflow.
///
/// Discarding rather than refusing: stderr is diagnostics, and a worker that
/// talks too much should not fail a render that otherwise succeeded.
fn spawn_diagnostic_reader(stderr: ChildStderr, captured: Arc<Mutex<Vec<u8>>>) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut chunk = [0_u8; 8192];
        loop {
            match reader.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(read) => {
                    let mut captured = captured
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let free = MAX_WORKER_STDERR_BYTES.saturating_sub(captured.len());
                    captured.extend_from_slice(&chunk[..read.min(free)]);
                }
            }
        }
    })
}

/// The refusal for a worker that could not be started.
fn startup_failure(source: &std::io::Error) -> BackendError {
    protocol_failure("startup", &source.to_string())
}

/// The refusal for a spawned child that did not expose a standard pipe.
fn missing_pipe(pipe: &str) -> BackendError {
    protocol_failure("startup", &format!("the worker's {pipe} was not captured"))
}

/// Builds the protocol refusal for `request_id`.
fn protocol_failure(request_id: &str, message: &str) -> BackendError {
    BackendError::Protocol {
        request_id: request_id.to_owned(),
        message: message.to_owned(),
    }
}
