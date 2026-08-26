#!/usr/bin/env python3
"""Run the E0-S3 fixed-seed Chatterbox qualification outside the product pipeline."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import random
import resource
import secrets
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SAMPLE_RATE_HZ = 24_000
MAX_INPUT_BYTES = 8_192
MAX_GENERATED_DURATION_SECONDS = 60
MAX_GENERATED_FRAMES = SAMPLE_RATE_HZ * MAX_GENERATED_DURATION_SECONDS
FLOAT_BYTES_PER_FRAME = 4
MAX_TOTAL_OUTPUT_BYTES = MAX_GENERATED_FRAMES * FLOAT_BYTES_PER_FRAME * 10
BLAKE3_HEX_LENGTH = 64
REQUIRED_VOICE_USE = "voice_qualification"
GENERATION_PARAMETERS = {
    "repetition_penalty": 1.2,
    "min_p": 0.05,
    "top_p": 1.0,
    "exaggeration": 0.5,
    "cfg_weight": 0.5,
    "temperature": 0.8,
}
OFFLINE_ENVIRONMENT = {
    "HF_HUB_OFFLINE": "1",
    "TRANSFORMERS_OFFLINE": "1",
    "HF_HUB_DISABLE_PROGRESS_BARS": "1",
}
THREAD_ENVIRONMENT_NAMES = (
    "OMP_NUM_THREADS",
    "MKL_NUM_THREADS",
    "OPENBLAS_NUM_THREADS",
    "NUMEXPR_NUM_THREADS",
)


class QualificationError(Exception):
    """Report a qualification refusal without exposing private artifact paths."""


@dataclass(frozen=True)
class Configuration:
    """Validated command-line configuration for one immutable qualification run."""

    code_root: Path
    model_root: Path
    voice_profile_root: Path
    input_root: Path
    input_name: str
    output_root: Path
    bundle_manifest: Path
    dependency_freeze: Path
    blake3_executable: Path
    ffmpeg_executable: Path
    ffprobe_executable: Path
    seed: int
    run_count: int
    torch_threads: int
    torch_interop_threads: int


@dataclass(frozen=True)
class BundleApproval:
    """Bind the acquisition inputs to the immutable E0-S3 approval record."""

    acquisition_record_sha256: str
    manifest_sha256: str


@dataclass(frozen=True)
class VoiceApproval:
    """Bind the governed voice records and private artifacts to one approval."""

    profile_sha256: str
    consent_sha256: str
    reference_sha256: str
    reference_blake3: str
    conditionals_sha256: str
    conditionals_blake3: str
    profile_id: str
    rights_record_id: str


# These identities bind the reviewed E0-S3 acquisition records and the approved v2 voice rights
# record. Changing one requires superseding governance before this harness may accept it.
TRUSTED_BUNDLE_APPROVAL = BundleApproval(
    acquisition_record_sha256=(
        "f034c52e4ace6467e993eb9d8e29efe37df14d21deca41349060cba5c7407a9d"
    ),
    manifest_sha256="ff1c09d66f069ff4b797d520fa22cfd9c888a43796825c1525237689ef9ed24f",
)
TRUSTED_VOICE_APPROVAL = VoiceApproval(
    profile_sha256="d17e73efd281af2dbdc0adf2e772dad856e30fb3bc572f4b7ce6459994b98de1",
    consent_sha256="a46bdfc090a955227c5674c863aecc6e75ec8dd0cfa8778a2fe79779d64dcc6d",
    reference_sha256="1d6b2c247f9e66e23e9d27819920430993ae2296c138dd88a4b39a8f38b117e8",
    reference_blake3="b57455db4712257ab102af210098ef8b0592d03c296178640c6e47ef129c61db",
    conditionals_sha256=(
        "f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00"
    ),
    conditionals_blake3=(
        "4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47"
    ),
    profile_id="owner-fallback-v1",
    rights_record_id="rights-voice-owner-fallback-v2",
)


def positive_integer(value: str) -> int:
    """Parse a strictly positive integer for an argparse option."""

    parsed = int(value)
    if parsed <= 0:
        raise argparse.ArgumentTypeError("value must be greater than zero")
    return parsed


def parse_arguments() -> Configuration:
    """Parse and validate the qualification command line."""

    parser = argparse.ArgumentParser(
        description="Run a network-isolated, qualification-only Chatterbox experiment."
    )
    parser.add_argument("--code-root", required=True, type=Path)
    parser.add_argument("--model-root", required=True, type=Path)
    parser.add_argument("--voice-profile-root", required=True, type=Path)
    parser.add_argument("--input-root", required=True, type=Path)
    parser.add_argument("--input-name", default="qualification.txt")
    parser.add_argument("--output-root", required=True, type=Path)
    parser.add_argument("--bundle-manifest", required=True, type=Path)
    parser.add_argument("--dependency-freeze", required=True, type=Path)
    parser.add_argument("--blake3-executable", required=True, type=Path)
    parser.add_argument("--ffmpeg-executable", required=True, type=Path)
    parser.add_argument("--ffprobe-executable", required=True, type=Path)
    parser.add_argument("--seed", required=True, type=int)
    parser.add_argument("--run-count", required=True, type=positive_integer)
    parser.add_argument("--torch-threads", required=True, type=positive_integer)
    parser.add_argument("--torch-interop-threads", required=True, type=positive_integer)
    arguments = parser.parse_args()

    if Path(arguments.input_name).name != arguments.input_name:
        parser.error("--input-name must be one plain file name")

    return Configuration(
        code_root=arguments.code_root,
        model_root=arguments.model_root,
        voice_profile_root=arguments.voice_profile_root,
        input_root=arguments.input_root,
        input_name=arguments.input_name,
        output_root=arguments.output_root,
        bundle_manifest=arguments.bundle_manifest,
        dependency_freeze=arguments.dependency_freeze,
        blake3_executable=arguments.blake3_executable,
        ffmpeg_executable=arguments.ffmpeg_executable,
        ffprobe_executable=arguments.ffprobe_executable,
        seed=arguments.seed,
        run_count=arguments.run_count,
        torch_threads=arguments.torch_threads,
        torch_interop_threads=arguments.torch_interop_threads,
    )


def command_output(arguments: list[str], *, cwd: Path | None = None) -> str:
    """Run one checked argv vector and return decoded standard output."""

    result = subprocess.run(
        arguments,
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        executable = Path(arguments[0]).name
        raise QualificationError(f"{executable} failed during qualification preflight")
    return result.stdout


def reject_symlink_components(path: Path, label: str) -> None:
    """Refuse a root whose existing path chain contains a symbolic link."""

    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        if current.is_symlink():
            raise QualificationError(f"{label} contains a symbolic-link component")


def filesystem_type(path: Path) -> str:
    """Return the mount filesystem type owning a resolved path."""

    output = command_output(
        ["findmnt", "--noheadings", "--output", "FSTYPE", "--target", str(path)]
    )
    filesystem = output.strip()
    if not filesystem:
        raise QualificationError("findmnt did not identify a filesystem")
    return filesystem


def validate_existing_root(path: Path, label: str) -> Path:
    """Resolve and validate an existing ext4 directory without symlink traversal."""

    reject_symlink_components(path.absolute(), label)
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise QualificationError(f"{label} is unavailable") from error
    if not resolved.is_dir():
        raise QualificationError(f"{label} is not a directory")
    if resolved == Path("/mnt/c") or Path("/mnt/c") in resolved.parents:
        raise QualificationError(f"{label} resolves through /mnt/c")
    if filesystem_type(resolved) != "ext4":
        raise QualificationError(f"{label} is not on the qualified ext4 filesystem")
    return resolved


def prepare_output_root(path: Path) -> Path:
    """Create a fresh ext4 output root without overwriting prior evidence."""

    absolute = path.absolute()
    reject_symlink_components(absolute.parent, "output root parent")
    parent = absolute.parent.resolve(strict=True)
    if filesystem_type(parent) != "ext4":
        raise QualificationError("output root parent is not on the qualified ext4 filesystem")
    if absolute.exists() or absolute.is_symlink():
        raise QualificationError("output root already exists; qualification evidence is immutable")
    absolute.mkdir(mode=0o700)
    return absolute.resolve(strict=True)


def validate_regular_file(path: Path, root: Path, label: str) -> Path:
    """Validate one regular file is directly contained by its governed root."""

    if path.is_symlink():
        raise QualificationError(f"{label} must not be a symbolic link")
    try:
        resolved = path.resolve(strict=True)
    except OSError as error:
        raise QualificationError(f"{label} is unavailable") from error
    if resolved.parent != root or not resolved.is_file():
        raise QualificationError(f"{label} is not a regular file in its governed root")
    return resolved


def sha256_file(path: Path) -> str:
    """Hash a file without loading a large artifact into memory."""

    digest = hashlib.sha256()
    with path.open("rb") as artifact:
        for chunk in iter(lambda: artifact.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_blake3_hex(value: Any) -> bool:
    """Recognize the normalized digest form emitted by the Rust helper."""

    return (
        isinstance(value, str)
        and len(value) == BLAKE3_HEX_LENGTH
        and all(character in "0123456789abcdef" for character in value)
    )


def blake3_file(path: Path, executable: Path) -> str:
    """Hash actual file bytes through the workspace-pinned BLAKE3 implementation."""

    digest = command_output([str(executable), str(path)]).strip()
    if not is_blake3_hex(digest):
        raise QualificationError("BLAKE3 helper returned a malformed digest")
    return digest


def canonical_json_hash(value: Any) -> str:
    """Hash a JSON value using stable key order and separators."""

    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def load_json(path: Path, label: str) -> dict[str, Any]:
    """Load a JSON object or refuse malformed evidence input."""

    try:
        raw = path.read_bytes()
    except OSError as error:
        raise QualificationError(f"{label} is not valid UTF-8 JSON") from error
    return decode_json_object(raw, label)


def decode_json_object(raw: bytes, label: str) -> dict[str, Any]:
    """Decode one JSON object from previously read bytes."""

    try:
        value = json.loads(raw.decode("utf-8"))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise QualificationError(f"{label} is not valid UTF-8 JSON") from error
    if not isinstance(value, dict):
        raise QualificationError(f"{label} must contain one JSON object")
    return value


def load_trusted_json(
    path: Path,
    label: str,
    expected_sha256: str,
) -> tuple[dict[str, Any], str]:
    """Authenticate one governed JSON record before accepting any of its fields."""

    try:
        raw = path.read_bytes()
    except OSError as error:
        raise QualificationError(f"{label} is not valid UTF-8 JSON") from error
    actual_sha256 = hashlib.sha256(raw).hexdigest()
    if actual_sha256 != expected_sha256:
        raise QualificationError(f"{label} does not match its trusted approval record")
    return decode_json_object(raw, label), actual_sha256


def validate_network_isolation() -> dict[str, Any]:
    """Require the loopback-only namespace used by the offline qualification."""

    interfaces = sorted(
        line.split(":", maxsplit=1)[0].strip()
        for line in Path("/proc/net/dev").read_text(encoding="utf-8").splitlines()[2:]
        if ":" in line
    )
    if interfaces != ["lo"]:
        raise QualificationError("qualification must run in a loopback-only network namespace")
    routes = [
        line
        for line in Path("/proc/net/route").read_text(encoding="utf-8").splitlines()[1:]
        if line.strip()
    ]
    if routes:
        raise QualificationError("qualification network namespace contains an IP route")
    return {
        "interfaces": interfaces,
        "routes": routes,
        "namespace_inode": os.stat("/proc/self/ns/net").st_ino,
        "offline_environment": OFFLINE_ENVIRONMENT,
    }


def configure_process_environment(configuration: Configuration, output_root: Path) -> None:
    """Set offline and thread controls before importing numerical libraries."""

    for name, value in OFFLINE_ENVIRONMENT.items():
        os.environ[name] = value
    thread_value = str(configuration.torch_threads)
    for name in THREAD_ENVIRONMENT_NAMES:
        os.environ[name] = thread_value
    cache_root = output_root / "runtime-cache"
    cache_root.mkdir(mode=0o700)
    os.environ["HF_HOME"] = str(cache_root)
    os.environ["HF_HUB_CACHE"] = str(cache_root / "hub")


def hash_code_tree(code_root: Path, harness_path: Path) -> str:
    """Derive one hash from executable local Chatterbox and harness inputs."""

    candidates = [code_root / "pyproject.toml", harness_path]
    candidates.extend((code_root / "src").rglob("*.py"))
    digest = hashlib.sha256()
    for path in sorted(candidates, key=lambda candidate: candidate.as_posix()):
        if not path.is_file() or path.is_symlink():
            raise QualificationError("the executable code tree contains an invalid file")
        label = (
            "qualification-harness"
            if path == harness_path
            else path.relative_to(code_root).as_posix()
        )
        digest.update(label.encode("utf-8"))
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def hash_python_package(package_root: Path) -> str:
    """Hash Python sources by package-relative path for installed-source comparison."""

    candidates = sorted(
        package_root.rglob("*.py"), key=lambda candidate: candidate.as_posix()
    )
    if not candidates:
        raise QualificationError("Chatterbox package contains no Python sources")
    digest = hashlib.sha256()
    for path in candidates:
        if not path.is_file() or path.is_symlink():
            raise QualificationError("Chatterbox package contains an invalid source file")
        digest.update(path.relative_to(package_root).as_posix().encode("utf-8"))
        digest.update(b"\0")
        digest.update(bytes.fromhex(sha256_file(path)))
    return digest.hexdigest()


def verify_acquired_bundle(
    configuration: Configuration,
    code_root: Path,
    model_root: Path,
    approval: BundleApproval = TRUSTED_BUNDLE_APPROVAL,
) -> tuple[dict[str, Any], list[dict[str, Any]]]:
    """Verify the acquired code revision and every recorded model artifact."""

    acquisition_record, _ = load_trusted_json(
        configuration.bundle_manifest.parent / "acquisition-approval.json",
        "acquisition approval",
        approval.acquisition_record_sha256,
    )
    manifest, _ = load_trusted_json(
        configuration.bundle_manifest,
        "bundle manifest",
        approval.manifest_sha256,
    )
    approved_scope = acquisition_record.get("scope")
    approved_code = acquisition_record.get("code")
    approved_model = acquisition_record.get("model")
    if (
        acquisition_record.get("schema_version") != "1.0"
        or not isinstance(approved_scope, list)
        or "owner_only_voice_qualification" not in approved_scope
        or not isinstance(approved_code, dict)
        or not isinstance(approved_model, dict)
    ):
        raise QualificationError("acquisition approval does not authorize this bundle")
    if manifest.get("schema_version") != "1.0":
        raise QualificationError("bundle manifest schema version is unsupported")
    code = manifest.get("code")
    model = manifest.get("model")
    if not isinstance(code, dict) or not isinstance(model, dict):
        raise QualificationError("bundle manifest is missing code or model identity")
    if (
        code.get("commit") != approved_code.get("commit")
        or model.get("revision") != approved_model.get("revision")
    ):
        raise QualificationError("bundle manifest does not match the trusted approval record")

    commit = command_output(["git", "-C", str(code_root), "rev-parse", "HEAD"]).strip()
    status = command_output(["git", "-C", str(code_root), "status", "--porcelain"]).strip()
    if commit != code.get("commit") or status:
        raise QualificationError("acquired Chatterbox code does not match the approved commit")

    artifacts = model.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise QualificationError("bundle manifest has no model artifacts")
    verified: list[dict[str, Any]] = []
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise QualificationError("bundle manifest contains an invalid artifact row")
        name = artifact.get("name")
        expected_hash = artifact.get("sha256")
        expected_bytes = artifact.get("bytes")
        if not isinstance(name, str) or Path(name).name != name:
            raise QualificationError("bundle manifest contains an unsafe artifact name")
        path = validate_regular_file(model_root / name, model_root, "model artifact")
        if path.suffix not in {".safetensors", ".json"}:
            raise QualificationError("bundle manifest contains a prohibited model format")
        found_hash = sha256_file(path)
        found_bytes = path.stat().st_size
        if found_hash != expected_hash or found_bytes != expected_bytes:
            raise QualificationError("model artifact does not match the acquisition manifest")
        verified.append({"name": name, "bytes": found_bytes, "sha256": found_hash})

    unrecorded = sorted(
        path.name
        for path in model_root.iterdir()
        if path.is_file() and path.name not in {row["name"] for row in verified}
    )
    if unrecorded:
        raise QualificationError("model root contains an unrecorded artifact")
    if any(path.suffix in {".pt", ".pth", ".pkl", ".pickle"} for path in model_root.iterdir()):
        raise QualificationError("model root contains a prohibited pickle-style artifact")
    return manifest, verified


def verify_voice_profile(
    voice_root: Path,
    blake3_executable: Path,
    approval: VoiceApproval = TRUSTED_VOICE_APPROVAL,
) -> dict[str, Any]:
    """Verify approved consent linkage and hash the private voice artifacts."""

    profile_path = validate_regular_file(voice_root / "profile.json", voice_root, "voice profile")
    consent_path = validate_regular_file(voice_root / "consent.json", voice_root, "voice consent")
    reference_path = validate_regular_file(
        voice_root / "reference.wav", voice_root, "voice reference"
    )
    conditionals_path = validate_regular_file(
        voice_root / "conditionals.pt", voice_root, "voice conditionals"
    )
    profile, profile_sha256 = load_trusted_json(
        profile_path,
        "voice profile",
        approval.profile_sha256,
    )
    consent, consent_sha256 = load_trusted_json(
        consent_path,
        "voice consent",
        approval.consent_sha256,
    )
    if profile.get("approval") != "approved" or consent.get("consent_status") != "granted":
        raise QualificationError("voice profile or consent is not approved")
    if (
        profile.get("profile_id") != approval.profile_id
        or consent.get("rights_record_id") != approval.rights_record_id
    ):
        raise QualificationError("voice consent does not name the superseding v2 rights record")
    permitted_use = consent.get("permitted_use")
    if not isinstance(permitted_use, list) or REQUIRED_VOICE_USE not in permitted_use:
        raise QualificationError("voice consent does not permit voice qualification")
    reference_blake3 = profile.get("reference_wav_blake3")
    conditionals_blake3 = profile.get("conditionals_blake3")
    if not is_blake3_hex(reference_blake3) or not is_blake3_hex(conditionals_blake3):
        raise QualificationError("voice profile contains a malformed BLAKE3 identity")
    consent_reference_blake3 = consent.get("reference_wav_blake3")
    if not is_blake3_hex(consent_reference_blake3):
        raise QualificationError("voice consent contains a malformed BLAKE3 identity")
    if (
        reference_blake3 != approval.reference_blake3
        or conditionals_blake3 != approval.conditionals_blake3
        or consent_reference_blake3 != approval.reference_blake3
    ):
        raise QualificationError("voice profile and consent disagree on the reference identity")
    if blake3_file(reference_path, blake3_executable) != reference_blake3:
        raise QualificationError(
            "voice reference does not match its approved BLAKE3 identity"
        )
    if blake3_file(conditionals_path, blake3_executable) != conditionals_blake3:
        raise QualificationError(
            "voice conditionals do not match their approved BLAKE3 identity"
        )
    reference_sha256 = sha256_file(reference_path)
    conditionals_sha256 = sha256_file(conditionals_path)
    if reference_sha256 != approval.reference_sha256:
        raise QualificationError(
            "voice reference does not match its approved SHA-256 identity"
        )
    if conditionals_sha256 != approval.conditionals_sha256:
        raise QualificationError(
            "voice conditionals do not match their approved SHA-256 identity"
        )
    return {
        "profile_id": profile.get("profile_id"),
        "profile_sha256": profile_sha256,
        "consent_sha256": consent_sha256,
        "reference_wav_sha256": reference_sha256,
        "reference_wav_blake3": reference_blake3,
        "conditionals_sha256": conditionals_sha256,
        "conditionals_blake3": conditionals_blake3,
        "extractor_identity": profile.get("extractor_identity"),
        "rights_record_id": consent.get("rights_record_id"),
        "conditionals_path": conditionals_path,
    }


def read_qualification_input(input_root: Path, input_name: str) -> tuple[str, dict[str, Any]]:
    """Read bounded reviewed text while returning only redacted identity metadata."""

    input_path = validate_regular_file(input_root / input_name, input_root, "qualification input")
    raw = input_path.read_bytes()
    if not raw or len(raw) > MAX_INPUT_BYTES:
        raise QualificationError("qualification input is empty or exceeds the bounded size")
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise QualificationError("qualification input is not UTF-8") from error
    if not text.strip():
        raise QualificationError("qualification input contains no spoken text")
    return text, {"sha256": hashlib.sha256(raw).hexdigest(), "bytes": len(raw)}


def build_experiment_identity(
    *,
    seed: int,
    run_count: int,
    input_sha256: str,
    worker_identity_sha256: str,
) -> dict[str, Any]:
    """Bind the fixed-seed request controls recorded by a raw result."""

    identity = {
        "schema_version": "1.0-e0-s3-experiment-identity",
        "seed": seed,
        "run_count": run_count,
        "input_sha256": input_sha256,
        "worker_identity_sha256": worker_identity_sha256,
        "generation_parameters": GENERATION_PARAMETERS,
    }
    identity["sha256"] = canonical_json_hash(identity)
    return identity


def reset_seeds(seed: int, numpy_module: Any, torch_module: Any) -> None:
    """Reset Python, NumPy, and Torch RNGs immediately before generation."""

    random.seed(seed)
    numpy_module.random.seed(seed)
    torch_module.manual_seed(seed)


def reserve_output_bytes(samples: Any, used_bytes: int) -> int:
    """Reserve bounded output capacity before a generated sample is written."""

    frame_count = int(samples.shape[0])
    if frame_count > MAX_GENERATED_FRAMES:
        raise QualificationError("Chatterbox output exceeds the per-run frame limit")
    total_bytes = used_bytes + frame_count * FLOAT_BYTES_PER_FRAME
    if total_bytes > MAX_TOTAL_OUTPUT_BYTES:
        raise QualificationError("Chatterbox output exceeds the total output budget")
    return total_bytes


def write_float_wav(path: Path, samples: Any, soundfile_module: Any) -> None:
    """Atomically write one explicit mono 24 kHz IEEE-float WAV."""

    handle, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    os.close(handle)
    temporary = Path(temporary_name)
    try:
        soundfile_module.write(
            temporary,
            samples,
            SAMPLE_RATE_HZ,
            format="WAV",
            subtype="FLOAT",
        )
        with temporary.open("rb") as artifact:
            os.fsync(artifact.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def inspect_wav(
    path: Path,
    soundfile_module: Any,
    numpy_module: Any,
    accepted_formats: frozenset[str],
) -> dict[str, Any]:
    """Decode every sample and enforce the qualification WAV contract."""

    info = soundfile_module.info(path)
    samples, rate = soundfile_module.read(path, dtype="float32", always_2d=True)
    if rate != SAMPLE_RATE_HZ or info.channels != 1 or samples.shape[1] != 1:
        raise QualificationError("generated WAV does not have the required rate and channel count")
    if info.format not in accepted_formats or info.subtype != "FLOAT":
        raise QualificationError("generated WAV is not explicit 32-bit IEEE-float WAV")
    if samples.shape[0] == 0 or not numpy_module.isfinite(samples).all():
        raise QualificationError("generated WAV is empty or contains non-finite samples")
    maximum = float(numpy_module.max(numpy_module.abs(samples)))
    if maximum > 1.0:
        raise QualificationError("generated WAV contains an over-range float sample")
    return {
        "format": info.format,
        "subtype": info.subtype,
        "sample_rate_hz": rate,
        "channels": info.channels,
        "frames": int(samples.shape[0]),
        "duration_seconds": float(samples.shape[0] / SAMPLE_RATE_HZ),
        "maximum_absolute_sample": maximum,
    }


def probe_wav(path: Path, executable: Path) -> dict[str, Any]:
    """Inspect one WAV through the recorded ffprobe executable."""

    output = command_output(
        [
            str(executable),
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,sample_fmt,sample_rate,channels,bits_per_sample,duration",
            "-show_entries",
            "format=format_name,duration,size",
            "-of",
            "json",
            path.name,
        ],
        cwd=path.parent,
    )
    try:
        return json.loads(output)
    except json.JSONDecodeError as error:
        raise QualificationError("ffprobe returned malformed JSON") from error


def aligned_similarity(
    reference: Any,
    candidate: Any,
    numpy_module: Any,
    librosa_module: Any,
    scipy_signal: Any,
) -> dict[str, Any]:
    """Measure maximum-lag waveform correlation and aligned log-mel cosine similarity."""

    reference = numpy_module.asarray(reference, dtype=numpy_module.float64)
    candidate = numpy_module.asarray(candidate, dtype=numpy_module.float64)
    centered_reference = reference - numpy_module.mean(reference)
    centered_candidate = candidate - numpy_module.mean(candidate)
    correlation = scipy_signal.correlate(
        centered_candidate,
        centered_reference,
        mode="full",
        method="fft",
    )
    lags = scipy_signal.correlation_lags(candidate.size, reference.size, mode="full")
    lag = int(lags[int(numpy_module.argmax(correlation))])
    candidate_start = max(lag, 0)
    reference_start = max(-lag, 0)
    overlap = min(candidate.size - candidate_start, reference.size - reference_start)
    if overlap < 1_024:
        raise QualificationError("aligned waveform overlap is too short for comparison")
    aligned_candidate = candidate[candidate_start : candidate_start + overlap]
    aligned_reference = reference[reference_start : reference_start + overlap]
    aligned_candidate -= numpy_module.mean(aligned_candidate)
    aligned_reference -= numpy_module.mean(aligned_reference)
    denominator = float(
        numpy_module.linalg.norm(aligned_candidate)
        * numpy_module.linalg.norm(aligned_reference)
    )
    waveform_correlation = (
        float(numpy_module.dot(aligned_candidate, aligned_reference) / denominator)
        if denominator > 0.0
        else 0.0
    )

    reference_mel = librosa_module.feature.melspectrogram(
        y=aligned_reference.astype(numpy_module.float32),
        sr=SAMPLE_RATE_HZ,
        n_fft=1_024,
        hop_length=256,
        n_mels=80,
        power=2.0,
    )
    candidate_mel = librosa_module.feature.melspectrogram(
        y=aligned_candidate.astype(numpy_module.float32),
        sr=SAMPLE_RATE_HZ,
        n_fft=1_024,
        hop_length=256,
        n_mels=80,
        power=2.0,
    )
    reference_log = librosa_module.power_to_db(reference_mel, ref=1.0).ravel()
    candidate_log = librosa_module.power_to_db(candidate_mel, ref=1.0).ravel()
    mel_denominator = float(
        numpy_module.linalg.norm(reference_log) * numpy_module.linalg.norm(candidate_log)
    )
    mel_cosine = (
        float(numpy_module.dot(reference_log, candidate_log) / mel_denominator)
        if mel_denominator > 0.0
        else 0.0
    )
    return {
        "alignment_lag_samples": lag,
        "overlap_frames": int(overlap),
        "waveform_correlation": waveform_correlation,
        "log_mel_cosine_similarity": mel_cosine,
    }


def atomic_write_json(path: Path, value: Any) -> None:
    """Atomically publish one durable JSON result."""

    encoded = json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    handle, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(handle, "wb") as output:
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def create_listening_set(output_root: Path, runs: list[dict[str, Any]]) -> dict[str, Any]:
    """Create randomized blind copies plus separate review and mapping records."""

    listening_root = output_root / "listening"
    listening_root.mkdir(mode=0o700)
    order = list(range(len(runs)))
    secrets.SystemRandom().shuffle(order)
    if len(order) > 1 and order == list(range(len(runs))):
        order[0], order[1] = order[1], order[0]

    review_samples = []
    mapping = []
    for blind_index, run_index in enumerate(order, start=1):
        blind_id = f"sample-{blind_index:02d}"
        destination = listening_root / f"{blind_id}.wav"
        source = output_root / runs[run_index]["wav"]
        shutil.copyfile(source, destination)
        copied_hash = sha256_file(destination)
        if copied_hash != runs[run_index]["wav_sha256"]:
            raise QualificationError("a randomized listening copy changed bytes")
        review_samples.append(
            {
                "blind_id": blind_id,
                "wav": destination.relative_to(output_root).as_posix(),
                "sha256": copied_hash,
                "findings": {
                    "omissions_or_additions": None,
                    "pronunciation": None,
                    "voice_consistency": None,
                    "pacing": None,
                    "noise": None,
                    "audible_difference_from_other_runs": None,
                },
                "disposition": None,
            }
        )
        mapping.append(
            {
                "blind_id": blind_id,
                "source_run": runs[run_index]["run"],
                "sha256": copied_hash,
            }
        )

    review_sheet = {
        "schema_version": "1.0-e0-s3-listening-review",
        "status": "pending_human_review",
        "instructions": "Review every sample before opening randomization-key.json.",
        "reviewer": None,
        "playback_environment": None,
        "reviewed_at": None,
        "samples": review_samples,
        "overall_finding": None,
    }
    randomization_key = {
        "schema_version": "1.0-e0-s3-randomization-key",
        "mapping": mapping,
    }
    review_path = listening_root / "review-sheet.json"
    key_path = listening_root / "randomization-key.json"
    atomic_write_json(review_path, review_sheet)
    atomic_write_json(key_path, randomization_key)
    return {
        "status": "pending_human_review",
        "review_sheet": review_path.relative_to(output_root).as_posix(),
        "review_sheet_sha256": sha256_file(review_path),
        "randomization_key": key_path.relative_to(output_root).as_posix(),
        "randomization_key_sha256": sha256_file(key_path),
    }


def convert_with_ffmpeg(output_root: Path, executable: Path) -> dict[str, Any]:
    """Create and record the required FFmpeg pcm_f32le conversion variant."""

    input_name = "run-01.wav"
    output_name = "ffmpeg-pcm-f32le.wav"
    arguments = [
        "-nostdin",
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        input_name,
        "-map_metadata",
        "-1",
        "-ac",
        "1",
        "-ar",
        str(SAMPLE_RATE_HZ),
        "-c:a",
        "pcm_f32le",
        output_name,
    ]
    command_output([str(executable), *arguments], cwd=output_root)
    return {
        "arguments": arguments,
        "argument_profile_sha256": canonical_json_hash(arguments),
        "output": output_name,
        "output_sha256": sha256_file(output_root / output_name),
    }


def tool_identity(executable: Path) -> dict[str, Any]:
    """Record one executable's complete version and build configuration."""

    version_output = command_output([str(executable), "-version"])
    return {
        "executable_sha256": sha256_file(executable.resolve(strict=True)),
        "version_output": version_output.splitlines(),
    }


