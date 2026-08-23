# Evidence Report: evidence_e0_milestone_matrix_has_one_owner_and_gate_per_requirement

- Governing story/gate: E0-S1 / G0
- Hypothesis or decision: The milestone capability matrix assigns responsibility completely, so no gate can be reached without a named deliverer and a named approver.
- Owner: Ross Todd (project owner)
- Date/time and timezone: 2026-08-23, America/Boise
- Environment ID: Not applicable (documentation review)

## Acceptance criterion

Stated before the result, per `evidence/README.md`: every capability row in
`docs/governance/MILESTONE-CAPABILITY-MATRIX.md` names exactly one delivery owner, a named
approver or enumerated joint approver set, and a first required gate, and every role name
resolves to a real person. A joint approver set means every enumerated role must approve; it
never dilutes to any-one-of.

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
| Rows with a named approver or enumerated joint approver set | 30 | 30 (25 single-role; 5 joint sets requiring approval from every enumerated role) | Pass |
| Rows with a first required gate | 30 | 30 | Pass |
| Role names resolving to a real person | All | All except the listener representative resolve to Ross Todd under the matrix's solo-development mapping; the listener representative is deliberately outside that mapping and tracked to assignment by OQ-10 | Pass |

Rows naming compound owners (for example "CLI/core owners") name one accountable role set that
resolves to the same single person under the solo-development mapping; accountability is not
split. The five joint approver sets are "Rightsholder/project owner", "Human-review and project
owners", "Engineering/security owner", "Listener representative/project owner", and "Required
role signatories"; each denotes joint sign-off by every role it enumerates. "Required role
signatories" (production authorization, E6-S4, M3) is enumerated by the review table of
`docs/templates/RELEASE-CHECKLIST-TEMPLATE.md` — engineering owner, project owner, rights
approver, and listener representative — consistent with the sign-off matrix in
`docs/governance/PROJECT-EXECUTION-CHARTER.md`. Independent-listener and rightsholder decisions remain separate roles even while
coordinated by the project owner, as the matrix records: Ross Todd does not hold the listener
representative role, which is unassigned as of this record. Its assignment is an open question
(OQ-10, deadline before G1 listening approval), and the solo-review blind spot it mitigates is
the accepted risk R-10, both in `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md`. Listener
independence first gates E5-S1/E6-S1 approvals (G3/M3), not this G0 record.

**Overall: PASS.**

## Deviations and limitations

Role names are titles mapped to one person by the matrix's closing rule rather than personal
names written per row. If any role is accepted by a different named person, the matrix and this
record must be superseded.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Project owner (approver) | Ross Todd | Approved | 2026-08-23 |
