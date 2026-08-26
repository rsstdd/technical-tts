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


def read_riff_chunks(path: Path) -> list[dict[str, Any]]:
    """Read and hash every RIFF chunk in one qualification WAV."""

    raw = path.read_bytes()
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
    samples, sample_rate = soundfile_module.read(
        path, dtype="float32", always_2d=True
    )
    if sample_rate != 24_000 or samples.shape[1] != 1:
        raise AnalysisError("qualification WAV has an unexpected media format")
    if not numpy_module.isfinite(samples).all():
        raise AnalysisError("qualification WAV contains a non-finite sample")
    sample_bytes = samples.astype("<f4", copy=False).tobytes(order="C")
    return {
        "name": path.name,
        "container_sha256": sha256_bytes(path.read_bytes()),
        "decoded_pcm_sha256": sha256_bytes(sample_bytes),
        "frames": int(samples.shape[0]),
        "chunks": read_riff_chunks(path),
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


def analyze(arguments: argparse.Namespace) -> dict[str, Any]:
    """Analyze the complete expected run set against run one."""

    input_root = arguments.input_root.resolve(strict=True)
    if not input_root.is_dir() or input_root.is_symlink():
        raise AnalysisError("input root must be a non-symlink directory")
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
