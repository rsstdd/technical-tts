"""The worker process: read frames, answer what this build can answer, refuse
the rest by name.

The Chatterbox backend is loaded here, once per process. ADR-0001 §10.1 makes
that the reason this process is persistent at all: the model load is the
expensive part, and a worker that reloaded per segment would pay it for nothing.
``initialize`` performs it and reports the identities every cache key is built
from; every later frame is served against what that load produced.

Two orderings are load-bearing and neither is incidental. The offline
environment is applied and the protocol descriptor reserved **before** any
backend import, because `huggingface_hub` and `transformers` read those
variables as they load and a backend's dependencies must not be able to write to
the protocol channel. And every check that can be made without importing a
backend is made first, so a misconfigured root is reported before seconds and
hundreds of megabytes are spent proving it.

What this module will not do is claim more than it did: a request it cannot
honor is answered with a correlated failure frame carrying the backend's own
code, never with silence, a placeholder tone, or a success frame naming audio
nobody produced. ``AGENTS.md`` forbids shipping a stub as though it were
implemented, and the cache would publish a tone under a key claiming a real
model made it.

The deterministic tone double used by the workspace's tests lives in
``study-tts-testkit`` and is never mistaken for this process, because it is a
different executable with a different declared bundle identity.
"""

from __future__ import annotations

import json
import os
import random
import sys
from pathlib import Path
from typing import Any, Final, NamedTuple

from . import WORKER_PROTOCOL_VERSION
from .protocol import (
    FrameError,
    Object,
    UNSIGNED_32_MAXIMUM,
    UNSIGNED_64_MAXIMUM,
    boolean,
    check_object,
    failure,
    nested,
    positive,
    read_line,
    read_request,
    reserve_protocol_stream,
    string_map,
    text,
    unsigned,
    write_frame,
)

CANONICAL_SAMPLE_RATE_HZ: Final[int] = 24_000
"""Sample rate every published take carries.

Mirrors `study_tts_core::CANONICAL_SAMPLE_RATE`; a worker rendering at another
rate is refused at load rather than resampled, because resampling here would
publish audio no cache key describes.
"""

CANONICAL_CHANNELS: Final[int] = 1
"""Channel count every published take carries, mirroring
`study_tts_core::CANONICAL_CHANNELS`."""

CANONICAL_SAMPLE_FORMAT: Final[str] = "f32le"
"""Sample format every published take carries, mirroring
`study_tts_core::CANONICAL_SAMPLE_FORMAT`."""

CALM_EXPLANATORY_STYLE: Final[str] = "calm_explanatory"
"""The one delivery style `worker/launcher.json` parameterises.

Mirrors the `DeliveryStyle::CalmExplanatory` serde spelling in
`crates/study-tts-core/src/plan.rs`. Chatterbox has no style axis, so this build
honors exactly this one and refuses every other by name rather than mapping two
styles onto identical parameters.
"""

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

