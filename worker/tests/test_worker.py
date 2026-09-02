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
import types
import unittest
import unittest.mock
from pathlib import Path

WORKER_ROOT = Path(__file__).resolve().parent.parent

sys.path.insert(0, str(WORKER_ROOT))

try:  # noqa: SIM105 - the failure is the signal, not an error to suppress
    import soundfile
    import torch

    RESTORED_ENVIRONMENT = True
except ImportError:
    RESTORED_ENVIRONMENT = False
"""Whether this interpreter is the restored worker environment.

The suite is otherwise standard-library only, which `.github/workflows/ci.yml`
relies on to run it with the system interpreter and no installation step. One
class below needs the real numerical libraries and skips without them, so that
property is unchanged: nothing here ever *fails* for a missing import.

Run it where it can run, per `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §1:

    worker/.venv/bin/python -m unittest discover --start-directory worker/tests
"""

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


def run_worker(
    lines: list[str], environment: dict[str, str] | None = None
) -> subprocess.CompletedProcess[str]:
    """Serves `lines` to a real worker process and collects both channels.

    `environment` adds to this process's own rather than replacing it, so a case
    sets only the variable it is about and inherits the interpreter's.
    """
    return subprocess.run(
        [sys.executable, "-m", "study_tts_worker.worker"],
        input="\n".join(lines) + "\n",
        capture_output=True,
        text=True,
        cwd=WORKER_ROOT,
        env={**os.environ, **(environment or {})},
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

    def test_health_reports_no_loaded_backend_before_initialize(self) -> None:
        result = run_worker([request("health", "req-1"), request("shutdown", "req-2")])

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


class BackendRefusalTests(unittest.TestCase):
    """Every refusal the worker can make without importing a backend.

    These are the cases a hosted runner can prove: the configuration checks all
    precede the Torch and Chatterbox imports, so they hold where neither is
    installed. Loading a real model is a T5 qualification criterion on the
    reference machine, not a test here.
    """

    def launcher(self) -> dict[str, object]:
        return json.loads((WORKER_ROOT / "launcher.json").read_text(encoding="utf-8"))

    def test_initialize_without_a_governed_root_refuses_by_naming_the_variable(self) -> None:
        launcher = self.launcher()
        variable = launcher["model_root_environment_variable"]

        with tempfile.TemporaryDirectory() as staging:
            result = run_worker(
                [
                    request(
                        "initialize",
                        "req-1",
                        parameters={
                            "worker_bundle_hash": "a" * 64,
                            "threads": 1,
                            "staging_root": staging,
                        },
                    ),
                    request("shutdown", "req-2"),
                ],
                environment={variable: "", launcher["voice_root_environment_variable"]: ""},
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            frames = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual(frames[0]["event"], "failure")
            self.assertEqual(frames[0]["code"], "initialization_failed")
            self.assertFalse(frames[0]["recoverable"])
            # The operator has to be told which variable to set. A refusal that
            # only said "no model" would send them to the model rather than the
            # launch.
            self.assertIn(variable, frames[0]["message"])

    def test_initialize_refuses_a_model_root_holding_another_repository(self) -> None:
        # The likelier misconfiguration than a missing root: a real, readable
        # governed root for a different model. Its audio would otherwise be
        # published under a key naming the repository this bundle was built for.
        launcher = self.launcher()
        with tempfile.TemporaryDirectory() as directory:
            model_root = Path(directory) / "models"
            model_root.mkdir()
            (model_root / "bundle-manifest.json").write_text(
                json.dumps(
                    {
                        "model": {"repository": "someone-else/other-model", "revision": "abc"},
                        "code": {"commit": "def"},
                    }
                ),
                encoding="utf-8",
            )
            voice_root = Path(directory) / "voices"
            voice_root.mkdir()
            staging = Path(directory) / "staging"
            staging.mkdir()

            result = run_worker(
                [
                    request(
                        "initialize",
                        "req-1",
                        parameters={
                            "worker_bundle_hash": "a" * 64,
                            "threads": 1,
                            "staging_root": str(staging),
                        },
                    ),
                    request("shutdown", "req-2"),
                ],
                environment={
                    launcher["model_root_environment_variable"]: str(model_root),
                    launcher["voice_root_environment_variable"]: str(voice_root),
                },
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(frames[0]["code"], "initialization_failed")
        self.assertIn(str(launcher["model_repository"]), frames[0]["message"])

    def test_synthesize_before_initialize_publishes_no_audio(self) -> None:
        # The refusal that matters most: a success frame here would have the
        # cache publish whatever is at the assigned path under a key claiming a
        # model produced it.
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "take.wav"
            result = run_worker(
                [
                    request(
                        "synthesize",
                        "req-1",
                        parameters={
                            "text": "one",
                            "voice": "owner-fallback-v1",
                            "style": "calm_explanatory",
                            "seed": 42,
                            "take": 0,
                            "output": str(output),
                        },
                    ),
                    request("shutdown", "req-2"),
                ],
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            frames = [json.loads(line) for line in result.stdout.splitlines()]
            self.assertEqual(frames[0]["event"], "failure")
            self.assertEqual(frames[0]["code"], "initialization_failed")
            self.assertFalse(output.exists(), "nothing may be written without a loaded model")

    def test_capabilities_declares_no_voice_until_one_is_loaded(self) -> None:
        # ADR-0001 §12.1 makes an unresolved voice a refusal rather than a
        # default, so a voice declared before it is loaded would be a claim that
        # survives into a cache key.
        result = run_worker([request("capabilities", "req-1"), request("shutdown", "req-2")])

        self.assertEqual(result.returncode, 0, result.stderr)
        frames = [json.loads(line) for line in result.stdout.splitlines()]
        self.assertEqual(frames[0]["capabilities"]["voices"], [])
        self.assertEqual(frames[0]["capabilities"]["styles"], [])
        # `True` since E1-S5, and measured rather than declared: see the
        # capability's own comment in `worker.py`. Asserted here so the
        # declaration cannot drift without a test saying so.
        self.assertTrue(frames[0]["capabilities"]["deterministic_seed"])


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
            "seed": 42,
            "model_repository": "ResembleAI/chatterbox",
            "generation_parameters": {"cfg_weight": "0.5"},
            "offline_environment": {
                name: "1"
                for name in (*REQUIRED_OFFLINE_ENVIRONMENT, *OPTIONAL_OFFLINE_ENVIRONMENT)
            },
            "local_files_only": True,
            "model_root_environment_variable": "STUDY_TTS_MODEL_ROOT",
            "voice_root_environment_variable": "STUDY_TTS_VOICE_ROOT",
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

    def test_a_generation_parameter_written_as_a_number_is_refused(self) -> None:
        # ADR-0001 §12.5 admits no floating point into an identity, so the Rust
        # end keys these as the text the launcher records. A launcher that wrote
        # `0.5` as a number would be keyed by one end as a string it never saw
        # and used by this one as a float, which is two builds disagreeing about
        # what produced the audio a key names.
        for spoiled in ({"cfg_weight": 0.5}, {"cfg_weight": None}, {"cfg_weight": ["0.5"]}):
            with self.subTest(spoiled=spoiled):
                with self.assertRaises(SystemExit) as refused:
                    self.load(self.launcher(generation_parameters=spoiled))
                self.assertIn("generation_parameters.cfg_weight", str(refused.exception))

        # The shape admits any parameter name, because which parameters a
        # backend takes is not this build's to fix.
        self.assertEqual(
            self.load(self.launcher(generation_parameters={"a": "1", "b": "2"}))[
                "generation_parameters"
            ],
            {"a": "1", "b": "2"},
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



class SeedOrdering(unittest.TestCase):
    """Every generator is seeded before the model is constructed.

    The property is an *ordering*, and nothing about the code's shape proves
    one: seeding after `ChatterboxTTS.from_local` would look almost identical
    and read the same launcher. What it would produce is a vocoder whose noise
    was drawn once per process from an unseeded generator, so two workers given
    the same seed would render the same request differently -- a difference no
    ADR-0001 §12.5 cache key can see, because the key names the seed both
    workers were given.

    Torch, NumPy, and Chatterbox are replaced by recorders rather than
    installed. This is the boundary, not the logic: what is under test is the
    order in which this module calls them, which a recorder observes exactly and
    a real backend would only make expensive.
    """

    def _load_with_recorders(self, seed: int) -> list[str]:
        """Runs `_load_backend` against recording backends, returning the calls."""
        calls: list[str] = []

        class Recorder(types.ModuleType):
            """A module whose every recorded call appends its own name."""

            def __init__(self, name: str, **attributes: object) -> None:
                super().__init__(name)
                for attribute, value in attributes.items():
                    setattr(self, attribute, value)

        def record(label: str, result: object = None):
            def called(*_arguments: object, **_keywords: object) -> object:
                calls.append(label)
                return result

            return called

        model = unittest.mock.Mock()
        model.sr = worker_module.CANONICAL_SAMPLE_RATE_HZ
        torch = Recorder(
            "torch",
            manual_seed=record("torch.manual_seed"),
            set_num_threads=record("torch.set_num_threads"),
            set_num_interop_threads=record("torch.set_num_interop_threads"),
        )
        numpy = Recorder("numpy", random=Recorder("numpy.random", seed=record("numpy.seed")))
        chatterbox = Recorder("chatterbox")
        chatterbox_tts = Recorder(
            "chatterbox.tts",
            ChatterboxTTS=Recorder("ChatterboxTTS", from_local=record("from_local", model)),
            Conditionals=Recorder(
                "Conditionals", load=record("Conditionals.load", unittest.mock.Mock())
            ),
        )

        modules = {
            "torch": torch,
            "numpy": numpy,
            "chatterbox": chatterbox,
            "chatterbox.tts": chatterbox_tts,
        }
        # `random` is the standard library's, so it is patched rather than
        # replaced: a recorder there would hide the module the worker really
        # seeds.
        with unittest.mock.patch.dict(sys.modules, modules), unittest.mock.patch.object(
            worker_module.random, "seed", record("random.seed")
        ), unittest.mock.patch.object(
            worker_module, "_governed_root", lambda *_: Path("/nonexistent")
        ), unittest.mock.patch.object(
            worker_module, "_model_identities", lambda *_: ("rev", "codec")
        ), unittest.mock.patch.object(
            worker_module, "_voice_conditioning", lambda *_: {"owner-fallback-v1": "d" * 64}
        ):
            worker_module._load_backend({"seed": seed, "device": "cpu"}, threads=1)

        return calls

    def test_every_generator_is_seeded_before_the_model_is_constructed(self) -> None:
        calls = self._load_with_recorders(seed=7)

        self.assertIn("from_local", calls)
        construction = calls.index("from_local")
        for generator in ("random.seed", "numpy.seed", "torch.manual_seed"):
            self.assertIn(generator, calls)
            self.assertLess(
                calls.index(generator),
                construction,
                f"{generator} must precede model construction, got {calls}",
            )

    def test_the_seed_a_lifetime_uses_is_the_one_its_launcher_records(self) -> None:
        recorded: list[int] = []
        with unittest.mock.patch.object(
            worker_module, "_seed_generators", lambda seed: recorded.append(seed)
        ), unittest.mock.patch.dict(
            sys.modules,
            {
                "torch": unittest.mock.Mock(),
                "numpy": unittest.mock.Mock(),
                "chatterbox": unittest.mock.Mock(),
                "chatterbox.tts": unittest.mock.Mock(),
            },
        ), unittest.mock.patch.object(
            worker_module, "_governed_root", lambda *_: Path("/nonexistent")
        ), unittest.mock.patch.object(
            worker_module, "_model_identities", lambda *_: ("rev", "codec")
        ), unittest.mock.patch.object(
            worker_module, "_voice_conditioning", lambda *_: {"owner-fallback-v1": "d" * 64}
        ):
            with contextlib.suppress(worker_module.BackendUnavailable):
                worker_module._load_backend({"seed": 4242, "device": "cpu"}, threads=1)

        self.assertEqual(recorded, [4242])

if __name__ == "__main__":
    unittest.main()


class AssignedOutputContainmentTests(unittest.TestCase):
    """The worker writes inside the staging root it was given, or refuses.

    `t5_e1_worker_output_cannot_escape_staging_root` names this property. While
    the worker was told one path and no root it could recognise only a literal
    `..` in the path it was handed, so the criterion asserted more than the code
    could prove. `initialize` now carries the root, and containment is decided
    against the resolved parent rather than the spelling: a lexical check passes
    a path whose parent is a symlink out of the root, which is the shape an
    attacker reaches for first.
    """

    def setUp(self) -> None:
        self._workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self._workspace.cleanup)
        self.root = Path(self._workspace.name, "staging").resolve()
        self.root.mkdir()
        self.outside = Path(self._workspace.name, "outside").resolve()
        self.outside.mkdir()
        self.staging = worker_module._staging_root(str(self.root))
        self.addCleanup(os.close, self.staging.descriptor)

    def contained(self, assigned: Path) -> tuple[int, str]:
        """The containment decision, with the descriptor closed for the test."""
        directory, name = worker_module._contained_output(self.staging, str(assigned))
        self.addCleanup(os.close, directory)
        return directory, name

    def test_a_symlinked_parent_inside_the_root_is_refused(self) -> None:
        # Containment used to resolve the parent and then ask where it had
        # landed, so a symlink was admitted as long as it pointed somewhere
        # lawful -- which made the answer depend on what the pathname resolved
        # to at that instant. The walk now starts at the staging root's own
        # descriptor and opens each component with `O_NOFOLLOW`, so a symlink
        # is refused for being one rather than for where it points, and there
        # is no longer a moment at which the answer could change.
        shard = self.root / "shard"
        shard.mkdir()
        bridge = self.root / "bridge"
        bridge.symlink_to(shard, target_is_directory=True)

        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._contained_output(self.staging, str(bridge / "take.wav"))

    def test_a_path_inside_the_root_is_accepted(self) -> None:
        shard = self.root / "shard"
        shard.mkdir()

        directory, name = self.contained(shard / "take.wav")

        self.assertEqual(name, "take.wav")
        self.assertEqual(Path(os.readlink(f"/proc/self/fd/{directory}")), shard)

    def test_an_ancestor_swapped_after_the_check_cannot_redirect_the_write(self) -> None:
        # `O_NOFOLLOW` covers the final component only. While containment
        # returned a pathname for the caller to open later, replacing a
        # directory *above* it with a symlink between the two sent the write
        # wherever the symlink pointed -- with every refusal above having
        # passed on the way through. The check and the write now name one
        # directory descriptor, so there is no pathname left to swap.
        shard = self.root / "shard"
        shard.mkdir()
        directory, name = self.contained(shard / "take.wav")

        shard.rename(self.root / "moved")
        shard.symlink_to(self.outside, target_is_directory=True)
        os.close(worker_module._create_contained_file(directory, name))

        self.assertFalse(
            (self.outside / "take.wav").exists(),
            "the write must not follow a directory swapped in after the check",
        )
        self.assertTrue(
            (self.root / "moved" / "take.wav").is_file(),
            "the write must land in the directory containment was decided about",
        )

    def test_an_absolute_path_outside_the_root_is_refused(self) -> None:
        with self.assertRaises(worker_module.BackendUnavailable) as refused:
            worker_module._contained_output(self.staging, str(self.outside / "take.wav"))

        self.assertIn("staging root", str(refused.exception))

    def test_a_path_that_climbs_out_of_the_root_is_refused(self) -> None:
        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._contained_output(
                self.staging, str(self.root / os.pardir / "outside" / "take.wav")
            )

    def test_a_symlinked_parent_leaving_the_root_is_refused(self) -> None:
        # The case a lexical check cannot see: every component of the assigned
        # path is inside the root by spelling, and the parent resolves out.
        bridge = self.root / "bridge"
        bridge.symlink_to(self.outside, target_is_directory=True)

        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._contained_output(self.staging, str(bridge / "take.wav"))

    def test_a_parent_that_does_not_exist_is_refused(self) -> None:
        # The worker creates exactly the file it was assigned and never a
        # directory, so an absent parent is the parent's mistake, not a path to
        # be built.
        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._contained_output(self.staging, str(self.root / "absent" / "take.wav"))

    def test_the_root_itself_is_not_an_assignable_output(self) -> None:
        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._contained_output(self.staging, str(self.root / os.curdir))


class VoiceProfileResolutionTests(unittest.TestCase):
    """A profile's identity may not choose which directory the worker reads.

    `profile_id` is read out of `profile.json`, and the conditioning artifact
    used to be loaded from `voice_root / profile_id`. That is a path component
    taken from a file's contents: a record stating `../../elsewhere` selects an
    artifact outside the governed voice root, and the existence check ran
    against the directory the record was *found* in rather than the one it
    named, so the two could differ. Requiring the identity to be its own
    directory's name makes containment hold by construction and makes the check
    and the load agree.
    """

    def setUp(self) -> None:
        self._workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self._workspace.cleanup)
        self.voice_root = Path(self._workspace.name, "voices")
        self.voice_root.mkdir()

    def profile(self, directory: str, profile_id: str) -> None:
        """Writes one voice profile whose record states `profile_id`."""
        home = self.voice_root / directory
        home.mkdir()
        (home / "conditionals.pt").write_bytes(b"synthetic conditioning")
        (home / "profile.json").write_text(
            json.dumps({"profile_id": profile_id, "conditionals_blake3": "a" * 64}),
            encoding="utf-8",
        )

    def test_a_profile_whose_identity_is_its_directory_is_read(self) -> None:
        self.profile("nadia-v1", "nadia-v1")

        self.assertEqual(
            worker_module._voice_conditioning(self.voice_root),
            {"nadia-v1": "a" * 64},
        )

    def test_a_profile_naming_another_directory_is_refused(self) -> None:
        self.profile("nadia-v1", "someone-else-v1")

        with self.assertRaises(worker_module.BackendUnavailable) as refused:
            worker_module._voice_conditioning(self.voice_root)

        self.assertIn("nadia-v1", str(refused.exception))

    def test_a_profile_identity_that_walks_out_of_the_root_is_refused(self) -> None:
        # The shape the containment argument is about: a record that selects a
        # conditioning artifact outside the governed voice root entirely.
        self.profile("nadia-v1", "../../elsewhere")

        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._voice_conditioning(self.voice_root)


class RefusalRedactionTests(unittest.TestCase):
    """A backend fault's own text never reaches the protocol channel.

    ADR-0001 §16 keeps source text and voice paths off that channel, and
    `protocol.failure` states it cannot check the message it is handed. Raw
    exception strings are exactly what carries both: an `OSError` renders the
    filename it failed on, and a generation fault can echo the text it was
    asked to speak. What an operator needs is which operation failed and why in
    the kernel's words, neither of which is data.
    """

    def test_an_os_error_is_reported_without_the_path_it_names(self) -> None:
        error = FileNotFoundError(2, "No such file or directory")
        error.filename = "/governed/voices/nadia-v1/conditionals.pt"

        detail = worker_module._redacted_detail(error)

        self.assertIn("No such file or directory", detail)
        self.assertNotIn("/governed", detail)
        self.assertNotIn("nadia-v1", detail)

    def test_an_arbitrary_backend_fault_is_reported_by_type_alone(self) -> None:
        # A generation fault can quote the text it was asked to speak, so the
        # message is dropped entirely and only the type survives.
        error = RuntimeError("failed while speaking 'the reviewed lesson text'")

        detail = worker_module._redacted_detail(error)

        self.assertIn("RuntimeError", detail)
        self.assertNotIn("reviewed lesson text", detail)

    def test_a_refused_output_path_is_not_quoted_back(self) -> None:
        with tempfile.TemporaryDirectory() as workspace:
            root = Path(workspace, "staging")
            root.mkdir()
            staging = worker_module._staging_root(str(root))
            self.addCleanup(os.close, staging.descriptor)

            with self.assertRaises(worker_module.BackendUnavailable) as refused:
                worker_module._contained_output(staging, str(root / "absent" / "take.wav"))

            self.assertNotIn(str(root), str(refused.exception))


@unittest.skipUnless(
    RESTORED_ENVIRONMENT, "needs the restored worker environment's numerical libraries"
)
class RenderPlumbingTests(unittest.TestCase):
    """`_render` is driven end to end, with a stub model and no weights.

    Every other test here reaches the worker's *refusals*, which the standard
    library can express. This one reaches the path that actually renders, and
    nothing did: it imports `numpy`, `soundfile`, and `torch` unconditionally,
    so no test running on the system interpreter could enter it, and the whole
    of it was covered only by `t5_` on the reference machine.

    That gap shipped a defect. Splitting a helper out of `_render` left `voice`
    behind in the caller, and all sixty-one tests here passed because none of
    them could execute the line that used it; the real worker died on the first
    synthesis. A stub model costs nothing and closes the class.

    Stub *model*, real libraries. Faking `numpy` and `torch` well enough to
    satisfy this function would mean writing a second copy of what it does, and
    a test that re-derives its subject agrees with any implementation including
    a wrong one. The model is the only expensive part, and it is the only part
    replaced.
    """

    class _StubModel:
        """Answers `generate` with silence, and records what it was conditioned on."""

        def __init__(self, frames: int) -> None:
            self.frames = frames
            self.conds: object | None = None
            self.calls: list[str] = []

        def generate(self, text: str, **parameters: float) -> object:
            self.calls.append(text)
            return torch.zeros(1, self.frames)

    def setUp(self) -> None:
        self._workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self._workspace.cleanup)
        self.root = Path(self._workspace.name, "staging").resolve()
        self.root.mkdir()
        self.staging = worker_module._staging_root(str(self.root))
        self.addCleanup(os.close, self.staging.descriptor)

        self.launcher = worker_module._load_launcher()
        self.model = self._StubModel(frames=1_000)
        self.backend = worker_module._Backend(
            model=self.model,
            model_revision="v1",
            codec_revision="none",
            conditioning={"owner-fallback-v1": "a" * 64},
            conditionals={"owner-fallback-v1": object()},
            sample_rate=worker_module.CANONICAL_SAMPLE_RATE_HZ,
        )

    def parameters(self, output: Path) -> dict[str, object]:
        return {
            "text": "One. Two. Three.",
            "voice": "owner-fallback-v1",
            "style": worker_module.CALM_EXPLANATORY_STYLE,
            "seed": 42,
            "take": 0,
            "output": str(output),
        }

    def test_a_render_writes_the_assigned_file_inside_the_staging_root(self) -> None:
        shard = self.root / "shard"
        shard.mkdir()
        assigned = shard / "take.wav"

        frames = worker_module._render(
            self.backend, self.launcher, self.parameters(assigned), self.staging
        )

        self.assertEqual(frames, 1_000)
        self.assertTrue(assigned.is_file(), "the take must land at its assigned path")
        self.assertEqual(self.model.calls, ["One. Two. Three."])
        # Set per request rather than once at load, so a take is never rendered
        # under the voice the previous request happened to leave behind.
        self.assertIs(self.model.conds, self.backend.conditionals["owner-fallback-v1"])

    def test_a_render_refused_for_containment_leaves_no_file_and_no_descriptor(self) -> None:
        # The gate runs before generation, so a refused path costs no render --
        # and the descriptor the walk opened is closed on the way out, which a
        # long-running worker depends on for not exhausting its own limit.
        outside = Path(self._workspace.name, "outside")
        outside.mkdir()
        before = len(os.listdir("/proc/self/fd"))

        with self.assertRaises(worker_module.BackendUnavailable):
            worker_module._render(
                self.backend, self.launcher, self.parameters(outside / "take.wav"), self.staging
            )

        self.assertEqual(self.model.calls, [], "no audio may be generated for a refused path")
        self.assertEqual(len(os.listdir("/proc/self/fd")), before, "no descriptor may leak")
