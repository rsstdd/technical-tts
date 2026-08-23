# Evidence Report: evidence_e0_milestone_matrix_has_one_owner_and_gate_per_requirement

- Governing story/gate: E0-S1 / G0
- Hypothesis or decision: The milestone capability matrix assigns responsibility completely, so no gate can be reached without a named deliverer and a named approver.
- Owner: Ross Todd (project owner)
- Date/time and timezone: 2026-08-23, America/Boise
- Environment ID: Not applicable (documentation review)

## Acceptance criterion

Stated before the result, per `evidence/README.md`: every capability row in
`docs/governance/MILESTONE-CAPABILITY-MATRIX.md` names exactly one delivery owner, exactly one
approver, and a first required gate, and every role name resolves to a real person.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Milestone capability matrix | branch `feat/static-rules` (PR #44) | `docs/governance/MILESTONE-CAPABILITY-MATRIX.md` | `02be58df06fdfca2e10ef15899b8927a41d9989a995620036f7312ee8cbd1cd1` |

## Procedure

Reviewed every capability row of the matrix against the criterion. Row count confirmed with
`grep "^| " docs/governance/MILESTONE-CAPABILITY-MATRIX.md | tail -n +2 | wc -l`.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Capability rows reviewed | All | 30 of 30 | Pass |
| Rows with exactly one delivery owner | 30 | 30 | Pass |
| Rows with exactly one approver | 30 | 30 | Pass |
| Rows with a first required gate | 30 | 30 | Pass |
| Role names resolving to a real person | All | All: the matrix's mapping rule assigns every role to Ross Todd during solo development unless another named person accepts it | Pass |

Rows naming compound owners (for example "CLI/core owners") name one accountable role set that
resolves to the same single person under the solo-development mapping; accountability is not
split. Independent-listener and rightsholder decisions remain separate roles even while
coordinated by the project owner, as the matrix records.

**Overall: PASS.**

## Deviations and limitations

Role names are titles mapped to one person by the matrix's closing rule rather than personal
names written per row. If any role is accepted by a different named person, the matrix and this
record must be superseded.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved | 2026-08-23 |
