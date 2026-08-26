from __future__ import annotations

import hashlib
import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
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


def require_blake3_executable() -> None:
    if not BLAKE3_EXECUTABLE.exists():
        raise HARNESS.QualificationError(
            "BLAKE3 helper is missing; build the qualification_blake3_file example"
        )


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class VoiceProfilePreflightTests(unittest.TestCase):
    def setUp(self) -> None:
        require_blake3_executable()
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
        self.voice_approval = HARNESS.VoiceApproval(
            profile_sha256=sha256_file(self.voice_root / "profile.json"),
            consent_sha256=sha256_file(self.voice_root / "consent.json"),
            reference_sha256=sha256_file(self.reference_path),
            reference_blake3=ABC_BLAKE3,
            conditionals_sha256=sha256_file(self.conditionals_path),
            conditionals_blake3=ABC_BLAKE3,
            profile_id="test-owner-v1",
            rights_record_id="rights-voice-owner-fallback-v2",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def test_t5_e0_tampered_reference_wav_blocks_qualification(self) -> None:
        self.reference_path.write_bytes(b"abd")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice reference does not match its approved BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(
                self.voice_root,
                BLAKE3_EXECUTABLE,
                self.voice_approval,
            )

    def test_t5_e0_tampered_conditionals_block_qualification(self) -> None:
        self.conditionals_path.write_bytes(b"abd")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice conditionals do not match their approved BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(
                self.voice_root,
                BLAKE3_EXECUTABLE,
                self.voice_approval,
            )

    def test_t5_e0_voice_qualification_requires_explicit_consent_scope(self) -> None:
        consent_path = self.voice_root / "consent.json"
        consent = json.loads(consent_path.read_text(encoding="utf-8"))
        consent["permitted_use"] = ["private_synthesis"]
        consent_path.write_text(json.dumps(consent), encoding="utf-8")
        approval = replace(
            self.voice_approval,
            consent_sha256=sha256_file(consent_path),
        )

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice consent does not permit voice qualification",
        ):
            HARNESS.verify_voice_profile(
                self.voice_root,
                BLAKE3_EXECUTABLE,
                approval,
            )

    def test_t5_e0_malformed_consent_digest_is_rejected_as_malformed(self) -> None:
        consent_path = self.voice_root / "consent.json"
        consent = json.loads(consent_path.read_text(encoding="utf-8"))
        consent["reference_wav_blake3"] = ABC_BLAKE3.upper()
        consent_path.write_text(json.dumps(consent), encoding="utf-8")
        approval = replace(
            self.voice_approval,
            consent_sha256=sha256_file(consent_path),
        )

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice consent contains a malformed BLAKE3 identity",
        ):
            HARNESS.verify_voice_profile(
                self.voice_root,
                BLAKE3_EXECUTABLE,
                approval,
            )

    def test_t5_e0_mutable_profile_cannot_replace_the_trusted_approval(self) -> None:
        profile_path = self.voice_root / "profile.json"
        profile = json.loads(profile_path.read_text(encoding="utf-8"))
        profile["extractor_identity"] = "caller-controlled-extractor"
        profile_path.write_text(json.dumps(profile), encoding="utf-8")

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "voice profile does not match its trusted approval record",
        ):
            HARNESS.verify_voice_profile(
                self.voice_root,
                BLAKE3_EXECUTABLE,
                self.voice_approval,
            )


class BundleApprovalPreflightTests(unittest.TestCase):
    def test_t4_e0_unreadable_trusted_json_uses_the_redacted_error_contract(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            missing = Path(temporary_directory) / "missing.json"

            with self.assertRaisesRegex(
                HARNESS.QualificationError,
                "trusted record is not valid UTF-8 JSON",
            ):
                HARNESS.load_trusted_json(
                    missing,
                    "trusted record",
                    "0" * 64,
                )

    def test_t5_e0_mutable_bundle_manifest_cannot_replace_approval(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = Path(temporary_directory)
            acquisition_path = root / "acquisition-approval.json"
            acquisition_path.write_text(
                json.dumps(
                    {
                        "schema_version": "1.0",
                        "scope": ["owner_only_voice_qualification"],
                        "code": {"commit": "approved-code"},
                        "model": {"revision": "approved-model"},
                    }
                ),
                encoding="utf-8",
            )
            manifest_path = root / "bundle-manifest.json"
            manifest_path.write_text(
                json.dumps({"approval": "caller-controlled"}),
                encoding="utf-8",
            )
            approval = HARNESS.BundleApproval(
                acquisition_record_sha256=sha256_file(acquisition_path),
                manifest_sha256="0" * 64,
            )
            configuration = HARNESS.Configuration(
                code_root=root,
                model_root=root,
                voice_profile_root=root,
                input_root=root,
                input_name="qualification.txt",
                output_root=root / "output",
                bundle_manifest=manifest_path,
                dependency_freeze=root / "requirements.freeze.txt",
                blake3_executable=BLAKE3_EXECUTABLE,
                ffmpeg_executable=root / "ffmpeg",
                ffprobe_executable=root / "ffprobe",
                seed=42,
                run_count=1,
                torch_threads=1,
                torch_interop_threads=1,
            )

            with self.assertRaisesRegex(
                HARNESS.QualificationError,
                "bundle manifest does not match its trusted approval record",
            ):
                HARNESS.verify_acquired_bundle(
                    configuration,
                    root,
                    root,
                    approval,
                )


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

    def test_t1_e0_empty_similarity_distribution_has_no_characterization(self) -> None:
        self.assertIsNone(HARNESS.describe_distribution([]))

    def test_t5_e0_per_run_output_frame_limit_is_enforced_before_write(self) -> None:
        samples = type(
            "OversizedSamples",
            (),
            {"shape": (HARNESS.MAX_GENERATED_FRAMES + 1,)},
        )()

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "Chatterbox output exceeds the per-run frame limit",
        ):
            HARNESS.reserve_output_bytes(samples, 0)

    def test_t5_e0_total_output_budget_is_enforced_before_write(self) -> None:
        samples = type("Samples", (), {"shape": (1,)})()

        with self.assertRaisesRegex(
            HARNESS.QualificationError,
            "Chatterbox output exceeds the total output budget",
        ):
            HARNESS.reserve_output_bytes(
                samples,
                HARNESS.MAX_TOTAL_OUTPUT_BYTES,
            )


class Blake3HelperTests(unittest.TestCase):
    def test_t1_e0_qualification_helper_hashes_actual_file_bytes(self) -> None:
        require_blake3_executable()
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
