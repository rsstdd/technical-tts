#!/usr/bin/env python3
"""Verify a completed listening review, then reveal what it was blind to.

The listening set is produced by the `listening-render` example, which writes
`review-sheet.json` pending and `randomization-key.json` beside it. This is the
sanctioned way to open the key: it refuses while the sheet is incomplete, and it
refuses when the sheet's digests no longer describe the audio on disk.

**What is and is not enforced.** Nothing here can stop an operator reading
`randomization-key.json` with `cat`, and claiming otherwise would be theatre.
The blinding is a discipline. What *is* mechanical is the half that matters at
acceptance: a judgment recorded against a digest that no longer matches its file
is a judgment about audio nobody can produce any more, and this refuses it.

    python3 scripts/qualification/check_listening_review.py <listening directory>

Exit `0` prints the mapping. Any refusal exits `1` and names what to fix.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

REVIEW_SHEET_SCHEMA = "1.0-e1-s3-listening-review"
RANDOMIZATION_KEY_SCHEMA = "1.0-e1-s3-randomization-key"

# The five the E1-S3 story record's §Review result tabulates, in its order and
# spelling. `REVIEW_CRITERIA` in
# `crates/study-tts-testkit/examples/listening-render.rs` writes them and names
# this constant in return; a sheet the renderer wrote can only disagree with
# this list if one end moved without the other.
REVIEW_CRITERIA = (
    "omissions_or_additions",
    "pronunciation",
    "voice_consistency",
    "pacing",
    "noise_or_artifacts",
)

# A reviewer's verdict on one sample. `reject` is as complete an answer as
# `accept`: what the checker refuses is an *unanswered* sample, never an
# unfavourable one.
DISPOSITIONS = ("accept", "reject")

# Fields naming who reviewed, where, and when. A judgment with no listener and
# no playback environment cannot be read months later, and ADR-0001 §17.5 makes
# the review a gate condition rather than a note.
ATTRIBUTION_FIELDS = ("reviewer", "playback_environment", "reviewed_at")


class ReviewError(Exception):
    """Report a listening review that cannot be accepted as it stands."""


def parse_arguments() -> argparse.Namespace:
    """Parse the listening-review command line."""

    parser = argparse.ArgumentParser(
        description="Verify a completed listening review and reveal its randomization key."
    )
    parser.add_argument(
        "listening_root",
        type=Path,
        help="the listening directory holding review-sheet.json and the samples",
    )
    return parser.parse_args()


def read_json(path: Path, what: str) -> Any:
    """Read one JSON document, naming what it was for if it cannot be read."""

    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as error:
        raise ReviewError(f"the {what} is missing at {path.name}") from error
    except json.JSONDecodeError as error:
        raise ReviewError(f"the {what} is not readable as JSON: {error}") from error


def sha256_file(path: Path) -> str:
    """SHA-256 of a file, in the spelling every digest in this project uses."""

    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def check_sheet_is_complete(sheet: dict[str, Any]) -> None:
    """Refuse a sheet a human has not finished.

    Every criterion on every sample, a disposition on every sample, and the
    three attribution fields. A blank is not a finding of "nothing heard": the
    renderer writes `null`, and `none` is what a reviewer writes to say they
    listened and heard nothing.

    Raises:
        ReviewError: naming the first thing left unanswered.
    """
    if sheet.get("schema_version") != REVIEW_SHEET_SCHEMA:
        raise ReviewError(
            f"the review sheet declares {sheet.get('schema_version')!r}, "
            f"not {REVIEW_SHEET_SCHEMA!r}"
        )
    for field in ATTRIBUTION_FIELDS:
        if not sheet.get(field):
            raise ReviewError(f"the review sheet states no {field.replace('_', ' ')}")

    samples = sheet.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ReviewError("the review sheet names no samples")

    for sample in samples:
        blind_id = sample.get("blind_id", "an unnamed sample")
        findings = sample.get("findings")
        if not isinstance(findings, dict):
            raise ReviewError(f"{blind_id} records no findings")
        for criterion in REVIEW_CRITERIA:
            if criterion not in findings:
                raise ReviewError(f"{blind_id} has no {criterion} criterion at all")
            if findings[criterion] is None:
                raise ReviewError(f"{blind_id} leaves {criterion} unanswered")
        if sample.get("disposition") not in DISPOSITIONS:
            raise ReviewError(
                f"{blind_id} records no disposition; one of {', '.join(DISPOSITIONS)} is required"
            )
    if not sheet.get("overall_finding"):
        raise ReviewError("the review sheet states no overall finding")


def check_sheet_matches_audio(listening_root: Path, sheet: dict[str, Any]) -> None:
    """Refuse a sheet whose digests no longer describe the audio beside it.

    This is what binds a judgment to bytes rather than to a filename. A retake
    that rendered new audio into the same directory would otherwise inherit the
    previous review, which is the failure this whole instrument exists to stop.

    Raises:
        ReviewError: naming the first sample that has moved or gone.
    """
    for sample in sheet["samples"]:
        blind_id = sample.get("blind_id", "an unnamed sample")
        name = sample.get("wav")
        recorded = sample.get("sha256")
        if not isinstance(name, str) or not isinstance(recorded, str):
            raise ReviewError(f"{blind_id} names no audio file and digest")
        audio = listening_root / name
        if not audio.is_file():
            raise ReviewError(f"{blind_id} names {name}, which is not beside the sheet")
        actual = sha256_file(audio)
        if actual != recorded:
            raise ReviewError(
                f"{blind_id} was reviewed against {recorded[:12]}… but {name} now hashes "
                f"{actual[:12]}…; the judgment describes audio this directory no longer holds"
            )


def check_key_covers_sheet(sheet: dict[str, Any], key: dict[str, Any]) -> dict[str, str]:
    """Refuse a key that does not describe exactly this sheet.

    Returns the blind-id to line-id mapping.

    Raises:
        ReviewError: if the two records describe different sets of samples.
    """
    if key.get("schema_version") != RANDOMIZATION_KEY_SCHEMA:
        raise ReviewError(
            f"the randomization key declares {key.get('schema_version')!r}, "
            f"not {RANDOMIZATION_KEY_SCHEMA!r}"
        )
    mapping = key.get("mapping")
    if not isinstance(mapping, list):
        raise ReviewError("the randomization key records no mapping")

    by_blind_id = {entry.get("blind_id"): entry for entry in mapping}
    revealed = {}
    for sample in sheet["samples"]:
        blind_id = sample["blind_id"]
        entry = by_blind_id.get(blind_id)
        if entry is None:
            raise ReviewError(f"the randomization key does not name {blind_id}")
        if entry.get("sha256") != sample.get("sha256"):
            raise ReviewError(
                f"the randomization key and the review sheet disagree about {blind_id}'s bytes"
            )
        revealed[blind_id] = entry.get("line_id", "an unnamed line")
    surplus = sorted(set(by_blind_id) - {sample["blind_id"] for sample in sheet["samples"]})
    if surplus:
        raise ReviewError(
            f"the randomization key names samples the sheet does not review: {', '.join(surplus)}"
        )
    return revealed


def verify(listening_root: Path) -> dict[str, str]:
    """Run every check in order and return the revealed mapping.

    Completeness before digests before the key, so a reviewer who has not
    finished is told that rather than being handed a checksum complaint about a
    sheet they were still filling in.
    """
    sheet = read_json(listening_root / "review-sheet.json", "review sheet")
    check_sheet_is_complete(sheet)
    check_sheet_matches_audio(listening_root, sheet)
    key = read_json(listening_root / "randomization-key.json", "randomization key")
    return check_key_covers_sheet(sheet, key)


def main() -> int:
    """Verify the review and print the mapping it was blind to."""

    arguments = parse_arguments()
    try:
        revealed = verify(arguments.listening_root)
    except ReviewError as error:
        print(f"listening review not acceptable: {error}", file=sys.stderr)
        return 1

    print("listening review complete and bound to the audio beside it.\n")
    for blind_id, line_id in sorted(revealed.items()):
        print(f"  {blind_id} -> {line_id}")
    print(
        "\nTranscribe the findings into the story record's §Review result, and cite the sheet "
        "by its SHA-256."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
