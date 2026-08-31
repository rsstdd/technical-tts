"""Frame-boundary tests for the worker's protocol reader.

Outside `study_tts_worker/` on purpose. ADR-0001 §12.5 makes every file the
bundle manifest declares a synthesis-key input, so a test module beside the
worker sources would either change the bundle identity or have to be left
undeclared — and `worker/AGENTS.md` treats an undeclared file beside a declared
one as the mistake the manifest exists to prevent.

`unittest` rather than `pytest` for the same reason: `worker/requirements.lock`
is a declared bundle input, so adding a test dependency to it would invalidate
every cache entry in the project to run a test.

    python3 -m unittest discover --start-directory worker/tests
"""

from __future__ import annotations

import io
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from study_tts_worker import (  # noqa: E402
    WORKER_PROTOCOL_EXTENSION_VERSION,
    WORKER_PROTOCOL_VERSION,
)
from study_tts_worker.protocol import (  # noqa: E402
    MAX_REQUEST_ID_BYTES,
    MAX_WORKER_FRAME_BYTES,
    UNSIGNED_32_MAXIMUM,
    UNSIGNED_64_MAXIMUM,
    FrameError,
    failure,
    read_line,
    read_request,
    write_frame,
)


CONTRACT_CASES = [
    json.loads(line)
    for line in (
        Path(__file__).resolve().parents[2]
        / "fixtures/contracts/e1-s1-worker-protocol-cases.ndjson"
    )
    .read_text(encoding="utf-8")
    .splitlines()
]
"""Every frame both ends of the protocol must accept or refuse alike.

Committed rather than built in either suite. A rule only one end enforces is a
rule the other end can send past, and two suites that each wrote their own cases
would agree only by coincidence.
"""


def initialize_frame(**overrides: object) -> dict[str, object]:
    """A well-formed `initialize` request, before a test spoils one field."""
    frame: dict[str, object] = {
        "method": "initialize",
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": "req-1",
        "parameters": {
            "worker_bundle_hash": "a" * 64,
            "threads": 4,
            "staging_root": "/staging",
        },
    }
    frame.update(overrides)
    return frame


def encode(frame: object) -> bytes:
    return json.dumps(frame).encode("utf-8")


def synthesize(version: str, trace_context: object = ..., **overrides: object) -> bytes:
    """A well-formed `synthesize` request at `version`, before a test spoils it.

    `trace_context` is left out entirely unless a test names one, because
    whether the field is *present* is itself a rule rather than a detail of its
    value.
    """
    parameters: dict[str, object] = {
        "text": "one",
        "voice": "nadia",
        "style": "explanation",
        "seed": 1,
        "take": 0,
        "output": "segments/seg-1.wav",
    }
    parameters.update(overrides)
    if trace_context is not ...:
        parameters["trace_context"] = trace_context
    return encode(
        {
            "method": "synthesize",
            "protocol_version": version,
            "request_id": "req-2",
            "parameters": parameters,
        }
    )


