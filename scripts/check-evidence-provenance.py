#!/usr/bin/env python3
"""Refuse stale repository citations in evidence records still in force."""

import hashlib
import pathlib
import re
import sys

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parent.parent
EVIDENCE_ROOT = REPOSITORY_ROOT / "evidence"

BACKTICKED = re.compile(r"`([^`]+)`")
FINAL_DIGEST = re.compile(r"`([0-9a-f]{64})`\s*\|\s*$")
STATUS = re.compile(r"^- Status:\s*(.+?)\s*$", re.MULTILINE)
SUPERSESSION = re.compile(r"^- Supersedes:[^`\n]*`([^`]+)`", re.MULTILINE)
ACCOUNTING_ROW = re.compile(r"^\|\s*`([^`]+)`\s*\|\s*`([^`]+)`\s*\|\s*$")

REPOSITORY_DIRECTORIES = {
    ".github",
    "crates",
    "docs",
    "evidence",
    "fixtures",
    "schemas",
    "scripts",
    "worker",
}
REPOSITORY_FILES = {
    "AGENTS.md",
    "Cargo.lock",
    "Cargo.toml",
    "DELIVERY-PLAN.md",
    "PRINCIPLES.md",
    "README.md",
    "deny.toml",
    "rust-toolchain.toml",
}
ACCEPTED_DECISIONS = ("Accepted", "Adopted", "Approved")
UNACCEPTED_DECISIONS = ("Pending", "Proposed")
# A record that declares this is a draft: not in force, and the only
# record `--write` may re-pin. `evidence/README.md` §Provenance states
# both halves and names this script in return.
PROPOSED_STATUS = "Proposed"
ACCOUNTING_HEADING = "## Accounted provenance mismatches"
# `evidence/README.md` §Provenance: a mismatch "can be suppressed only by
# an exact row under `## Accounted provenance mismatches` in an accepted
# reconciliation record. A proposed record, an unapproved superseding
# record, or a prose mention has no effect." Nothing inside a record
# declares its kind, so the kind is read from the identifier; that README
# states the convention and names this script in return.
RECONCILIATION_TOKEN = "reconciliation"
SUPERSESSION_HEADING = "## Superseded without supersession metadata"
# Only the first cell is read; any further cell is prose for the reader.
SUPERSESSION_ROW = re.compile(r"^\|\s*`([^`]+)`\s*\|")

# `evidence/README.md` states the policy rather than recording evidence under
# it, so it cites nothing and is not a record to check.
POLICY_FILE = "README.md"


def record_id(record: pathlib.Path) -> str:
    """Returns the stable identifier an evidence record uses in prose."""
    return record.parent.name if record.name == "record.md" else record.stem


def section(text: str, heading: str) -> list[str]:
    """Returns lines beneath one second-level heading."""
    lines = text.splitlines()
    try:
        start = lines.index(heading) + 1
    except ValueError:
        return []
    end = next(
        (index for index in range(start, len(lines)) if lines[index].startswith("## ")),
        len(lines),
    )
    return lines[start:end]


def review_decisions(text: str) -> list[str]:
    """Returns decision cells from a record's review or approval table."""
    for heading in ("## Review", "## Approval"):
        rows = section(text, heading)
        decisions = []
        for row in rows:
            cells = [cell.strip() for cell in row.strip().strip("|").split("|")]
            if len(cells) >= 4 and cells[0] not in {"Role", "---"}:
                decisions.append(cells[2])
        if decisions:
            return decisions
    return []


def declared_status(record: pathlib.Path) -> str | None:
    """Returns a record's explicit `- Status:` field, or `None` if it has none."""
    declared = STATUS.search(record.read_text(encoding="utf-8"))
    return declared.group(1) if declared else None


def is_accepted(record: pathlib.Path) -> bool:
    """Returns whether a record carries an affirmative, completed approval."""
    text = record.read_text(encoding="utf-8")
    declared = declared_status(record)
    if declared:
        return declared == "Accepted"
    if "- [x] Approved for recorded scope" in section(text, "## Decision"):
        return True
    decisions = review_decisions(text)
    approved = any(decision.startswith(ACCEPTED_DECISIONS) for decision in decisions)
    unfinished = any(
        rejected.casefold() in decision.casefold()
        for decision in decisions
        for rejected in UNACCEPTED_DECISIONS
    )
    return approved and not unfinished


