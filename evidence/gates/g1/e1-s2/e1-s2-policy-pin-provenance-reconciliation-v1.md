# E1-S2 Policy-Pin Provenance Reconciliation v1

- Status: Accepted
- Supersedes: nothing
- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: `e1s2/follow-up-2` with the issue #58 policy-pin follow-up
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner

## Scope and decision

Issue #58 moves two files pinned by the accepted
`e1-s1-provisional-contract-baseline-v15`: `crates/study-tts-core/src/lesson.rs` gains two
colocated T3 policy tests, and `docs/governance/TRACEABILITY-MATRIX.md` names those controls while
preserving the role/style limitation. v15 is immutable, and superseding it would be wrong: none of
its conclusions changed and its complete controlled-record table remains the E1-S1 baseline. This
record instead accounts for exactly those two moved pins.

No production constant, validation branch, schema, public API, fixture, dependency, or audio path
moves. The new tests compare production constants with literal values a reviewer can read against
their controlling documents; they do not re-derive those values from production behavior. The
closed `SegmentRole` and `DeliveryStyle` vocabularies remain deliberately outside the new control
because ADR-0001 describes them in prose rather than supplying an exact normative list.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all five hold:

1. `t3_e1_recall_response_interval_matches_adr` independently pins ADR-0001 §13.2's exact
   `1_500` and `4_000` millisecond endpoints.
2. `t3_e1_provisional_lesson_resource_ceilings_match_walking_skeleton_document` independently
   pins all ten lesson constants represented by
   `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings, with a failing case
   naming its resource.
3. An isolated mutation of one recall constant and one lesson ceiling proves that each new test
   detects policy drift which the existing derived-boundary tests do not.
4. Traceability describes role/style as a two-sided but non-mechanized mirror and preserves the
   trigger after issue #58 closes: revisit when ADR-0001 gains an exact normative list or real
   drift occurs.
5. The required local checks pass, and `python3 scripts/check-evidence-provenance.py` reports zero
   mismatches after this record receives both approvals and becomes `Accepted`.

## Result

| Criterion | Result |
|---|---|
| 1. Recall endpoints | Met. The T3 compares the pair of production constants directly with `(1_500, 4_000)` and passes after restoration. |
| 2. Lesson ceilings | Met. A named ten-case table pins lesson JSON, segment count, objectives, lesson references, the shared display/spoken text ceiling, segment source references, and aggregate authored text. |
| 3. Independent mutation proof | Met. Changing the recall floor to `1_499` left the existing recall behavior test green and failed only the new ADR pin. Changing the learning-objective count to `65` left the other 97 core library tests green and failed only the new ceiling pin, naming `learning objectives per lesson`. Both constants were restored. |
| 4. Remaining limitation | Met. The traceability row names both T3 controls and states why no role/style expected array exists and what triggers reconsideration after issue #58 closes. |
| 5. Verification and provenance | Met. Every implementation check below passes, and accepting this record makes the provenance checker recognize exactly the two accounting rows below and report zero mismatches. |

## Accounted provenance mismatches

These rows take effect only while this reconciliation is accepted.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-core/src/lesson.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/governance/TRACEABILITY-MATRIX.md` |

## Remaining limitation

`SegmentRole` and `DeliveryStyle` are still interpretation-backed vocabularies. Adding an expected
array now would duplicate the implementation without an independent normative list to transcribe,
so it would give a false impression of mechanized agreement. Issue #58 closes with that limitation
recorded; revisit it when ADR-0001 supplies the list or real drift demonstrates a narrower
independent control.

## Verification

| Command | Result |
|---|---|
| `cargo test --offline --locked -p study-tts-core t3_e1_recall_response_interval_matches_adr` | Pass, 1 test |
| `cargo test --offline --locked -p study-tts-core t3_e1_provisional_lesson_resource_ceilings_match_walking_skeleton_document` | Pass, 1 test |
| `cargo fmt --all -- --check` | Pass |
| `python3 scripts/check-rust-conventions.py` | Pass |
| `cargo clippy --workspace --all-targets --all-features --offline --locked -- -D warnings` | Pass |
| `cargo test --workspace --offline --locked --all-targets` | Pass, 311 tests |
| `cargo test --offline -p study-tts-testkit --test walking_skeleton --locked` | Pass, 35 tests with real FFmpeg and ffprobe |
| `python3 scripts/check-evidence-provenance.py` | Pass, zero mismatches after acceptance; while Proposed it reported exactly the two v15 pins named above |
| `git diff --check` | Pass |

## Decision

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project but requires each role to accept its risk separately.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept the two test-only policy pins, their isolated mutation evidence, and accounting for v15's `lesson.rs` pin without changing production behavior or the baseline conclusion | 2026-08-30 |
| Project owner | Ross Todd | Approve — accept the updated traceability route, close issue #58 with the role/style limitation and revisit trigger recorded, and account for v15's traceability pin without superseding v15 | 2026-08-30 |
