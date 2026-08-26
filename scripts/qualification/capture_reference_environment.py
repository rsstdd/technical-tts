#!/usr/bin/env python3
"""Capture and enforce the E0-S3 WSL2 reference-environment inventory."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

REQUIRED_ROOT_LABELS = {
    "repository",
    "qualification_environment",
    "model",
    "voice",
    "cache",
    "jobs",
    "staging",
    "output",
    "raw_evidence",
}


class EnvironmentCaptureError(Exception):
    """Report a failed reference-environment requirement."""


def parse_root(value: str) -> tuple[str, Path]:
    """Parse one NAME=PATH managed-root option."""

    label, separator, raw_path = value.partition("=")
    if not separator or not label or not raw_path:
        raise argparse.ArgumentTypeError("managed roots use NAME=PATH")
    return label, Path(raw_path)


def parse_arguments() -> argparse.Namespace:
    """Parse the reference-environment capture command line."""

    parser = argparse.ArgumentParser(
        description="Capture the E0-S3 WSL2 reference environment as private JSON evidence."
    )
    parser.add_argument("--output-file", required=True, type=Path)
    parser.add_argument("--wsl-executable", required=True, type=Path)
    parser.add_argument("--managed-root", action="append", required=True, type=parse_root)
    arguments = parser.parse_args()
    labels = [label for label, _ in arguments.managed_root]
    if len(labels) != len(set(labels)):
        parser.error("each managed-root label must appear exactly once")
    missing = REQUIRED_ROOT_LABELS - set(labels)
    extra = set(labels) - REQUIRED_ROOT_LABELS
    if missing or extra:
        parser.error("managed-root labels must match the required E0-S3 set")
    return arguments


def run_bytes(arguments: list[str]) -> bytes:
    """Run one checked argv vector and return raw standard output."""

    result = subprocess.run(
        arguments,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if result.returncode != 0:
        executable = Path(arguments[0]).name
        raise EnvironmentCaptureError(f"{executable} failed during environment capture")
    return result.stdout


def decode_command_output(output: bytes) -> str:
    """Decode ordinary UTF-8 tools and Windows UTF-16LE interop output."""

    encoding = "utf-16-le" if b"\x00" in output else "utf-8"
    return output.decode(encoding).replace("\r\n", "\n").strip("\x00\n")


def run_text(arguments: list[str]) -> str:
    """Run one checked argv vector and return normalized text."""

    return decode_command_output(run_bytes(arguments))


def sha256_file(path: Path) -> str:
    """Hash one environment input without loading it entirely into memory."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def reject_symlink_components(path: Path, label: str) -> None:
    """Refuse any existing symbolic-link component in a managed root."""

    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        if current.is_symlink():
            raise EnvironmentCaptureError(f"{label} contains a symbolic-link component")


def inspect_root(label: str, path: Path) -> dict[str, Any]:
    """Canonicalize a managed root and prove its ext4 mount identity."""

    absolute = path.absolute()
    reject_symlink_components(absolute, label)
    try:
        resolved = absolute.resolve(strict=True)
    except OSError as error:
        raise EnvironmentCaptureError(f"{label} root is unavailable") from error
    if not resolved.is_dir():
        raise EnvironmentCaptureError(f"{label} root is not a directory")
    if resolved == Path("/mnt/c") or Path("/mnt/c") in resolved.parents:
        raise EnvironmentCaptureError(f"{label} root resolves through /mnt/c")
    mount = run_text(
        [
            "findmnt",
            "--json",
            "--target",
            str(resolved),
            "--output",
            "TARGET,SOURCE,FSTYPE,OPTIONS",
        ]
    )
    try:
        parsed = json.loads(mount)
        filesystems = parsed["filesystems"]
        filesystem = filesystems[0]
    except (json.JSONDecodeError, KeyError, IndexError, TypeError) as error:
        raise EnvironmentCaptureError(
            f"findmnt returned an invalid record for {label}"
        ) from error
    if filesystem.get("fstype") != "ext4":
        raise EnvironmentCaptureError(
            f"{label} root is not on the qualified ext4 filesystem"
        )
    usage = os.statvfs(resolved)
    return {
        "resolved_path": str(resolved),
        "mount": filesystem,
        "capacity_bytes": usage.f_blocks * usage.f_frsize,
        "available_bytes": usage.f_bavail * usage.f_frsize,
        "symlink_components": False,
        "mnt_c": False,
    }


def parse_lscpu_json(raw: str) -> dict[str, str]:
    """Turn lscpu's field/value list into a searchable map."""

    try:
        rows = json.loads(raw)["lscpu"]
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise EnvironmentCaptureError("lscpu returned malformed JSON") from error
    parsed = {}
    for row in rows:
        field = row.get("field", "").rstrip(":")
        data = row.get("data")
        if field and isinstance(data, str):
            parsed[field] = data
    return parsed


def physical_core_derivation() -> dict[str, Any]:
    """Derive visible physical cores from unique socket/core pairs."""

    raw = run_text(["lscpu", "--parse=socket,core"])
    pairs = {
        tuple(line.split(","))
        for line in raw.splitlines()
        if line and not line.startswith("#")
    }
    logical = os.cpu_count()
    if logical is None or not pairs:
        raise EnvironmentCaptureError(
            "CPU topology did not expose logical and physical cores"
        )
    return {
        "logical_cores": logical,
        "physical_cores": len(pairs),
        "derivation": "unique socket/core pairs from lscpu --parse=socket,core",
        "raw_parse_sha256": hashlib.sha256(raw.encode("utf-8")).hexdigest(),
    }


