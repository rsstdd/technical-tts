# ADR-0002: Model, Hardware, Voice, and Format Qualification

- **Status:** Accepted on 2026-08-26 with a constrained-development performance waiver
- **Owner:** Engineering owner
- **Approver:** Ross Todd, project owner
- **Depends on:** ADR-0001, E0-S2, E0-S3
- **Explicitly supersedes:** ADR-0001 §3.4, §5.1, §17.16, and §19 Phase 0 only for the
  development-blocking consequence of the CPU performance gate, as scoped below
- **Approved deviation:**
  `deviations/ADR-0001-D002-constrained-development-performance-gate.md`

## Decision

Accept the pinned Chatterbox, model, owner-only voice, worker-format, and FFmpeg configuration as
the development baseline. The measured WSL2 allocation is accepted as a constrained development
environment rather than the final deployment reference machine. Its worst single-worker RTF of
`14.9804` and cold 60-minute projection of 53,947.516 seconds remain failed measurements against
the respective `6.0` and 21,600-second targets; this decision does not relabel either result.

The project owner accepts the planning assumption that a normally provisioned full-box
configuration will supply the approximately 2.5-times throughput required by both targets. That
assumption is sufficient to unblock E0-S4, G1, M2, and later implementation work, but it is not
performance evidence and does not authorize a production-performance claim.

For this scope, this ADR supersedes ADR-0001's requirement that `RTF > 6.0` block application
integration. Standard Chatterbox remains the only production-backend candidate. No alternate
backend, GPU path, remote execution path, or weaker audio/integrity control is authorized.

The original failed measurement decision is
`evidence/gates/g0/e0-s3/e0-s3-g0-qualification-report-v1.md`. The superseding E0-S3 progression
decision is `evidence/gates/g0/e0-s3/e0-s3-g0-qualification-decision-v2.md`. Fixed-seed evidence
is recorded at the exact Delivery Plan path
`evidence/gates/g0/e0-s3/evidence_e0_fixed_seed_synthesis_determinism_is_characterized.md`;
its checksum-linked ten-sample human review passed on 2026-08-26.

## Required evidence

- exact source revisions, artifact URIs, checksums, licenses, and permitted scope;
- reference-machine inventory;
- offline real render;
- model load time and peak RAM;
- pool-size-one RTF and six-hour projection;
- output WAV compatibility;
- ten-run fixed-seed determinism characterization;
- voice consent/checksum and listener assessment;
- worker-bundle and FFmpeg identities;
- schedule reforecast.

## Decision table

| Item | Candidate | Evidence | Decision |
|---|---|---|---|
| Chatterbox revision | `v0.1.2`, commit `eb90621fa748f341a5b768aed0c0c12fc561894b` | Clean acquired tree; MIT SHA-256 `4248e910a928849fe5815a0f9236e17fa07768d95b9193212752c464b93d6caa` | Accepted development baseline |
| Model/tokenizer/codec | `ResembleAI/chatterbox` revision `1b475dffa71fb191cb6d5901215eb6f55635a9b6` | Three SafeTensors plus tokenizer JSON match `rights-chatterbox-weights-v2` | Accepted development baseline |
| Worker identity | SHA-256 `f5628884678f52de2f3a65ea51c9bc2a86e4f5919044fa9b4340eb62465dc2a9` | Code, model, dependency, voice conditional, parameters, CPU, Python/ABI, and thread controls | Recorded |
| Voice profile | `owner-fallback-v1`, single instructor | `rights-voice-owner-fallback-v2`; conditionals loaded weights-only | Qualified for owner-only private use |
| Reference hardware | `reference-wsl2-d9d550f06b783405`; Ryzen AI 9 HX 370; WSL-visible 4 physical/8 logical cores; 16.77 GB RAM | `docs/operations/REFERENCE-ENVIRONMENT.md` | Accepted only as constrained development environment; performance failed |
| Canonical worker format | 24 kHz mono 32-bit IEEE-float WAV | Ten runs plus actual hound worker/cache/master/FFmpeg probe | Qualified |
| Fixed-seed behavior | Container bytes vary; decoded PCM identical in 10/10 | 10 container hashes, 1 `data` hash, `PEAK` timestamp only, correlation/cosine `1.0`; 10/10 blind samples accepted with no required-category findings | Characterized, not a reconstruction guarantee |
| Supported FFmpeg identity | Ubuntu `6.1.1-3ubuntu5`; executable SHA-256 `ed16af623947494a72e284b6eb8ff225f2da22b38b5d5069c2fd4b4ba3384e41` | Exact argv and profile hash in reference-environment record | Qualified for this spike |

## Waiver scope and compensating controls

The performance waiver applies to development progression through E0-S4, G1, M2, and later
implementation stories. It permits slow local private-preview rendering. It does not:

- claim that the constrained WSL2 allocation satisfies either performance target;
- qualify that allocation as the production reference environment;
- waive offline, rights, WAV, determinism-characterization, resource-governance, soak, or
  listening requirements;
- permit production authorization based on projected hardware performance.

Until the waiver expires, retain per-run wall time, RTF, peak-RAM, thread-budget, worker identity,
and hardware identity in qualification and run reports. Keep pool size one unless later measured
resource-governance evidence authorizes more. User-visible estimates must use recorded throughput,
not the assumed full-box result.

The first-valid-artifact cache rule remains unchanged: byte-identical reconstruction requires the
retained cache artifact or archived segment bundle even when a bounded fixed-seed sample has
identical PCM.

## Expiry, qualification, and rollback

This waiver expires at the earliest of:

1. the G3 gate review;
2. selection of a different deployment machine, device path, model, voice conditional, or other
   speech-affecting worker input;
3. evidence that the constrained environment causes resource exhaustion, backend instability,
   or unusable private-preview operation.

Before G3 can pass, name and inventory the intended full-box deployment configuration and rerun
the single-worker RTF and 60-minute projection with the pinned worker identity or its governed
successor. Both original targets remain unchanged. A failing full-box result reopens the
hardware-acceleration/backend decision and blocks G3; it does not invalidate backend-agnostic
contracts, cache data, or completed private-preview work.

Rollback requires no data migration. Revoke this waiver, mark the affected gate blocked, retain
all valid artifacts, and stop real-backend expansion while the hardware/backend decision is
reopened.

## Acceptance

ADR-0002 is accepted with the scoped waiver above. E0-S3 closes through an approved deviation,
and its feasibility portion of G0 receives a conditional pass for development progression. E0-S4
may begin. Overall G0 remains open until E0-S4 supplies the provisional contract baseline. The
failed performance results remain visible, and full-box performance qualification remains
mandatory before G3 acceptance.

The M2 and M3 implementation forecasts resume after overall G0 closure. A forecast is not gate
evidence, and G3/M3 acceptance remains subject to the waiver's expiry condition.
