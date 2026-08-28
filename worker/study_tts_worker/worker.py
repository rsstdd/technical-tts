"""The worker process: read frames, answer what this build can answer, refuse
the rest by name.

**Synthesis is not implemented here, and this module does not pretend it is.**
ADR-0001 §7.2 and DELIVERY-PLAN E1-S3 place the real Chatterbox backend in
E1-S3; E1-S1 delivers the locked environment, the bundle identity, and the
protocol surface. A ``synthesize`` request is therefore answered with the
``initialization_failed`` code and a message saying which story supplies the
backend -- not with silence, not with a placeholder tone, and not with a
success frame naming audio nobody produced. ``AGENTS.md`` forbids shipping a
stub as though it were implemented, and a worker that returned a tone would be
exactly that: the cache would publish it under a key claiming a real model made
it.

The deterministic tone double used by the workspace's tests lives in
``study-tts-testkit`` and is never mistaken for this process, because it is a
different executable with a different declared bundle identity.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from typing import Any, Final

from . import WORKER_PROTOCOL_VERSION
from .protocol import (
    FrameError,
    Object,
    UNSIGNED_32_MAXIMUM,
    boolean,
    check_object,
    failure,
    nested,
    positive,
    read_line,
    read_request,
    reserve_protocol_stream,
    text,
    write_frame,
)

REQUIRED_OFFLINE_ENVIRONMENT: Final[tuple[str, ...]] = (
    "HF_HUB_OFFLINE",
    "TRANSFORMERS_OFFLINE",
)
"""Offline variables ADR-0001 §14 requires the launcher to set to ``1``.

Named in Python rather than read from `worker/launcher.json` alone, because a
launcher that decides which variables matter decides whether the worker is
offline. `docs/operations/WORKER-ENVIRONMENT.md` #Offline behavior names this
constant in return, and lists the third variable the launcher also carries --
`HF_HUB_DISABLE_PROGRESS_BARS`, which is applied like the others but is not
required, since a progress bar is noise rather than a fetch.
"""

OPTIONAL_OFFLINE_ENVIRONMENT: Final[tuple[str, ...]] = (
    "HF_HUB_DISABLE_PROGRESS_BARS",
)
"""Offline variables the launcher may carry but need not.

Together with :data:`REQUIRED_OFFLINE_ENVIRONMENT` this is the *complete* set
of variables this worker will put into its own environment, and that closure is
the point rather than a tidiness. `offline_environment` used to be copied entry
by entry into ``os.environ``, so a launcher that named `PYTHONPATH` chose what
the backend imported one statement later -- from a file that is a declared
bundle input and therefore looks governed. The two tuples are the allowlist;
:data:`LAUNCHER_SHAPE` refuses anything outside them at the parse.
"""

LAUNCHER_SCHEMA_VERSION: Final[str] = "1.0"
"""Launcher layout this build reads.

Refused rather than guessed at, like every other versioned record here: a
launcher written for a later layout may mean something different by a field
this build would otherwise read under the old meaning.
`docs/operations/WORKER-ENVIRONMENT.md` #The launcher is read closed names this
constant in return.
"""

LAUNCHER_SHAPE: Final[Object] = Object(
    required={
        "schema_version": text,
        "device": text,
        # The same count reaches the Rust end as `initialize.parameters.threads`,
        # so it is held to the width and the floor that field is read at rather
        # than left to Python's unbounded integers. Nothing applies it before
        # E1-S3; refusing a value no application could honor is still this
        # parse's job.
        "threads": positive(UNSIGNED_32_MAXIMUM),
        "offline_environment": nested(
            Object(
                required={name: text for name in REQUIRED_OFFLINE_ENVIRONMENT},
                optional={name: text for name in OPTIONAL_OFFLINE_ENVIRONMENT},
            )
        ),
        "local_files_only": boolean,
        "model_root_environment_variable": text,
    }
)
"""The complete shape of `worker/launcher.json`, with nothing else admitted.

Expressed with :mod:`study_tts_worker.protocol`'s object checker rather than a
second validator written here, because it is the same job on the same kind of
input: missing, unknown, then malformed, reported by JSON path. Reusing it also
means `offline_environment` gets the unknown-field rule for free, and that rule
is what closes the injection above -- an extra variable is a refusal at startup
rather than an entry in ``os.environ``.
"""

LAUNCHER_CONFIG = Path(__file__).resolve().parent.parent / "launcher.json"
"""Launcher configuration, beside the package rather than inside it.

