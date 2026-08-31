"""Frame reading and writing for the worker's stdin/stdout channel.

The Rust side owns the contract; this module owns only the reading and writing
of it. Three rules from ``docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md``
are enforced here because they cannot be enforced anywhere else in this process:

* **One frame per line, bounded before allocation.** :func:`read_line` reads
  from the binary stream with the ceiling as its own hard limit, so an
  unterminated line cannot make this process allocate megabytes and object
  afterwards. Checking the length of a line the runtime has already buffered
  would be a bound that describes the refusal rather than one that prevents it.
* **stdout carries protocol only, and is taken away to make that true.**
  :func:`reserve_protocol_stream` duplicates the real stdout for
  :func:`write_frame` and then points file descriptor 1 at stderr, so an
  ordinary ``print`` -- or a C library writing to fd 1 from inside a model
  load -- lands on stderr instead of between two frames. A convention that
  diagnostics go to stderr is not enforceable against a dependency; taking the
  descriptor is.
* **Unknown fields are refused, at every depth.** This is a project-owned
  format, so a field this worker cannot honor is an error rather than something
  to ignore -- the requester would otherwise believe it had been applied. The
  shapes below mirror ``schemas/worker-protocol-v1.schema.json``, which
  describes each method's parameters and not only the frame that carries them.

:class:`Object`, :func:`check_object`, and the field checks beside them carry no
underscore because they are the package's shape vocabulary rather than this
module's private business: :mod:`study_tts_worker.worker` describes
`worker/launcher.json` with them, so that file gets the same missing-then-
unknown-then-malformed treatment a frame does. The frame tables themselves stay
private -- what is shared is the checker, not the contract.
"""

from __future__ import annotations

import json
import os
import sys
from collections.abc import Callable, Mapping
from typing import Any, BinaryIO, Final, NamedTuple, TextIO

from . import WORKER_PROTOCOL_EXTENSION_VERSION, WORKER_PROTOCOL_VERSION

MAX_WORKER_FRAME_BYTES: Final[int] = 1024 * 1024
"""Largest frame accepted, mirroring ``study_tts_runtime::MAX_WORKER_FRAME_BYTES``.

``docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`` records the ceiling and
names both spellings of it.
"""

MAX_REQUEST_ID_BYTES: Final[int] = 256
"""Longest correlation identity this worker will accept, in UTF-8 bytes.

Mirrors ``study_tts_runtime::MAX_WORKER_REQUEST_ID_BYTES``, which names this
constant in return. A refusal has to name the request it refuses, so an
identity bounded only by ``MAX_WORKER_FRAME_BYTES`` is one that can make the
answer to a frame larger than the ceiling that answer must fit inside.

Refused here rather than shortened in the response. A shortened identity is a
*different* identity: it comes back looking like some other request that was
answered, and the supervisor correlates nothing while believing it did.
"""

MAX_REFUSAL_MESSAGE_CHARS: Final[int] = 4096
"""Longest ``message`` a failure frame carries.

Diagnostic prose, not an identity, so truncating it loses nothing a reader
needs -- unlike ``request_id``, which is only useful byte for byte. Parser
refusals contain only invariant names, schema-owned paths, and derived bounds;
this ceiling is the final backstop for other failure diagnostics.
"""

MAX_JSON_NESTING_DEPTH: Final[int] = 32
"""Deepest object or array nesting a frame may carry.

A provisional resource ceiling in the sense of
``docs/architecture/WALKING-SKELETON.md`` #Provisional resource ceilings, which
names this constant in return. ``MAX_WORKER_FRAME_BYTES`` bounds a frame's
breadth -- a megabyte holds only so many containers -- but says nothing about
its depth, and depth is what recurses: ``json`` descends nested containers on
the C stack, so a few kilobytes of ``[[[[`` exhausts it. The deepest shape this
protocol defines is four levels (frame, parameters, trace context, its fields),
so this is generous against the contract and well below the interpreter's own
limit.
"""

MAX_JSON_NUMBER_DIGITS: Final[int] = 32
"""Longest numeric literal a frame may carry, in characters.

Also a provisional resource ceiling recorded in that section. The reason is
CPython rather than memory: decimal-to-``int`` conversion is quadratic, and
since 3.11 the interpreter refuses past 4300 digits by raising a bare
``ValueError``. That is not a ``JSONDecodeError``, so it left the parse by a
path nothing caught and ended the process on a frame well inside the byte
ceiling. Bounding the literal first means the refusal names the frame instead.
Every number this protocol defines is a thread count, a seed, or a take.
"""

