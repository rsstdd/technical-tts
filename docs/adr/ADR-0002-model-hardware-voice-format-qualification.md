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
decision is `evidence/gates/g0/e0-s3/e0-s3-g0-qualification-decision-v3.md`, which carries the v2
decision forward with corrected provenance. Fixed-seed evidence is recorded at the exact Delivery
Plan path
`evidence/gates/g0/e0-s3/evidence_e0_fixed_seed_synthesis_determinism_is_characterized.md`; the
accepted `evidence_e0_fixed_seed_synthesis_determinism_is_characterized_v2.md` correction
supersedes its procedural wording without changing the result, and its checksum-linked ten-sample
human review passed on 2026-08-26.

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
| Fixed-seed behavior | Container bytes vary; decoded PCM identical in 10/10 | 10 container hashes, 1 `data` hash, `PEAK` timestamp only, correlation/cosine `1.0` — ten generations of one identical approved input under seed `42` through one loaded worker | Characterized, not a reconstruction guarantee |
| Listener assessment | Perceptual acceptability of the rendered voice | The approved listening-review procedure, blind and checksum-bound. At G0 that was the ten fixed-seed outputs, accepted 10/10 with no required-category findings; the count came from reusing that run's artifacts and is not a requirement of this row | Qualified for owner-only private use |
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

**Re-earning the waiver after a condition-2 expiry** requires re-establishing every §Required
evidence item the change could have invalidated, and no more. An item the change cannot reach
carries forward on its original evidence; an item it can reach is re-supplied for the new
configuration. Nothing is dropped, and nothing unaffected is repeated for its own sake. The record
re-earning the waiver states, item by item, which it re-established and which it carried forward,
so a reader can see the standard being met rather than infer it.

For a **speech-affecting worker input** — condition 2's own example, and the case a backend uplift
presents — the items it can reach are the worker identity, the fixed-seed determinism
characterization, the listener assessment, the performance measurements, and the worker-bundle and
FFmpeg identities. Source revisions, licences, permitted scope, reference-machine inventory, and
voice consent and checksum records are unreached by such a change and carry forward.

The paragraph above this one is scoped to **G3** and is not the requalification standard; reading it
as one was the ambiguity §Amendments records.

Rollback requires no data migration. Revoke this waiver, mark the affected gate blocked, retain
all valid artifacts, and stop real-backend expansion while the hardware/backend decision is
reopened.

## Amendments

### 2026-09-02 — the fixed-seed and listener obligations are separate

**What was ambiguous.** §Required evidence has always listed "ten-run fixed-seed determinism
characterization" and "voice consent/checksum and listener assessment" as separate items. The
Decision table did not: one **Fixed-seed behavior** cell carried both the ten container hashes and
"10/10 blind samples accepted", so the two obligations read as one and the number `10` appeared to
belong to both.

**The observed consequence.** At the 2026-09-02 backend uplift the requalification record cited the
*listener assessment* requirement while quoting the *fixed-seed* row's `10/10`, and a reviewer then
read the six-line committed listening fixture
(`fixtures/listening/e1-s3-listening-script.json`) as four samples short of a requirement. It was
not: the two are different instruments answering different questions, and no numerical relationship
between them was ever intended. That misreading is exactly the coupling this amendment removes.

**The correction.** The Decision table now carries a separate **Listener assessment** row. The
fixed-seed row keeps its own evidence and states what the ten are — ten generations of one identical
approved input under seed `42` through one loaded worker. The listener row records that G0's count
of ten came from reusing that run's artifacts rather than from any requirement.

**Neither requirement is reduced, and no historical conclusion changes.** The ten-run
characterization is still ten runs. The listener assessment is still the approved blind,
checksum-bound procedure. G0's recorded results stand exactly as measured.

**What follows from it.** A re-characterization of fixed-seed determinism does not owe a blind
listening of its ten takes. Where decoded-PCM evidence establishes the ten are identical, listening
to all ten adds nothing to the characterization of determinism — one is the same audio as the other
nine. The listener assessment is satisfied by the approved procedure on its own terms, at whatever
sample count that procedure defines.

**And one thing this amendment does not do.** It does not revisit the G0 results, the waiver, or
the failed performance measurements, all of which stand unchanged.

## Acceptance

ADR-0002 is accepted with the scoped waiver above. E0-S3 closes through an approved deviation,
and its feasibility portion of G0 receives a conditional pass for development progression. E0-S4
may begin. Overall G0 remains open until E0-S4 supplies the provisional contract baseline. The
failed performance results remain visible, and full-box performance qualification remains
mandatory before G3 acceptance.

The M2 and M3 implementation forecasts resume after overall G0 closure. A forecast is not gate
evidence, and G3/M3 acceptance remains subject to the waiver's expiry condition.
