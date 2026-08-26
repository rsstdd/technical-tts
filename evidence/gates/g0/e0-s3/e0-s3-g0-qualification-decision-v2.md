# Gate Review: G0 — E0-S3 constrained-development decision v2

- Date/time and timezone: 2026-08-26, Europe/Berlin
- Candidate revision: Git base `07be527c54c8373f74050d63d06e87a43ce4ed69` plus the
  accepted E0-S3/ADR-0002 worktree
- Candidate artifact checksum: failed measurement report SHA-256
  `4ee5820fd0434c88a8b69c42a6f42b1b61756e7bc1d8ced2e7923c87c35b0ec2`
- Accountable owner: Project owner
- Approvers: Engineering owner and project owner
- Supersedes: G0 progression decision in `e0-s3-g0-qualification-report-v1.md`; it does not
  supersede or alter that report's measurements

## Scope and criteria

This decision applies accepted ADR-0002's constrained-development performance waiver. It does
not lower the `RTF <= 6.0` or 21,600-second targets and does not convert the measured failures
into passes. A conditional G0 progression decision requires every non-performance G0 control to
pass, the failed performance evidence to remain visible, and an accepted ADR to define scope,
compensating controls, expiry, and rollback.

| Requirement | Story | Test/evidence ID | Result | Artifact link/checksum |
|---|---|---|---|---|
| Lawful pinned code, weights, source, and owner voice | E0-S2/E0-S3 | Rights readiness review | Pass | Four v2 records cited by the v1 report |
| Real Chatterbox render without network routes | E0-S3 | `t5_e0_real_chatterbox_smoke_render_succeeds_offline` | Pass | V1 smoke result |
| Fixed-seed behavior and blind listening characterized | E0-S3 | `evidence_e0_fixed_seed_synthesis_determinism_is_characterized` | Pass | Completed review SHA-256 `9ad19dba45dadb00de480c9493a981481cdce753a80f231f12c53d464e97d012` |
| Worker/cache/master/FFmpeg float WAV path | E0-S3 | `t4_e0_pipeline_wav_variants_round_trip` | Pass | V1 actual-hound report |
| Reference-environment report | E0-S3 | `t5_e0_reference_environment_report_complete` | Pass | V1 environment record |
| Worst single-worker RTF `<= 6.0` | E0-S3 | `t5_e0_single_worker_rtf_is_at_or_below_6_0` | **Fail: 14.9804; waived only for development progression** | V1 ten-run result; ADR-0002 |
| Cold 60-minute projection `<= 21,600` seconds | E0-S3 | `t5_e0_projected_sixty_minute_runtime_is_at_or_below_six_hours` | **Fail: 53,947.516 seconds; waived only for development progression** | V1 ten-run result; ADR-0002 |
| Approved deviation control | E0-S3/G0 | Accepted ADR-0002 | Pass | Scoped waiver, compensating controls, expiry, and rollback recorded |

## Accepted limitation

The WSL2 measurement exposed four physical/eight logical cores and 16.77 GB RAM on a larger AMD
Ryzen AI 9 HX 370 host. The project owner accepts this allocation as a constrained development
environment and accepts the planning assumption that the intended full-box configuration will
provide the approximately 2.5-times throughput needed by both performance targets.

That assumption is not measured qualification. The original values remain the only current
performance evidence and must be used for local estimates. ADR-0002 requires full-box
qualification before G3 can pass.

## Decision inputs

| Input | SHA-256 |
|---|---|
| `DELIVERY-PLAN.md` | `85bf7659fa60467dd116ee80fb2a2da962d3e56b87a5fc42625f4e7a05da1bc5` |
| `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `397dd2efa3094aca8c8f0aca11f67e44f4014ed0b0d018684fe06c24978c9b53` |
| `docs/adr/deviations/ADR-0001-D002-constrained-development-performance-gate.md` | `00e09cd31b470d17e5eb55018d9c3546af16dab45f6a0a3f09b0310da3df132a` |
| `docs/operations/REFERENCE-ENVIRONMENT.md` | `a673a4b1570df39d2458493a6ec3b033b0545ebe9aa9adcaca4ad51021cdfd50` |
| `docs/perf/BUDGETS.md` | `9e160ed3d6311cbb5390fc43e4da02121143988f3ad15d656fa6203cd1090b31` |
| `e0-s3-g0-qualification-report-v1.md` | `4ee5820fd0434c88a8b69c42a6f42b1b61756e7bc1d8ced2e7923c87c35b0ec2` |
| `evidence_e0_fixed_seed_synthesis_determinism_is_characterized.md` | `8f98b97531994109965ce5bf54712a63e1d6f2b380341b756dab4a71fb64b060` |
| `e0-s3-audit-remediation-v1.md` | `75278c0deba3d80fc8ca97e863c3583d483c8e0533d713369b2dbe75ee8c8bea` |

## Open findings

| Finding | Severity | Owner | Required action | Deadline |
|---|---|---|---|---|
| Full-box performance is assumed, not measured | Blocking for G3; accepted for development and private preview | Engineering owner | Name the deployment configuration and rerun both performance checks | Before G3 gate review |
| No qualified backup reference machine exists | Accepted risk | Engineering owner | Rebuild the pinned environment and rerun critical qualification within eight working hours after primary loss | Before M3 |

## Decision

- [ ] Pass
- [x] **Conditional pass of the E0-S3 feasibility portion, permitted by accepted ADR-0002 and
  the amended Delivery Plan**
- [ ] Fail

E0-S3 is complete through an approved deviation, and E0-S4 may begin. Overall G0 remains open
until E0-S4 supplies the provisional contract baseline. After that baseline closes G0, G1, M2,
and later implementation work may proceed with slow local rendering. This decision does not
authorize a production-performance claim or G3 acceptance without the required full-box
measurement.

The post-decision completion audit is resolved by `e0-s3-audit-remediation-v1.md`. Its
fail-closed voice checks and future raw-result identity correct the harness without altering the
historical measurements, the development-only waiver, or the full-box G3 obligation.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept measured failure and compensating controls for development progression | 2026-08-26 |
| Project owner | Ross Todd | Accept constrained-environment limitation and E0-S3 progression | 2026-08-26 |
