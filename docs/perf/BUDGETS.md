# Performance Budgets

Status: Proposed — no budgets are ratified yet.

This is the registry for the committed performance budgets required by `PRINCIPLES.md` P15 and the
Bench enforcement mechanism. A budget is a named measurement, a target, a tolerance, and a pinned
baseline, measured on the reference machine defined in `docs/operations/REFERENCE-ENVIRONMENT.md`
at a lesson length representative of production.

No budgets exist at E0: the walking skeleton renders a two-segment fixture with a deterministic
tone synthesizer, which is not representative of production load. The first entries land with the
Chatterbox worker qualification (ADR-0002) and must be recorded here before the measurements that
satisfy them are claimed, per `evidence/README.md`.

## Budget register

| Budget | Target | Tolerance | Baseline | Measured on | Evidence |
| --- | --- | --- | --- | --- | --- |
| _none ratified_ | — | — | — | — | — |

Planned budget areas, from PRINCIPLES.md P15: segments per minute, peak resident memory, worker
restart cost, cache and quarantine disk use, and end-to-end render time at representative length.
