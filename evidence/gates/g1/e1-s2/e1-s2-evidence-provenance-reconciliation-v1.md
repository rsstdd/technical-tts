# E1-S2 Evidence Provenance Reconciliation v1

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Candidate revision: working tree on `main` at the E1-S2 implementation
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted

## Scope and decision

E1-S2 edited ten repository files that `e1-s1-provisional-contract-baseline-v13` pins by SHA-256.
That record is Accepted and therefore immutable, and
`python3 scripts/check-evidence-provenance.py` reports each as an unaccounted mismatch until this
record is accepted.

`evidence/README.md` §Provenance allows two instruments: recompute and supersede, or write a
record showing the conclusion stands. Superseding v13 is wrong here on both counts the README
gives. It would be E1-S2 rewriting E1-S1's baseline record, and a version after acceptance "means
a conclusion was wrong" — E1-S1's conclusions did not become wrong. Every one of the ten files
still delivers what v13 measured; E1-S2 extended the boundaries they implement. So this is a
reconciliation, and it accounts for exactly the ten pairs below.

This record adds no rows to any other citing record and drops none of the twenty-eight rows
`e1-s1-evidence-provenance-reconciliation-v2` carries. That record remains in force for its own
accounting; this one grants only what is listed here.

Every digest in this record is written truncated, for the reason
`e1-s1-evidence-provenance-reconciliation-v2` §Scope and decision gives: the checker pins every
backticked repository path preceding a full-length SHA-256 in a table row, so a record stating
current digests in a table would strand itself the moment those files moved again.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all four hold:

1. Each of the ten mismatches names a file E1-S2 changed for a reason recorded in
   `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md` or
   `e1-s2-canonical-lesson-workflow-v1.md`.
2. No change among them contradicts a conclusion `e1-s1-provisional-contract-baseline-v13`
   reached, as opposed to extending the boundary it measured.
3. No control the pinned files carried was weakened: no validation, containment, rights,
   checksum, consent, offline, or recovery behavior was removed or relaxed to let this story pass.
4. `python3 scripts/check-evidence-provenance.py` exits zero once this record is accepted, with no
   other record's rows altered.

## Result

| Cited path | Pinned at | What E1-S2 changed, and why the v13 conclusion stands |
|---|---|---|
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef…` | Lesson `1.1` → `2.1`: required `speakers`, optional `editorial`, three new error variants, and `LessonDiagnostic`. v13 concluded the lesson boundary validates authored data with one distinct variant per invariant before planning. That is still what it does, over more invariants and with each refusal located. |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6f…` | `BuildError::Lesson` now carries `Box<LessonDiagnostic>` instead of `LessonError`. v13's conclusion about this file is the 80-byte `BuildError` baseline and the transparent category boundaries; both hold, and `t1_e0_build_error_does_not_grow_during_category_refactor` still passes at 80 bytes, which is why the box is there. |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317…` | The two version-gate tests moved to the `2.x` fixtures and now also assert the field path; `PUBLISHED_REQUIRED_SURFACE` gained the `lesson 2.1` rows. v13 concluded that the published required-field surface is explicit and reviewable where it is made. This change is exactly that mechanism doing its job: the surface could not move without editing the table. |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e8…` | The seam scenarios supply a voice-conditioning map and a voice-profile root. v13 concluded every provisional seam has a fake that passes the shared suite; it still does, at `e1.tts-executor.2.0`. |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `c9cffad3e858…` | `t4_e1_pr_suite_performs_no_model_download` installs the synthetic voice profiles the fixture lesson now names. Its conclusion — the PR suite renders a real lesson through the fake and reaches no model artifact — is unchanged and still asserted. |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a5…` | Rows re-pinned for the edited lesson and contract fixtures; the `e1-s1-prior-minor` and `e1-s1-unknown-major` rows replaced by their `e1-s2` successors. v13 concluded every active fixture carries a row whose checksum matches its bytes, enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest`. That test passes, which is the conclusion. |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e509…` | The lesson-boundary ownership row, the version paragraph, and a note that `speakers` and `editorial` add no ceiling. v13 concluded the integration order and the provisional ceilings are recorded and executable; both still are, and the walking-skeleton suite is green. |
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae…` | The executor row moved to `e1.tts-executor.2.0` and a paragraph records why. v13 named the voice-conditioning hash as an input "not resolved to real values until E1-S2"; this edit is that sentence coming true, which confirms the conclusion rather than contradicting it. |
| `AGENTS.md` | `a561d78d628e…` | §State gained one sentence describing E1-S2, as its own last sentence requires: "Do not describe planned commands, schemas, workers, or audio behavior as present until they exist and are verified." Leaving it unedited would have made the file wrong. |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4…` | The "Canonical reviewed lesson only" row names declared-voice resolution and located diagnostics beside the review-state rejection it already named. Its story column already said E1-S2; only the controls column moved. |

Criterion 3 in particular: nothing here relaxed a control. The changes in the direction of strength
are that a voice profile must now be declared, resolved, consent-gated, and checksummed before a
plan exists at all, and that `BuildRequest` refuses a build with nowhere to resolve one rather
than defaulting to none.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no others; neither this
record's existence nor a prose mention suppresses a mismatch.

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

## Open findings

None. With this record accepted, `python3 scripts/check-evidence-provenance.py` exits zero and
CI's `lint` job is green. Each of the ten rows was read against the change that moved it before
acceptance; the §Result table above is that reading, and it is what the decision below rests on.

## Verification

| Command | Result |
|---|---|
| `python3 scripts/check-evidence-provenance.py` | 0 unaccounted mismatches with this record accepted; 10 while it was Proposed |
| `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | Pass |

## Decision

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept that each of the ten cited documents moved for a reason recorded in `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md` or `e1-s2-canonical-lesson-workflow-v1.md`, and that none of the changes weakened a validation, containment, rights, checksum, consent, offline, or recovery control | 2026-08-29 |
| Project owner | Ross Todd | Accept that `e1-s1-provisional-contract-baseline-v13` stands unsuperseded: its conclusions were extended by E1-S2, not contradicted, so a successor would signal a wrong conclusion where none was made | 2026-08-29 |
