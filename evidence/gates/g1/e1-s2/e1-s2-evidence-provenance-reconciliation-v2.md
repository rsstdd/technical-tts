# E1-S2 Evidence Provenance Reconciliation v2

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Candidate revision: working tree on `main` at the E1-S2 implementation, after the third review
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: `e1-s2-evidence-provenance-reconciliation-v1`

## Scope and decision

This record supersedes `e1-s2-evidence-provenance-reconciliation-v1`, SHA-256
`a85b3f5116ba78cb2d5c5a4646e1391ea3a6cafec0b3aee8b42aa543abd58ef1`, carries its ten accounted
pairs forward against the bytes those files hold now, and adds an eleventh.

**Why a successor and not an edit.** v1 is Accepted, and `evidence/README.md` §Provenance forbids
amending an accepted record in place. A version after acceptance signals that a conclusion was
wrong, and one here was: v1's §Result reads each of the ten mismatches against a lesson document
moving `1.1` → `2.1` and against the change recorded in
[`../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-001.md).
Two further rounds of work landed after it was accepted —
[`E1-S2-INTERFACE-CHANGE-002.md`](../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-002.md) and
the third review recorded in its §Identification items 5 through 8 — and the lesson document is now
`3.1`, the render plan `2.0`, and `LessonError` has 41 variants. A reconciliation grants a
suppression on the strength of its stated reading; a reading that describes fewer changes than the
bytes carry grants more than it examined. That is the defect this record closes.

**What is not being claimed.** Superseding v1 does not withdraw its decision. Every change v1 read
is still read here, and the conclusion is the same: `e1-s1-provisional-contract-baseline-v13`
stands unsuperseded, because E1-S2 extended the boundaries its ten files implement rather than
contradicting anything it measured. What changes is that the reading now covers all of E1-S2
instead of its first round.

**The eleventh pair is `docs/INDEX.md`.** v1 could not grant it, and said so: the index had no row
for either E1-S2 interface-change record, because `e1-s1-provisional-contract-baseline-v13` pins
that file and no accepted record accounted for it moving. That was a real deadlock — the index was
wrong about what the repository contains, and correcting it was blocked on the record that is now
being written. This record breaks it in the only order that works: the reading below covers the
index edit, so the edit and its accounting are accepted together.

This record adds no rows to any other citing record and drops none of the twenty-eight rows
`e1-s1-evidence-provenance-reconciliation-v2` carries. That record remains in force for its own
accounting; this one grants only the eleven pairs listed below.

Every digest in this record is written truncated, for the reason
`e1-s1-evidence-provenance-reconciliation-v2` §Scope and decision gives: the checker pins every
backticked repository path preceding a full-length SHA-256 in a table row, so a record stating
current digests in a table would strand itself the moment those files moved again.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all five hold:

1. Each of the eleven mismatches names a file E1-S2 changed for a reason recorded in
   `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`,
   `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`, or
   `e1-s2-canonical-lesson-workflow-v1.md`.
2. No change among them contradicts a conclusion `e1-s1-provisional-contract-baseline-v13`
   reached, as opposed to extending the boundary it measured.
3. No control the pinned files carried was weakened: no validation, containment, rights,
   checksum, consent, offline, or recovery behavior was removed or relaxed to let this story pass.
4. The reading below covers E1-S2 as it now stands, including the four corrections in
   `E1-S2-INTERFACE-CHANGE-002.md` §Identification items 5 through 8, rather than its first round.
5. `python3 scripts/check-evidence-provenance.py` exits zero once this record is accepted and v1 is
   superseded, with no other record's rows altered.

## Result

| Cited path | Pinned at | What E1-S2 changed, and why the v13 conclusion stands |
|---|---|---|
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef…` | Lesson `1.1` → `3.1`: required `speakers`, optional `editorial`, ADR-0001 §8.1's `learning_objectives` and `source`, the closed `role` and `style` vocabularies, §13.2's recall response interval enforced at both ends, `LessonDiagnostic`, and `LessonError` grown from 29 to 41 variants. v13 concluded the lesson boundary validates authored data with one distinct variant per invariant before planning. That is what it does, over more invariants and with each refusal located — and more exactly than when v1 read it: the six vocabulary variants exist because the intermediate design let three invariants share `InvalidJson`, which would have made v13's conclusion false. Correcting that is what restored it. |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6f…` | `BuildError::Lesson` now carries `Box<LessonDiagnostic>` instead of `LessonError`. v13's conclusion about this file is the 80-byte `BuildError` baseline and the transparent category boundaries; both hold, and `t1_e0_build_error_does_not_grow_during_category_refactor` still passes at 80 bytes, which is why the box is there. Unchanged since v1 read it. |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317…` | The two version-gate tests moved to the `3.x` fixtures and now also assert the field path; `PUBLISHED_REQUIRED_SURFACE` carries the `lesson 3.1` rows, and its plan rows moved from `plan 1.0` to `plan 2.0`. v13 concluded that the published required-field surface is explicit and reviewable where it is made. Both moves are that mechanism doing its job: neither surface could change without editing the table, and the plan's major bump was found *because* the table made the added required field visible. |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e8…` | The seam scenarios supply a voice-conditioning map and a voice-profile root, and construct `PlannedSegment` with `display_text` and a typed `style`. v13 concluded every provisional seam has a fake that passes the shared suite; it still does, at `e1.tts-executor.2.0`. |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `c9cffad3e858…` | `t4_e1_pr_suite_performs_no_model_download` installs the synthetic voice profiles the fixture lesson now names. Its conclusion — the PR suite renders a real lesson through the fake and reaches no model artifact — is unchanged and still asserted. |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a5…` | Rows re-pinned for the edited lesson and contract fixtures; the `e1-s1-prior-minor` and `e1-s1-unknown-major` rows replaced by their `e1-s2` successors at `3.0` and `4.0`. v13 concluded every active fixture carries a row whose checksum matches its bytes, enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest`. That test passes, which is the conclusion. |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e509…` | The lesson-boundary ownership row, the version paragraph, a note that `speakers` and `editorial` add no ceiling, and the four `3.1` resource ceilings for objectives and references. v13 concluded the integration order and the provisional ceilings are recorded and executable; both still are, and the walking-skeleton suite is green at 35 tests. |
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae…` | The executor row moved to `e1.tts-executor.2.0`, a paragraph records why, and the render-plan paragraph now records the move to `plan 2.0`. v13 named the voice-conditioning hash as an input "not resolved to real values until E1-S2"; that edit is the sentence coming true. The plan paragraph is a correction: it previously said the plan's shape moved under an unmoved `1.0`, which `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes does not permit. |
| `AGENTS.md` | `a561d78d628e…` | §State gained a sentence describing E1-S2, as its own last sentence requires: "Do not describe planned commands, schemas, workers, or audio behavior as present until they exist and are verified." That sentence is now narrower than when v1 read it: it said the build "resolves each declared voice profile", and `voice_gate::resolve_speakers` resolves the profile of every speaker a *segment names*. The claim was corrected to what the code does rather than the code widened to the claim, because an unused declaration is never synthesized from — the reasoning is in `e1-s2-canonical-lesson-workflow-v1.md` §Deviations. |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4…` | The "Canonical reviewed lesson only" row names declared-voice resolution and located diagnostics beside the review-state rejection it already named. Its story column already said E1-S2; only the controls column moved. |
| `docs/INDEX.md` | `3d1806b32e6d…` | Three rows added: the two E1-S2 interface-change records and the E1-S2 evidence pair. v13 concluded the index names every governing document a reader must find. Adding rows for documents that exist is that conclusion holding; leaving them out is what made the file wrong. No existing row changed. |

Criterion 3 in particular: nothing here relaxed a control, and one row strengthened one. The changes
in the direction of strength are that a voice profile must be declared, resolved, consent-gated, and
checksummed before a plan exists at all; that `BuildRequest` refuses a build with nowhere to resolve
one rather than defaulting to none; that a role, style, and review state outside the vocabularies
ADR-0001 §8.2 declares are refused rather than passed through; and that a recall prompt outside
§13.2's 1.5-4 second range is now refused, where the generic 10,000 ms segment ceiling had accepted
4,001-10,000 ms. The AGENTS.md row narrows a *claim*, not a control: no behavior changed with it.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no others; neither this
record's existence nor a prose mention suppresses a mismatch. They are the ten pairs
`e1-s2-evidence-provenance-reconciliation-v1` carried, restated because superseding that record
withdraws its rows along with its reading.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v13` | `crates/study-tts-core/src/lesson.rs` |
| `e1-s1-provisional-contract-baseline-v13` | `crates/study-tts-runtime/src/error/mod.rs` |
| `e1-s1-provisional-contract-baseline-v13` | `crates/study-tts-testkit/tests/schemas.rs` |
| `e1-s1-provisional-contract-baseline-v13` | `crates/study-tts-testkit/tests/provisional_contracts.rs` |
| `e1-s1-provisional-contract-baseline-v13` | `crates/study-tts-testkit/tests/worker_contract.rs` |
| `e1-s1-provisional-contract-baseline-v13` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v13` | `docs/architecture/WALKING-SKELETON.md` |
| `e1-s1-provisional-contract-baseline-v13` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e1-s1-provisional-contract-baseline-v13` | `AGENTS.md` |
| `e1-s1-provisional-contract-baseline-v13` | `docs/governance/TRACEABILITY-MATRIX.md` |
| `e1-s1-provisional-contract-baseline-v13` | `docs/INDEX.md` |

## Open findings

None. With this record accepted and v1 superseded,
`python3 scripts/check-evidence-provenance.py` exits zero and CI's `lint` job is green. Each of the
eleven rows was read against the whole of E1-S2 before acceptance; the §Result table above is that
reading, and it is what the decision below rests on.

Two things this record deliberately does not do. It does not supersede
`e1-s1-provisional-contract-baseline-v13`, for the reason v1 gave and this record's criterion 2
re-checks: E1-S1's conclusions were extended, not contradicted. And it does not accept
`e1-s2-canonical-lesson-workflow-v1`, which stays `Proposed` until G1 for the reason that record's
own §Open findings gives — a story record is accepted at the gate it serves, against the bytes that
gate approved.

## Verification

| Command | Result |
|---|---|
| `python3 scripts/check-evidence-provenance.py` | 0 unaccounted mismatches with this record accepted and v1 superseded |
| `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | Pass |

## Decision

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that each of the eleven cited documents moved for a reason recorded in `E1-S2-INTERFACE-CHANGE-001.md`, `E1-S2-INTERFACE-CHANGE-002.md`, or `e1-s2-canonical-lesson-workflow-v1.md`, that the reading above covers all three rounds of E1-S2 rather than the first, and that none of the changes weakened a validation, containment, rights, checksum, consent, offline, or recovery control | 2026-08-29 |
| Project owner | Ross Todd | Approve — accept superseding `-v1` on the ground that its reading, not its decision, was wrong: it granted ten suppressions against a description of E1-S2 that two later rounds outgrew, and it left `docs/INDEX.md` stranded, and a reconciliation may only suppress what it has actually read. Accept that `e1-s1-provisional-contract-baseline-v13` stands unsuperseded | 2026-08-29 |
