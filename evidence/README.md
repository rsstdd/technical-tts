# Evidence Records

This directory contains redacted, immutable evidence indexes and reports. Raw models, voices, private lessons, corpora, and generated audio remain in governed external locations and are referenced by stable URI and checksum.

Use the templates under `docs/templates/` and follow `docs/testing/EVIDENCE-AND-QUALIFICATION.md`.

```text
evidence/
  gates/<gate>/<review-id>/
  qualification/<area>/<run-id>/
  listening/<lesson-id>/<review-id>/
  rights/<record-id>/
  releases/<version>/<release-id>/
```

Never overwrite an accepted report. Create a new record that explicitly supersedes the prior evidence ID.


## Naming and criteria

Every `evidence_*` item named in `DELIVERY-PLAN.md` becomes exactly one file under
`gates/<gate>/<review-id>/`, named character for character as the delivery plan names it. A name
that a test or a checklist looks up must be written once and matched literally, never paraphrased.

Each record states its acceptance criterion **before** its result. A criterion written after the
result is not a criterion, because a bar set afterwards cannot fail.

Evidence is committed to Git. `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` OQ-02 proposes a
single-machine, local-filesystem scope, and OQ-06 leaves the backup and recovery plan open until
before M3, so until that plan exists, committing evidence is what protects it from machine loss.

## Provenance

A digest inside a record names the exact bytes a conclusion was reached against. Revising a cited
document does not invalidate the record — `git show` still produces what the approver read — but
it does mean the record can no longer be checked against the working tree, and nobody can tell a
routine revision from a rewritten control without reading both versions.

So editing a governed document obliges you to say what happened to every accepted record pinning
it: recompute and supersede, or write a record showing the conclusion stands.
`scripts/check-evidence-provenance.py` enforces this over every unsuperseded record that is in
force, and is wired into the `lint` job of `.github/workflows/ci.yml`. Digests are transcribed by
that script, not by hand. `python3 scripts/check-evidence-provenance.py --write <record.md>`
re-pins that record's digest cells from current bytes, and the diff is what a reviewer reads. The
record is named by its author and never inferred, so a sweep can never rewrite one nobody was
looking at. It refuses an accepted record, one declaring no status, and a superseded one —
amending the first is what this file forbids, the second is the legacy case that rule protects,
and the third pins what it measured. It also leaves a row citing two paths alone, because one
digest cannot name two files' bytes and picking one silently is the error being replaced.
Copying a digest by hand is how a record comes to pin a tree that moved under it.

A mismatch can be suppressed only by an exact row
under `## Accounted provenance mismatches` in an accepted reconciliation record. A proposed
record, an unapproved superseding record, or a prose mention has no effect. Nothing inside a
record declares its kind, so a reconciliation record is one carrying `reconciliation` as a
hyphen-separated word in its record ID; a baseline record's own accounting section grants
nothing, however it is worded. The reconciliation this repository carries today is
`gates/g1/e1-s1/e1-s1-evidence-provenance-reconciliation-v2.md`, which is accepted, supersedes
`-v1`, and names that script in return.

New records declare acceptance with an exact `- Status: Accepted` field and supersession with
``- Supersedes: `<record-id>` ``. Immutable legacy records without a status field remain readable
through their completed Review/Approval table or checked rights decision; a table containing a
Pending or Proposed decision is not accepted.

Superseded records are not checked, and must not be. They pin what they measured; that is what
supersession is for. Supersession is read from a `- Supersedes:` metadata line, so every new
record must carry one; prose alone does not remove a record from checking. Records accepted
before that rule are listed under `## Superseded without supersession metadata` in an accepted
reconciliation record, because adding the line to them would be the in-place amendment forbidden
above. That declaration grants only while the reconciliation remains active under explicit
supersession metadata. An accepted successor must repeat every declaration that is still needed.

Acceptance decides who may *grant* — supersede a record, or account for a mismatch — not who is
*checked*. A record that declares no status is checked rather than skipped, because the reverse
fails open: the records least likely to declare a status are the oldest, whose cited documents
have had the longest to move.

A record declaring `- Status: Proposed` is the one exception, and it is narrow. A proposal is not
in force — `CLAUDE.md` says a proposed ADR authorizes nothing — so nothing rests on its pins and
there is no conclusion for a moved document to invalidate. Checking one costs a supersession per
commit for a claim no reader may rely on. This takes nothing away from the paragraph above, which
is about records that declare *nothing*; those stay checked.

## Load-bearing pins and context references

A digest is an obligation: every commit touching those bytes owes this record a supersession or a
reconciliation row. Spend it only where a conclusion actually rests.

- A **load-bearing pin** carries a trailing SHA-256 in its table row. Use it for bytes a stated
  result was measured from, or that a control's behavior was verified against — the code, schema,
  fixture, protocol, workflow, or ratified policy whose movement obliges someone to re-decide
  whether the conclusion still holds.
- A **context reference** is cited in prose or in a row carrying no digest. Use it for a document
  that orients a reader but that no conclusion was measured against — an index, an operating
  standard, a delivery plan, a README.

The test is one question: *if this file changed, would anything in this record have to be re-run
or re-decided?* If no, it is context, and pinning it buys a false sense of coverage at the price
of a supersession every time an unrelated sentence moves.

This needs no separate enforcement. `scripts/check-evidence-provenance.py` reads only a row whose
final cell is a SHA-256, so a context reference is already outside the check — which is why the
distinction has to be made deliberately rather than by leaving a digest off in haste.

## Accepting a record at its gate

Supersession means *a conclusion an approver relied on turned out to be wrong*. That signal is
worth keeping legible, and it is lost when supersession doubles as a way to track a tree that is
still moving.

So a story keeps **one** record, `Proposed` for as long as the work runs, accumulating findings as
they are made. It is pinned once — with `--write` — and accepted once, at the gate it serves,
against the bytes that gate actually approved. A version after that means a conclusion was wrong,
and a reader who sees one should expect to find out which.
