"""Persistent NDJSON speech worker for technical-tts.

The package deliberately exports nothing. ADR-0001 §10.4 makes the worker a
replaceable process behind a frame protocol, not a library Rust links against,
so a public Python surface here would be a second boundary nobody governs.
"""

WORKER_PROTOCOL_VERSION = "e0.worker.0.1"
"""Baseline protocol version this worker speaks and answers in.

Mirrors ``study_tts_runtime::WORKER_PROTOCOL_VERSION`` and the row for
``worker_frames`` in ``docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md``,
which names this constant in return. Two spellings of a wire version are two
versions.
"""

WORKER_PROTOCOL_EXTENSION_VERSION = "e0.worker.0.2"
"""Version a frame must declare to carry the optional trace extension.

Mirrors ``study_tts_runtime::WORKER_PROTOCOL_EXTENSION_VERSION``. Accepted on a
request and never written on a response: this build adds no trace correlation
of its own, so answering in ``0.2`` would claim an extension it does not use.
A version the Rust end accepts and this one refuses is a frame the supervisor
sends and never gets an answer to, which it can only report as a timeout.
"""