class ParameterShapeTests(unittest.TestCase):
    """Each method's complete parameter shape is checked before dispatch."""

    def test_a_well_formed_request_is_accepted(self) -> None:
        self.assertEqual(read_request(encode(initialize_frame()))["request_id"], "req-1")

    def test_an_unknown_nested_field_is_refused(self) -> None:
        # The failure this suite exists for: `worker.py` reads
        # `parameters["worker_bundle_hash"]`, so a parameters object nobody
        # checked is a KeyError dressed up as a protocol frame.
        frame = initialize_frame(
            parameters={
                "worker_bundle_hash": "a" * 64,
                "threads": 4,
                "staging_root": "/staging",
                "extra": 1,
            }
        )

        with self.assertRaises(FrameError) as refused:
            read_request(encode(frame))

        self.assertIn("frame.parameters", str(refused.exception))
        self.assertIn("unknown field", str(refused.exception))
        self.assertNotIn("extra", str(refused.exception))
        self.assertEqual(refused.exception.request_id, "req-1")

    def test_a_missing_nested_field_is_refused_by_name(self) -> None:
        frame = initialize_frame(parameters={"threads": 4})

        with self.assertRaises(FrameError) as refused:
            read_request(encode(frame))

        self.assertIn("worker_bundle_hash", str(refused.exception))

    def test_a_nested_field_of_the_wrong_type_is_refused(self) -> None:
        for parameters, expected in [
            ({"worker_bundle_hash": 1, "threads": 4, "staging_root": "/s"}, "worker_bundle_hash"),
            ({"worker_bundle_hash": "a" * 64, "threads": -1, "staging_root": "/s"}, "threads"),
            # `bool` is an `int` in Python, and `true` is not a thread count.
            ({"worker_bundle_hash": "a" * 64, "threads": True, "staging_root": "/s"}, "threads"),
            ({"worker_bundle_hash": "a" * 64, "threads": "4", "staging_root": "/s"}, "threads"),
        ]:
            with self.subTest(parameters=parameters):
                with self.assertRaises(FrameError) as refused:
                    read_request(encode(initialize_frame(parameters=parameters)))

                self.assertIn(expected, str(refused.exception))

    def test_an_identity_that_is_not_a_digest_is_refused(self) -> None:
        # The Rust end parses `worker_bundle_hash` into a value object, so a
        # spelling accepted here and refused there is a frame this worker would
        # answer and its counterpart would drop. Each case below is one way a
        # 64-character string can still not be a digest.
        for spelling, why in [
            ("a" * 63, "one character short"),
            ("a" * 65, "one character long"),
            ("A" * 64, "uppercase"),
            ("g" * 64, "outside the hexadecimal alphabet"),
            ("a_" * 32, "underscore separators, which int(value, 16) accepts"),
            (" " + "a" * 63, "leading whitespace, which int(value, 16) strips"),
        ]:
            with self.subTest(why=why):
                frame = initialize_frame(
                    parameters={
                        "worker_bundle_hash": spelling,
                        "threads": 4,
                        "staging_root": "/staging",
                    }
                )

                with self.assertRaises(FrameError) as refused:
                    read_request(encode(frame))

                self.assertIn("worker_bundle_hash", str(refused.exception))

    def test_parameters_that_are_not_an_object_are_refused(self) -> None:
        with self.assertRaises(FrameError):
            read_request(encode(initialize_frame(parameters=[])))

    def test_the_optional_trace_context_is_checked_when_present(self) -> None:
        # Declared under the version that introduced it: the check below is that
        # an *optional* field is optional and not unvalidated, which is a
        # different rule from the one deciding whether it may appear at all.
        read_request(synthesize(WORKER_PROTOCOL_EXTENSION_VERSION, None))
        read_request(synthesize(WORKER_PROTOCOL_EXTENSION_VERSION, {"trace_id": "t"}))
        for malformed in [{"trace_id": 1}, {"trace_id": "t", "extra": 1}, {}]:
            with self.subTest(trace_context=malformed):
                with self.assertRaises(FrameError):
                    read_request(
                        synthesize(WORKER_PROTOCOL_EXTENSION_VERSION, malformed)
                    )

    def test_the_trace_extension_requires_the_version_that_introduced_it(self) -> None:
        # `null` is the case worth naming. It is a sender saying it knows about
        # the extension, and this end used to accept it under the baseline
        # version while `parse_worker_request` refused it -- so the two ends
        # disagreed about a frame that looked well formed to both.
        for trace_context in [None, {"trace_id": "trace-1"}]:
            with self.subTest(trace_context=trace_context):
                with self.assertRaises(FrameError) as refused:
                    read_request(synthesize(WORKER_PROTOCOL_VERSION, trace_context))

                self.assertIn(WORKER_PROTOCOL_EXTENSION_VERSION, str(refused.exception))
                self.assertEqual(refused.exception.request_id, "req-2")

    def test_a_version_the_rust_end_accepts_is_accepted_here(self) -> None:
        # A frame refused only by this end is a request the supervisor sends and
        # never gets an answer to, which it reports as a timeout rather than as
        # the refusal it never received.
        for version in [WORKER_PROTOCOL_VERSION, WORKER_PROTOCOL_EXTENSION_VERSION]:
            with self.subTest(version=version):
                frame = initialize_frame(protocol_version=version)

                self.assertEqual(read_request(encode(frame))["request_id"], "req-1")

    def test_an_integer_wider_than_its_field_is_refused(self) -> None:
        # Python integers have no width and the Rust fields do, so a frame
        # answered here was dropped there. The ceilings are the widths those
        # fields are read at, and the published schema carries the same ones.
        read_request(synthesize(WORKER_PROTOCOL_VERSION, seed=UNSIGNED_64_MAXIMUM))
        read_request(synthesize(WORKER_PROTOCOL_VERSION, take=UNSIGNED_32_MAXIMUM))
        for field, value in [
            ("seed", UNSIGNED_64_MAXIMUM + 1),
            ("take", UNSIGNED_32_MAXIMUM + 1),
        ]:
            with self.subTest(field=field):
                with self.assertRaises(FrameError) as refused:
                    read_request(synthesize(WORKER_PROTOCOL_VERSION, **{field: value}))

                self.assertIn(field, str(refused.exception))

        with self.assertRaises(FrameError) as refused:
            read_request(
                encode(
                    initialize_frame(
                        parameters={
                            "worker_bundle_hash": "a" * 64,
                            "threads": UNSIGNED_32_MAXIMUM + 1,
                            "staging_root": "/staging",
                        }
                    )
                )
            )
        self.assertIn("threads", str(refused.exception))

    def test_a_frame_missing_its_parameters_is_refused(self) -> None:
        frame = initialize_frame()
        del frame["parameters"]

        with self.assertRaises(FrameError) as refused:
            read_request(encode(frame))

        self.assertIn("parameters", str(refused.exception))


