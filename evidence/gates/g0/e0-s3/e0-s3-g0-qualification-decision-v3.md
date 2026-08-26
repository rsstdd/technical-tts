# Gate Review: G0 — E0-S3 constrained-development decision v3

- Date/time and timezone: 2026-08-26, Europe/Berlin
- Candidate revision: Git base `07be527c54c8373f74050d63d06e87a43ce4ed69` plus the
  accepted E0-S3/ADR-0002 worktree and follow-up review corrections
- Candidate artifact checksum: failed measurement report SHA-256
  `4ee5820fd0434c88a8b69c42a6f42b1b61756e7bc1d8ced2e7923c87c35b0ec2`
- Accountable owner: Project owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: `e0-s3-g0-qualification-decision-v2`, SHA-256
  `bb1828ceca17be2e37a5c9071f618cdce18f2bbcf848f25f808cb0bc2ac10a6e`

The accepted v2 decision remains byte-for-byte unchanged. This record supersedes v2 to carry
forward its decision with current document and evidence hashes after the immutable-evidence
correction. It does not alter the failed measurements, waiver scope, compensating controls,
expiry, rollback, or full-box G3 obligation.

## Scope and criteria

Stated before the result, per `evidence/README.md`: the `RTF <= 6.0` and 21,600-second controls
must still run and remain recorded. Accepted ADR-0002 may waive only their blocking effect for
its explicit development scope and expiry. Every non-performance G0 control must pass. Any
evidence correction must preserve accepted predecessors and use an explicitly superseding
record with verified hashes.

| Requirement | Result | Governing evidence |
|---|---|---|
| Lawful pinned code, weights, source, and owner voice | Pass | Rights readiness records cited by the v1 qualification report |
| Real Chatterbox render without network routes | Pass | `t5_e0_real_chatterbox_smoke_render_succeeds_offline` |
| Fixed-seed behavior and blind listening characterized | Pass | `evidence_e0_fixed_seed_synthesis_determinism_is_characterized_v2` |
| Worker/cache/master/FFmpeg float WAV path | Pass | `t4_e0_pipeline_wav_variants_round_trip`; missing FFmpeg now fails the gate |
| Reference-environment report | Pass | `t5_e0_reference_environment_report_complete` |
| Worst single-worker RTF `<= 6.0` | **Fail: 14.9804; blocking effect waived only for development progression** | V1 ten-run result; ADR-0002 |
| Cold 60-minute projection `<= 21,600` seconds | **Fail: 53,947.516 seconds; blocking effect waived only for development progression** | V1 ten-run result; ADR-0002 |
| Follow-up qualification-integrity audit | Pass | `e0-s3-audit-remediation-v2` |

## Accepted limitation

The WSL2 measurement exposed four physical/eight logical cores and 16.77 GB RAM on a larger AMD
Ryzen AI 9 HX 370 host. The planning assumption that the intended full-box configuration will
provide the approximately 2.5-times throughput needed by both performance targets remains
unmeasured. The original failed values remain the current performance evidence. ADR-0002
requires full-box qualification before G3 can pass.

## Decision inputs

| Input | SHA-256 |
|---|---|
| `e0-s3-g0-qualification-decision-v2.md` | `bb1828ceca17be2e37a5c9071f618cdce18f2bbcf848f25f808cb0bc2ac10a6e` |
| `DELIVERY-PLAN.md` | `add598619c5e1bc00dabc447abe6fdf4c13bc5acce41bbfc687fc6ff6962347e` |
| `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `397dd2efa3094aca8c8f0aca11f67e44f4014ed0b0d018684fe06c24978c9b53` |
| `docs/adr/deviations/ADR-0001-D002-constrained-development-performance-gate.md` | `00e09cd31b470d17e5eb55018d9c3546af16dab45f6a0a3f09b0310da3df132a` |
| `docs/operations/REFERENCE-ENVIRONMENT.md` | `a673a4b1570df39d2458493a6ec3b033b0545ebe9aa9adcaca4ad51021cdfd50` |
| `docs/perf/BUDGETS.md` | `9e160ed3d6311cbb5390fc43e4da02121143988f3ad15d656fa6203cd1090b31` |
| `e0-s3-g0-qualification-report-v1.md` | `4ee5820fd0434c88a8b69c42a6f42b1b61756e7bc1d8ced2e7923c87c35b0ec2` |
| `evidence_e0_fixed_seed_synthesis_determinism_is_characterized_v2.md` | `59817e71054d73735382715c10671ab74b7275a929f7c997a16cba155c0b201b` |
| `e0-s3-audit-remediation-v2.md` | `bcda43efddca6908d9848cbfafbf9b2c8d15fc511a2acdc62c8c1b387a9569d4` |

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

E0-S3 remains complete through the approved deviation, and E0-S4 may proceed. Overall G0
remains open until E0-S4 supplies the provisional contract baseline. This record does not
authorize a production-performance claim or G3 acceptance without the required full-box
measurement.

The v2 engineering-owner and project-owner decisions on the measured failures, the waiver, and
its compensating controls are carried forward unchanged. This supersession corrects evidence
provenance only, and the approvals below accept that corrected provenance; they do not
re-decide the measurements, waiver scope, expiry, rollback, or the full-box G3 obligation.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approved the corrected evidence provenance; the v2 decision on measured failure and compensating controls is carried forward unchanged | 2026-08-26 |
| Project owner | Ross Todd | Approved v3 as the routed E0-S3 G0 decision of record, superseding v2 without altering it | 2026-08-26 |
