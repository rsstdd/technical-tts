# ADR-0001-D002 — Constrained-development performance gate

- **Status:** Approved through accepted ADR-0002
- **Date:** 2026-08-26
- **Controlling ADR and sections:** ADR-0001 §3.4, §5.1, §17.16, and §19 Phase 0
- **Requesting story:** E0-S3
- **Owner:** Engineering owner
- **Approver:** Ross Todd, project owner
- **Expiry:** Before G3 gate review, or earlier under ADR-0002's recorded triggers

## Proposed deviation

Permit development progression after the pinned Chatterbox configuration passed rights,
offline-rendering, reliability, fixed-seed characterization, listening, and WAV-compatibility
controls but failed both CPU performance targets on a constrained WSL2 allocation. Preserve the
`RTF <= 6.0` and 21,600-second targets; waive only their immediate blocking effect on E0-S4 and
subsequent implementation work.

Accepted ADR-0002 is the controlling amendment. The v1 G0 report remains the authoritative
measurement result, and the v2 decision records the project owner's risk acceptance.

## Impact

- **Architecture and authority boundaries:** No change. Rust retains durable authority, and
  standard Chatterbox remains the sole backend candidate.
- **Schemas and interfaces:** No change. E0-S4's contracts remain provisional and
  backend-agnostic until G1 freeze.
- **Synthesis, verification, and cache identities:** No fields are removed. Hardware, device,
  thread controls, worker bundle, model, voice, and speech-affecting inputs retain their governed
  identity effects.
- **Security, rights, and privacy:** No control is waived. Offline rendering, containment,
  checksums, consent, and private-use scope remain mandatory.
- **Tests and evidence:** Both performance checks remain failed. Reports retain measured wall
  time, RTF, RAM, thread budget, and environment identity. The full-box configuration must rerun
  and pass both checks before G3 acceptance.
- **Existing artifacts and migration:** No migration or invalidation is required. Valid spike and
  private-preview artifacts remain governed by their recorded identities and retention rules.
- **Schedule and scope:** E0-S3 closes through this approved deviation, allowing E0-S4 to begin.
  Overall G0 remains open until E0-S4 supplies its provisional contract baseline.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Block all implementation until full-box access is available | Performance is unresolved, but the passing real-worker observations are sufficient to design provisional replaceable contracts; waiting would not reduce contract risk |
| Permanently raise or remove the performance targets | The constrained result does not establish an acceptable production budget |
| Add a faster backend immediately | ADR-0001 selects one standard Chatterbox backend; the current evidence does not justify a backend collection |
| Treat the failed measurements as passing | Factually incorrect and incompatible with the evidence protocol |

## Compensating control and expiry

The intended deployment configuration must be named, inventoried, and measured before G3 gate
review. It must meet both unchanged targets with the pinned worker identity or a governed
successor. The deviation expires earlier if a relevant identity changes or the constrained
environment causes resource exhaustion, backend instability, or unusable private-preview
operation.

## Rollback

Revoke the waiver, mark the affected gate blocked, and stop additional real-backend expansion.
Retain valid artifacts and backend-agnostic contracts; reopen the hardware-acceleration/backend
decision without deleting or reclassifying prior evidence.

## Decision

- [x] **Approve through accepted ADR-0002**
- [ ] Reject
- [ ] Defer

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept compensating controls and expiry | 2026-08-26 |
| Project owner | Ross Todd | Accept constrained-environment risk for development progression | 2026-08-26 |
