"""Behavioral tests for the worker process, run as a subprocess.

Outside `study_tts_worker/` for the reason `test_protocol.py` gives: a test
module beside the worker sources would either change the bundle identity or sit
undeclared next to declared files.

**A subprocess rather than a function call, and that is the whole point.** The
three properties here are properties of a *process*: which file descriptor
carries the protocol, what the environment holds by the time a backend would
import, and whether the thing is still alive to answer the next frame. Calling
`main` in-process would test none of them -- file descriptor 1 would be the test
runner's, `os.environ` would be the test runner's, and a process that died would
take the assertions with it.

    python3 -m unittest discover --start-directory worker/tests
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

WORKER_ROOT = Path(__file__).resolve().parent.parent

sys.path.insert(0, str(WORKER_ROOT))

from study_tts_worker import WORKER_PROTOCOL_VERSION  # noqa: E402
from study_tts_worker import worker as worker_module  # noqa: E402
from study_tts_worker.protocol import (  # noqa: E402
    MAX_REQUEST_ID_BYTES,
    MAX_WORKER_FRAME_BYTES,
)
from study_tts_worker.worker import (  # noqa: E402
    LAUNCHER_SCHEMA_VERSION,
    OPTIONAL_OFFLINE_ENVIRONMENT,
    REQUIRED_OFFLINE_ENVIRONMENT,
)

STRAY_WRITE_HARNESS = """
import os
import sys

from study_tts_worker.protocol import reserve_protocol_stream, write_frame

protocol = reserve_protocol_stream()

print("stray print")
sys.stdout.write("stray write\\n")
sys.stdout.flush()
os.write(1, b"stray descriptor write\\n")

write_frame(protocol, {"event": "shutdown", "request_id": "req-1"})
"""
"""Everything that could corrupt the frame stream, then one frame.

