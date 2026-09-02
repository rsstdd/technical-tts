# E0-S3 Citation Provenance Reconciliation v1

- Date/time and timezone: 2026-09-02, Europe/Berlin
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted

## Scope

**Exactly fourteen citations, in four records, enumerated individually below.** Nothing else.

Each pins a SHA-256 that appears in no commit of the file it names.
`scripts/qualification/check_evidence_citations.py` reports each as a version never committed, in a
record nothing supersedes, and exits `1`.

**This reconciles a provenance defect in how those conclusions were cited. It does not touch any
conclusion, and it is not evidence that one was wrong.** It also does not, and cannot, reconcile any
other unverifiable citation: the mechanism matches an exact record-and-path pair, so a citation not
listed here fails exactly as it did before.

## What the cited digests are

Each names a working-tree state that existed while its record was being written and was never
committed under those exact bytes. The following commit carried a different version — a further
edit, a formatting pass, a review correction — so the pinned digest describes a real intermediate
that git never stored. **It cannot be recovered from repository history, and no attempt was made to
manufacture it.** Rewriting history to produce a commit matching a recorded digest would fabricate
the very evidence a pin exists to supply, which is a worse fault than the one being reconciled.

The digests match neither history nor the current bytes, so the claim "measured against these bytes"
is unverifiable in both directions rather than merely stale. Both columns are given below so a
reader can confirm that for themselves.

## The fourteen

| Citing record | Cited path | Pinned digest | Current digest |
|---|---|---|---|
| `e0-s3-audit-remediation-v2` | `scripts/qualification/chatterbox_spike.py` | `e90124eecc94e255…` | `2a6b4e9f43c9306a…` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_chatterbox_spike.py` | `37fe190bc7e81ace…` | `cad0a451115cd2a0…` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/analyze_wav_variation.py` | `3ddff01a90121900…` | `454be8ec3aecefc6…` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_analyze_wav_variation.py` | `fe833de1174be977…` | `4c7460523b5afbe6…` |
| `e0-s3-g0-qualification-decision-v3` | `evidence/gates/g0/e0-s3/e0-s3-audit-remediation-v2.md` | `bcda43efddca6908…` | `8d7ebbfe9a6551a8…` |
| `e0-s3-g0-qualification-report-v1` | `DELIVERY-PLAN.md` | `8faf22a6d5a5e61e…` | `f85bd1575c268c49…` |
| `e0-s3-g0-qualification-report-v1` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `5bada66fe25cb4a0…` | `d52845a4a0b6029f…` |
| `e0-s3-g0-qualification-report-v1` | `docs/operations/REFERENCE-ENVIRONMENT.md` | `09b88760258a53e5…` | `a91244867b93568f…` |
| `e0-s3-g0-qualification-report-v1` | `docs/testing/TEST-DATA-MANIFEST.md` | `56b2fa747a5bdb38…` | `a966bcdc3a48ee50…` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/chatterbox_spike.py` | `67153661bd41b6e9…` | `2a6b4e9f43c9306a…` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/capture_reference_environment.py` | `2a389393dee6e5a5…` | `b0a417b11698a0b7…` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/analyze_wav_variation.py` | `0bd07d43e30ca38b…` | `454be8ec3aecefc6…` |
| `e0-s3-g0-qualification-report-v1` | `crates/study-tts-testkit/tests/wav_variants.rs` | `182fce44a50a2e70…` | `711ceedc5397f47b…` |
| `e0-s4-provisional-contract-baseline-v1` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `9defb41bb9f09932…` | `954c6badf053c61f…` |

## What supports each conclusion now

Taken by citing record, because the four are in different positions.

### `e0-s3-g0-qualification-report-v1` — eight citations

Its own §Status already says what it is: **"measurement result preserved; progression decision
superseded on 2026-08-26 by `e0-s3-g0-qualification-decision-v2.md`."** So the progression decision
these eight pins were read for no longer rests here at all — it rests on
`e0-s3-g0-qualification-decision-v3`, which is accepted, supersedes v2, and pins this report at
`4ee5820fd0434c88…`, a digest that does verify.

What remains here is the measurement, and the measurement has since been retaken on the current
backend: `e0-s3-g0-requalification-torch-2-10-0-v1.md` reruns the single-worker RTF, the 60-minute
projection, the fixed-seed determinism characterization, and the environment integrity checks, and
records a randomized listening assessment. A reader who cannot verify the 2026-08-25 inputs can
verify the current ones.

Three of the eight name qualification scripts whose current versions are exercised on every pull
request: `.github/workflows/ci.yml` runs `scripts/qualification/tests`, covering
`chatterbox_spike.py`, `analyze_wav_variation.py`, and `capture_reference_environment.py`. Their
behavior is checked continuously rather than attested once.

### `e0-s3-audit-remediation-v2` — four citations

Accepted, and the four pins name the scripts and tests that record's remediation changed. Those
scripts' current behavior is verified by the same CI step above, and the remediation's *acceptance*
is carried forward by `e0-s3-g0-qualification-decision-v3`, which cites this record and is itself
accepted.

