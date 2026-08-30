# E1-S1 Retired-Grant Provenance Reconciliation v1

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: branch `fix/issue-59-retired-grant`
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Proposed
- Supersedes: nothing

## Scope and decision

Issue #59 closes the open item carried by
`e1-s1-provisional-contract-baseline-v13`, `-v14`, and `-v15`:
`declared_superseded_ids` read legacy supersession declarations from every accepted record,
including a record that an accepted successor had retired. A retired reconciliation could
therefore continue granting a suppression.

The checker now reads `## Superseded without supersession metadata` only from active accepted
records. Activity is determined by accepted `- Supersedes:` metadata, using the same
`active_accepted_records` path that already gates accounted provenance mismatches. A successor
must carry forward every legacy declaration that remains necessary.

This is deliberately narrower than recursive grant semantics. Legacy declarations do not decide
whether their own declarer is active; explicit accepted supersession metadata does. The active
E1-S1 reconciliation v2 already carries all six declarations from its retired v1 predecessor, so
the change removes no suppression from the current tree.

The adjacent property in
`e1-s2-evidence-provenance-reconciliation-v3` §Open findings is unchanged: an accounted
`(citing record, cited path)` pair does not expire when later edits move the cited bytes. That
property remains governed by the standing reviewer obligation recorded there.

## Acceptance criterion

Accepted when all five hold:

1. A regression test proves that a retired reconciliation cannot keep granting a legacy
   supersession declaration.
2. An active accepted reconciliation can still grant the declaration, while a proposed one
   cannot.
3. The active E1-S1 reconciliation v2 still contributes its six declarations and the repository
   provenance check reports no unaccounted mismatch after this record is accepted.
4. The evidence policy states that successors must carry forward declarations still needed.
5. The engineering owner and project owner approve the behavior and the exact accounting rows
   below.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `scripts/check-evidence-provenance.py` |
| `e1-s1-provisional-contract-baseline-v15` | `scripts/tests/test_check_evidence_provenance.py` |
| `e1-s1-provisional-contract-baseline-v15` | `evidence/README.md` |

These three files move together because they are the enforcement, its regression proof, and the
policy it enforces. No accepted evidence record is edited, and this record supersedes nothing.

## Verification

The regression was added before the implementation change. Against the prior collector it failed
because the checker returned zero violations; the retired reconciliation still suppressed the
stale `legacy-v1` citation. After filtering through `active_accepted_records`, the targeted test
passes and reports the violation when the filter is removed.

| Command | Result |
|---|---|
| `python3 -m unittest scripts.tests.test_check_evidence_provenance.EvidenceProvenanceTests.test_a_superseded_record_cannot_declare_a_prose_supersession` | Pass, 1 test; failed before the fix with 0 violations instead of 1 |
| `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | Pass, 21 tests |
| `python3 scripts/check-evidence-provenance.py` | Expected refusal while Proposed: exactly the three v15 pins accounted above |
| `git diff --check` | Pass |

No Rust, schema, dependency, worker, model, audio, or product behavior changes. Cargo checks,
model qualification, ASR, rendering, and listening are unchanged surfaces and are not required by
this reconciliation.

## Decision

Ross Todd holds both roles below. The rows remain unsigned while this record is Proposed.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Pending | |
| Project owner | Ross Todd | Pending | |
