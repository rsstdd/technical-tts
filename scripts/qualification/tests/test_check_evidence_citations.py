"""The citation checker's accounted-mismatch mechanism.

`check_evidence_citations.py` fails a citation naming a digest no commit of that
file ever had. Some are historical pins of working-tree states that were never
committed and cannot be recovered, in records that must not be superseded merely
to satisfy a validator — supersession in this repository signals that a
conclusion was wrong.

The mechanism under test is the account: an exact `| record-id | path |` row
under `## Accounted citation mismatches` in an **accepted reconciliation
record**, mirroring `scripts/check-evidence-provenance.py`. These tests exist
because such a mechanism is one bad predicate away from being an exemption, so
each case below is a way it could be too permissive.

    python3 -m unittest discover -s scripts/qualification/tests \
        -p 'test_check_evidence_citations.py'
"""

from __future__ import annotations

import hashlib
import importlib.util
import pathlib
import subprocess
import sys
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).parents[1] / "check_evidence_citations.py"
SPEC = importlib.util.spec_from_file_location("check_evidence_citations", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
CITATIONS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CITATIONS)

UNCOMMITTED = "b" * 64


class AccountedCitationTests(unittest.TestCase):
    """The accounting predicate, driven against a scratch evidence tree."""

    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary_directory.name)
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()
        self.previous_evidence = CITATIONS.EVIDENCE
        CITATIONS.EVIDENCE = self.evidence

    def tearDown(self):
        CITATIONS.EVIDENCE = self.previous_evidence
        self.temporary_directory.cleanup()

    def reconciliation(self, name: str, status: str, rows: str) -> None:
        """Writes a reconciliation record carrying `rows` under the heading."""
        (self.evidence / f"{name}.md").write_text(
            f"# Reconciliation\n\n- Status: {status}\n\n"
            f"{CITATIONS.ACCOUNTED_CITATION_HEADING}\n\n{rows}\n",
            encoding="utf-8",
        )

    def test_exact_pair_in_accepted_reconciliation_passes(self):
        self.reconciliation(
            "e0-citation-reconciliation-v1",
            "Accepted",
            "| `report-v1` | `docs/A.md` |",
        )

        self.assertEqual(
            CITATIONS.accounted_citations(), {("report-v1", "docs/A.md")}
        )

    def test_pair_in_unaccepted_reconciliation_fails(self):
        # A proposal grants nothing: this is what makes the row an approval
        # rather than a note somebody wrote.
        for status in ("Proposed", "Pending"):
            with self.subTest(status=status):
                self.reconciliation(
                    "e0-citation-reconciliation-v1",
                    status,
                    "| `report-v1` | `docs/A.md` |",
                )
                self.assertEqual(CITATIONS.accounted_citations(), set())

    def test_a_record_that_is_not_a_reconciliation_grants_nothing(self):
        # The kind is read from the identifier, as the provenance checker reads
        # it. A baseline record carrying the same heading and row is not a
        # reconciliation and authorizes nothing.
        self.reconciliation(
            "e0-provisional-contract-baseline-v1",
            "Accepted",
            "| `report-v1` | `docs/A.md` |",
        )

        self.assertEqual(CITATIONS.accounted_citations(), set())

    def test_wrong_path_still_fails(self):
        self.reconciliation(
            "e0-citation-reconciliation-v1",
            "Accepted",
            "| `report-v1` | `docs/A.md` |",
        )

        accounted = CITATIONS.accounted_citations()

        self.assertIn(("report-v1", "docs/A.md"), accounted)
        self.assertNotIn(("report-v1", "docs/B.md"), accounted)

    def test_wrong_record_still_fails(self):
        self.reconciliation(
            "e0-citation-reconciliation-v1",
            "Accepted",
            "| `report-v1` | `docs/A.md` |",
        )

        self.assertNotIn(("report-v2", "docs/A.md"), CITATIONS.accounted_citations())

    def test_wildcard_accounting_grants_nothing(self):
        # Every shape a reader might expect to broaden a row. None is parsed as
        # a pattern, because nothing here parses patterns: a wildcard row simply
        # authorizes a record or path spelled with an asterisk in it, which no
        # citation is.
        self.reconciliation(
            "e0-citation-reconciliation-v1",
            "Accepted",
            "| `*` | `*` |\n"
            "| `report-v1` | `*` |\n"
            "| `report-v1` | `docs/*` |\n"
            "| `report-v1` | `docs/` |",
        )

        accounted = CITATIONS.accounted_citations()

        for pair in (
            ("report-v1", "docs/A.md"),
            ("report-v1", "docs/B.md"),
            ("report-v2", "docs/A.md"),
        ):
            self.assertNotIn(pair, accounted)

    def test_a_row_outside_the_heading_grants_nothing(self):
        # The provenance checker reads rows only beneath its heading, and a
        # citation row in a table about something else must not travel.
        (self.evidence / "e0-citation-reconciliation-v1.md").write_text(
            "# Reconciliation\n\n- Status: Accepted\n\n"
            "## Some other table\n\n| `report-v1` | `docs/A.md` |\n",
            encoding="utf-8",
        )

        self.assertEqual(CITATIONS.accounted_citations(), set())

    def test_a_provenance_accounting_row_does_not_account_a_citation(self):
        # The two checkers answer different questions — current bytes against
        # git history — so a row written for one must not suppress the other.
        (self.evidence / "e0-citation-reconciliation-v1.md").write_text(
            "# Reconciliation\n\n- Status: Accepted\n\n"
            "## Accounted provenance mismatches\n\n| `report-v1` | `docs/A.md` |\n",
            encoding="utf-8",
        )

        self.assertEqual(CITATIONS.accounted_citations(), set())


