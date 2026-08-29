# Test Strategy

## TDD rule

Deterministic product behavior follows red-green-refactor. Write the smallest failing test that proves the intended behavior, implement the minimum coherent change, then refactor while the suite remains green. Experiments, measurements, legal review, and listener judgments use evidence protocols rather than fake automated tests.

## Test tiers

| Tier | Purpose | Pull-request policy | Budget |
|---|---|---|---:|
| T1 | Pure unit behavior | Every PR | 30 seconds total |
| T2 | Properties and invariants | Every PR | 2 minutes total |
| T3 | Schemas and reviewed goldens | Every PR | 30 seconds total |
| T4 | Filesystem, fake worker, fixture audio, FFmpeg | Every PR | 5 minutes total |
| T5 | Real Chatterbox, ASR, reference-machine qualification | Scheduled/manual | 30 minutes unless declared |
| T6 | Full lessons, listening, soak, clean-machine release | Gate/release | Hours |

## Test location and naming

- Keep pure tests beside their owning module.
- Put shared fixtures, fake workers, fault injection, and audio helpers in `study-tts-testkit`.
- Name every test `t<tier>_e<epic>_<behavior>`, as in `t3_e1_unknown_major_version_is_rejected`.
- Name evidence `evidence_<epic>_<claim>` and store its protocol separately from its result.
- Give every regression a name describing the externally meaningful invariant.

**A name in the Delivery Plan is a contract.** Copy it character for character and never rename
it: the plan, the traceability matrix, and the evidence records all look it up literally. A helper
test the plan does not name is free to rename. `grep DELIVERY-PLAN.md` before claiming either.

**The tier prefix must match what the test does**, not where it happens to live. Spawning or
resolving an interpreter is T4 whether or not the test is colocated with its module; T1 is for
pure deterministic functions. This is now load-bearing rather than cosmetic: the tier prefix is
the filter `.github/workflows/ci.yml` §Report tier durations runs the suite under, so a
mislabelled test bills its time against the wrong budget above and makes the report say something
untrue. That report is what satisfies `DELIVERY-PLAN.md` §3.3, "CI reports tier duration so a
budget regression is visible."

## Required suites

| Suite | Minimum coverage |
|---|---|
| Walking skeleton | Two segments, fake synthesis, cache, Rust PCM, real M4A, manifest, offline CI |
| Contract | Fake and real worker frames, size limits, errors, cancellation, paths |
| Identity | Field sensitivity, canonical bytes, worker bundle, ASR-only invalidation |
| Filesystem | Atomic writes, checksums, locks, crash points, reconciliation, containment |
| Audio | Format validation, over-range, padding, ramps, joins, sample arithmetic, codecs |
| Takes/cache | Stale base, manifest propagation, retake isolation, prune roots |
| Authoring | Parser properties, Unicode, idempotence, protected terms, reviewed goldens |
| Verification | Fixed decoder, expected lattice, pattern promotion, seeded defects, state transitions |
| Runtime | Parallel dispatch, CPU/RAM budgets, timeouts, cancellation, orphan cleanup |
| Release | Clean machine, soak, rights, SBOM, checksums, rollback, publication refusal |

## Offline and fixture policy

Dependency restoration may use network access. After restoration, T1–T4 run offline and never download models. Fixtures must be small, deterministic, licensed, nonsensitive, and stored beneath assigned temporary roots during execution.

## Failure policy

- A flaky test is a defect. Quarantine requires an owner, expiry, issue, and unaffected-gate analysis.
- An ignored or weakened test blocks story closure unless an approved deviation explicitly replaces its control.
- Golden changes require a reviewed reason and before/after artifact.
- Environment-specific failures remain gate failures when the named reference environment is part of the requirement.
- No single metric answers content accuracy, pronunciation, voice quality, continuity, loudness, and structural integrity.

## Pull-request minimum

Run formatting, check, Clippy, targeted tests, and every affected T1–T4 suite. Speech-affecting changes also require an in-context listening record or an explicit statement that listening remains a gate prerequisite.

