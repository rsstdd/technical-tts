import hashlib
import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).parents[1] / "check-evidence-provenance.py"
SPEC = importlib.util.spec_from_file_location("check_evidence_provenance", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
PROVENANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROVENANCE)


class EvidenceProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary_directory.name)
        self.evidence = self.root / "evidence"
        self.evidence.mkdir()

    def tearDown(self):
        self.temporary_directory.cleanup()

    def write(self, relative: str, content: str) -> pathlib.Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
        return path

    def accepted_record(self, name: str, cited: str, pinned: str) -> pathlib.Path:
        return self.write(
            f"evidence/{name}.md",
            f"# Evidence\n\n- Status: Accepted\n\n| Record | SHA-256 |\n"
            f"|---|---|\n| `{cited}` | `{pinned}` |\n",
        )

    def test_a_proposed_record_cannot_supersede_an_accepted_record(self):
        accepted = self.write(
            "evidence/baseline-v1.md",
            "# Baseline\n\n- Status: Accepted\n",
        )
        proposed = self.write(
            "evidence/baseline-v2.md",
            "# Baseline\n\n- Status: Proposed\n"
            "- Supersedes: `baseline-v1`\n",
        )

        active = PROVENANCE.active_accepted_records([accepted, proposed])

        self.assertEqual([PROVENANCE.record_id(record) for record in active], ["baseline-v1"])

    def test_supersession_requires_explicit_metadata(self):
        predecessor = self.write(
            "evidence/baseline-v1.md",
            "# Baseline\n\n- Status: Accepted\n",
        )
        prose_only = self.write(
            "evidence/baseline-v2.md",
            "# Baseline\n\n- Status: Accepted\n\n"
            "This record supersedes `baseline-v1`.\n",
        )

        active = PROVENANCE.active_accepted_records([predecessor, prose_only])

        self.assertIn(predecessor, active)

    def test_an_accepted_explicit_supersession_replaces_its_predecessor(self):
        predecessor = self.write(
            "evidence/baseline-v1.md",
            "# Baseline\n\n- Status: Accepted\n",
        )
        successor = self.write(
            "evidence/baseline-v2.md",
            "# Baseline\n\n- Status: Accepted\n"
            "- Supersedes: `baseline-v1`\n",
        )

        active = PROVENANCE.active_accepted_records([predecessor, successor])

        self.assertNotIn(predecessor, active)
        self.assertIn(successor, active)

    def test_a_proposed_reconciliation_cannot_suppress_a_mismatch(self):
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.accepted_record("baseline-v1", "docs/control.md", pinned)
        self.write(
            "evidence/reconciliation-v1.md",
            "# Reconciliation\n\n- Status: Proposed\n\n"
            "## Accounted provenance mismatches\n\n"
            "| Citing record | Cited repository path |\n"
            "|---|---|\n"
            "| `baseline-v1` | `docs/control.md` |\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 1)
        self.assertIn("docs/control.md", found[0])

    def test_an_accepted_reconciliation_covers_only_the_named_mismatch(self):
        self.write("docs/first.md", "current")
        self.write("docs/second.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.accepted_record("first-v1", "docs/first.md", pinned)
        self.accepted_record("second-v1", "docs/second.md", pinned)
        self.write(
            "evidence/reconciliation-v1.md",
            "# Reconciliation\n\n- Status: Accepted\n\n"
            "## Accounted provenance mismatches\n\n"
            "| Citing record | Cited repository path |\n"
            "|---|---|\n"
            "| `first-v1` | `docs/first.md` |\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 1)
        self.assertIn("docs/second.md", found[0])

    def test_an_ordinary_accepted_record_cannot_suppress_a_mismatch(self):
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.accepted_record("baseline-v1", "docs/control.md", pinned)
        self.write(
            "evidence/baseline-v2.md",
            "# Baseline\n\n- Status: Accepted\n\n"
            "## Accounted provenance mismatches\n\n"
            "| Citing record | Cited repository path |\n"
            "|---|---|\n"
            "| `baseline-v1` | `docs/control.md` |\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 1)
        self.assertIn("docs/control.md", found[0])

    def test_a_missing_repository_citation_is_a_violation(self):
        pinned = hashlib.sha256(b"missing").hexdigest()
        self.accepted_record("baseline-v1", "docs/missing.md", pinned)

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 1)
        self.assertIn("cited file `docs/missing.md` does not exist", found[0])

    def test_a_completed_legacy_review_is_accepted(self):
        accepted = self.write(
            "evidence/legacy.md",
            "# Legacy\n\n## Review\n\n"
            "| Role | Name | Decision | Date |\n"
            "|---|---|---|---|\n"
            "| Project owner (approver) | Ross | Approved | 2026-08-28 |\n"
            "| Reviewer | Ross | Scope reviewed | 2026-08-28 |\n",
        )
        pending = self.write(
            "evidence/pending.md",
            "# Pending\n\n## Review\n\n"
            "| Role | Name | Decision | Date |\n"
            "|---|---|---|---|\n"
            "| Contract owner | Ross | Approved | 2026-08-28 |\n"
            "| Runtime owner | | Pending review | |\n",
        )

        self.assertTrue(PROVENANCE.is_accepted(accepted))
        self.assertFalse(PROVENANCE.is_accepted(pending))

    def test_a_record_declaring_no_status_is_still_checked(self):
        # Acceptance gates who may grant something, not who is checked: an
        # undeclared record that skipped the check would take its stale pins
        # with it, silently.
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.write(
            "evidence/undeclared-v1.md",
            f"# Undeclared\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{pinned}` |\n",
        )
        self.accepted_record("anchor-v1", "docs/control.md", pinned)

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 2)
        self.assertTrue(any("undeclared-v1" in message for message in found))

    def test_an_accepted_record_may_declare_a_prose_supersession(self):
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.write(
            "evidence/legacy-v1.md",
            f"# Legacy\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{pinned}` |\n",
        )
        self.write(
            "evidence/reconciliation-v1.md",
            "# Reconciliation\n\n- Status: Accepted\n\n"
            "## Superseded without supersession metadata\n\n"
            "| Superseded record | Superseded by |\n"
            "|---|---|\n"
            "| `legacy-v1` | `something-v2` |\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(found, [])

    def test_a_superseded_record_cannot_declare_a_prose_supersession(self):
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.write(
            "evidence/legacy-v1.md",
            f"# Legacy\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{pinned}` |\n",
        )
        self.write(
            "evidence/reconciliation-v1.md",
            "# Reconciliation\n\n- Status: Accepted\n\n"
            "## Superseded without supersession metadata\n\n"
            "| Superseded record | Superseded by |\n"
            "|---|---|\n"
            "| `legacy-v1` | `something-v2` |\n",
        )
        self.write(
            "evidence/reconciliation-v2.md",
            "# Reconciliation\n\n- Status: Accepted\n"
            "- Supersedes: `reconciliation-v1`\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(len(found), 1)
        self.assertIn("legacy-v1", found[0])
        self.assertIn("docs/control.md", found[0])

    def test_a_proposed_record_cannot_declare_a_prose_supersession(self):
        self.write("docs/control.md", "current")
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.write(
            "evidence/legacy-v1.md",
            f"# Legacy\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{pinned}` |\n",
        )
        self.accepted_record("anchor-v1", "docs/control.md", pinned)
        self.write(
            "evidence/reconciliation-v1.md",
            "# Reconciliation\n\n- Status: Proposed\n\n"
            "## Superseded without supersession metadata\n\n"
            "| Superseded record | Superseded by |\n"
            "|---|---|\n"
            "| `legacy-v1` | `something-v2` |\n",
        )

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertTrue(any("legacy-v1" in message for message in found))

    def test_the_policy_readme_is_not_checked_as_a_record(self):
        pinned = hashlib.sha256(b"previous").hexdigest()
        self.write(
            "evidence/README.md",
            f"# Evidence\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/missing.md` | `{pinned}` |\n",
        )
        self.accepted_record("anchor-v1", "docs/anchor.md", pinned)
        self.write("docs/anchor.md", "previous")

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(found, [])


    def test_a_proposed_record_is_not_checked(self):
        # `evidence/README.md` §Provenance: a proposal is not in force, so its
        # pins bind nothing yet. Only an explicit `Proposed` earns this; a
        # record declaring no status stays checked, per the test above.
        self.write("docs/control.md", "current")
        current = hashlib.sha256(b"current").hexdigest()
        self.write(
            "evidence/draft-v1.md",
            f"# Draft\n\n- Status: Proposed\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{hashlib.sha256(b'previous').hexdigest()}` |\n",
        )
        self.accepted_record("anchor-v1", "docs/control.md", current)

        found = PROVENANCE.check(self.root, self.evidence)

        self.assertEqual(found, [])

    def test_repin_rewrites_the_named_proposed_record_from_current_bytes(self):
        control = self.write("docs/control.md", "current")
        stale = hashlib.sha256(b"previous").hexdigest()
        draft = self.write(
            "evidence/draft-v1.md",
            f"# Draft\n\n- Status: Proposed\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{stale}` |\n",
        )

        self.assertIsNone(PROVENANCE.repin_refusal(draft, self.evidence))
        self.assertTrue(PROVENANCE.repin(draft, self.root))
        self.assertIn(PROVENANCE.digest(control), draft.read_text(encoding="utf-8"))
        self.assertNotIn(stale, draft.read_text(encoding="utf-8"))

    def test_repin_refuses_an_accepted_record(self):
        # `evidence/README.md`: never overwrite an accepted report. Re-pinning
        # one would rewrite the bytes an approver signed off against.
        self.write("docs/control.md", "current")
        accepted = self.accepted_record(
            "anchor-v1", "docs/control.md", hashlib.sha256(b"previous").hexdigest()
        )

        self.assertIn("`Accepted`", PROVENANCE.repin_refusal(accepted, self.evidence))

    def test_repin_refuses_a_record_declaring_no_status(self):
        self.write("docs/control.md", "current")
        legacy = self.write(
            "evidence/legacy-v1.md",
            f"# Legacy\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/control.md` | `{hashlib.sha256(b'previous').hexdigest()}` |\n",
        )

        self.assertIn("no status", PROVENANCE.repin_refusal(legacy, self.evidence))

    def test_repin_refuses_a_superseded_record(self):
        # A superseded record pins what it measured; that is what supersession
        # is for. It is out of the check, and equally out of any rewrite.
        retired = self.write(
            "evidence/draft-v1.md",
            "# Draft\n\n- Status: Proposed\n",
        )
        self.write(
            "evidence/draft-v2.md",
            "# Draft\n\n- Status: Accepted\n- Supersedes: `draft-v1`\n",
        )

        self.assertIn("superseded", PROVENANCE.repin_refusal(retired, self.evidence))

    def test_repin_refuses_a_path_outside_the_evidence_tree(self):
        # `--write` rewrites whatever it is handed, so the containment lives in
        # the guard rather than in the caller's care.
        outside = self.write(
            "docs/not-a-record.md",
            f"# Doc\n\n- Status: Proposed\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/not-a-record.md` | `{hashlib.sha256(b'x').hexdigest()}` |\n",
        )

        self.assertIn("outside", PROVENANCE.repin_refusal(outside, self.evidence))

    def test_repin_refuses_a_record_that_does_not_exist(self):
        missing = self.evidence / "absent-v1.md"

        self.assertIn("does not exist", PROVENANCE.repin_refusal(missing, self.evidence))

    def test_repin_leaves_a_row_citing_two_paths_to_its_author(self):
        # One digest cannot name two files' bytes, so rewriting would have to
        # choose one silently. The check still reports the row.
        self.write("docs/first.md", "first")
        self.write("docs/second.md", "second")
        stale = hashlib.sha256(b"previous").hexdigest()
        draft = self.write(
            "evidence/draft-v1.md",
            f"# Draft\n\n- Status: Proposed\n\n| Record | SHA-256 |\n|---|---|\n"
            f"| `docs/first.md` and `docs/second.md` | `{stale}` |\n",
        )

        self.assertFalse(PROVENANCE.repin(draft, self.root))
        self.assertIn(stale, draft.read_text(encoding="utf-8"))

if __name__ == "__main__":
    unittest.main()
