# E1-S1 CI Citation-Step Provenance Reconciliation v1

- Date/time and timezone: 2026-09-02, Europe/Berlin
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted

## What this reconciles

One provenance mismatch, created deliberately and named here rather than left to be discovered.

`evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v15.md` pins
`.github/workflows/ci.yml` at `ff80cf2ec767…`. Wiring `check_evidence_citations.py` into the lint
job changed that file — a new step, and `fetch-depth: 0` on the job's checkout — so it now hashes
`f36a2d65e6fb…` and `scripts/check-evidence-provenance.py` reports the pin as unaccounted.

## Why account rather than supersede or re-pin

**Re-pinning is forbidden.** `evidence/README.md` says never to overwrite an accepted report, and
v15 is accepted. Editing its digest in place would silently rewrite what an approver signed.

**Superseding would assert something false.** A successor version in this repository signals that a
conclusion an approver relied on turned out to be wrong. Nothing v15 concluded is wrong. It read
`ci.yml` to establish what the pull-request checks run; that reading still holds, and the workflow
now runs one check more than it did.

**What actually changed is additive and in the same direction as the conclusion.** The lint job
gained a step that verifies evidence citations against git history, and the checkout it runs under
gained the history that step needs. No existing step was removed, weakened, or reordered.

## Accounted provenance mismatches

| `e1-s1-provisional-contract-baseline-v15` | `.github/workflows/ci.yml` |

That row authorizes exactly one pin of one path in one record. Every other pin in v15, and every
other record pinning `.github/workflows/ci.yml`, is checked as before.

## What this record does not grant

- **No standing permission to edit `.github/workflows/ci.yml`.** The next change to it faces the
  same obligation: recompute, supersede, or account.
- **No relief for any other pin**, in v15 or elsewhere.
- **No re-attestation of the E1-S1 baseline**, whose standing rests on what it measured.
- **No relaxation of the citation check it wires in.** That control's own exception semantics live
  in `evidence/gates/g0/e0-s3/e0-s3-citation-provenance-reconciliation-v1.md`, and are enforced by
  `scripts/qualification/tests/test_check_evidence_citations.py`.

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept one accounted pin for an additive CI change that adds a check and removes none | 2026-09-02 |
| Project owner | Ross Todd | Accept that `e1-s1-provisional-contract-baseline-v15` retains a pin no longer matching current bytes, accounted rather than superseded | 2026-09-02 |
