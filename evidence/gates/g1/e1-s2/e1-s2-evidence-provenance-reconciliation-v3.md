# E1-S2 Evidence Provenance Reconciliation v3

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: working tree on `main` at the E1-S2 implementation, after the fifth review
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: `e1-s2-evidence-provenance-reconciliation-v2`

## Scope and decision

This record supersedes `e1-s2-evidence-provenance-reconciliation-v2` and carries its eleven
accounted pairs forward against the bytes those files hold now. It adds no pair.

**Why a successor and not an edit.** v2 is Accepted, and `evidence/README.md` §Provenance forbids
amending an accepted record in place. A version after acceptance signals that a conclusion was
wrong, and one here was — the same way v2's predecessor was wrong, one round later. v2 names its
candidate revision "after the third review" and its criterion 4 limits the reading to
"the four corrections in `E1-S2-INTERFACE-CHANGE-002.md` §Identification items 5 through 8". Two
further rounds have landed since:

- The **fourth review**, recorded as §Identification items 9 through 11, changed content in two of
  the eleven files this record accounts for. `AGENTS.md` had described
  `E1-S2-INTERFACE-CHANGE-002` as accepted and signed when no role had decided anything, and
  `docs/INDEX.md` repeated the claim; both now describe that record as `Proposed` and unsigned.
  Those are corrections to what a reader is told about the repository's own governance state, and
  they are exactly the kind of edit a suppression must have read before it grants one.
- The **fifth review**, recorded as §Identification item 12, changed
  `crates/study-tts-core/src/lesson.rs` to refuse a `speakers` object binding one name twice, and
  corrected three stale statements in `docs/architecture/WALKING-SKELETON.md`,
  `docs/INDEX.md`, and `E1-S2-INTERFACE-CHANGE-002.md` itself.

A reconciliation grants a suppression on the strength of its stated reading. `check-evidence-provenance.py`
suppresses by `(citing record, cited path)` pair, so a pair granted for one reason stays suppressed
through every later edit of that file: the script cannot see that the reading has fallen behind the
bytes. That is the defect this record closes, and it is the second time it has been closed — which
is why §Open findings below records the standing obligation rather than declaring the class solved.

**What is not being claimed.** Superseding v2 does not withdraw its decision, and does not withdraw
v1's. Every change both records read is still read here, and the conclusion is unchanged:
`e1-s1-provisional-contract-baseline-v13` stands unsuperseded, because E1-S2 extended the
boundaries its ten files implement rather than contradicting anything it measured. What changes is
that the reading now covers all five rounds of E1-S2 instead of the first three.

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
4. The reading below covers E1-S2 as it now stands, including all twelve corrections in
   `E1-S2-INTERFACE-CHANGE-002.md` §Identification items 1 through 12, rather than the first eight.
5. `python3 scripts/check-evidence-provenance.py` exits zero once this record is accepted and v2 is
   superseded, with no other record's rows altered.

## Result

