from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
HARNESS_PATH = REPOSITORY_ROOT / "scripts/qualification/chatterbox_spike.py"
BLAKE3_EXECUTABLE = (
    REPOSITORY_ROOT / "target/debug/examples/qualification_blake3_file"
)
ABC_BLAKE3 = "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"


def load_harness():
    spec = importlib.util.spec_from_file_location("chatterbox_spike", HARNESS_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("qualification harness could not be imported")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


HARNESS = load_harness()


class VoiceProfilePreflightTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.voice_root = Path(self.temporary_directory.name)
        self.reference_path = self.voice_root / "reference.wav"
        self.conditionals_path = self.voice_root / "conditionals.pt"
        self.reference_path.write_bytes(b"abc")
        self.conditionals_path.write_bytes(b"abc")
        (self.voice_root / "profile.json").write_text(
            json.dumps(
                {
                    "profile_id": "test-owner-v1",
                    "approval": "approved",
                    "reference_wav_blake3": ABC_BLAKE3,
                    "conditionals_blake3": ABC_BLAKE3,
                    "extractor_identity": "test-extractor-v1",
                }
            ),
            encoding="utf-8",
        )
        (self.voice_root / "consent.json").write_text(
            json.dumps(
                {
                    "consent_status": "granted",
                    "rights_record_id": "rights-voice-owner-fallback-v2",
                    "permitted_use": ["private_synthesis", "voice_qualification"],
                    "reference_wav_blake3": ABC_BLAKE3,
                }
            ),
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_t5_e0_tampered_reference_wav_blocks_qualification(self) -> None:
        self.reference_path.write_bytes(b"abd")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice reference does not match its approved BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(self.voice_root, BLAKE3_EXECUTABLE)

    def test_t5_e0_tampered_conditionals_block_qualification(self) -> None:
        self.conditionals_path.write_bytes(b"abd")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice conditionals do not match their approved BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(self.voice_root, BLAKE3_EXECUTABLE)

    def test_t5_e0_voice_qualification_requires_explicit_consent_scope(self) -> None:
        consent_path = self.voice_root / "consent.json"
        consent = json.loads(consent_path.read_text(encoding="utf-8"))
        consent["permitted_use"] = ["private_synthesis"]
        consent_path.write_text(json.dumps(consent), encoding="utf-8")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice consent does not permit voice qualification",
        ):
            HARNESS.verify_voice_profile(self.voice_root, BLAKE3_EXECUTABLE)

    def test_t5_e0_malformed_consent_digest_is_rejected_as_malformed(self) -> None:
        consent_path = self.voice_root / "consent.json"
        consent = json.loads(consent_path.read_text(encoding="utf-8"))
        consent["reference_wav_blake3"] = ABC_BLAKE3.upper()
        consent_path.write_text(json.dumps(consent), encoding="utf-8")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice consent contains a malformed BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(self.voice_root, BLAKE3_EXECUTABLE)


class ExperimentIdentityTests(unittest.TestCase):
    def test_t5_e0_raw_experiment_identity_records_seed_and_run_count(self) -> None:
        identity = HARNESS.build_experiment_identity(
            seed=42,
            run_count=10,
            input_sha256="1" * 64,
            worker_identity_sha256="2" * 64,
        )

        self.assertEqual(identity["seed"], 42)
        self.assertEqual(identity["run_count"], 10)
        self.assertEqual(identity["input_sha256"], "1" * 64)
        self.assertEqual(identity["worker_identity_sha256"], "2" * 64)
        self.assertEqual(len(identity["sha256"]), 64)

    def test_t5_e0_seed_changes_experiment_identity(self) -> None:
        first = HARNESS.build_experiment_identity(
            seed=42,
            run_count=10,
            input_sha256="1" * 64,
            worker_identity_sha256="2" * 64,
        )
        second = HARNESS.build_experiment_identity(
            seed=43,
            run_count=10,
            input_sha256="1" * 64,
            worker_identity_sha256="2" * 64,
        )

        self.assertNotEqual(first["sha256"], second["sha256"])


class Blake3HelperTests(unittest.TestCase):
    def test_t1_e0_qualification_helper_hashes_actual_file_bytes(self) -> None:
        with tempfile.NamedTemporaryFile() as fixture:
            fixture.write(b"abc")
            fixture.flush()
            result = subprocess.run(
                [str(BLAKE3_EXECUTABLE), fixture.name],
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), ABC_BLAKE3)


if __name__ == "__main__":
    unittest.main()
