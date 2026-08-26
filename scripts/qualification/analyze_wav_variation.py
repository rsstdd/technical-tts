#!/usr/bin/env python3
"""Explain fixed-seed WAV byte variation without exposing private audio."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import struct
import sys
import tempfile
from pathlib import Path
from typing import Any

SAMPLE_RATE_HZ = 24_000
MAX_WAV_DURATION_SECONDS = 60
MAX_WAV_FRAMES = SAMPLE_RATE_HZ * MAX_WAV_DURATION_SECONDS
MAX_WAV_BYTES = MAX_WAV_FRAMES * 4 + 1024 * 1024


class AnalysisError(Exception):
    """Report an invalid or incomplete fixed-seed artifact set."""


def parse_arguments() -> argparse.Namespace:
    """Parse the private WAV-analysis command line."""

    parser = argparse.ArgumentParser(
        description="Separate RIFF metadata variation from decoded PCM variation."
    )
    parser.add_argument("--input-root", required=True, type=Path)
    parser.add_argument("--output-file", required=True, type=Path)
    parser.add_argument("--expected-count", required=True, type=int)
    arguments = parser.parse_args()
    if arguments.expected_count <= 0:
        parser.error("--expected-count must be greater than zero")
    return arguments


def sha256_bytes(value: bytes) -> str:
    """Hash one bounded byte sequence."""

    return hashlib.sha256(value).hexdigest()


def read_bounded_wav(path: Path) -> bytes:
    """Read one WAV only after enforcing its configured file-size limit."""

    if path.stat().st_size > MAX_WAV_BYTES:
        raise AnalysisError("qualification WAV exceeds the configured input limits")
    raw = path.read_bytes()
    if len(raw) > MAX_WAV_BYTES:
        raise AnalysisError("qualification WAV exceeds the configured input limits")
    return raw


def read_riff_chunks(path: Path) -> list[dict[str, Any]]:
    """Read and hash every RIFF chunk in one qualification WAV."""

    raw = read_bounded_wav(path)
    if len(raw) < 12 or raw[:4] != b"RIFF" or raw[8:12] != b"WAVE":
        raise AnalysisError("qualification audio is not a RIFF/WAVE file")
    declared_size = struct.unpack_from("<I", raw, 4)[0] + 8
    if declared_size != len(raw):
        raise AnalysisError("qualification WAV RIFF length is inconsistent")
    chunks = []
    offset = 12
    while offset < len(raw):
        if offset + 8 > len(raw):
            raise AnalysisError("qualification WAV has a truncated chunk header")
        chunk_id = raw[offset : offset + 4].decode("ascii", errors="strict")
        length = struct.unpack_from("<I", raw, offset + 4)[0]
        payload_start = offset + 8
        payload_end = payload_start + length
        if payload_end > len(raw):
            raise AnalysisError("qualification WAV has a truncated chunk payload")
        payload = raw[payload_start:payload_end]
        chunks.append(
            {
                "id": chunk_id,
                "bytes": length,
                "sha256": sha256_bytes(payload),
            }
        )
        offset = payload_end + (length % 2)
    return chunks


def analyze_wav(path: Path, soundfile_module: Any, numpy_module: Any) -> dict[str, Any]:
    """Hash the container, RIFF chunks, and decoded little-endian float samples."""

    if path.is_symlink() or not path.is_file():
        raise AnalysisError("qualification WAV must be a regular file")
    if path.stat().st_size > MAX_WAV_BYTES:
        raise AnalysisError("qualification WAV exceeds the configured input limits")
    info = soundfile_module.info(path)
    if info.format != "WAV" or info.subtype != "FLOAT":
        raise AnalysisError("qualification WAV has an unexpected media format")
    if (
        info.frames > MAX_WAV_FRAMES
        or info.duration > MAX_WAV_DURATION_SECONDS
    ):
        raise AnalysisError("qualification WAV exceeds the configured input limits")
    chunks = read_riff_chunks(path)
    samples, sample_rate = soundfile_module.read(
        path, dtype="float32", always_2d=True
    )
    if sample_rate != SAMPLE_RATE_HZ or samples.shape[1] != 1:
        raise AnalysisError("qualification WAV has an unexpected media format")
    if samples.shape[0] > MAX_WAV_FRAMES:
        raise AnalysisError("qualification WAV exceeds the configured input limits")
    if not numpy_module.isfinite(samples).all():
        raise AnalysisError("qualification WAV contains a non-finite sample")
    sample_bytes = samples.astype("<f4", copy=False).tobytes(order="C")
    return {
        "name": path.name,
        "container_sha256": sha256_bytes(read_bounded_wav(path)),
        "decoded_pcm_sha256": sha256_bytes(sample_bytes),
        "frames": int(samples.shape[0]),
        "chunks": chunks,
    }


def atomic_write_json(path: Path, value: Any) -> None:
    """Publish one immutable variation report."""

    absolute = path.absolute()
    if absolute.exists() or absolute.is_symlink():
        raise AnalysisError("variation report already exists")
    parent = absolute.parent.resolve(strict=True)
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


def reject_symlink_components(path: Path) -> None:
    """Refuse any existing symbolic-link component in the input root."""

    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        if current.is_symlink():
            raise AnalysisError("input root must be a non-symlink directory")


def analyze(arguments: argparse.Namespace) -> dict[str, Any]:
    """Analyze the complete expected run set against run one."""

    supplied_input_root = arguments.input_root.absolute()
    reject_symlink_components(supplied_input_root)
    if not supplied_input_root.is_dir():
        raise AnalysisError("input root must be a non-symlink directory")
    try:
        input_root = supplied_input_root.resolve(strict=True)
    except OSError as error:
        raise AnalysisError("input root must be a non-symlink directory") from error
    paths = sorted(input_root.glob("run-*.wav"))
    if len(paths) != arguments.expected_count:
        raise AnalysisError("fixed-seed WAV count does not match the expected run count")

    import numpy as np
    import soundfile as sf

    runs = [analyze_wav(path, sf, np) for path in paths]
    reference_chunks = {chunk["id"]: chunk["sha256"] for chunk in runs[0]["chunks"]}
    for run in runs:
        run["chunks_differing_from_run_one"] = [
            chunk["id"]
            for chunk in run["chunks"]
            if reference_chunks.get(chunk["id"]) != chunk["sha256"]
        ]
    container_hashes = {run["container_sha256"] for run in runs}
    pcm_hashes = {run["decoded_pcm_sha256"] for run in runs}
    data_hashes = {
        chunk["sha256"]
        for run in runs
        for chunk in run["chunks"]
        if chunk["id"] == "data"
    }
    differing_chunks = sorted(
        {
            chunk
            for run in runs
            for chunk in run["chunks_differing_from_run_one"]
        }
    )
    return {
        "schema_version": "1.0-e0-s3-wav-variation",
        "runs": runs,
        "summary": {
            "run_count": len(runs),
            "unique_container_sha256_count": len(container_hashes),
            "unique_decoded_pcm_sha256_count": len(pcm_hashes),
            "unique_data_chunk_sha256_count": len(data_hashes),
            "chunks_differing_from_run_one": differing_chunks,
            "variation_is_container_metadata_only": len(container_hashes) > 1
            and len(pcm_hashes) == 1
            and len(data_hashes) == 1,
        },
    }


def main() -> int:
    """Run the analysis and print only a redacted completion marker."""

    try:
        arguments = parse_arguments()
        report = analyze(arguments)
        atomic_write_json(arguments.output_file, report)
    except (AnalysisError, OSError, UnicodeError, ValueError) as error:
        print(f"variation analysis failed: {error}", file=sys.stderr)
        return 1
    print(f"variation analysis complete: {arguments.output_file.name}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