| Cited path | Pinned at | What E1-S2 changed, and why the v13 conclusion stands |
|---|---|---|
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef…` | Lesson `1.1` → `3.1`: required `speakers`, optional `editorial`, ADR-0001 §8.1's `learning_objectives` and `source`, the closed `role` and `style` vocabularies, §13.2's recall response interval enforced at both ends, `LessonDiagnostic`, and `LessonError` grown from 29 to 42 variants. The forty-second is `DuplicateSpeaker`, added by the fifth review: `speakers` is a `BTreeMap`, a map keeps the last value for a repeated key, and `serde` has no rule of its own for one, so a document binding `nadia` to two voice profiles validated and was synthesized under whichever the author wrote last. v13 concluded the lesson boundary validates authored data with one distinct variant per invariant before planning. That conclusion was *false* while a repeated binding was resolved silently — a document could violate the invariant "one speaker, one voice" and receive no refusal at all — and it is true again now. The same is so of the six vocabulary variants, for the reason v2 gave. |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6f…` | `BuildError::Lesson` now carries `Box<LessonDiagnostic>` instead of `LessonError`. v13's conclusion about this file is the 80-byte `BuildError` baseline and the transparent category boundaries; both hold, and `t1_e0_build_error_does_not_grow_during_category_refactor` still passes at 80 bytes, which is why the box is there. Unchanged since v1 read it. |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317…` | The two version-gate tests moved to the `3.x` fixtures and now also assert the field path; `PUBLISHED_REQUIRED_SURFACE` carries the `lesson 3.1` rows, and its plan rows moved from `plan 1.0` to `plan 2.0`. v13 concluded that the published required-field surface is explicit and reviewable where it is made. Both moves are that mechanism doing its job: neither surface could change without editing the table, and the plan's major bump was found *because* the table made the added required field visible. Unchanged since v2 read it. |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e8…` | The seam scenarios supply a voice-conditioning map and a voice-profile root, and construct `PlannedSegment` with `display_text` and a typed `style`. v13 concluded every provisional seam has a fake that passes the shared suite; it still does, at `e1.tts-executor.2.0`. Unchanged since v2 read it. |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `c9cffad3e858…` | `t4_e1_pr_suite_performs_no_model_download` installs the synthetic voice profiles the fixture lesson now names. Its conclusion — the PR suite renders a real lesson through the fake and reaches no model artifact — is unchanged and still asserted. Unchanged since v2 read it. |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a5…` | Rows re-pinned for the edited lesson and contract fixtures; the `e1-s1-prior-minor` and `e1-s1-unknown-major` rows replaced by their `e1-s2` successors at `3.0` and `4.0`. v13 concluded every active fixture carries a row whose checksum matches its bytes, enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest`. That test passes, which is the conclusion. The fifth review added no fixture — its refusal is a repeated JSON key, which no committed fixture can carry without being a fixture whose only readers are the parsers that reject it — so no row moved. |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e509…` | The lesson-boundary ownership row, the version paragraph, a note that `speakers` and `editorial` add no ceiling, and the four `3.1` resource ceilings for objectives and references. The fifth review corrected the version paragraph again: it still said the schema was published at `schemas/lesson-v2.schema.json` with `2.0`/`2.1` as the current pair, three rounds after the file was deleted and `lesson-v3.schema.json` published in its place. v13 concluded the integration order and the provisional ceilings are recorded and executable; both still are, and correcting a paragraph that named a deleted schema is that conclusion being restored rather than moved. The walking-skeleton suite is green at 35 tests. |
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae…` | The executor row moved to `e1.tts-executor.2.0`, a paragraph records why, and the render-plan paragraph now records the move to `plan 2.0`. v13 named the voice-conditioning hash as an input "not resolved to real values until E1-S2"; that edit is the sentence coming true. The plan paragraph is a correction: it previously said the plan's shape moved under an unmoved `1.0`, which `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes does not permit. Unchanged since v2 read it. |
| `AGENTS.md` | `a561d78d628e…` | §State gained a sentence describing E1-S2, as its own last sentence requires: "Do not describe planned commands, schemas, workers, or audio behavior as present until they exist and are verified." That sentence has been narrowed twice since v1 read it. v2 read the first narrowing: it had said the build "resolves each declared voice profile", where `voice_gate::resolve_speakers` resolves the profile of every speaker a *segment names*. The **fourth review** made the second, which v2 did not read: §State described `E1-S2-INTERFACE-CHANGE-002` as accepted and signed while no role had decided anything, so a reader was told an implemented change was an accepted one. It now describes that record as `Proposed` and unsigned, names four rounds of correction, and names this reconciliation rather than v2. The fifth review added the repeated-speaker refusal to the same sentence. Every one of those edits moves a claim toward what the tree holds; none changes a control, and v13's conclusion about §State is that it describes the build truthfully. |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4…` | The "Canonical reviewed lesson only" row names declared-voice resolution and located diagnostics beside the review-state rejection it already named, and the fifth review added "one voice per declared speaker" to the same list. Its story column already said E1-S2; only the controls column has moved, twice. v13 concluded that every requirement names the controls that hold it; a control that exists and is not named is that conclusion half-held, which is why the row moved rather than the refusal going unlisted. |
| `docs/INDEX.md` | `3d1806b32e6d…` | Three rows added: the two E1-S2 interface-change records and the E1-S2 evidence pair. The **fourth review** then corrected the interface-change row, which had described `E1-S2-INTERFACE-CHANGE-002` as accepted; it now says `Proposed` and unsigned. The fifth review corrected two more statements in the same two rows: the reconciliation row said v2 accounted for ten pins where v2 itself accounts for eleven, and the interface-change row did not say how many corrections that record now carries. v13 concluded the index names every governing document a reader must find. An index that names them but misstates their status is that conclusion half-held; these corrections restore it. No row was removed. |