def superseded_ids(records: list[pathlib.Path]) -> set[str]:
    """Returns ids an accepted record supersedes by `- Supersedes:` metadata."""
    known = {record_id(record) for record in records}
    superseded = set()
    for record in records:
        if not is_accepted(record):
            continue
        for named in SUPERSESSION.findall(record.read_text(encoding="utf-8")):
            path = pathlib.PurePosixPath(named)
            named_id = path.parent.name if path.name == "record.md" else path.stem
            if named_id in known and named_id != record_id(record):
                superseded.add(named_id)
    return superseded


def declared_superseded_ids(records: list[pathlib.Path]) -> set[str]:
    """Returns ids an accepted reconciliation record declares superseded.

    `- Supersedes:` metadata is what a new record must carry, and
    `test_supersession_requires_explicit_metadata` keeps prose from counting.
    Records accepted before that rule cannot gain the line, because amending an
    accepted report is what `evidence/README.md` forbids. Declaring the
    exception in an accepted record puts it somewhere a reviewer approves and a
    wrong entry is visible, which editing each record in place would not.
    """
    declared = set()
    for record in records:
        if not is_accepted(record):
            continue
        for line in section(record.read_text(encoding="utf-8"), SUPERSESSION_HEADING):
            match = SUPERSESSION_ROW.match(line)
            if match:
                declared.add(match.group(1))
    return declared


def active_accepted_records(records: list[pathlib.Path]) -> list[pathlib.Path]:
    """Returns accepted records that no accepted record supersedes."""
    superseded = superseded_ids(records)
    return [
        record
        for record in records
        if is_accepted(record) and record_id(record) not in superseded
    ]


def records_to_check(records: list[pathlib.Path]) -> list[pathlib.Path]:
    """Returns every record whose citations must still hold.

    Acceptance gates who may *grant* something — supersede a record, or account
    for a mismatch — and not who is *checked*. A record declaring no status is
    checked rather than skipped, because skipping it fails open: the records
    least likely to declare a status are the oldest, whose cited documents have
    had the longest to move. Only a supersession removes a record from this
    list, so nothing drops out of scope by staying silent.

    A record declaring `Proposed` is the one exception, and it is narrow: a
    proposal is not in force, so nothing rests on its pins and holding one open
    while the tree moves costs a supersession per commit. The fail-open concern
    above is untouched, because it is about records that declare *nothing*.
    """
    superseded = superseded_ids(records) | declared_superseded_ids(records)
    return [
        record
        for record in records
        if record.name != POLICY_FILE
        and record_id(record) not in superseded
        and declared_status(record) != PROPOSED_STATUS
    ]


def accounted_mismatches(records: list[pathlib.Path]) -> set[tuple[str, str]]:
    """Returns mismatch pairs authorized by accepted reconciliation records."""
    accounted = set()
    for record in records:
        if RECONCILIATION_TOKEN not in record_id(record).split("-"):
            continue
        for line in section(record.read_text(encoding="utf-8"), ACCOUNTING_HEADING):
            match = ACCOUNTING_ROW.match(line)
            if match:
                accounted.add(match.groups())
    return accounted


def repository_target(
    record: pathlib.Path,
    cited: str,
    repository_root: pathlib.Path,
) -> pathlib.Path | None:
    """Resolves a repository citation and ignores governed or external artifacts."""
    if "://" in cited:
        return None
    path = pathlib.PurePosixPath(cited)
    if path.is_absolute() or ".." in path.parts:
        return None
    if path.parts[0] in REPOSITORY_DIRECTORIES or cited in REPOSITORY_FILES:
        return repository_root / path
    if len(path.parts) == 1 and path.suffix == ".md":
        return record.parent / path
    return None


def citations(
    record: pathlib.Path,
    line: str,
    repository_root: pathlib.Path,
) -> list[tuple[str, pathlib.Path, str]]:
    """Returns repository paths pinned by the final SHA-256 in a table row."""
    digest_match = FINAL_DIGEST.search(line)
    if not digest_match:
        return []
    found = []
    for cited in BACKTICKED.findall(line[: digest_match.start()]):
        target = repository_target(record, cited, repository_root)
        if target is not None:
            normalized = target.relative_to(repository_root).as_posix()
            found.append((normalized, target, digest_match.group(1)))
    return found