def describe_distribution(values: list[float]) -> dict[str, float] | None:
    """Report a cross-run characterization when at least one comparison exists."""

    if not values:
        return None
    return {
        "minimum": min(values),
        "median": statistics.median(values),
        "maximum": max(values),
    }


def run_qualification(configuration: Configuration) -> Path:
    """Execute the complete qualification experiment and return its result path."""

    code_root = validate_existing_root(configuration.code_root, "code root")
    model_root = validate_existing_root(configuration.model_root, "model root")
    voice_root = validate_existing_root(configuration.voice_profile_root, "voice root")
    input_root = validate_existing_root(configuration.input_root, "input root")
    output_root = prepare_output_root(configuration.output_root)
    network = validate_network_isolation()
    configure_process_environment(configuration, output_root)

    bundle_manifest, model_artifacts = verify_acquired_bundle(
        configuration, code_root, model_root
    )
    voice_identity = verify_voice_profile(voice_root, configuration.blake3_executable)
    text, input_identity = read_qualification_input(input_root, configuration.input_name)
    dependency_hash = sha256_file(configuration.dependency_freeze.resolve(strict=True))
    if dependency_hash != bundle_manifest.get("environment", {}).get(
        "requirements_freeze_sha256"
    ):
        raise QualificationError("restored dependency inventory does not match the bundle")

    import librosa
    import numpy as np
    import soundfile as sf
    import torch
    from chatterbox.tts import ChatterboxTTS, Conditionals
    from scipy import signal as scipy_signal

    torch.set_num_threads(configuration.torch_threads)
    torch.set_num_interop_threads(configuration.torch_interop_threads)
    if torch.get_num_threads() != configuration.torch_threads:
        raise QualificationError("Torch intra-op thread control was not applied")
    if torch.get_num_interop_threads() != configuration.torch_interop_threads:
        raise QualificationError("Torch inter-op thread control was not applied")

    imported_tts = Path(sys.modules["chatterbox.tts"].__file__).resolve(strict=True)
    installed_package_hash = hash_python_package(imported_tts.parent)
    acquired_package_hash = hash_python_package(code_root / "src/chatterbox")
    if installed_package_hash != acquired_package_hash:
        raise QualificationError("installed Chatterbox sources differ from the acquired code root")

    load_started = time.perf_counter()
    model = ChatterboxTTS.from_local(model_root, "cpu")
    model.conds = Conditionals.load(
        voice_identity["conditionals_path"], map_location="cpu"
    ).to("cpu")
    load_seconds = time.perf_counter() - load_started
    if model.sr != SAMPLE_RATE_HZ:
        raise QualificationError("Chatterbox reported an unexpected native sample rate")

    code_tree_hash = hash_code_tree(code_root, Path(__file__).resolve(strict=True))
    worker_identity = {
        "code_commit": bundle_manifest["code"]["commit"],
        "code_tree_sha256": code_tree_hash,
        "installed_package_sha256": installed_package_hash,
        "model_revision": bundle_manifest["model"]["revision"],
        "model_artifacts": model_artifacts,
        "dependency_freeze_sha256": dependency_hash,
        "voice_conditionals_sha256": voice_identity["conditionals_sha256"],
        "generation_parameters": GENERATION_PARAMETERS,
        "device": "cpu",
        "torch_threads": configuration.torch_threads,
        "torch_interop_threads": configuration.torch_interop_threads,
        "python": sys.version,
        "platform": platform.platform(),
    }
    worker_identity["sha256"] = canonical_json_hash(worker_identity)
    experiment_identity = build_experiment_identity(
        seed=configuration.seed,
        run_count=configuration.run_count,
        input_sha256=input_identity["sha256"],
        worker_identity_sha256=worker_identity["sha256"],
    )

    runs: list[dict[str, Any]] = []
    reference_samples = None
    waveform_correlations: list[float] = []
    log_mel_similarities: list[float] = []
    used_output_bytes = 0
    for index in range(1, configuration.run_count + 1):
        reset_seeds(configuration.seed, np, torch)
        started = time.perf_counter()
        generated = model.generate(text, **GENERATION_PARAMETERS)
        wall_seconds = time.perf_counter() - started
        samples = generated.squeeze().detach().cpu().numpy().astype(np.float32, copy=False)
        if samples.ndim != 1:
            raise QualificationError("Chatterbox returned audio with an unexpected shape")
        used_output_bytes = reserve_output_bytes(samples, used_output_bytes)
        wav_name = f"run-{index:02d}.wav"
        wav_path = output_root / wav_name
        write_float_wav(wav_path, samples, sf)
        media = inspect_wav(wav_path, sf, np, frozenset({"WAV"}))
        similarity = (
            {
                "alignment_lag_samples": 0,
                "overlap_frames": media["frames"],
                "waveform_correlation": 1.0,
                "log_mel_cosine_similarity": 1.0,
            }
            if reference_samples is None
            else aligned_similarity(
                reference_samples,
                samples,
                np,
                librosa,
                scipy_signal,
            )
        )
        if reference_samples is None:
            reference_samples = samples.copy()
        else:
            waveform_correlations.append(similarity["waveform_correlation"])
            log_mel_similarities.append(similarity["log_mel_cosine_similarity"])
        duration_seconds = media["duration_seconds"]
        runs.append(
            {
                "run": index,
                "wav": wav_name,
                "wav_sha256": sha256_file(wav_path),
                "synthesis_wall_seconds": wall_seconds,
                "audio_duration_seconds": duration_seconds,
                "real_time_factor": wall_seconds / duration_seconds,
                "media": media,
                "ffprobe": probe_wav(wav_path, configuration.ffprobe_executable),
                "similarity_to_run_one": similarity,
                "process_peak_rss_kib_after_run": resource.getrusage(
                    resource.RUSAGE_SELF
                ).ru_maxrss,
            }
        )

    cache_copy = output_root / "cache-preserved.wav"
    shutil.copyfile(output_root / "run-01.wav", cache_copy)
    if sha256_file(cache_copy) != runs[0]["wav_sha256"]:
        raise QualificationError("cache-preserved copy is not byte-identical")
    ffmpeg_conversion = convert_with_ffmpeg(output_root, configuration.ffmpeg_executable)
    inspect_wav(
        output_root / ffmpeg_conversion["output"],
        sf,
        np,
        frozenset({"WAVEX"}),
    )
    listening = create_listening_set(output_root, runs)

    rtfs = [run["real_time_factor"] for run in runs]
    durations = [run["audio_duration_seconds"] for run in runs]
    worst_rtf = max(rtfs)
    projected_seconds = load_seconds + worst_rtf * 3_600.0
    summary = {
        "model_load_seconds": load_seconds,
        "process_peak_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
        "worst_real_time_factor": worst_rtf,
        "projected_sixty_minute_seconds": projected_seconds,
        "unique_wav_sha256_count": len({run["wav_sha256"] for run in runs}),
        "duration_seconds": {
            "minimum": min(durations),
            "maximum": max(durations),
            "population_standard_deviation": statistics.pstdev(durations),
        },
        "waveform_correlation": describe_distribution(waveform_correlations),
        "log_mel_cosine_similarity": describe_distribution(log_mel_similarities),
        "gates": {
            "single_worker_rtf_at_or_below_6_0": worst_rtf <= 6.0,
            "projected_sixty_minutes_at_or_below_21_600_seconds": projected_seconds
            <= 21_600.0,
            "canonical_float_wav": True,
            "offline_namespace": True,
            "listening_review_complete": False,
        },
    }
    result = {
        "schema_version": "1.1-e0-s3-qualification",
        "created_at_epoch_seconds": time.time(),
        "qualification_boundary": "non_product_disposable_adapter",
        "network": network,
        "input": input_identity,
        "voice": {
            key: value
            for key, value in voice_identity.items()
            if key != "conditionals_path"
        },
        "experiment_identity": experiment_identity,
        "worker_identity": worker_identity,
        "tools": {
            "blake3": {
                "executable_sha256": sha256_file(
                    configuration.blake3_executable.resolve(strict=True)
                ),
            },
            "ffmpeg": tool_identity(configuration.ffmpeg_executable),
            "ffprobe": tool_identity(configuration.ffprobe_executable),
            "ffmpeg_conversion": ffmpeg_conversion,
        },
        "runs": runs,
        "cache_preserved_copy": {
            "wav": cache_copy.relative_to(output_root).as_posix(),
            "sha256": sha256_file(cache_copy),
        },
        "listening": listening,
        "summary": summary,
    }
    result_path = output_root / "qualification-result-v1.json"
    atomic_write_json(result_path, result)
    return result_path


def main() -> int:
    """Run qualification and emit only a redacted completion marker."""

    try:
        configuration = parse_arguments()
        result_path = run_qualification(configuration)
    except (QualificationError, OSError, ValueError) as error:
        print(f"qualification failed: {error}", file=sys.stderr)
        return 1
    print(f"qualification complete: {result_path.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
