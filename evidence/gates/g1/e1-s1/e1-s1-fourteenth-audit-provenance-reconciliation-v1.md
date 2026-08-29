# E1-S1 Fourteenth-Audit Provenance Reconciliation v1

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Accountable owner: Engineering owner
- Approver: Repository user
- Status: Accepted
- Supersedes: nothing

## Scope and decision

The fourteenth E1-S1 audit gives the worker-environment integrity probe its own
two-minute ceiling. That changes
`docs/architecture/WALKING-SKELETON.md` after four active evidence records
pinned its earlier bytes. This reconciliation accounts only for those four
record/path pairs. It changes none of their measured results or approvals.

The repository user explicitly authorized the governed cross-file timeout
change and its accepted provenance reconciliation on 2026-08-29. Per
[`../../../README.md`](../../../README.md), the exact rows below have
mechanical effect without amending any earlier accepted record.

## Acceptance criterion

Accepted when all four hold:

1. Every newly reported mismatch appears exactly once below.
2. The old and replacement digests are reproduced from the named document.
3. The compatibility impact on each citing record's conclusion is stated.
4. The repository user authorizes the timeout policy and these exact rows.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-evidence-provenance-reconciliation-v1` | `docs/architecture/WALKING-SKELETON.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/architecture/WALKING-SKELETON.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/architecture/WALKING-SKELETON.md` |
| `e1-s1-provisional-contract-baseline-v7` | `docs/architecture/WALKING-SKELETON.md` |

## Reproduced movement and conclusion impact

All four records pin `docs/architecture/WALKING-SKELETON.md` at
`79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac`.
It now hashes
`3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc`.

The movement separates the worker-environment integrity walk from the
five-second version-only probe and pins the new two-minute ceiling to
`WORKER_ENVIRONMENT_PROBE_POLICY` and
`t1_e0_external_tool_supervision_policies_are_pinned`. The earlier records'
conclusions about bounded execution, worker-lock provenance, contract
compatibility, and prior evidence movements still stand. No previous
measurement, approval, or bundle identity is rewritten.

## Verification

Run after recording acceptance:

```text
python3 scripts/check-evidence-provenance.py
```

On 2026-08-29 the command passed with no unaccounted mismatch after this
authorized record and the proposed v8 baseline were updated.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Repository authority | repository user | Approved the timeout policy and four exact accounting rows | 2026-08-29 |