class EndToEndCitationTests(unittest.TestCase):
    """The verdict itself, over a scratch repository with real history."""

    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary_directory.name)
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()
        (self.root / "docs").mkdir()
        for name in ("A.md", "B.md"):
            (self.root / "docs" / name).write_text(f"{name} content\n", encoding="utf-8")
        for command in (
            ["git", "init", "-q"],
            ["git", "config", "user.email", "t@example.com"],
            ["git", "config", "user.name", "t"],
            # A developer with `commit.gpgsign` set globally would otherwise be
            # asked to sign this throwaway fixture, and the commit fails with no
            # pinentry. CI has no key and never saw it.
            ["git", "config", "commit.gpgsign", "false"],
            ["git", "add", "-A"],
            ["git", "commit", "-qm", "seed"],
        ):
            subprocess.run(command, cwd=self.root, check=True, capture_output=True)
        self.previous = (CITATIONS.REPO, CITATIONS.EVIDENCE)
        CITATIONS.REPO, CITATIONS.EVIDENCE = self.root, self.evidence

    def tearDown(self):
        CITATIONS.REPO, CITATIONS.EVIDENCE = self.previous
        self.temporary_directory.cleanup()

    def verdict(self) -> int:
        argv = sys.argv
        sys.argv = ["check_evidence_citations.py"]
        try:
            return CITATIONS.main()
        finally:
            sys.argv = argv

    def cite(self, record: str, paths: list[str]) -> None:
        rows = "\n".join(f"| `{path}` | `{UNCOMMITTED}` |" for path in paths)
        (self.evidence / f"{record}.md").write_text(
            f"# Record\n\n- Status: Accepted\n\n{rows}\n", encoding="utf-8"
        )

    def test_unaccounted_missing_citation_fails(self):
        self.cite("report-v1", ["docs/A.md"])

        self.assertEqual(self.verdict(), 1)

    def test_accounted_missing_citation_passes(self):
        self.cite("report-v1", ["docs/A.md"])
        (self.evidence / "e0-citation-reconciliation-v1.md").write_text(
            "# Reconciliation\n\n- Status: Accepted\n\n"
            f"{CITATIONS.ACCOUNTED_CITATION_HEADING}\n\n"
            "| `report-v1` | `docs/A.md` |\n",
            encoding="utf-8",
        )

        self.assertEqual(self.verdict(), 0)

    def test_unrelated_citation_still_fails(self):
        # Accounting one citation must not clear another in the same record.
        self.cite("report-v1", ["docs/A.md", "docs/B.md"])
        (self.evidence / "e0-citation-reconciliation-v1.md").write_text(
            "# Reconciliation\n\n- Status: Accepted\n\n"
            f"{CITATIONS.ACCOUNTED_CITATION_HEADING}\n\n"
            "| `report-v1` | `docs/A.md` |\n",
            encoding="utf-8",
        )

        self.assertEqual(self.verdict(), 1)

    def test_a_matching_citation_needs_no_account(self):
        # The mechanism changes nothing for a citation that verifies.
        digest = hashlib.sha256((self.root / "docs" / "A.md").read_bytes()).hexdigest()
        (self.evidence / "report-v1.md").write_text(
            f"# Record\n\n- Status: Accepted\n\n| `docs/A.md` | `{digest}` |\n",
            encoding="utf-8",
        )

        self.assertEqual(self.verdict(), 0)


if __name__ == "__main__":
    unittest.main()
