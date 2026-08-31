from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
CHECKER_PATH = REPOSITORY_ROOT / "scripts/qualification/check_listening_review.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("check_listening_review", CHECKER_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("listening review checker could not be imported")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


checker = load_checker()


class ListeningReviewTests(unittest.TestCase):
    """A review is acceptable only when a human finished it against these bytes."""

    def setUp(self) -> None:
        self._workspace = tempfile.TemporaryDirectory()
        self.addCleanup(self._workspace.cleanup)
        self.root = Path(self._workspace.name)
        self.audio = {}
        for index in (1, 2):
            blind_id = f"sample-{index:02}"
            path = self.root / f"{blind_id}.wav"
            path.write_bytes(f"synthetic audio {index}".encode())
            self.audio[blind_id] = hashlib.sha256(path.read_bytes()).hexdigest()

    def sheet(self, **overrides: object) -> dict:
        """A completed sheet, before a test spoils one field."""
        document = {
            "schema_version": checker.REVIEW_SHEET_SCHEMA,
            "status": "reviewed",
            "reviewer": "Ross Todd",
            "playback_environment": "laptop built-in speakers",
            "reviewed_at": "2026-08-31",
            "samples": [
                {
                    "blind_id": blind_id,
                    "wav": f"{blind_id}.wav",
                    "sha256": digest,
                    "findings": dict.fromkeys(checker.REVIEW_CRITERIA, "none"),
                    "disposition": "accept",
                }
                for blind_id, digest in sorted(self.audio.items())
            ],
            "overall_finding": "accepted",
        }
        document.update(overrides)
        return document

    def key(self, **overrides: object) -> dict:
        """The mapping that sheet was blind to."""
        document = {
            "schema_version": checker.RANDOMIZATION_KEY_SCHEMA,
            "mapping": [
                {"blind_id": blind_id, "line_id": f"line-{index:02}", "sha256": digest}
                for index, (blind_id, digest) in enumerate(sorted(self.audio.items()), start=1)
            ],
        }
        document.update(overrides)
        return document

    def write(self, sheet: dict | None = None, key: dict | None = None) -> Path:
        (self.root / "review-sheet.json").write_text(
            json.dumps(sheet if sheet is not None else self.sheet()), encoding="utf-8"
        )
        (self.root / "randomization-key.json").write_text(
            json.dumps(key if key is not None else self.key()), encoding="utf-8"
        )
        return self.root

    def test_a_completed_review_reveals_its_mapping(self) -> None:
        revealed = checker.verify(self.write())

        self.assertEqual(revealed, {"sample-01": "line-01", "sample-02": "line-02"})

    def test_a_pending_sheet_is_refused(self) -> None:
        # What the renderer writes. Refusing this is the whole point: the
        # instrument records listening as pending precisely so a human has to
        # answer it.
        pending = self.sheet()
        pending["samples"][0]["findings"]["pronunciation"] = None

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(pending))

        self.assertIn("pronunciation", str(refused.exception))

    def test_a_sample_without_a_disposition_is_refused(self) -> None:
        undecided = self.sheet()
        undecided["samples"][1]["disposition"] = None

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(undecided))

        self.assertIn("sample-02", str(refused.exception))

    def test_a_rejection_is_as_complete_as_an_acceptance(self) -> None:
        # The checker refuses an unanswered sample, never an unfavourable one.
        # A gate that only passed on approval would pressure the answer.
        rejected = self.sheet()
        rejected["samples"][0]["findings"]["noise_or_artifacts"] = "a click at 0:02"
        rejected["samples"][0]["disposition"] = "reject"

        self.assertEqual(len(checker.verify(self.write(rejected))), 2)

    def test_an_unattributed_review_is_refused(self) -> None:
        for field in checker.ATTRIBUTION_FIELDS:
            with self.subTest(field=field):
                with self.assertRaises(checker.ReviewError) as refused:
                    checker.verify(self.write(self.sheet(**{field: None})))

                self.assertIn(field.replace("_", " "), str(refused.exception))

    def test_audio_that_changed_since_the_review_is_refused(self) -> None:
        # The binding that matters. A retake rendering new audio into the same
        # directory would otherwise inherit the previous judgment, which is the
        # failure this instrument exists to stop.
        root = self.write()
        (root / "sample-01.wav").write_bytes(b"different audio entirely")

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(root)

        self.assertIn("no longer holds", str(refused.exception))

    def test_missing_audio_is_refused(self) -> None:
        root = self.write()
        (root / "sample-02.wav").unlink()

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(root)

        self.assertIn("sample-02", str(refused.exception))

    def test_audio_paths_must_stay_beneath_the_listening_root(self) -> None:
        outside_workspace = tempfile.TemporaryDirectory()
        self.addCleanup(outside_workspace.cleanup)
        outside = Path(outside_workspace.name) / "outside.wav"
        outside.write_bytes(b"outside audio")
        (self.root / "linked.wav").symlink_to(outside)

        for name in (str(outside), "../outside.wav", "linked.wav"):
            with self.subTest(name=name):
                sheet = self.sheet()
                sheet["samples"][0]["wav"] = name

                with self.assertRaises(checker.ReviewError) as refused:
                    checker.verify(self.write(sheet))

                self.assertIn("listening root", str(refused.exception))

    def test_reviewed_sample_ids_must_be_nonempty_and_unique(self) -> None:
        for blind_ids in (("", "sample-02"), ("sample-01", "sample-01")):
            with self.subTest(blind_ids=blind_ids):
                sheet = self.sheet()
                for sample, blind_id in zip(sheet["samples"], blind_ids, strict=True):
                    sample["blind_id"] = blind_id

                with self.assertRaises(checker.ReviewError) as refused:
                    checker.verify(self.write(sheet))

                self.assertIn("blind ID", str(refused.exception))

    def test_mapping_ids_must_be_nonempty_and_unique(self) -> None:
        for blind_ids in (("", "sample-02"), ("sample-01", "sample-01")):
            with self.subTest(blind_ids=blind_ids):
                key = self.key()
                for entry, blind_id in zip(key["mapping"], blind_ids, strict=True):
                    entry["blind_id"] = blind_id

                with self.assertRaises(checker.ReviewError) as refused:
                    checker.verify(self.write(key=key))

                self.assertIn("blind ID", str(refused.exception))

    def test_a_key_that_does_not_cover_the_sheet_is_refused(self) -> None:
        short = self.key()
        short["mapping"] = short["mapping"][:1]

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(key=short))

        self.assertIn("sample-02", str(refused.exception))

    def test_a_key_naming_samples_the_sheet_never_reviewed_is_refused(self) -> None:
        surplus = self.key()
        surplus["mapping"].append(
            {"blind_id": "sample-03", "line_id": "line-03", "sha256": "0" * 64}
        )

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(key=surplus))

        self.assertIn("sample-03", str(refused.exception))

    def test_a_key_disagreeing_about_bytes_is_refused(self) -> None:
        disagreeing = self.key()
        disagreeing["mapping"][0]["sha256"] = "f" * 64

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(key=disagreeing))

        self.assertIn("disagree", str(refused.exception))

    def test_a_sheet_of_another_layout_is_refused(self) -> None:
        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(self.write(self.sheet(schema_version="9.9-something-else")))

        self.assertIn(checker.REVIEW_SHEET_SCHEMA, str(refused.exception))

    def test_completeness_is_reported_before_a_digest_complaint(self) -> None:
        # A reviewer who has not finished is told that, rather than handed a
        # checksum complaint about a sheet they were still filling in.
        unfinished = self.sheet()
        unfinished["samples"][0]["findings"]["pacing"] = None
        root = self.write(unfinished)
        (root / "sample-01.wav").write_bytes(b"changed as well")

        with self.assertRaises(checker.ReviewError) as refused:
            checker.verify(root)

        self.assertIn("pacing", str(refused.exception))


if __name__ == "__main__":
    unittest.main()