ACCEPTED_PROTOCOL_VERSIONS: Final[tuple[str, ...]] = (
    WORKER_PROTOCOL_VERSION,
    WORKER_PROTOCOL_EXTENSION_VERSION,
)
"""Every version a request frame may declare.

Exactly ``study_tts_runtime::worker_protocol::validate_version``. Accepting one
fewer than the Rust end is not a conservative choice: the supervisor writes
``1.1`` whenever it sends a trace context, and a worker that refused it would
turn a supported frame into a refusal the supervisor did not ask for.
"""

UNSIGNED_32_MAXIMUM: Final[int] = 2**32 - 1
"""Largest value a field the Rust end reads as ``u32`` can carry.

``threads``, ``take``, ``frames``, and ``sample_rate``. Python integers have no
width, so without this a frame carrying ``4294967296`` was answered here and
dropped by ``serde_json`` there -- the sender seeing a response for a request
its counterpart never accepted.
``schemas/worker-protocol-v1.schema.json`` publishes the same ceilings from the
Rust types, in ``study_tts_runtime::schemas::publish_integer_bounds``.
"""

UNSIGNED_64_MAXIMUM: Final[int] = 2**64 - 1
"""Largest value a field the Rust end reads as ``u64`` can carry: ``seed``."""

_DISCARD_CHUNK_BYTES: Final[int] = 64 * 1024
"""Bytes held at once while dropping the remainder of an over-long line.

The remainder has to be consumed for the next frame to start at a line
boundary, but it must not be consumed into memory: a hostile sender would
otherwise turn the refusal itself into the allocation it was meant to prevent.
"""


class FrameError(Exception):
    """A frame could not be accepted.

    Carries the request ID when one was recoverable, so the caller can correlate
    the refusal with what it sent; ``None`` means the frame was unreadable
    before any ID could be trusted. Its message is published on the protocol
    channel, so it may name only invariants, schema-owned paths, and derived
    bounds -- never a sender-controlled value or field name.
    """

    def __init__(self, message: str, request_id: str | None = None) -> None:
        super().__init__(message)
        self.request_id = request_id


Check = Callable[[Any, str], None]
"""Checks one field, raising :class:`FrameError` naming its JSON path."""


def _bounded_int(literal: str) -> int:
    """Converts one JSON integer literal, refusing one this worker will not parse.

    Passed to :func:`json.loads` as ``parse_int`` so the refusal happens before
    the conversion rather than inside it. Raising :class:`FrameError` here is
    deliberate: it is not a :class:`ValueError`, so it travels out through
    ``json.loads`` untouched and reaches the caller as the refusal it is.
    """
    if len(literal) > MAX_JSON_NUMBER_DIGITS:
        raise FrameError(
            f"frame carries a {len(literal)}-character number but the ceiling is "
            f"{MAX_JSON_NUMBER_DIGITS}"
        )
    return int(literal)


def _distinct_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Builds one JSON object, refusing a name that appears twice.

    Passed to :func:`json.loads` as ``object_pairs_hook``. Without it ``json``
    keeps the last value silently, so a frame naming ``request_id`` twice was
    answered here under one of them while ``serde_json`` refused it outright --
    and a sender could choose which end read which value. Raising
    :class:`FrameError` from inside the hook is the same deliberate trick
    :func:`_bounded_int` uses: it is not a :class:`ValueError`, so it travels
    out through ``json.loads`` as the refusal it is.
    """
    frame: dict[str, Any] = {}
    for name, value in pairs:
        if name in frame:
            raise FrameError("frame names an object field twice")
        frame[name] = value
    return frame


def _check_nesting(value: Any) -> None:
    """Refuses a frame nested deeper than :data:`MAX_JSON_NESTING_DEPTH`.

    Iterative, with its own stack, because a checker that recursed would fail
    the same way the parser it is guarding does. Reached only for frames the
    parser accepted, which is why it is a ceiling and not the whole defense:
    anything deeper than the C stack is refused during the parse instead.
    """
    pending: list[tuple[Any, int]] = [(value, 1)]
    while pending:
        current, depth = pending.pop()
        if depth > MAX_JSON_NESTING_DEPTH:
            raise FrameError(
                f"frame nests deeper than {MAX_JSON_NESTING_DEPTH} levels"
            )
        if isinstance(current, dict):
            pending.extend((child, depth + 1) for child in current.values())
        elif isinstance(current, list):
            pending.extend((child, depth + 1) for child in current)


def text(value: Any, path: str) -> None:
    """Accepts a JSON string."""
    if not isinstance(value, str):
        raise FrameError(f"`{path}` is not a string")


_BLAKE3_HEX_DIGITS: Final[int] = 64
"""Characters in a BLAKE3 digest written as lowercase hexadecimal."""

_BLAKE3_HEX_ALPHABET: Final[frozenset[str]] = frozenset("0123456789abcdef")
"""The only characters a digest may hold.

