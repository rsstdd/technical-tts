# ADR-0001-D001 — ASR release condition

- **Status:** Approved
- **Date:** 2026-08-23
- **Raised by:** Ross Todd
- **Approved by:** Ross Todd (project owner; single-participant project, see
  `docs/governance/PROJECT-EXECUTION-CHARTER.md`)
- **Affects:** ADR-0001 §17.18, §18, §10.5; `DELIVERY-PLAN.md` §1 and E4-S4;
  `docs/governance/RELEASE-PROFILES.md` §3

## The contradiction

ADR-0001 states two different ASR conditions for release, and they are not compatible.

**§17.18, release gate:**

> ADR-0005's complete ASR calibration corpus passes every per-class numerical gate, stability
> check, and order-invariance check.

**§18, acceptance criteria:**

> Post-render ASR triage runs for every selected cached segment and records its verification
> identity, lattice evidence, findings, and adjudication.

Under §17.18, a failed calibration gate blocks version 1.0 entirely. Under §18, calibration is
not mentioned; only the running and recording of triage is required. The two readings are both
defensible from the document as written, and the difference is material: §17.18 places a
listening-corpus labeling exercise of at least 100 human-verified clean segments plus 50 seeded
examples per defect class on the critical path to release.

This was not noticed while ASR was distant. It would have surfaced at M3 with no way to settle
it, which is the reason to resolve it now rather than later.

## Decision

**Version 1.0 adopts the §18 condition.** ASR triage must run for every selected cached segment
and must record its evidence. That is the gate, recorded as `asr_triage_recorded` in
`docs/governance/RELEASE-PROFILES.md` §3.

Failure of ADR-0005's numerical calibration gates:

- blocks any claim that version 1.0 has automated text-integrity coverage;
- keeps human review authoritative as the correctness gate;
- requires the release record to state that ASR is advisory in 1.0;
- does **not** block release.

## Rationale

ADR-0001 §10.5 already establishes the governing principle, and it points the same way:

> ASR remains a triage sensor: it never changes approved text or independently establishes
> correctness.

A sensor that cannot establish correctness cannot reasonably be the thing that blocks a release
whose correctness is established by human review. §18 is consistent with §10.5; §17.18 is not.
Adopting §18 therefore resolves the contradiction in the direction the ADR's own reasoning
already supports, rather than picking the more convenient of two arbitrary options.

The practical consequence is that a calibration corpus that cannot be labeled in time degrades
the release's claims rather than preventing the release. `DELIVERY-PLAN.md` §9 already reflects
this as the first pre-M3 descope step.

## What is given up

Version 1.0 may ship without demonstrated automated detection of omissions, insertions,
substitutions, repetitions, or hallucinated continuations. The false-negative rate of human
review over a 60-minute lesson is unmeasured, and on a single-participant project there is one
reviewer. That risk is accepted and recorded here rather than mitigated.

## Amendment to ADR-0001 §17.18

Replace:

> - ADR-0005's complete ASR calibration corpus passes every per-class numerical gate, stability
>   check, and order-invariance check;

with:

> - post-render ASR triage runs for every selected cached segment and records its verification
>   identity, lattice evidence, findings, and adjudication. ADR-0005's calibration gates qualify
>   ASR as a release control; failing them keeps human review authoritative and requires the
>   release record to state that ASR is advisory, but does not block release
>   (ADR-0001-D001);

## Verification

`REQUIRED_PRODUCTION_GATES` in `crates/study-tts-core/src/release.rs` contains
`asr_triage_recorded` and does not contain a calibration gate. The list and
`docs/governance/RELEASE-PROFILES.md` §3 must agree.