def memory_inventory() -> dict[str, int]:
    """Read byte totals for RAM and swap from procfs."""

    fields: dict[str, int] = {}
    for line in Path("/proc/meminfo").read_text(encoding="utf-8").splitlines():
        name, separator, remainder = line.partition(":")
        if not separator:
            continue
        parts = remainder.strip().split(maxsplit=1)
        value = parts[0]
        unit = parts[1] if len(parts) == 2 else ""
        multiplier = 1024 if unit == "kB" else 1
        fields[name] = int(value) * multiplier
    required = ("MemTotal", "MemAvailable", "SwapTotal", "SwapFree")
    if any(name not in fields for name in required):
        raise EnvironmentCaptureError(
            "procfs did not expose the required memory fields"
        )
    return {
        "ram_total_bytes": fields["MemTotal"],
        "ram_available_bytes": fields["MemAvailable"],
        "swap_total_bytes": fields["SwapTotal"],
        "swap_free_bytes": fields["SwapFree"],
    }


def version_record(arguments: list[str]) -> dict[str, Any]:
    """Capture a tool's checked argv and complete version output."""

    return {"arguments": arguments[1:], "output": run_text(arguments).splitlines()}


def atomic_write_json(path: Path, value: Any) -> None:
    """Publish the private environment record without replacing prior evidence."""

    absolute = path.absolute()
    if absolute.exists() or absolute.is_symlink():
        raise EnvironmentCaptureError(
            "output file already exists; environment evidence is immutable"
        )
    parent = absolute.parent.resolve(strict=True)
    reject_symlink_components(parent, "environment evidence parent")
    encoded = json.dumps(value, indent=2, sort_keys=True).encode("utf-8") + b"\n"
    handle, temporary_name = tempfile.mkstemp(
        prefix=f".{absolute.name}.", suffix=".tmp", dir=parent
    )
    temporary = Path(temporary_name)
    try:
        with os.fdopen(handle, "wb") as output:
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, absolute)
        directory = os.open(parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        temporary.unlink(missing_ok=True)


def capture_environment(arguments: argparse.Namespace) -> dict[str, Any]:
    """Capture all required host, tool, topology, and managed-root evidence."""

    roots = {
        label: inspect_root(label, path) for label, path in arguments.managed_root
    }
    lscpu_raw = run_text(["lscpu", "--json"])
    lscpu = parse_lscpu_json(lscpu_raw)
    topology = physical_core_derivation()
    machine_material = {
        "cpu_model": lscpu.get("Model name"),
        "kernel": platform.release(),
        "logical_cores": topology["logical_cores"],
        "physical_cores": topology["physical_cores"],
    }
    machine_hash = hashlib.sha256(
        json.dumps(machine_material, sort_keys=True).encode("utf-8")
    ).hexdigest()
    repository = Path(roots["repository"]["resolved_path"])
    qualification_environment = Path(
        roots["qualification_environment"]["resolved_path"]
    )
    toolchain_file = repository / "rust-toolchain.toml"
    environment_python = qualification_environment / "bin/python"
    freeze = repository / "data/models/chatterbox/environment-e0-s2/requirements.freeze.txt"

    return {
        "schema_version": "1.0-e0-s3-reference-environment",
        "captured_at_epoch_seconds": time.time(),
        "machine_identifier": f"reference-wsl2-{machine_hash[:16]}",
        "wsl": version_record([str(arguments.wsl_executable), "--version"]),
        "distribution": version_record(["lsb_release", "-a"]),
        "kernel": version_record(["uname", "-a"]),
        "cpu": {
            "model": lscpu.get("Model name"),
            "vendor": lscpu.get("Vendor ID"),
            "architecture": lscpu.get("Architecture"),
            "topology": topology,
            "lscpu_sha256": hashlib.sha256(lscpu_raw.encode("utf-8")).hexdigest(),
            "lscpu": lscpu,
        },
        "memory": memory_inventory(),
        "roots": roots,
        "tools": {
            "rustc": version_record(["rustc", "-Vv"]),
            "cargo": version_record(["cargo", "-V"]),
            "rust_toolchain_file_sha256": sha256_file(toolchain_file),
            "system_python": version_record(["python3", "-VV"]),
            "qualification_python": version_record([str(environment_python), "-VV"]),
            "qualification_freeze_sha256": sha256_file(freeze),
            "gcc": version_record(["gcc", "--version"]),
            "cmake": version_record(["cmake", "--version"]),
            "ffmpeg": version_record(["ffmpeg", "-version"]),
            "ffprobe": version_record(["ffprobe", "-version"]),
        },
    }


def main() -> int:
    """Capture the environment and print only a redacted completion marker."""

    try:
        arguments = parse_arguments()
        record = capture_environment(arguments)
        atomic_write_json(arguments.output_file, record)
    except (EnvironmentCaptureError, OSError, ValueError) as error:
        print(f"environment capture failed: {error}", file=sys.stderr)
        return 1
    print(f"environment capture complete: {arguments.output_file.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