class EnvelopeIdentityTests(unittest.TestCase):
    """The envelope rules both ends apply before a method's parameters."""

    def test_an_empty_request_id_is_refused(self) -> None:
        # A refusal the supervisor cannot correlate is one it reports as a
        # timeout against whatever it was waiting for.
        with self.assertRaises(FrameError) as refused:
            read_request(encode(initialize_frame(request_id="")))

        self.assertIn("request_id", str(refused.exception))

    def test_a_name_that_appears_twice_is_refused(self) -> None:
        # `json` keeps the last value and `serde_json` refuses the frame, so a
        # sender could choose which end read which value. Built as text because
        # a duplicate name cannot survive being expressed as a Python dict.
        for duplicated in [
            '{"method":"capabilities","method":"shutdown",'
            f'"protocol_version":"{WORKER_PROTOCOL_VERSION}","request_id":"req-1"}}',
            '{"method":"capabilities",'
            f'"protocol_version":"{WORKER_PROTOCOL_VERSION}",'
            '"request_id":"req-1","request_id":"req-2"}',
        ]:
            with self.subTest(frame=duplicated):
                with self.assertRaises(FrameError) as refused:
                    read_request(duplicated.encode("utf-8"))

                self.assertIn("twice", str(refused.exception))


class SharedContractCaseTests(unittest.TestCase):
    """The committed cases both ends of the protocol must decide the same way."""

    def test_every_committed_case_is_decided_as_the_fixture_says(self) -> None:
        # The file rather than a list written here, because the Rust end reads
        # the same one in `t3_e1_both_protocol_ends_decide_the_committed_cases_alike`.
        # A disagreement between two suites that each own their cases is a
        # disagreement neither suite can see.
        for case in CONTRACT_CASES:
            with self.subTest(case=case["case"], why=case["why"]):
                frame = case["frame"].encode("utf-8")
                if case["accepted"]:
                    read_request(frame)
                    continue
                with self.assertRaises(FrameError):
                    read_request(frame)


class FrameCeilingTests(unittest.TestCase):
    """The ceiling bounds what this process allocates, not only what it accepts."""

    def test_a_frame_at_the_ceiling_is_read_whole(self) -> None:
        line = b"x" * MAX_WORKER_FRAME_BYTES

        self.assertEqual(read_line(io.BytesIO(line + b"\n")), line)

    def test_an_unterminated_frame_past_the_ceiling_is_refused(self) -> None:
        # No newline anywhere: iterating `sys.stdin` would buffer all of it
        # before the ceiling could be consulted.
        stream = io.BytesIO(b"x" * (MAX_WORKER_FRAME_BYTES * 4))

        with self.assertRaises(FrameError) as refused:
            read_line(stream)

        self.assertIn(str(MAX_WORKER_FRAME_BYTES), str(refused.exception))

    def test_the_frame_after_an_oversized_one_is_still_read(self) -> None:
        stream = io.BytesIO(b"x" * (MAX_WORKER_FRAME_BYTES + 10) + b"\n" + b"next\n")

        with self.assertRaises(FrameError):
            read_line(stream)

        self.assertEqual(read_line(stream), b"next")

    def test_end_of_input_reports_no_frame(self) -> None:
        self.assertIsNone(read_line(io.BytesIO(b"")))

    def test_a_final_frame_without_a_terminator_is_read(self) -> None:
        self.assertEqual(read_line(io.BytesIO(b"last")), b"last")

    def test_read_request_refuses_bytes_past_the_ceiling(self) -> None:
        with self.assertRaises(FrameError):
            read_request(b"x" * (MAX_WORKER_FRAME_BYTES + 1))

    def test_an_oversized_internal_diagnostic_stays_under_the_ceiling(self) -> None:
        # Parser refusals never echo input, but backend and process failures also
        # use this builder. The shared bound keeps those diagnostics readable by
        # the supervisor even if their internal prose grows unexpectedly.
        oversized = "internal diagnostic " * MAX_WORKER_FRAME_BYTES
        written = io.StringIO()

        write_frame(
            written,
            failure("req-1", "invalid_request", oversized, False),
        )

        self.assertLess(len(written.getvalue().encode("utf-8")), MAX_WORKER_FRAME_BYTES)