Together with :data:`_BLAKE3_HEX_DIGITS` this is
``study_tts_core::digest::is_blake3_hex`` and the ``BLAKE3_HEX_PATTERN`` it
publishes into ``schemas/``, which names this function in return.

Spelled as an alphabet rather than checked with ``int(value, 16)``: that call
also accepts underscore separators, a leading sign, and surrounding whitespace,
so ``"a_" * 32`` -- 64 lowercase characters -- would satisfy it and be refused
by the Rust end.
"""


def blake3_hex(value: Any, path: str) -> None:
    """Accepts a BLAKE3 digest as 64 lowercase hexadecimal characters.

    The Rust end parses these fields into value objects that refuse anything
    else, so a worker accepting a looser identity here would accept frames its
    counterpart refuses -- and that disagreement surfaces as a dropped frame
    rather than as a refusal naming the field.
    """
    text(value, path)
    if len(value) != _BLAKE3_HEX_DIGITS or not _BLAKE3_HEX_ALPHABET.issuperset(value):
        raise FrameError(f"`{path}` is not a BLAKE3 digest in lowercase hexadecimal")


def unsigned(maximum: int) -> Check:
    """Builds a check for a JSON integer that fits an unsigned field of `maximum`.

    Bounded rather than merely non-negative, because the field it is checking
    has a width at the other end and Python's integers do not. ``bool`` is
    refused too: it is a subclass of ``int``, so ``true`` would otherwise pass
    as a thread count, a seed, or a take number.
    """

    def check(value: Any, path: str) -> None:
        if not isinstance(value, int) or isinstance(value, bool) or value < 0:
            raise FrameError(f"`{path}` is not an unsigned integer")
        if value > maximum:
            raise FrameError(f"`{path}` exceeds the unsigned integer ceiling of {maximum}")

    return check


def positive(maximum: int) -> Check:
    """Builds a check for an unsigned field that zero cannot satisfy.

    ``threads`` is the field this exists for. Zero is not a smaller allowance
    but an unanswerable instruction -- a worker given no threads cannot run --
    and both ends accepted it while nothing yet reads the value. The Rust end
    reads it as ``NonZeroU32``, so a zero refused there was answered here.
    """
    bounded = unsigned(maximum)

    def check(value: Any, path: str) -> None:
        bounded(value, path)
        if value == 0:
            raise FrameError(f"`{path}` is zero, but at least one is required")

    return check


def request_identity(value: Any, path: str) -> None:
    """Accepts a correlation identity: a string with something in it, bounded.

    Empty is refused rather than tolerated because the identity is what a
    supervisor matches a response to a request by, and a refusal it cannot
    correlate is one it reports as a timeout against whatever it was waiting
    for. ``study_tts_runtime::WorkerFrameError::EmptyRequestId`` is the same
    rule at the other end, and ``RequestIdTooLong`` is the other half:
    :data:`MAX_REQUEST_ID_BYTES` bounds what either end agrees to carry back.
    """
    text(value, path)
    if not value:
        raise FrameError(f"`{path}` is empty")
    if not value.isascii():
        raise FrameError(f"`{path}` must contain only ASCII characters")
    encoded = len(value.encode("utf-8"))
    if encoded > MAX_REQUEST_ID_BYTES:
        raise FrameError(f"`{path}` is {encoded} bytes but the ceiling is {MAX_REQUEST_ID_BYTES}")


def boolean(value: Any, path: str) -> None:
    """Accepts a JSON boolean.

    Not a truthiness test. ``1`` and ``"true"`` are values a reader would take
    for a decision that was made, and this project reads booleans out of files
    that gate whether a worker may reach a network.
    """
    if not isinstance(value, bool):
        raise FrameError(f"`{path}` is not a boolean")


class Object(NamedTuple):
    """One JSON object shape: the fields it must carry and those it may."""

    required: Mapping[str, Check]
    optional: Mapping[str, Check] = {}


def nested(shape: Object) -> Check:
    """Checks a field that is itself an object of the given shape."""
    return lambda value, path: check_object(value, shape, path)


def string_map(value: Any, path: str) -> None:
    """Accepts an object of arbitrary keys whose values are all strings.

    For a record whose *field names* are not this build's to fix --
    ``generation_parameters`` is the one case -- so :class:`Object` cannot
    describe it. The values are still checked, because they reach the synthesis
    key as text: ADR-0001 §12.5 admits no floating point into an identity, so
    the launcher records each parameter's exact spelling and this refuses a
    launcher that wrote one as a number instead.
    """
    if not isinstance(value, dict):
        raise FrameError(f"`{path}` is not a JSON object")
    for name in sorted(value):
        text(value[name], f"{path}.{name}")


def nullable(shape: Object) -> Check:
    """Checks an optional-by-null object, which ``null`` satisfies."""

    def check(value: Any, path: str) -> None:
        if value is not None:
            check_object(value, shape, path)

    return check


_TRACE_CONTEXT: Final[Object] = Object(required={"trace_id": text})

_INITIALIZE_PARAMETERS: Final[Object] = Object(
    required={
        "worker_bundle_hash": blake3_hex,
        "threads": positive(UNSIGNED_32_MAXIMUM),
        # The one directory the worker may write inside. Required rather than
        # optional: a worker that defaulted the boundary would confine writes to
        # somewhere the supervisor never chose, which is not containment.
        "staging_root": text,
    }
)

_SYNTHESIZE_PARAMETERS: Final[Object] = Object(
    required={
        "text": text,
        "voice": text,
        "style": text,
        "seed": unsigned(UNSIGNED_64_MAXIMUM),
        "take": unsigned(UNSIGNED_32_MAXIMUM),
        "output": text,
    },
    optional={"trace_context": nullable(_TRACE_CONTEXT)},
)

_ENVELOPE: Final[Mapping[str, Check]] = {
    "method": text,
    "protocol_version": text,
    "request_id": request_identity,
}

_REQUEST_FRAMES: Final[dict[str, Object]] = {
    "initialize": Object(
        required={**_ENVELOPE, "parameters": nested(_INITIALIZE_PARAMETERS)}
    ),
    "capabilities": Object(required=dict(_ENVELOPE)),
    "health": Object(required=dict(_ENVELOPE)),
    "synthesize": Object(
        required={**_ENVELOPE, "parameters": nested(_SYNTHESIZE_PARAMETERS)}
    ),
    "cancel": Object(required={**_ENVELOPE, "active_request_id": request_identity}),
    "shutdown": Object(required=dict(_ENVELOPE)),
}


def check_object(value: Any, shape: Object, path: str) -> None:
    """Applies one object shape: no field missing, none unknown, each typed.

    Missing before unknown before malformed, so a frame with two mistakes is
    reported by the one an author fixes first rather than by whichever check
    happens to run earliest.
    """
    if not isinstance(value, dict):
        raise FrameError(f"`{path}` is not a JSON object")
    missing = sorted(set(shape.required) - set(value))
    if missing:
        raise FrameError(f"`{path}` is missing {missing}")
    if set(value) - set(shape.required) - set(shape.optional):
        raise FrameError(f"`{path}` carries an unknown field")
    for name, check in (*shape.required.items(), *shape.optional.items()):
        if name in value:
            check(value[name], f"{path}.{name}")


def read_line(stream: BinaryIO) -> bytes | None:
    """Reads one frame's bytes, without the line terminator.

    Returns ``None`` at end of input.

    Reading with ``MAX_WORKER_FRAME_BYTES + 1`` as the limit is what makes the
    ceiling a bound on this process rather than a report about the sender: one
    byte past the ceiling is enough to tell "exactly at the limit" from "over
    it", and nothing beyond it is ever held.

    Raises:
        FrameError: if the line exceeds the ceiling. The rest of that line is
            dropped in bounded chunks first, so the next frame is read from a
            line boundary rather than from the middle of a refused one.
    """
    line = stream.readline(MAX_WORKER_FRAME_BYTES + 1)
    if not line:
        return None
    if line.endswith(b"\n"):
        return line[:-1]
    if len(line) > MAX_WORKER_FRAME_BYTES:
        dropped = _discard_line(stream)
        raise FrameError(
            f"frame is at least {len(line) + dropped} bytes but the ceiling is "
            f"{MAX_WORKER_FRAME_BYTES}"
        )
    # End of input with no terminator: a complete frame within the ceiling.
    return line


def _discard_line(stream: BinaryIO) -> int:
    """Drops the remainder of an over-long line, returning how much it held."""
    dropped = 0
    while True:
        chunk = stream.readline(_DISCARD_CHUNK_BYTES)
        if not chunk:
            return dropped
        dropped += len(chunk)
        if chunk.endswith(b"\n"):
            return dropped


def read_request(line: bytes) -> dict[str, Any]:
    """Parses one request frame.

    Raises:
        FrameError: if the line exceeds the ceiling, is not UTF-8, is not JSON
            this worker will parse, contains a line terminator, is not a JSON
            object, nests deeper than :data:`MAX_JSON_NESTING_DEPTH`, declares
            an unknown protocol version or method, or carries a field -- at any
            depth -- outside the declared representation.

            Every one of those is a refusal and none of them is an exit. A
            hostile frame that ended the process would leave the supervisor
            correlating a timeout instead of reading the refusal it asked for,
            and would take every queued request down with it.
    """
    # Repeated from `read_line` rather than assumed, because this function is
    # the boundary a caller reaches with bytes of its own.
    if len(line) > MAX_WORKER_FRAME_BYTES:
        raise FrameError(
            f"frame is {len(line)} bytes but the ceiling is {MAX_WORKER_FRAME_BYTES}"
        )
    if b"\n" in line or b"\r" in line:
        raise FrameError("frame contains a line terminator")
    try:
        body = line.decode("utf-8")
    except UnicodeDecodeError as error:
        raise FrameError("frame is not UTF-8") from error
    try:
        frame = json.loads(
            body, parse_int=_bounded_int, object_pairs_hook=_distinct_keys
        )
    except json.JSONDecodeError as error:
        raise FrameError("frame is not valid JSON") from error
    # `json` refuses some syntactically valid input by raising outside
    # `JSONDecodeError`, and every such raise used to leave this function by a
    # path `worker.py` does not catch -- so a frame well inside the byte ceiling
    # ended the process instead of drawing a refusal. `RecursionError` is
    # nesting deeper than the C stack, and stays reachable: `_check_nesting`
    # runs after the parse and cannot help during it.
    #
    # The `ValueError` clause has no reachable case left and stays anyway.
    # `_bounded_int` closed the one that was found -- CPython's 4300-digit
    # integer-conversion limit -- but which valid-looking input raises what is
    # the interpreter's to decide and has changed between versions, and being
    # wrong costs a dead process rather than a refused frame. Caught after
    # `JSONDecodeError`, which is itself a `ValueError` and would otherwise be
    # reported as the vaguer of the two.
    except RecursionError as error:
        raise FrameError("frame nests deeper than this worker will parse") from error
    except ValueError as error:
        raise FrameError("frame is JSON this worker will not parse") from error
    if not isinstance(frame, dict):
        raise FrameError("frame is not a JSON object")
    _check_nesting(frame)

    method = frame.get("method")
    # Recovered before the shape checks so an early refusal is still
    # correlated, and dropped when it is past the ceiling those checks refuse
    # it at: an identity too long to carry back is answered as `unknown`
    # rather than as a shortened one the supervisor would match to nothing.
    request_id = frame.get("request_id")
    if (
        not isinstance(request_id, str)
        or not request_id.isascii()
        or len(request_id.encode("utf-8")) > MAX_REQUEST_ID_BYTES
    ):
        request_id = None
    version = frame.get("protocol_version")
    if not isinstance(method, str):
        raise FrameError("`frame.method` is not a string", request_id)
    if method not in _REQUEST_FRAMES:
        raise FrameError("frame method is unsupported", request_id)
    if not isinstance(version, str):
        raise FrameError("`frame.protocol_version` is not a string", request_id)
    if version not in ACCEPTED_PROTOCOL_VERSIONS:
        raise FrameError("frame protocol version is unsupported", request_id)
    try:
        check_object(frame, _REQUEST_FRAMES[method], "frame")
    except FrameError as error:
        # The shape checks know the path they failed at but not the correlation
        # ID, which was recovered above; the supervisor needs both.
        error.request_id = request_id
        raise
    _check_extension_version(frame, version, request_id)
    return frame


def _check_extension_version(
    frame: dict[str, Any], version: str, request_id: str | None
) -> None:
    """Refuses a 1.1 field on a frame that declares 1.0.

    Presence is what decides, not the value: an explicit ``"trace_context":
    null`` is a sender saying it knows about the extension, and a baseline frame
    that accepted it would accept a shape the Rust end refuses. What the field
    would have carried is irrelevant, so the version is checked before the value
    is read at all -- exactly
    ``study_tts_runtime::worker_protocol::parse_worker_request``, whose
    ``ExtensionRequiresVersion`` refusal this mirrors.

    Runs after the shape check, so ``parameters`` is already known to be an
    object where the method defines one and absent where it does not.
    """
    if version == WORKER_PROTOCOL_EXTENSION_VERSION:
        return
    if "trace_context" in frame.get("parameters", {}):
        raise FrameError(
            f"`frame.parameters.trace_context` requires protocol "
            f"{WORKER_PROTOCOL_EXTENSION_VERSION!r}",
            request_id,
        )


def reserve_protocol_stream() -> TextIO:
    """Takes stdout for the protocol and points ordinary writes at stderr.

    Returns the stream :func:`write_frame` must be given. Nothing else in this
    process can reach it, because it is a duplicate of the original descriptor
    rather than ``sys.stdout``.

    **Call this before importing a speech backend.** The descriptor is what a
    dependency writes to, not the ``sys.stdout`` object: a native library
    printing a progress bar or a CUDA notice writes to file descriptor 1 and
    never consults Python's file objects. After ``dup2`` that descriptor is
    stderr, so such a write is a diagnostic instead of a byte sequence in the
    middle of an NDJSON frame -- which the supervisor can only report as a
    protocol failure it cannot attribute. Import the backend afterwards and the
    whole load is covered; import it first and its module-level output is not.

    Idempotence is not attempted. Calling this twice would leak the first
    duplicate and hand out a second stream aliasing the same file, and there is
    one caller: :func:`study_tts_worker.worker.main`.
    """
    sys.stdout.flush()
    protocol_fd = os.dup(sys.stdout.fileno())
    os.dup2(sys.stderr.fileno(), sys.stdout.fileno())
    return os.fdopen(protocol_fd, "w", encoding="utf-8", newline="\n")


def write_frame(stream: TextIO, frame: dict[str, Any]) -> None:
    """Writes one response frame to the reserved stream and flushes it.

    Takes the stream rather than reaching for ``sys.stdout``, which after
    :func:`reserve_protocol_stream` is stderr: a writer that looked the stream
    up itself would send every frame to the diagnostic channel and report
    nothing.

    Flushed per frame rather than per buffer because the reader is a supervising
    process waiting on this line: a buffered frame is a frame the supervisor
    reports as a timeout.
    """
    json.dump(frame, stream, separators=(",", ":"), sort_keys=True)
    stream.write("\n")
    stream.flush()


def failure(request_id: str, code: str, message: str, recoverable: bool) -> dict[str, Any]:
    """Builds a failure frame in the vocabulary Rust parses.

    ``message`` must already be free of source text and voice paths; ADR-0001
    §16 keeps both off the protocol channel, and this function cannot check
    that for the caller.

    ``message`` is bounded here rather than at each refusal site, because every
    failure frame this worker writes is built by this function and a ceiling one
    caller forgets is not a ceiling. ``request_id`` is deliberately *not*
    bounded here: it is repeated exactly as given, because an identity the
    supervisor cannot match against what it sent is worse than no answer, and
    the ceiling that keeps it small belongs at validation
    (:func:`request_identity`) where an oversized one is refused rather than
    quietly rewritten into a different request's identity.
    """
    return {
        "event": "failure",
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": request_id,
        "code": code,
        "message": _bounded(message),
        "recoverable": recoverable,
    }


def _bounded(message: str) -> str:
    """Truncates a refusal message to :data:`MAX_REFUSAL_MESSAGE_CHARS`.

    The original length is kept in the replacement so a reader can tell a
    message that was truncated from one that was written that way. Only prose
    goes through here; see :func:`failure` for why an identity does not.
    """
    if len(message) <= MAX_REFUSAL_MESSAGE_CHARS:
        return message
    return f"{message[:MAX_REFUSAL_MESSAGE_CHARS]} (truncated from {len(message)} characters)"
