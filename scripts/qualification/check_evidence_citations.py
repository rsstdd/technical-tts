#!/usr/bin/env python3
"""Check that every SHA-256 an evidence record cites describes a real file version.

    python3 scripts/qualification/check_evidence_citations.py [--all]

Evidence records under `evidence/` cite their inputs by path and SHA-256. That
citation is a *historical* claim -- "this is what I decided against" -- so a
digest that no longer matches the file is **not** an error: documents move as
later stories land, and re-hashing a citation to current would falsify the
record rather than repair it. `e0-s3-g0-qualification-decision-v3.md` says the
governing rule in as many words: an evidence correction preserves accepted
predecessors and supersedes them with a new record.

What *is* an error is a digest matching **no version that has ever been
committed**. Such a citation can never be verified by anybody, which means the
control it exists to provide was never operating. Two causes produce it, and
this cannot tell them apart: a digest captured from the working tree and then
invalidated by a later edit in the same change, or a digest that was simply
wrong. Both need the same remedy.

So three classes are reported, and only the third fails:

  match    the file still hashes to the cited value
  drift    the value matches some committed version, just not the current one
  never    the value matches no committed version of that file

Exit `1` names every `never` citation that appears in a record **nothing
supersedes**, because that is a live record whose provenance cannot be checked.
The same fault in a superseded record is reported and not failed: those are kept
deliberately as history, and rewriting them is what the immutability rule
forbids. Pass `--all` to fail on every `never` regardless.

**Supersession is detected textually**, from a `Supersedes:` line or a
"supersedes `<name>`" phrase naming another record. That is a heuristic, and it
is the honest limit of this check: a record superseded only by prose this
pattern does not match is treated as live, which fails loudly rather than
passing silently.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
EVIDENCE = REPO / "evidence"


def _provenance_checker():
    """The provenance checker, loaded as a module.

    Imported rather than reimplemented, and that is the whole point. This check
    and `scripts/check-evidence-provenance.py` must agree on what an *accepted
    reconciliation record* is, or an accounting row would suppress a mismatch in
    one checker and not the other. Its filename is hyphenated and therefore not
    importable by name, so it is loaded by path; it does its work under
    `if __name__ == "__main__"`, so loading it runs nothing.
    """
    path = REPO / "scripts" / "check-evidence-provenance.py"
    specification = importlib.util.spec_from_file_location("_evidence_provenance", path)
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


_PROVENANCE = _provenance_checker()

# The heading a reconciliation writes citation accounting rows under. Distinct
# from the provenance checker's `## Accounted provenance mismatches` because the
# two answer different questions — that one compares a pin against the *current*
# bytes, this one against *git history* — and a row written for one must not
# silently suppress the other.
ACCOUNTED_CITATION_HEADING = "## Accounted citation mismatches"

# Row shape, acceptance test, and reconciliation-kind test are the provenance
# checker's own, referenced rather than restated: `| \`record-id\` | \`path\` |`
# and nothing else on the line, in a record whose identifier carries
# `reconciliation` as a hyphen-separated word and which is accepted.
# `evidence/README.md` §Provenance states the rule both checkers apply.
ACCOUNTING_ROW = _PROVENANCE.ACCOUNTING_ROW
RECONCILIATION_TOKEN = _PROVENANCE.RECONCILIATION_TOKEN

# Path then digest on one line, which is the order every citation table in
# `evidence/` uses. The `[^\n]*?` between them is what allows the `|`
# separators of a Markdown table; anchoring to the line is what stops a match
# spanning two rows. Accepting the reverse order too would mispair prose rather
# than widen coverage: a cell reading "SHA-256 `<digest>` as
# `docs/testing/TEST-DATA-MANIFEST.md` records" would bind the fixture's digest
# to that document, which cites nothing here.
CITATION = re.compile(
    r"`([A-Za-z0-9_./-]+\.(?:md|json|toml|lock|ndjson|py|rs))`[^\n]*?`([0-9a-f]{64})`"
)
SUPERSEDES = re.compile(r"[Ss]upersedes:?\s*`?([A-Za-z0-9_.-]+)`?")


def committed_digests(path: Path) -> set[str]:
    """Every SHA-256 this file has had in any commit that carried it."""
    relative = path.relative_to(REPO).as_posix()
    revisions = subprocess.run(
        ["git", "log", "--format=%H", "--", relative],
        capture_output=True, text=True, cwd=REPO, check=False,
    ).stdout.split()
    digests = set()
    for revision in revisions:
        blob = subprocess.run(
            ["git", "show", f"{revision}:{relative}"],
            capture_output=True, cwd=REPO, check=False,
        )
        if blob.returncode == 0:
            digests.add(hashlib.sha256(blob.stdout).hexdigest())
    return digests


def accounted_citations() -> set[tuple[str, str]]:
    """Citation pairs an accepted reconciliation record authorizes.

    A pair is `(citing record id, cited repository path)` and both must match
    exactly. There is deliberately no wildcard, prefix, glob, whole-record
    exemption, or directory exemption: an accounting row answers for one
    citation in one record, and a mechanism that could answer for more would be
    an exemption from citation integrity rather than an account of it.

    An unaccepted reconciliation grants nothing, which is what makes this an
    approval rather than a note: `evidence/README.md` §Provenance says a
    proposed record, an unapproved superseding record, or a prose mention has no
    effect, and `is_accepted` is the provenance checker's own predicate.
    """
    accounted = set()
    for record in sorted(EVIDENCE.rglob("*.md")):
        if RECONCILIATION_TOKEN not in _PROVENANCE.record_id(record).split("-"):
            continue
        if not _PROVENANCE.is_accepted(record):
            continue
        for line in _PROVENANCE.section(
            record.read_text(encoding="utf-8"), ACCOUNTED_CITATION_HEADING
        ):
            match = ACCOUNTING_ROW.match(line)
            if match:
                accounted.add(match.groups())
    return accounted


def superseded_records() -> set[str]:
    """Record stems some other record declares it supersedes."""
    superseded = set()
    for record in EVIDENCE.rglob("*.md"):
        for name in SUPERSEDES.findall(record.read_text(errors="ignore")):
            superseded.add(name.removesuffix(".md"))
    return superseded


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--all", action="store_true",
                        help="fail on a never-committed digest in a superseded record too")
    arguments = parser.parse_args()

    superseded = superseded_records()
    accounted = accounted_citations()
    counts = {"match": 0, "drift": 0, "unresolved": 0}
    never: list[tuple[Path, str, str, bool]] = []
    cache: dict[Path, set[str]] = {}

    for record in sorted(EVIDENCE.rglob("*.md")):
        for cited, digest in CITATION.findall(record.read_text(errors="ignore")):
            target = next(
                (candidate for candidate in (REPO / cited, record.parent / cited)
                 if candidate.is_file()), None)
            if target is None:
                counts["unresolved"] += 1
                continue
            if hashlib.sha256(target.read_bytes()).hexdigest() == digest:
                counts["match"] += 1
                continue
            if target not in cache:
                cache[target] = committed_digests(target)
            if digest in cache[target]:
                counts["drift"] += 1
            else:
                cited_path = target.relative_to(REPO).as_posix()
                # Accounted only for this exact record and this exact path.
                # Superseded records were never failed; an accounting row is
                # what answers for one that nothing supersedes.
                live = record.stem not in superseded and (
                    _PROVENANCE.record_id(record),
                    cited_path,
                ) not in accounted
                never.append((record, cited_path, digest, live))

    print(f"match {counts['match']}  drift {counts['drift']}  "
          f"never {len(never)}  unresolved-path {counts['unresolved']}")

    failing = [entry for entry in never if entry[3] or arguments.all]
    for record, cited, digest, live in never:
        if live:
            marker = "LIVE "
        elif (_PROVENANCE.record_id(record), cited) in accounted:
            marker = "acct "
        else:
            marker = "hist "
        print(f"  {marker}{digest[:12]}…  {cited}\n        cited in {record.name}")

    if failing:
        print(f"\n{len(failing)} citation(s) name a version that was never committed, "
              f"in a record nothing supersedes." if not arguments.all else
              f"\n{len(failing)} citation(s) name a version that was never committed.")
        print(
            "Correct them by superseding the record, or account for each one with an exact\n"
            "`| `record-id` | `path` |` row under "
            f"`{ACCOUNTED_CITATION_HEADING}`\n"
            "in an accepted reconciliation record. Never by editing the record in place."
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