The third write is the one a convention cannot reach. `os.write(1, ...)` stands
in for a native library printing from inside a model load: it never touches
`sys.stdout`, so a rule that diagnostics use stderr does not apply to it. Only
taking the descriptor does.
"""


def request(method: str, request_id: str, **extra: object) -> str:
    """One well-formed request frame, as the line a sender would write."""
    frame: dict[str, object] = {
        "method": method,
        "protocol_version": WORKER_PROTOCOL_VERSION,
        "request_id": request_id,
    }
    frame.update(extra)
    return json.dumps(frame)


def run_worker(lines: list[str]) -> subprocess.CompletedProcess[str]:
    """Serves `lines` to a real worker process and collects both channels."""
    return subprocess.run(
        [sys.executable, "-m", "study_tts_worker.worker"],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        cwd=WORKER_ROOT,
        timeout=60,
        check=False,
    )


class CancellationBoundaryTests(unittest.TestCase):
    """Cancellation remains correlatable at its exact process boundary."""

    def test_the_shared_session_echoes_the_active_id_at_the_ceiling(self) -> None:
        session = (
            WORKER_ROOT.parent
            / "fixtures/contracts/e1-s1-fake-worker-session.ndjson"
        ).read_text(encoding="utf-8").splitlines()

        result = run_worker(session)

        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        frames = [json.loads(line) for line in lines]
        cancelled = next(frame for frame in frames if frame["event"] == "cancelled")
        active_request_id = cancelled["active_request_id"]
        self.assertEqual(len(active_request_id.encode("utf-8")), MAX_REQUEST_ID_BYTES)
        self.assertTrue(
            all(len(line.encode("utf-8")) <= MAX_WORKER_FRAME_BYTES for line in lines)
        )


class ProtocolStreamTests(unittest.TestCase):
    """stdout carries frames and nothing else, whoever writes to it."""

    def test_stray_writes_land_on_stderr_and_leave_the_frame_stream_clean(self) -> None:
        result = subprocess.run(
            [sys.executable, "-c", STRAY_WRITE_HARNESS],
            capture_output=True,
            text=True,
            cwd=WORKER_ROOT,
            timeout=60,
            check=True,
        )

        # Exactly the frame, with nothing before, after, or inside it. A reader
        # parses this channel line by line, so one stray byte is a protocol
        # failure it can neither attribute nor recover from.
        self.assertEqual(
            result.stdout,
            '{"event":"shutdown","request_id":"req-1"}\n',
        )
        for stray in ("stray print", "stray write", "stray descriptor write"):
            self.assertIn(stray, result.stderr)


class HostileFrameTests(unittest.TestCase):
    """A frame this worker refuses must not be a frame that ends it."""

    def test_the_worker_answers_the_frame_after_a_hostile_one(self) -> None:
        # Each hostile line is inside the byte ceiling and syntactically valid
        # JSON. What refuses them is what the parser does with them, which is
        # why they used to travel out past the refusal path and kill the
        # process instead of drawing a failure frame.
        huge_number = (
            '{"method":"capabilities","protocol_version":'
            f'"{WORKER_PROTOCOL_VERSION}","request_id":"req-2","n":' + "9" * 5000 + "}"
        )
        deeply_nested = (
            '{"method":"capabilities","protocol_version":'
            f'"{WORKER_PROTOCOL_VERSION}","request_id":"req-3","n":'
            + "[" * 100_000
            + "]" * 100_000
            + "}"
        )

        result = run_worker(
            [
                request("capabilities", "req-1"),
                huge_number,
                deeply_nested,
                request("capabilities", "req-4"),
                request("shutdown", "req-5"),
            ]
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(
            [frame["event"] for frame in frames],
            ["capabilities", "failure", "failure", "capabilities", "shutdown"],
            result.stderr,
        )
        # The refusals are data the sender can act on, correlated to what it
        # sent -- not a stream that simply stopped.
        for frame in frames[1:3]:
            self.assertEqual(frame["code"], "invalid_request")
        self.assertEqual(frames[3]["request_id"], "req-4")


class RedactedRefusalTests(unittest.TestCase):
    """Refusal frames identify violated invariants without echoing input."""

    def test_sender_controlled_lesson_text_and_voice_path_are_not_published(self) -> None:
        lesson_text = "PRIVATE LESSON SENTINEL: cache invalidation notes"
        voice_path = "/private/voices/owner/reference.wav"

        result = run_worker(
            [
                request(voice_path, "req-1"),
                request("capabilities", "req-2", protocol_version=lesson_text),
                request("capabilities", "req-3", **{lesson_text: True}),
                request("shutdown", "req-4"),
            ]
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(
            [frame["event"] for frame in frames],
            ["failure", "failure", "failure", "shutdown"],
        )
        self.assertEqual(frames[0]["message"], "frame method is unsupported")
        self.assertEqual(
            frames[1]["message"],
            "frame protocol version is unsupported",
        )
        self.assertEqual(frames[2]["message"], "`frame` carries an unknown field")
        for sentinel in (lesson_text, voice_path):
            self.assertNotIn(sentinel, result.stdout)
            self.assertNotIn(sentinel, result.stderr)


class HealthTests(unittest.TestCase):
    """Health reports this build's actual readiness and model residency."""

    def test_health_reports_that_the_e1_s1_worker_has_no_loaded_backend(self) -> None:
        result = run_worker(
            [request("health", "req-1"), request("shutdown", "req-2")]
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(
            frames[0],
            {
                "event": "health",
                "protocol_version": WORKER_PROTOCOL_VERSION,
                "request_id": "req-1",
                "ready": False,
                "model_loaded": False,
            },
        )


class InitializationTests(unittest.TestCase):
    """Initialization fails closed until the Chatterbox backend exists."""

    def test_initialize_fails_nonrecoverably_and_health_stays_unready(self) -> None:
        result = run_worker(
            [
                request(
                    "initialize",
                    "req-1",
                    parameters={"worker_bundle_hash": "a" * 64, "threads": 1},
                ),
                request("health", "req-2"),
                request("shutdown", "req-3"),
            ]
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual([frame["event"] for frame in frames], ["failure", "health", "shutdown"])
        self.assertEqual(frames[0]["code"], "initialization_failed")
        self.assertFalse(frames[0]["recoverable"])
        self.assertFalse(frames[1]["ready"])
        self.assertFalse(frames[1]["model_loaded"])


class OfflineEnvironmentTests(unittest.TestCase):
    """The launcher's offline settings reach the process, not just the file."""

    def test_the_offline_environment_is_applied_before_frames_are_served(self) -> None:
        # Reported on stderr by the worker itself, because the environment of
        # another process cannot be read from here. The line is written before
        # the first frame is read, which is the ordering a backend import would
        # depend on.
        result = run_worker([request("shutdown", "req-1")])

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("offline environment applied", result.stderr)
        for variable in REQUIRED_OFFLINE_ENVIRONMENT:
            self.assertIn(variable, result.stderr)

    def test_a_launcher_that_permits_fetching_stops_the_worker(self) -> None:
        # Read from the module rather than restated, so a launcher edited to
        # drop an offline variable fails here instead of shipping.
        from study_tts_worker.worker import _apply_offline_environment

        offline = {name: "1" for name in REQUIRED_OFFLINE_ENVIRONMENT}
        cases = [
            ({"local_files_only": False, "offline_environment": offline}, "local_files_only"),
            (
                {
                    "local_files_only": True,
                    "offline_environment": {**offline, REQUIRED_OFFLINE_ENVIRONMENT[0]: "0"},
                },
                REQUIRED_OFFLINE_ENVIRONMENT[0],
            ),
            (
                {
                    "local_files_only": True,
                    "offline_environment": {
                        name: "1" for name in REQUIRED_OFFLINE_ENVIRONMENT[1:]
                    },
                },
                REQUIRED_OFFLINE_ENVIRONMENT[0],
            ),
        ]

        for launcher, named in cases:
            with self.subTest(named=named):
                with self.assertRaises(SystemExit) as refused:
                    _apply_offline_environment(launcher)
                self.assertIn(named, str(refused.exception))


class LauncherShapeTests(unittest.TestCase):
    """`worker/launcher.json` is read closed, because its entries become an
    environment."""

    def launcher(self, **overrides: object) -> dict[str, object]:
        """The checked-in launcher's shape, before a test spoils one field."""
        launcher: dict[str, object] = {
            "schema_version": LAUNCHER_SCHEMA_VERSION,
            "device": "cpu",
            "threads": 4,
            "offline_environment": {
                name: "1"
                for name in (*REQUIRED_OFFLINE_ENVIRONMENT, *OPTIONAL_OFFLINE_ENVIRONMENT)
            },
            "local_files_only": True,
            "model_root_environment_variable": "STUDY_TTS_MODEL_ROOT",
        }
        launcher.update(overrides)
        return launcher

    def load(self, launcher: object) -> dict[str, object]:
        """Runs `_load_launcher` against `launcher` written to a scratch file.

        The real `LAUNCHER_CONFIG` is a fixed path beside the package and is a
        declared bundle input, so a test that edited it would change the
        worker-bundle hash for the duration of a test run.
        """
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "launcher.json"
            path.write_text(json.dumps(launcher), encoding="utf-8")
            with unittest.mock.patch.object(worker_module, "LAUNCHER_CONFIG", path):
                return worker_module._load_launcher()

    def test_the_checked_in_launcher_is_the_shape_this_build_reads(self) -> None:
        # Read from the file rather than restated here, so a launcher edited to
        # carry a field this build does not describe fails in this suite rather
        # than at a qualification run.
        self.assertEqual(
            worker_module._load_launcher()["schema_version"], LAUNCHER_SCHEMA_VERSION
        )

    def test_a_launcher_field_this_build_does_not_describe_is_refused(self) -> None:
        for launcher, named in [
            (
                self.launcher(unexpected="value"),
                "`launcher` carries an unknown field",
            ),
            (self.launcher(schema_version="9.9"), "declares layout '9.9'"),
            (self.launcher(local_files_only="yes"), "local_files_only"),
            (self.launcher(threads=True), "threads"),
            (
                self.launcher(
                    offline_environment={
                        **{name: "1" for name in REQUIRED_OFFLINE_ENVIRONMENT},
                        "PYTHONPATH": "/tmp/injected",
                    }
                ),
                "launcher.offline_environment",
            ),
        ]:
            with self.subTest(named=named):
                with self.assertRaises(SystemExit) as refused:
                    self.load(launcher)
                self.assertIn(named, str(refused.exception))

    def test_a_future_launcher_is_refused_by_version_before_its_fields(self) -> None:
        launcher = self.launcher(schema_version="9.9", added_by_nine=True)

        with self.assertRaises(SystemExit) as refused:
            self.load(launcher)

        self.assertIn("declares layout '9.9'", str(refused.exception))

    def test_an_unreadable_launcher_stops_as_a_startup_error(self) -> None:
        for content in [None, "{"]:
            with self.subTest(content=content):
                with tempfile.TemporaryDirectory() as directory:
                    path = Path(directory) / "launcher.json"
                    if content is not None:
                        path.write_text(content, encoding="utf-8")
                    with unittest.mock.patch.object(
                        worker_module, "LAUNCHER_CONFIG", path
                    ):
                        with self.assertRaises(SystemExit):
                            worker_module._load_launcher()

    def test_only_the_named_offline_variables_reach_the_environment(self) -> None:
        # The mutation check for the allowlist: with the loop back over the
        # launcher's own entries, `PYTHONPATH` lands in `os.environ` here --
        # and a speech backend is imported from it one statement later.
        launcher = self.launcher(
            offline_environment={
                **{name: "1" for name in REQUIRED_OFFLINE_ENVIRONMENT},
                "PYTHONPATH": "/tmp/injected",
            }
        )
        before = os.environ.copy()
        try:
            # The applied-variables line goes to stderr by design; captured so
            # one in-process call does not print into the suite's own output.
            with contextlib.redirect_stderr(io.StringIO()):
                worker_module._apply_offline_environment(launcher)
            self.assertNotIn("PYTHONPATH", set(os.environ) - set(before))
            for name in REQUIRED_OFFLINE_ENVIRONMENT:
                self.assertEqual(os.environ[name], "1")
        finally:
            os.environ.clear()
            os.environ.update(before)


if __name__ == "__main__":
    unittest.main()