Criterion 3 in particular: nothing here relaxed a control, and the two rounds v2 could not read both
strengthened one. The fifth review's `LessonError::DuplicateSpeaker` closes a path by which a
reviewed lesson could be spoken in a voice the review never selected — ADR-0001 §12.5 makes the
resolved conditioning artifact a synthesis-key input, so the selection was reaching a cache key. The
fourth review changed no behavior at all: it corrected three documents that claimed an approval no
role had given, which is a control over what a reader may rely on rather than over what the build
does. Everything v2 recorded under this criterion still holds: a voice profile must be declared,
resolved, consent-gated, and checksummed before a plan exists; `BuildRequest` refuses a build with
nowhere to resolve one; a role, style, or review state outside ADR-0001 §8.2's vocabularies is
refused; and a recall prompt outside §13.2's range is refused where the generic 10,000 ms ceiling
had accepted 4,001-10,000 ms.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no others; neither this
record's existence nor a prose mention suppresses a mismatch. They are the eleven pairs
`e1-s2-evidence-provenance-reconciliation-v2` carried, restated because superseding that record
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

One, and it is the reason this is a third version rather than a second.

**A granted pair does not expire, and the script cannot tell that a reading has.** Suppression is
keyed on `(citing record, cited path)`, so once a pair is accounted for, every later edit of that
file passes silently — including one that reverses a conclusion. Two rounds of E1-S2 landed inside
that blind spot before anyone looked, and the only thing that found them was a person reading the
records against the tree. This is a property of the mechanism, not a defect in it: a checker that
re-decided semantics would be deciding what a reconciliation is for. What follows from it is a
standing obligation, recorded here so the next round does not have to rediscover it: **a review
that changes a file named in the table above owes this reconciliation a successor, whether or not
the script complains.** Mechanizing any part of that — a marker recording which review a pair was
granted under, say — is a change to `evidence/README.md` §Provenance and to the script, and
belongs to whoever proposes it rather than to this record.

Two things this record deliberately does not do. It does not supersede
`e1-s1-provisional-contract-baseline-v13`, for the reason v1 and v2 gave and this record's
criterion 2 re-checks: E1-S1's conclusions were extended, and the two that had come apart from the
tree — one distinct variant per lesson invariant, and an index that states each record's true
status — are extended again by being restored. And it does not accept
`e1-s2-canonical-lesson-workflow-v1`, which stays `Proposed` until G1 for the reason that record's
own §Open findings gives — a story record is accepted at the gate it serves, against the bytes that
gate approved.

## Verification

| Command | Result |
|---|---|
| `python3 scripts/check-evidence-provenance.py` | 0 unaccounted mismatches with this record accepted and v2 superseded |
| `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | Pass |
| `cargo test --workspace --all-targets --locked` | Pass |

## Decision

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that each of the eleven cited documents moved for a reason recorded in `E1-S2-INTERFACE-CHANGE-001.md`, `E1-S2-INTERFACE-CHANGE-002.md`, or `e1-s2-canonical-lesson-workflow-v1.md`, that the reading above covers all five rounds of E1-S2, and that none of the changes weakened a validation, containment, rights, checksum, consent, offline, or recovery control — two of them strengthened one | 2026-08-30 |
| Project owner | Ross Todd | Approve — accept superseding `-v2` on the ground that its reading, not its decision, was wrong: it granted eleven suppressions against E1-S2 as it stood after the third review, and two later rounds edited three of those files, including one that corrected `AGENTS.md` and `docs/INDEX.md` for claiming an approval no role had given. Accept the standing obligation in §Open findings, and that `e1-s1-provisional-contract-baseline-v13` stands unsuperseded | 2026-08-30 |
