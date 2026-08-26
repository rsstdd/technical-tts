# Performance Budgets

Status: Partially ratified by ADR-0002

This is the registry for the committed performance budgets required by `PRINCIPLES.md` P15 and the
Bench enforcement mechanism. A budget is a named measurement, a target, a tolerance, and a pinned
baseline, measured on the reference machine defined in `docs/operations/REFERENCE-ENVIRONMENT.md`
at a lesson length representative of production.

The E0 walking skeleton renders a two-segment fixture with a deterministic tone synthesizer, so it
does not establish production performance. ADR-0002 ratifies the Chatterbox single-worker targets
below. The constrained development environment does not meet them; its accepted waiver permits
development progression but expires before G3 acceptance.

## Budget register

| Budget | Target | Tolerance | Baseline | Measured on | Evidence |
| --- | --- | --- | --- | --- | --- |
| Chatterbox single-worker CPU RTF | `<= 6.0` | None | `14.9804` (non-conforming, waived for development only) | `reference-wsl2-d9d550f06b783405`, pool size one, three Torch threads | `e0-s3-g0-qualification-report-v1.md`; ADR-0002 |
| Chatterbox 60-minute projection | `<= 21,600` seconds | None | 53,947.516 seconds (non-conforming, waived for development only) | Same pinned constrained environment | `e0-s3-g0-qualification-report-v1.md`; ADR-0002 |

Planned budget areas, from PRINCIPLES.md P15: segments per minute, peak resident memory, worker
restart cost, cache and quarantine disk use, and end-to-end render time at representative length.