class RequestIdentityCeilingTests(unittest.TestCase):
    """An identity is correlated exactly or refused, never quietly rewritten."""

    def test_an_identity_at_the_ceiling_is_answered_byte_for_byte(self) -> None:
        # The whole purpose of the field. A response carrying a *different*
        # identity is worse than no response: the supervisor matches it to
        # nothing while believing the request was answered.
        identity = "r" * MAX_REQUEST_ID_BYTES

        answered = failure(identity, "invalid_request", "refused", False)

        self.assertEqual(answered["request_id"], identity)

    def test_an_identity_past_the_ceiling_is_refused_at_validation(self) -> None:
        oversized = "r" * (MAX_REQUEST_ID_BYTES + 1)
        frame = json.dumps(
            {
                "method": "capabilities",
                "protocol_version": WORKER_PROTOCOL_VERSION,
                "request_id": oversized,
            }
        ).encode("utf-8")

        with self.assertRaises(FrameError) as refused:
            read_request(frame)

        self.assertIn(str(MAX_REQUEST_ID_BYTES), str(refused.exception))

    def test_an_identity_past_the_ceiling_is_not_echoed_back_shortened(self) -> None:
        # `read_request` recovers the identity before the shape checks so an
        # early refusal can still be correlated. One it cannot carry back is
        # dropped there rather than shortened into a different request's.
        oversized = "r" * (MAX_REQUEST_ID_BYTES + 1)
        frame = json.dumps(
            {
                "method": "no-such-method",
                "protocol_version": WORKER_PROTOCOL_VERSION,
                "request_id": oversized,
            }
        ).encode("utf-8")

        with self.assertRaises(FrameError) as refused:
            read_request(frame)

        self.assertIsNone(refused.exception.request_id)


class EnvelopeTests(unittest.TestCase):
    """The checks that run before a method's parameters are looked at."""

    def test_an_unhashable_method_is_refused_with_its_request_id(self) -> None:
        for method in [[], {}]:
            with self.subTest(method=method):
                with self.assertRaises(FrameError) as refused:
                    read_request(encode(initialize_frame(method=method)))

                self.assertIn("frame.method", str(refused.exception))
                self.assertEqual(refused.exception.request_id, "req-1")

    def test_an_unhashable_protocol_version_is_refused_with_its_request_id(self) -> None:
        for version in [[], {}]:
            with self.subTest(protocol_version=version):
                with self.assertRaises(FrameError) as refused:
                    read_request(encode(initialize_frame(protocol_version=version)))

                self.assertIn("frame.protocol_version", str(refused.exception))
                self.assertEqual(refused.exception.request_id, "req-1")

    def test_an_unknown_method_is_refused(self) -> None:
        with self.assertRaisesRegex(FrameError, "frame method is unsupported"):
            read_request(encode(initialize_frame(method="teleport")))

    def test_a_protocol_version_neither_end_speaks_is_refused(self) -> None:
        with self.assertRaisesRegex(FrameError, "frame protocol version is unsupported"):
            read_request(encode(initialize_frame(protocol_version="e1.worker.9.9")))

    def test_bytes_that_are_not_utf8_are_refused_as_such(self) -> None:
        with self.assertRaises(FrameError) as refused:
            read_request(b"\xff\xfe")

        self.assertIn("UTF-8", str(refused.exception))

    def test_a_json_document_that_is_not_an_object_is_refused(self) -> None:
        with self.assertRaises(FrameError):
            read_request(b"[]")

    def test_a_frame_cannot_carry_a_carriage_return(self) -> None:
        frame = encode(
            {
                "method": "capabilities",
                "protocol_version": WORKER_PROTOCOL_VERSION,
                "request_id": "req-1",
            }
        )

        with self.assertRaises(FrameError):
            read_request(frame + b"\r")


if __name__ == "__main__":
    unittest.main()
