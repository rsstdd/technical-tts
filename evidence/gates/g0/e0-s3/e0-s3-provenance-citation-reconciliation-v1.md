# E0-S3 — provenance citation reconciliation v1

- Date: 2026-09-02
- Status: **Reconciliation recorded. No accepted decision, measurement, waiver, or acceptance changes.**
- Accountable owner: Project owner
- Preserves: every predecessor record byte-for-byte. Nothing here edits an accepted record.

**This record decides one thing:** that fourteen SHA-256 citations in live E0-S3 and E0-S4 evidence
name file versions that were never committed, that the values they should have carried are
recoverable from history and are recorded below, and that the fault is closed mechanically rather
than by editing the records that carry it.

Found while scoping the `torch 2.10.0` backend uplift (issue #69), where the schedule reforecast
required editing `DELIVERY-PLAN.md` — a document the E0-S3 qualification report cites by digest.

## A stale digest is not a fault. A digest that never existed is.

An evidence citation is a **historical** claim: *this is what I decided against*. Documents move as
later stories land, so a citation that no longer matches the current file is the mechanism working,
not failing — and re-hashing it to current would falsify the record rather than repair it.
`e0-s3-g0-qualification-decision-v3.md` states the governing rule: an evidence correction preserves
accepted predecessors and supersedes them with a new record.

So the audit classifies three ways, and only the third is a fault:

| Class | Meaning | Count | Fault? |
|---|---|---:|---|
| match | the file still hashes to the cited value | 98 | no |
| drift | the value matches a committed version, just not the current one | 324 | **no** — expected |
| never | the value matches **no** version ever committed | 45 | yes |
| unresolved | the cited path does not resolve from the record | 12 | out of scope |

A `never` citation can never be verified by anyone, which means the control it exists to provide
was never operating. Of the 45, **31 sit in records another record supersedes** — those are kept
deliberately as history and rewriting them is what the immutability rule forbids. **Fourteen are
live.**

## The fourteen live faults, and what each should have said

Recovered by hashing each cited path at the commit that first added the record citing it.

| Record | Cited input | Cited | Should have been |
|---|---|---|---|
| `e0-s3-g0-qualification-report-v1.md` | `DELIVERY-PLAN.md` | `8faf22a6d5a5…` | `add598619c5e…` |
| `e0-s3-g0-qualification-report-v1.md` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `5bada66fe25c…` | `397dd2efa309…` |
| `e0-s3-g0-qualification-report-v1.md` | `docs/operations/REFERENCE-ENVIRONMENT.md` | `09b88760258a…` | `a673a4b1570d…` |
| `e0-s3-g0-qualification-report-v1.md` | `docs/testing/TEST-DATA-MANIFEST.md` | `56b2fa747a5b…` | `cc5836100651…` |
| `e0-s3-g0-qualification-report-v1.md` | `scripts/qualification/chatterbox_spike.py` | `67153661bd41…` | `6f4c432f53cf…` |
| `e0-s3-g0-qualification-report-v1.md` | `scripts/qualification/capture_reference_environment.py` | `2a389393dee6…` | `b0a417b11698…` |
| `e0-s3-g0-qualification-report-v1.md` | `scripts/qualification/analyze_wav_variation.py` | `0bd07d43e30c…` | `454be8ec3aec…` |
| `e0-s3-g0-qualification-report-v1.md` | `crates/study-tts-testkit/tests/wav_variants.rs` | `182fce44a50a…` | `711ceedc5397…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/chatterbox_spike.py` | `e90124eecc94…` | `6f4c432f53cf…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/tests/test_chatterbox_spike.py` | `37fe190bc7e8…` | `cad0a451115c…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/analyze_wav_variation.py` | `3ddff01a9012…` | `454be8ec3aec…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/tests/test_analyze_wav_variation.py` | `fe833de1174b…` | `4c7460523b5a…` |
| `e0-s3-g0-qualification-decision-v3.md` | `evidence/gates/g0/e0-s3/e0-s3-audit-remediation-v2.md` | `bcda43efddca…` | `8d7ebbfe9a65…` |
| `e0-s4-provisional-contract-baseline-v1.md` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `9defb41bb9f0…` | `b6a61564cd66…` |

**Every one is recoverable**, so the provenance is repairable rather than lost. That is the reason
this is a reconciliation and not a re-take.

## The cause is procedural, and it is the same one every time

A digest was captured from the working tree and the file was then edited again before the change
merged, so the recorded value describes a state that was real for a while and never committed. Two
observations support that over any other explanation:

- **`e0-s3-g0-qualification-decision-v3.md` already fixed four of them.** Its purpose, in its own
  words, is to carry the v2 decision forward "with current document and evidence hashes." The values
  it carries for `DELIVERY-PLAN.md`, `ADR-0002`, and `REFERENCE-ENVIRONMENT.md` are exactly the
  should-have-been values above. The correction pass worked; it simply did not reach the report, the
  remediation record, or its own citation of the remediation record.
- **The one fault v3 introduced is of the same shape.** `bcda43ef…` matches no version of
  `e0-s3-audit-remediation-v2.md`, a file with exactly one commit that has always hashed
  `8d7ebbfe…`. A correction pass performed by hand reproduced the defect it existed to remove.

That is the argument for mechanizing rather than hand-correcting again.

## Closed mechanically

`scripts/qualification/check_evidence_citations.py` walks every citation under `evidence/`,
classifies it match/drift/never against the file's full commit history, and exits `1` for a `never`
citation in a record nothing supersedes. It is the check that would have caught all fourteen, and it
would have failed v3's correction pass at the moment it introduced its own.

Its limits are stated rather than implied: supersession is detected textually from a `Supersedes:`
line or a "supersedes `<name>`" phrase, so a record retired only by prose the pattern misses is
treated as live and fails loudly rather than passing silently; a cited path that does not resolve is
counted and skipped rather than guessed at.

```text
python3 scripts/qualification/check_evidence_citations.py
match 98  drift 324  never 45  unresolved-path 12
```

## What this record does not change

- No measurement, decision, waiver scope, expiry, rollback, or G3 obligation moves.
- No accepted record is edited. The fourteen citations remain as written in records that are
  preserved; this record is where their correct values live.
- The 324 drifted citations are **not** corrected, because they are not faults. Several drifted
  further today: the `torch 2.10.0` uplift edited `docs/operations/WORKER-ENVIRONMENT.md`, and the
  backend-uplift entry in §2.3 edited `DELIVERY-PLAN.md`. Both are ordinary.
- The 31 `never` citations in superseded records are recorded by the checker and deliberately left
  alone.

## Sign-off

| Role | Name | Decision | Status |
|---|---|---|---|
| Project owner | Ross Todd | Accept that live provenance citations are reconciled by this record rather than by editing accepted predecessors, and that the checker is the standing control | **Pending** |