`REQUIRED_OFFLINE_ENVIRONMENT` and `OPTIONAL_OFFLINE_ENVIRONMENT` in
`crates/study-tts-runtime/src/worker_launcher.rs` are the same allowlist on the
launching side, and `child_environment` applies it for the same reason: since
`WorkerClient::spawn` clears the environment, the parent sets these variables
too, and iterating the launcher's own entries there would reopen exactly the
hole this closure exists to shut.
"""

LAUNCHER_SCHEMA_VERSION: Final[str] = "1.1"
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
        # Both reach the ADR-0001 §12.5 synthesis key through the Rust end,
        # which reads this same file. They live here rather than in either
        # implementation because a parameter one end used and the other keyed
        # would name audio the key does not describe.
        "seed": unsigned(UNSIGNED_64_MAXIMUM),
        "model_repository": text,
        "generation_parameters": string_map,
        "offline_environment": nested(
            Object(
                required={name: text for name in REQUIRED_OFFLINE_ENVIRONMENT},
                optional={name: text for name in OPTIONAL_OFFLINE_ENVIRONMENT},
            )
        ),
        "local_files_only": boolean,
        "model_root_environment_variable": text,
        "voice_root_environment_variable": text,
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


class BackendUnavailable(Exception):
    """The backend cannot be loaded or used, with the reason to report.

    Carried rather than raised as ``SystemExit`` because these are answers to a
    correlated request: ADR-0001 §10.3 has the supervisor match a refusal to the
    request that caused it, and a process that vanished mid-stream leaves it
    correlating a timeout instead.
    """


class _Session:
    """What one worker process has loaded, for its lifetime.

    A small object rather than module state, so the loaded model is passed to
    the code that uses it and a test can serve frames without a process-wide
    backend left over from another one.
    """

    def __init__(self) -> None:
        self.backend: _Backend | None = None
        # The identity `initialize` was asked to confirm, echoed on every
        # success frame so the parent can see the worker did not change bundles
        # mid-session.
        self.worker_bundle_hash = ""
        # The one directory this worker may write inside, opened once at
        # `initialize` and held: every later containment decision is a walk
        # from this descriptor rather than a check on a name the supervisor
        # sent. See `_staging_root`.
        self.staging: _StagingRoot | None = None


class _Backend:
    """One loaded model and the voices it may speak with.

    Held for the process lifetime and never reloaded. ADR-0001 §10.1 gives each
    worker one model load per lifetime, which is the whole reason this process
    is persistent; a second load would make the cost of that design buy nothing.
    """

    def __init__(
        self,
        model: Any,
        model_revision: str,
        codec_revision: str,
        conditioning: dict[str, str],
        conditionals: dict[str, Any],
        sample_rate: int,
    ) -> None:
        self.model = model
        self.model_revision = model_revision
        self.codec_revision = codec_revision
        # Profile identity to the conditioning digest its own record states.
        self.conditioning = conditioning
        # Profile identity to the conditioning the model is set to before a
        # take. Held rather than reloaded per request: the artifact is the
        # expensive half of a voice, and ADR-0001 §10.1's one-load-per-lifetime
        # covers what a voice costs as much as what a model costs.
        self.conditionals = conditionals
        self.sample_rate = sample_rate


def _governed_root(launcher: dict[str, Any], variable_key: str) -> Path:
    """The governed root named by the launcher's own environment variable.

    Read by the name the launcher declares rather than a name written here:
    `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps the path out of every
    committed file, so the variable name is the only half of the arrangement
    both ends can agree on in writing. `WorkerLauncher::child_environment` in
    `crates/study-tts-runtime/src/worker_launcher.rs` is what sets it.

    Raises:
        BackendUnavailable: if the variable is unset or names no directory.
    """
    variable = launcher[variable_key]
    value = os.environ.get(variable, "")
    if not value:
        raise BackendUnavailable(
            f"{variable} is not set, so this worker was not told where its governed root is"
        )
    root = Path(value)
    if not root.is_dir():
        raise BackendUnavailable(f"{variable} does not name a directory this worker can read")
    return root


def _read_json(path: Path, label: str) -> dict[str, Any]:
    """Reads one governed JSON record, refusing anything else.

    Raises:
        BackendUnavailable: if the record cannot be read as a JSON object.
    """
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError, RecursionError) as error:
        raise BackendUnavailable(f"the {label} could not be read: {_redacted_detail(error)}") from error
    if not isinstance(document, dict):
        raise BackendUnavailable(f"the {label} is not a JSON object")
    return document


def _model_identities(model_root: Path, launcher: dict[str, Any]) -> tuple[str, str]:
    """The model and codec revisions this root's acquisition record states.

    Read from the record rather than asked of the model, because the model
    object knows which weights it loaded and not which approved acquisition they
    came from -- and ADR-0001 §12.5 keys every cache entry on the latter.

    The codec revision is the Chatterbox *code* commit: the tokenizer and codec
    ship with that code, so a change to them is a change to it, while the
    weights revision can stand still across either.

    Raises:
        BackendUnavailable: if the record is unreadable, or describes a
            different repository than the launcher both ends read.
    """
    manifest = _read_json(model_root / "bundle-manifest.json", "model acquisition record")
    model = manifest.get("model")
    code = manifest.get("code")
    if not isinstance(model, dict) or not isinstance(code, dict):
        raise BackendUnavailable("the model acquisition record has no model or code section")

    repository = model.get("repository")
    if repository != launcher["model_repository"]:
        raise BackendUnavailable(
            f"the governed model root holds {repository!r} but this bundle is built for "
            f"{launcher['model_repository']!r}"
        )

    revision = model.get("revision")
    commit = code.get("commit")
    if not isinstance(revision, str) or not revision:
        raise BackendUnavailable("the model acquisition record states no model revision")
    if not isinstance(commit, str) or not commit:
        raise BackendUnavailable("the model acquisition record states no code commit")
    return revision, commit


def _voice_conditioning(voice_root: Path) -> dict[str, str]:
    """Each profile beneath `voice_root`, and the digest its record states.

    Every entry is something this worker went and looked at, which is what lets
    the executor refuse a worker that rendered with a voice the plan did not ask
    for. It reports the digest `profile.json` records rather than one computed
    here: no locked distribution provides BLAKE3, and
    `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` §Why the reported digest
    is a conditioning hash records that limit and who verifies the artifact
    against the record instead -- `voice_gate::load_profile`, on the Rust side,
    before any synthesis runs.

    The skip list below is the other end of `voice_gate::admit_voice_root`,
    which runs the consent, rights, scope, and checksum gate over every entry
    this function would return -- before this process is started at all. It
    must skip at most what this skips, because `_load_backend` deserializes
    every profile named here, so anything skipped there and loaded here reaches
    `torch.load` ungated. The two filters are one rule written twice, and
    neither may narrow without the other.

    Raises:
        BackendUnavailable: if the root holds no usable profile.
    """
    conditioning: dict[str, str] = {}
    for candidate in sorted(voice_root.iterdir()):
        record = candidate / "profile.json"
        if not candidate.is_dir() or candidate.is_symlink() or not record.is_file():
            continue
        profile = _read_json(record, f"voice profile record in {candidate.name}")
        identity = profile.get("profile_id")
        digest = profile.get("conditionals_blake3")
        if not isinstance(identity, str) or not isinstance(digest, str):
            raise BackendUnavailable(
                f"the voice profile record in {candidate.name} states no identity and digest"
            )
        # The identity must be its own directory's name. It is read out of the
        # record, and the conditioning artifact is loaded from
        # `voice_root / identity`: a record free to state anything is a file's
        # contents choosing a path component, and one stating `../../elsewhere`
        # selects an artifact outside the governed voice root. Requiring the two
        # to agree makes containment hold by construction, and makes the
        # existence check below and the load in `_load_backend` speak about one
        # directory rather than two that may differ.
        if identity != candidate.name:
            raise BackendUnavailable(
                f"the voice profile record in {candidate.name} states the identity "
                f"{identity!r}; a profile is read only from the directory that names it"
            )
        if not (candidate / "conditionals.pt").is_file():
            raise BackendUnavailable(f"voice profile {identity!r} has no conditioning artifact")
        conditioning[identity] = digest
    if not conditioning:
        raise BackendUnavailable("the governed voice root holds no usable voice profile")
    return conditioning


def _load_backend(launcher: dict[str, Any], threads: int) -> _Backend:
    """Loads the model once, and every voice this root can speak with.

    Every check that can be made without importing a backend is made first. An
    import of Torch and Chatterbox costs seconds and hundreds of megabytes, and
    a worker whose roots are misconfigured should say so before paying for it --
    which is also what lets the refusals below be tested where Torch is not
    installed at all.

    Raises:
        BackendUnavailable: for any root, record, or backend fault, carrying the
            reason to publish as a correlated refusal.
    """
    model_root = _governed_root(launcher, "model_root_environment_variable")
    voice_root = _governed_root(launcher, "voice_root_environment_variable")
    model_revision, codec_revision = _model_identities(model_root, launcher)
    conditioning = _voice_conditioning(voice_root)

    # Imported here, not at module scope: these read the offline variables as
    # they load, and `main` applies those before serving any frame. An import at
    # module scope would read them before this process had set them.
    try:
        import torch
        from chatterbox.tts import ChatterboxTTS, Conditionals
    except ImportError as error:
        raise BackendUnavailable(f"the speech backend is not installed: {_redacted_detail(error)}") from error

    # Applied before the model loads, because the pools are sized as the native
    # libraries initialize. ADR-0001 §10.1 caps every one of them at the same
    # per-worker value.
    torch.set_num_threads(threads)
    torch.set_num_interop_threads(1)

    try:
        model = ChatterboxTTS.from_local(str(model_root / f"model-{model_revision}"), "cpu")
    except Exception as error:  # noqa: BLE001 - any backend fault is one refusal
        raise BackendUnavailable(f"the model could not be loaded: {_redacted_detail(error)}") from error

    sample_rate = getattr(model, "sr", None)
    if sample_rate != CANONICAL_SAMPLE_RATE_HZ:
        raise BackendUnavailable(
            f"the model renders at {sample_rate!r} Hz but this build publishes "
            f"{CANONICAL_SAMPLE_RATE_HZ} Hz"
        )

    # Loaded per profile so the conditioning reported above is conditioning this
    # process actually holds, rather than a record it read and could not use.
    conditionals: dict[str, Any] = {}
    try:
        for identity in conditioning:
            conditionals[identity] = Conditionals.load(
                str(voice_root / identity / "conditionals.pt"), map_location="cpu"
            ).to("cpu")
    except Exception as error:  # noqa: BLE001 - any backend fault is one refusal
        raise BackendUnavailable(f"a voice conditioning artifact could not be loaded: {_redacted_detail(error)}")

    return _Backend(
        model, model_revision, codec_revision, conditioning, conditionals, sample_rate
    )


def _redacted_detail(error: BaseException) -> str:
    """Describes a fault without quoting the text the fault carries.

    ADR-0001 16 keeps source text and voice paths off the protocol channel, and
    :func:`study_tts_worker.protocol.failure` says in as many words that it
    cannot check the message it is handed. A raw exception string is what
    carries both: ``OSError`` renders the filename it failed on, and a
    generation fault can echo the reviewed text it was asked to speak.

    So the fault's own message is dropped and the type name is reported instead,
    which is a fixed vocabulary rather than data. ``OSError`` additionally keeps
    ``strerror`` -- the kernel's words for *why*, with no path in them -- because
    "Permission denied" and "No such file or directory" send an operator to
    different repairs and neither reveals anything.
    """
    if isinstance(error, OSError) and error.strerror:
        return f"{type(error).__name__}: {error.strerror}"
    return type(error).__name__


class _StagingRoot(NamedTuple):
    """The one directory this worker may write inside, held open.

    The two travel together because neither is usable alone: the descriptor is
    the boundary, and the path is only how an assigned output is spelled
    relative to it.
    """

    path: Path
    descriptor: int


def _staging_root(assigned: str) -> _StagingRoot:
    """Opens the one directory this worker may write inside, and keeps it open.

    Opened once, at `initialize`, and held for the session. A descriptor rather
    than a resolved path, because containment then stops being a check made on
    a name and becomes a property of how the file is reached: every later
    request walks from *this* descriptor, opening each component with
    `O_NOFOLLOW`, so there is no pathname to re-walk and nothing a symlink
    swapped in later can redirect. ADR-0001 §10.3 confines worker writes to the
    assigned staging root.

    The path is kept beside it only for :func:`_contained_output` to spell an
    assigned output relative to the root. It is the supervisor's own spelling
    and deliberately not resolved: the supervisor builds both the root and
    every output path from one absolute base, so the two agree textually, and
    resolving here would only invite them to disagree.

    Raises:
        BackendUnavailable: if the assigned root is not an existing directory
            this worker can open.
    """
    root = Path(assigned)
    if not root.is_absolute():
        raise BackendUnavailable("the assigned staging root is not an absolute path")
    try:
        descriptor = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
    except OSError as error:
        raise BackendUnavailable(
            f"the assigned staging root cannot be opened: {_redacted_detail(error)}"
        ) from error
    return _StagingRoot(root, descriptor)


def _contained_output(staging: _StagingRoot, assigned: str) -> tuple[int, str]:
    """Walks from the staging root to the directory the assigned file goes in.

    Returns that directory as an open descriptor, with the file name to create
    inside it. **Containment is the walk, not a check performed after one.**
    Every component is opened relative to the descriptor before it, with
    `O_NOFOLLOW`, so nothing this returns can be outside the root: there is no
    pathname resolved and re-walked, and therefore no window in which a symlink
    planted at any level could redirect the write. A lexical check alone
    accepts a path whose every component is inside the root by name and whose
    parent is a symlink pointing out of it, which is the shape ADR-0001 10.3
    confines worker writes against.

    A symlinked component is refused for *being* a symlink rather than for
    where it points, so a link to a lawful directory inside the root is refused
    too. That is stricter than resolving and then asking where the answer
    landed, and deliberately: the supervisor composes every staging path itself
    and plants no links, so nothing lawful is turned away, while the decision
    no longer depends on what a name resolved to at one instant.

    The parent must already exist. The worker creates exactly the one file it
    was assigned and never a directory, so an absent parent is the supervisor's
    mistake rather than a path to be built.

    The caller closes the returned descriptor. It is always its own, never the
    session's, so closing it cannot shut the boundary for the next request.

    Residual limit, which no descriptor can close: a directory proven inside
    the root and then *moved* out of it carries this descriptor with it,
    because a descriptor follows the inode rather than the name. Closing that
    needs a filesystem sandbox rather than more care here, and whoever can move
    a directory out of the staging root can already write inside it.

    Raises:
        BackendUnavailable: if the assigned path does not name a file reachable
            from the staging root this worker was initialized with.
    """
    try:
        relative = Path(assigned).relative_to(staging.path)
    except ValueError:
        raise BackendUnavailable(
            "the assigned output path is not inside the staging root this worker was "
            "given, and this worker writes only inside it"
        ) from None
    parts = relative.parts
    if not parts:
        raise BackendUnavailable(
            "the assigned output path does not name a file this worker could create"
        )
    # `relative_to` is purely textual, so a path spelled through the root and
    # back out again keeps its `..` rather than being rejected as outside.
    if os.pardir in parts:
        raise BackendUnavailable(
            "the assigned output path climbs out of the staging root this worker was given"
        )

    # Duplicated so the caller owns exactly what it is handed, including when
    # the file goes directly in the root and no component is walked at all.
    directory = os.dup(staging.descriptor)
    try:
        for part in parts[:-1]:
            nested = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=directory
            )
            os.close(directory)
            directory = nested
    except OSError as error:
        os.close(directory)
        raise BackendUnavailable(
            f"the assigned output path has no directory this worker can write into: "
            f"{_redacted_detail(error)}"
        ) from error
    return directory, parts[-1]


def _create_contained_file(directory: int, name: str) -> int:
    """Creates `name` inside the directory `directory` holds, exactly once.

    `O_EXCL` and `O_NOFOLLOW`, and no `mkdir`: the worker writes exactly the
    one file it was assigned, never through a symlink planted at that path and
    never over something already there. Opened relative to the descriptor
    :func:`_contained_output` returned, so no part of the path is walked a
    second time and the containment decision still holds when the file appears.

    Durability is the parent's: it syncs and renames the staging transaction
    into place.

    Raises:
        BackendUnavailable: if the assigned file cannot be created exactly once.
    """
    try:
        return os.open(
            name,
            os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
            0o600,
            dir_fd=directory,
        )
    except OSError as error:
        raise BackendUnavailable(
            f"the assigned output path could not be created exactly once: {_redacted_detail(error)}"
        ) from error


def _render(
    backend: _Backend,
    launcher: dict[str, Any],
    parameters: dict[str, Any],
    staging: _StagingRoot,
) -> int:
    """Renders one request to the assigned path and returns the frames written.

    Raises:
        BackendUnavailable: if the request names something this worker cannot
            honor, if generation fails, or if the assigned path cannot be
            written exactly once.
    """
    voice = parameters["voice"]
    if voice not in backend.conditioning:
        raise BackendUnavailable(f"this worker holds no voice profile named {voice!r}")
    style = parameters["style"]
    if style not in _declared_styles(launcher):
        raise BackendUnavailable(
            f"this worker maps no generation parameters to the style {style!r}"
        )

    # Proven before anything is generated, not after. A path this worker may
    # not write is a refusal the supervisor should get without paying for a
    # render first, and a gate that runs after the work it guards cannot be
    # told from one that never ran. The directory is *held* across the
    # generation rather than re-derived after it, which is what keeps the
    # decision and the write about the same directory.
    directory, name = _contained_output(staging, parameters["output"])
    try:
        import numpy
        import soundfile
        import torch

        # Reset before every take, not once at load: generation advances the global
        # random state, so a second take under one seed would otherwise sample from
        # wherever the first one left off and the seed would describe only the first.
        random.seed(parameters["seed"])
        numpy.random.seed(parameters["seed"] % (2**32))
        torch.manual_seed(parameters["seed"])

        # Set per request, not once at load: each request names its own voice, and a
        # model left conditioned on the previous one would render a voice the plan
        # did not ask for -- under a key naming the voice it did.
        backend.model.conds = backend.conditionals[voice]

        try:
            generated = backend.model.generate(
                parameters["text"], **_generation_parameters(launcher)
            )
        except Exception as error:  # noqa: BLE001 - any backend fault is one refusal
            raise BackendUnavailable(f"synthesis failed: {_redacted_detail(error)}") from error

        samples = generated.squeeze().detach().cpu().numpy().astype(numpy.float32, copy=False)
        if samples.ndim != 1:
            raise BackendUnavailable("the backend returned audio that is not one channel")

        # ADR-0001 §10.3 confines worker writes to the assigned staging root, and
        # this is the half the worker itself can enforce.
        descriptor = _create_contained_file(directory, name)
        try:
            with os.fdopen(descriptor, "wb") as artifact:
                soundfile.write(
                    artifact,
                    samples,
                    backend.sample_rate,
                    format="WAV",
                    subtype="FLOAT",
                )
        except Exception as error:  # noqa: BLE001 - any write fault is one refusal
            raise BackendUnavailable(
                f"the assigned output could not be written: {_redacted_detail(error)}"
            ) from error

        return int(samples.shape[0])
    finally:
        os.close(directory)


def _generation_parameters(launcher: dict[str, Any]) -> dict[str, float]:
    """The launcher's generation parameters, parsed at the call.

    Recorded as strings and parsed here rather than stored as numbers: ADR-0001
    §12.5 admits no floating point into an identity, so the key hashes the exact
    spelling the launcher records and this is the one place it becomes a float.
    `string_map` in `protocol.py` refuses one written as a number.
    """
    return {name: float(value) for name, value in launcher["generation_parameters"].items()}


def _declared_styles(launcher: dict[str, Any]) -> tuple[str, ...]:
    """Styles this worker will accept.

    Chatterbox has no style axis, so this build honors exactly the one delivery
    the launcher parameterises and refuses every other by name. Declaring a
    style whose parameters are identical to another's would key two cache
    entries for byte-identical audio, which is what ADR-0001 §12.5 exists to
    prevent. `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` records the
    decision; widening it is a launcher change, not a code change.
    """
    _ = launcher
    return (CALM_EXPLANATORY_STYLE,)


def _capabilities(launcher: dict[str, Any], backend: _Backend | None) -> dict[str, Any]:
    """Reports the envelope this build can actually honor.

    ``voices`` is what this process has loaded, so it is empty until
    ``initialize`` has run and never a promise about a root nobody read.
    ADR-0001 §12.1 makes an unresolved voice a refusal rather than a default, so
    declaring one before it is loaded would be a claim that survives into a
    cache key.
    """
    return {
        "languages": ["en"],
        "max_text_bytes": 64 * 1024,
        "voices": sorted(backend.conditioning) if backend else [],
        "styles": list(_declared_styles(launcher)) if backend else [],
        "sample_rate": CANONICAL_SAMPLE_RATE_HZ,
        "channels": CANONICAL_CHANNELS,
        "sample_format": CANONICAL_SAMPLE_FORMAT,
        # Measured rather than assumed. E0-S3 rendered ten fixed-seed takes with
        # identical decoded samples and then declined to generalise past "this
        # environment and bounded run set"; ADR-0001 §12.5 says identical seeds
        # do not guarantee identical output across dependency, platform, or
        # execution changes. Claiming reproducibility here would put that claim
        # into every cache key.
        "deterministic_seed": False,
        "device": launcher["device"],
    }



def _respond(
    frame: dict[str, Any], launcher: dict[str, Any], session: _Session
) -> dict[str, Any]:
    """Produces the response frame for one accepted request."""
    request_id = frame["request_id"]
    method = frame["method"]

    if method == "initialize":
        # One load per lifetime is the property ADR-0001 §10.1 makes this
        # process persistent for, so a second `initialize` is refused rather
        # than served: a worker that reloaded would report identities its
        # earlier audio was not produced under.
        if session.backend is not None:
            return failure(
                request_id,
                "internal",
                "this worker has already loaded its model, and ADR-0001 §10.1 gives each "
                "worker one model load per lifetime",
                recoverable=False,
            )
        try:
            staging = _staging_root(frame["parameters"]["staging_root"])
            session.backend = _load_backend(launcher, frame["parameters"]["threads"])
        except BackendUnavailable as error:
            return failure(request_id, "initialization_failed", str(error), recoverable=False)
        session.worker_bundle_hash = frame["parameters"]["worker_bundle_hash"]
        session.staging = staging
        return {
            "event": "initialized",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "identities": {
                "model_revision": session.backend.model_revision,
                "tokenizer_revision": session.backend.codec_revision,
                # Echoed rather than computed: this worker cannot hash its own
                # bundle, and `WorkerConfiguration::for_bundle` derives the
                # identity with `WorkerBundle::verified_hash` before this
                # process is started at all.
                "worker_bundle_hash": frame["parameters"]["worker_bundle_hash"],
                "voice_conditioning_hashes": dict(session.backend.conditioning),
            },
        }
    if method == "capabilities":
        return {
            "event": "capabilities",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "capabilities": _capabilities(launcher, session.backend),
        }
    if method == "health":
        return {
            "event": "health",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "ready": session.backend is not None,
            "model_loaded": session.backend is not None,
        }
    if method == "synthesize":
        # Both, though `initialize` sets them together: the pair is what a
        # render needs, and a checker cannot see that one implies the other.
        if session.backend is None or session.staging is None:
            return failure(
                request_id,
                "initialization_failed",
                "this worker has no model loaded, and no audio may be published under a "
                "synthesis key claiming one produced it",
                recoverable=False,
            )
        parameters = frame["parameters"]
        try:
            frames = _render(session.backend, launcher, parameters, session.staging)
        except BackendUnavailable as error:
            return failure(request_id, "synthesis_failed", str(error), recoverable=True)
        return {
            "event": "synthesis_succeeded",
            "protocol_version": WORKER_PROTOCOL_VERSION,
            "request_id": request_id,
            "sample_rate": session.backend.sample_rate,
            "channels": CANONICAL_CHANNELS,
            "frames": frames,
            "model_revision": session.backend.model_revision,
            "codec_revision": session.backend.codec_revision,
            "worker_bundle_hash": session.worker_bundle_hash,
            # Read from this worker's own voice root at load, never echoed from
            # the request: that is what lets the parent refuse a worker that
            # rendered with a voice the plan did not ask for.
            "voice_conditioning_hash": session.backend.conditioning[parameters["voice"]],
            "voice_profile": parameters["voice"],
        }
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
    session = _Session()

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
        response = _respond(frame, launcher, session)
        write_frame(protocol, response)
        if response["event"] == "shutdown":
            break
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
