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
`scripts/check-evidence-provenance.py` enforces this over every unsuperseded record — an
unapproved draft included, per the last paragraph below — and is wired into
the `lint` job of `.github/workflows/ci.yml`. A mismatch can be suppressed only by an exact row
under `## Accounted provenance mismatches` in an accepted reconciliation record. A proposed
record, an unapproved superseding record, or a prose mention has no effect. The reconciliation
this repository carries today is
`gates/g1/e1-s1/e1-s1-evidence-provenance-reconciliation-v1.md`, which is accepted and names
that script in return.

New records declare acceptance with an exact `- Status: Accepted` field and supersession with
``- Supersedes: `<record-id>` ``. Immutable legacy records without a status field remain readable
through their completed Review/Approval table or checked rights decision; a table containing a
Pending or Proposed decision is not accepted.

Superseded records are not checked, and must not be. They pin what they measured; that is what
supersession is for. Supersession is read from a `- Supersedes:` metadata line, so every new
record must carry one; prose alone does not remove a record from checking. Records accepted
before that rule are listed under `## Superseded without supersession metadata` in an accepted
reconciliation record, because adding the line to them would be the in-place amendment forbidden
above.

Acceptance decides who may *grant* — supersede a record, or account for a mismatch — not who is
*checked*. A record that declares no status is checked rather than skipped, because the reverse
fails open: the records least likely to declare a status are the oldest, whose cited documents
have had the longest to move.