### `e0-s3-g0-qualification-decision-v3` — one citation

Accepted. Its single unverifiable pin is of `e0-s3-audit-remediation-v2.md`, the record directly
above, which is present, accepted, and readable in the repository today. The conclusion it supports
— that the audit remediation was complete before the progression decision — can be checked by
reading that record rather than by verifying a digest of an intermediate draft of it.

### `e0-s4-provisional-contract-baseline-v1` — one citation

Its pin is of `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`, the E0-S4 baseline document.
That document has since been succeeded: `docs/architecture/G1-FREEZE-CHARTER.md`, accepted
2026-09-02, declares itself its successor and freezes the contracts the baseline held provisionally.
The baseline record's conclusion — that E0-S4 published versioned fakes and shared contract suites
without claiming production contracts — is superseded by a charter that states what was frozen and
when.

## Why the missing pins do not invalidate the conclusions

A pin answers "which bytes was this measured against". Its loss means a reader cannot re-derive
that answer from history; it does not mean the measurement did not happen or produced something
else. In all four records the substance rests on things still checkable today: a superseding
accepted decision, a retaken qualification, scripts under continuous test, a successor charter, or
a record still present in the repository.

Nothing here was found to be unsupported. **Had the reconciliation uncovered a conclusion that no
current evidence supports, the remedy would have been re-qualification rather than accounting, and
this record would say so.**

## Forward obligation

**A future evidence record must cite committed, verifiable repository objects before it is
accepted.** Compute a citation's digest from the committed object, not from the working tree, and
if a cited file changes before the record is accepted, recompute the citation rather than accepting
a pin to bytes no commit carries. `check_evidence_citations.py` now runs on every pull request and
fails an unaccounted never-committed pin, so this obligation is enforced rather than merely stated.

## Accounted citation mismatches

Each row authorizes **one** citation of **one** path in **one** record. There is no wildcard,
prefix, glob, whole-record exemption, or directory exemption, and none is expressible: the checker
matches the pair exactly. `scripts/qualification/tests/test_check_evidence_citations.py` holds that,
including that a wildcard row grants nothing.

| `e0-s3-audit-remediation-v2` | `scripts/qualification/chatterbox_spike.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_chatterbox_spike.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/analyze_wav_variation.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_analyze_wav_variation.py` |
| `e0-s3-g0-qualification-decision-v3` | `evidence/gates/g0/e0-s3/e0-s3-audit-remediation-v2.md` |
| `e0-s3-g0-qualification-report-v1` | `DELIVERY-PLAN.md` |
| `e0-s3-g0-qualification-report-v1` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` |
| `e0-s3-g0-qualification-report-v1` | `docs/operations/REFERENCE-ENVIRONMENT.md` |
| `e0-s3-g0-qualification-report-v1` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/chatterbox_spike.py` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/capture_reference_environment.py` |
| `e0-s3-g0-qualification-report-v1` | `scripts/qualification/analyze_wav_variation.py` |
| `e0-s3-g0-qualification-report-v1` | `crates/study-tts-testkit/tests/wav_variants.rs` |
| `e0-s4-provisional-contract-baseline-v1` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |

## What this record does not grant

- **No exemption from citation integrity.** Fourteen historical pins are accounted, named
  individually. Every other citation is checked as before.
- **No relief for any other path in these four records.** Each is accounted only for the paths
  above.
- **No re-attestation of any G0 conclusion.** Their standing rests on what the records describe.
- **No supersession.** None of the four records is superseded by this one, and none should be
  superseded to repair a citation: a successor version in this repository signals that a conclusion
  was wrong, which would be untrue of all four.
- **No change to `scripts/check-evidence-provenance.py`**, which asks a different question — a live
  pin against the *current* bytes — and continues to check these records unchanged.

## Enforcement

| Rule | Enforced by |
|---|---|
| An unaccounted never-committed citation in a live record fails | `test_unaccounted_missing_citation_fails` |
| An exact pair in an accepted reconciliation passes | `test_accounted_missing_citation_passes` |
| The same pair in a non-accepted reconciliation still fails | `test_pair_in_unaccepted_reconciliation_fails` |
| A record that is not a reconciliation grants nothing | `test_a_record_that_is_not_a_reconciliation_grants_nothing` |
| A row naming another path grants nothing | `test_wrong_path_still_fails` |
| A row naming another record grants nothing | `test_wrong_record_still_fails` |
| A wildcard or broad row grants nothing | `test_wildcard_accounting_grants_nothing` |
| A row outside the heading grants nothing | `test_a_row_outside_the_heading_grants_nothing` |
| A provenance accounting row does not account a citation | `test_a_provenance_accounting_row_does_not_account_a_citation` |
| Accounting one citation leaves an unrelated one failing | `test_unrelated_citation_still_fails` |

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept the accounting mechanism as a mirror of the provenance checker's, and the fourteen rows as an exhaustive scope | 2026-09-02 |
| Project owner | Ross Todd | Accept that four G0-era records retain pins unverifiable against history, accounted rather than superseded, with no conclusion re-attested and none found unsupported | 2026-09-02 |