def digest(path: pathlib.Path) -> str:
    """Returns the SHA-256 spelling used by evidence records."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def violations(
    record: pathlib.Path,
    accounted: set[tuple[str, str]],
    repository_root: pathlib.Path,
):
    """Yields repository citations whose pinned bytes are unavailable or stale."""
    for number, line in enumerate(record.read_text(encoding="utf-8").splitlines(), 1):
        for cited, target, pinned in citations(record, line, repository_root):
            relative_record = record.relative_to(repository_root)
            if not target.is_file():
                yield f"{relative_record}:{number}: cited file `{cited}` does not exist"
                continue
            current = digest(target)
            if current == pinned or (record_id(record), cited) in accounted:
                continue
            yield (
                f"{relative_record}:{number}: `{cited}` is pinned at {pinned[:12]}… but "
                f"hashes {current[:12]}…; recompute the digest, supersede this record, "
                "or account for it in an accepted reconciliation record"
            )


def repin_refusal(record: pathlib.Path, evidence_root: pathlib.Path) -> str | None:
    """Returns why a record must not be rewritten, or `None` if it may be.

    `evidence/README.md` forbids overwriting an accepted report; a record
    declaring no status is among the legacy records that rule exists to protect;
    and a superseded record pins what it measured, which is what supersession is
    for. Only a live proposal is left, because only a proposal is not yet
    something a reader was told to rely on.
    """
    resolved = record.resolve()
    if not resolved.is_relative_to(evidence_root.resolve()):
        return f"`{record}` is outside `{evidence_root.name}/`; it is not an evidence record"
    if not resolved.is_file():
        return f"`{record}` does not exist"
    status = declared_status(record)
    if status != PROPOSED_STATUS:
        declared = f"declares `{status}`" if status else "declares no status"
        return f"`{record_id(record)}` {declared}; only a proposed record may be re-pinned"
    records = sorted(evidence_root.rglob("*.md"))
    if record_id(record) in superseded_ids(records) | declared_superseded_ids(records):
        return f"`{record_id(record)}` is superseded; it pins what it measured"
    return None


def repin(record: pathlib.Path, repository_root: pathlib.Path) -> bool:
    """Rewrites a record's stale digest cells, returning whether any moved.

    Callers gate this on `repin_refusal`. A row citing two paths is left alone:
    one digest cannot name two files' bytes, and choosing one silently is the
    transcription error this exists to replace.
    """
    original = record.read_text(encoding="utf-8")
    lines = original.splitlines(keepends=True)
    for index, line in enumerate(lines):
        cited = citations(record, line, repository_root)
        if len(cited) != 1:
            continue
        _, target, pinned = cited[0]
        if target.is_file():
            lines[index] = line.replace(pinned, digest(target))
    rewritten = "".join(lines)
    if rewritten == original:
        return False
    record.write_text(rewritten, encoding="utf-8")
    return True


def check(repository_root: pathlib.Path, evidence_root: pathlib.Path) -> list[str]:
    """Returns all provenance violations under one repository root."""
    records = sorted(evidence_root.rglob("*.md"))
    if not records:
        return [f"no evidence record found under `{evidence_root}`; nothing was checked"]
    accepted = active_accepted_records(records)
    if not accepted:
        return [f"no accepted evidence record found under `{evidence_root}`; nothing was checked"]
    accounted = accounted_mismatches(accepted)
    return [
        message
        for record in records_to_check(records)
        for message in violations(record, accounted, repository_root)
    ]


def main() -> int:
    arguments = sys.argv[1:]
    if arguments[:1] == ["--write"]:
        if len(arguments) != 2:
            print("usage: check-evidence-provenance.py --write <record.md>")
            return 2
        record = pathlib.Path(arguments[1]).resolve()
        refusal = repin_refusal(record, EVIDENCE_ROOT)
        if refusal:
            print(f"refusing to re-pin: {refusal}")
            return 1
        moved = repin(record, REPOSITORY_ROOT)
        print(f"re-pinned {arguments[1]}" if moved else f"{arguments[1]} was already current")
        return 0
    found = check(REPOSITORY_ROOT, EVIDENCE_ROOT)
    for message in found:
        print(message)
    if found:
        print(f"\n{len(found)} unaccounted provenance mismatch(es).")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