ADR-0001 §12.5 makes launcher configuration that affects inference a
worker-bundle input, and `worker/bundle-manifest.json` declares this exact path.
Reading it from anywhere else would let the bundle hash describe a file the
worker did not use.
"""


def _load_launcher() -> dict[str, Any]:
    """Reads the launcher configuration this worker was built against.

    Checked against :data:`LAUNCHER_SHAPE` here rather than at each read, so
    every later access is to a field this build has already agreed the file
    carries. An unreadable launcher is a :class:`SystemExit` and not a refused
    frame: it is the worker's own configuration, so there is no request to
    correlate a refusal with and nothing correct left to serve.

    The version is checked before the complete shape. A future layout may add
    fields this build does not know, and reporting one as an unknown field would
    send the operator to edit a record this build cannot read anyway.

    Raises:
        SystemExit: if the file is not the record this build reads.
    """
    try:
        launcher = json.loads(LAUNCHER_CONFIG.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        raise SystemExit(
            f"{LAUNCHER_CONFIG} cannot be read as launcher JSON: {error}"
        ) from error

    if isinstance(launcher, dict):
        version = launcher.get("schema_version")
        if isinstance(version, str) and version != LAUNCHER_SCHEMA_VERSION:
            raise SystemExit(
                f"{LAUNCHER_CONFIG} declares layout {version!r} but this build reads "
                f"{LAUNCHER_SCHEMA_VERSION!r}; align the launcher and the build rather "
                "than start a worker under a layout it only partly understands"
            )
    try:
        check_object(launcher, LAUNCHER_SHAPE, "launcher")
    except FrameError as error:
        raise SystemExit(
            f"{LAUNCHER_CONFIG} is not a launcher this build reads: {error}"
        ) from error
    return launcher


def _apply_offline_environment(launcher: dict[str, Any]) -> None:
    """Puts the launcher's offline settings into this process's environment.

    Configuration that is read and not applied is a claim, and ADR-0001 §14
    needs this one to be a boundary: `huggingface_hub` and `transformers` decide
    whether to reach a network by reading these variables at import time, from
    the environment, and neither of them will ever see `worker/launcher.json`.
    Until this ran, the file recorded an intention that nothing carried out.

    **Called before a speech backend is imported**, for that reason -- the
    variables are read as the backend's modules load, so setting them afterwards
    sets them for nobody. The same ordering rule covers
    :func:`study_tts_worker.protocol.reserve_protocol_stream`, and
    :func:`main` does both before any backend import.

    Refuses rather than corrects. A launcher missing one of
    :data:`REQUIRED_OFFLINE_ENVIRONMENT`, setting it to anything but ``1``, or
    turning `local_files_only` off describes a worker that may fetch, and this
    build has no way to publish audio under a cache key that says it did not.
    Filling in a default here would hide the disagreement instead.

    **Only the named variables are applied**, and the loop is over the allowlist
    rather than over the file. This function writes into the environment a
    speech backend imports from one statement later, so iterating the launcher's
    own entries made `worker/launcher.json` a place to set `PYTHONPATH` for that
    import -- in a file that is a declared bundle input and therefore reads as
    governed. :data:`LAUNCHER_SHAPE` refuses such an entry at the parse and this
    loop could not apply one anyway; both, because a single guard on a path this
    short is a guard the next edit can walk around.

    Raises:
        SystemExit: if the launcher does not describe an offline worker.
    """
    if launcher["local_files_only"] is not True:
        raise SystemExit(
            f"{LAUNCHER_CONFIG} sets local_files_only to "
            f"{launcher['local_files_only']!r}; ADR-0001 §14 renders offline, so a worker "
            "that may resolve a model from a network must not start"
        )

    offline = launcher["offline_environment"]
    for name in REQUIRED_OFFLINE_ENVIRONMENT:
        if offline.get(name) != "1":
            raise SystemExit(
                f"{LAUNCHER_CONFIG} sets {name} to {offline.get(name)!r}, not '1'; "
                "ADR-0001 §14 renders offline, so a worker that may resolve a model from a "
                "network must not start"
            )

    applied = [
        name
        for name in (*REQUIRED_OFFLINE_ENVIRONMENT, *OPTIONAL_OFFLINE_ENVIRONMENT)
        if name in offline
    ]
    for name in applied:
        os.environ[name] = offline[name]

    # On stderr because stdout is the protocol channel, and as evidence rather
    # than decoration: it is what a subprocess test reads to see that the
    # variables were applied in this process and not merely present in a file.
    # The names printed are the ones applied rather than the ones present, so
    # the line reports what happened rather than what was asked for.
    print(
        f"study-tts-worker: offline environment applied: {sorted(applied)}",
        file=sys.stderr,
        flush=True,
    )


def _capabilities(launcher: dict[str, Any]) -> dict[str, Any]:
    """Reports the envelope this build can actually honor.

    ``voices`` is empty and stays empty: this build resolves no voice profile,
    and ADR-0001 §12.1 makes an unresolved voice a refusal rather than a
    default. Declaring a voice here would be a claim that survives into a cache
    key.
    """
    return {
        "languages": ["en"],
        "max_text_bytes": 64 * 1024,
        "voices": [],
        "styles": [],
        "sample_rate": 24000,
        "channels": 1,
        "sample_format": "f32le",
        "deterministic_seed": False,
        "device": launcher["device"],
    }


def _respond(frame: dict[str, Any], launcher: dict[str, Any]) -> dict[str, Any]:
    """Produces the response frame for one accepted request."""
    request_id = frame["request_id"]
    method = frame["method"]

    if method == "initialize":
        return {
            "event": "initialized",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            # No model identity, because no model was loaded. An identity map
            # reporting a revision this process never read would be the exact
            # provenance lie the Rust cache refuses at publication.
            "identities": {
                "worker_bundle_hash": frame["parameters"]["worker_bundle_hash"],
            },
        }
    if method == "capabilities":
        return {
            "event": "capabilities",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "capabilities": _capabilities(launcher),
        }
    if method == "health":
        return {
            "event": "health",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "ready": False,
            "model_loaded": False,
        }
    if method == "synthesize":
        return failure(
            request_id,
            "initialization_failed",
            "this worker build has no speech backend loaded; DELIVERY-PLAN E1-S3 "
            "wires the qualified Chatterbox backend, and until then no audio may "
            "be published under a synthesis key claiming one produced it",
            recoverable=False,
        )
    if method == "cancel":
        return {
            "event": "cancelled",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "active_request_id": frame["active_request_id"],
        }
    return {
        "event": "shutdown",
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": request_id,
    }


def _refusal(error: FrameError) -> dict[str, Any]:
    """Publishes a parser-owned invariant/path diagnostic as a failure."""
    return failure(
        error.request_id or "unknown",
        "invalid_request",
        str(error),
        recoverable=False,
    )


def main() -> int:
    """Serves frames from stdin until shutdown or end of input.

    A frame this worker cannot read is answered with a failure frame rather than
    by exiting: the supervisor correlates refusals by request ID, and a process
    that vanished mid-stream leaves it correlating a timeout instead.

    Read from ``sys.stdin.buffer`` rather than by iterating ``sys.stdin``:
    iteration buffers a whole line before anything can object to its length, so
    the frame ceiling would be enforced only after a hostile sender had already
    been given the memory.

    Applying the offline environment and reserving the protocol descriptor are
    ordered and load-bearing. A speech backend must be imported after both
    rather than at module scope: its modules read the offline variables while
    loading, and its dependencies must not be able to write to the protocol
    descriptor.
    """
    launcher = _load_launcher()
    _apply_offline_environment(launcher)
    protocol = reserve_protocol_stream()

    while True:
        try:
            line = read_line(sys.stdin.buffer)
        except FrameError as error:
            write_frame(protocol, _refusal(error))
            continue
        if line is None:
            break
        if not line:
            continue
        try:
            frame = read_request(line)
        except FrameError as error:
            write_frame(protocol, _refusal(error))
            continue
        response = _respond(frame, launcher)
        write_frame(protocol, response)
        if response["event"] == "shutdown":
            break
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
