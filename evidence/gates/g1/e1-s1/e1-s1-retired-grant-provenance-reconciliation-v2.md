# E1-S1 Retired-Grant Provenance Reconciliation v2

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: branch `fix/issue-59-retired-grant`, after pull-request review
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: `e1-s1-retired-grant-provenance-reconciliation-v1`

## Scope and decision

This record corrects one inverted verification sentence in v1 while preserving its implementation,
scope, decision, and three accounted pairs. V1 says that the targeted regression reports the stale
`legacy-v1` citation "when the filter is removed." The observed result is the reverse: before the
filter existed the checker returned zero violations; with the filter it reports the stale citation.

V1 is Accepted and remains immutable. This successor carries every load-bearing row forward so
accepting it retires the inaccurate wording without withdrawing the approved issue #59 decision.
The adjacent non-expiring `(citing record, cited path)` behavior remains unchanged and under the
standing reviewer obligation in `e1-s2-evidence-provenance-reconciliation-v3` §Open findings.

## Acceptance criterion

Accepted when all four hold:

1. The red and green outcomes are stated in their observed order.
2. All three v1 accounting rows are carried forward unchanged.
3. The checker suite passes at 21 tests and repository provenance returns zero after acceptance.
4. The engineering owner and project owner approve this correction and the carried rows.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `scripts/check-evidence-provenance.py` |
| `e1-s1-provisional-contract-baseline-v15` | `scripts/tests/test_check_evidence_provenance.py` |
| `e1-s1-provisional-contract-baseline-v15` | `evidence/README.md` |

## Verification

The regression was added before the implementation change. Against the prior collector it failed
because the checker returned zero violations; the retired reconciliation still suppressed the
stale `legacy-v1` citation. With `declared_superseded_ids` filtered through
`active_accepted_records`, the targeted test passes and reports the violation.

| Command | Result |
|---|---|
| `python3 -m unittest scripts.tests.test_check_evidence_provenance.EvidenceProvenanceTests.test_a_superseded_record_cannot_declare_a_prose_supersession` | Pass, 1 test; failed before the fix with 0 violations instead of 1 |
| `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | Pass, 21 tests |
| `python3 scripts/check-evidence-provenance.py` | Pass, zero unaccounted mismatches after acceptance |
| `git diff --check` | Pass |

## Decision

Ross Todd holds both roles below. On 2026-08-30 the repository codeowner explicitly approved both
decisions after reviewing this Proposed successor; each row records the separate role judgment.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept the corrected red/green statement and that it changes no implementation behavior | 2026-08-30 |
| Project owner | Ross Todd | Approve — accept v2 superseding immutable v1 and carrying its three accounting rows unchanged | 2026-08-30 |
