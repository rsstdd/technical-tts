# E1-S2 Interface Change 003 — One corrected statement in E1-S2 Interface Change 002

## Identification

- Record ID: `E1-S2-INTERFACE-CHANGE-003`
- Contract owner: T-CORE, as the owner of the record amended
- Engineering owner: Engineering owner
- Affected-track reviewers: none. No track's interface, fake, fixture, or suite is touched
- Accepted ADR, if architectural: not applicable. No authority boundary moves, and no ADR
  statement is implemented, narrowed, or reinterpreted here

This record amends one sentence of
[`E1-S2-INTERFACE-CHANGE-002.md`](E1-S2-INTERFACE-CHANGE-002.md) §Delivery and recovery, in its
evidence-provenance bullet. It supersedes nothing: that record stays in force for every contract,
version, refusal, and test it records, and nothing here contradicts any of them.

**The sentence.** It says of `e1-s2-evidence-provenance-reconciliation-v3` that "v3 restates the
same eleven pairs against the bytes those files hold now and extends the reading to every round of
this record."

**What is true.** v3 read E1-S2 through the **fifth** review. Its own §Scope and decision says so
in those terms — "the reading now covers all five rounds of E1-S2" — and its candidate revision is
recorded as the working tree "after the fifth review". `E1-S2-INTERFACE-CHANGE-002` §Identification
numbers **six** rounds, closing fourteen gaps. The sixth review landed after v3 was accepted and
moved four of the eleven files v3 accounts for: `crates/study-tts-core/src/lesson.rs`,
`docs/architecture/WALKING-SKELETON.md`, `AGENTS.md`, and `docs/INDEX.md` itself. Read the
amended record as: *v3 restates the same eleven pairs against the bytes those files held when it
was written, and extends the reading through the fifth review of this record; the sixth landed
after it was accepted.*

**Why an amendment and not an edit.** `E1-S2-INTERFACE-CHANGE-002` §Approval is signed and says a
further correction "amends this record from outside, in a successor, now that it is in force". The
same rule `evidence/README.md` §Provenance states for evidence records — a reader must be able to
tell a routine revision from a rewritten control without reading both versions — is why that
record says it about itself.

**This was false at signature, not stale after it.** The sixth review was folded into
`E1-S2-INTERFACE-CHANGE-002` §Identification items 13 and 14 before any row was signed, and this
bullet was not updated with it. Every approval on 2026-08-30 was recorded over a sentence that had
already stopped being true — which is why the correction is owed rather than merely tidy.

**It is the fourth instance of one defect class**: a record crediting a reading it did not
perform. `e1-s2-evidence-provenance-reconciliation-v1` and `-v2` were each superseded for it, and
the `docs/INDEX.md` row carrying the same credit was corrected in the change that also wrote
`evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v15.md`. This is the copy that
survived, in the one document that could not be edited in place. The standing obligation that
answers the class is already recorded — `e1-s2-evidence-provenance-reconciliation-v3` §Open
findings: **a review that changes a file named in that record's table owes the reconciliation a
successor, whether or not the script complains.** This record adds no new obligation and does not
claim the class is closed.

## Version and compatibility

- Contract ID: none. No contract is named, moved, or reinterpreted
- Old version: not applicable
- New version: not applicable
- Compatibility class: **unchanged**
- Required/defaulted fields: none
- Unknown-field behavior: unchanged
- Wire or Rust representation changed: none

No schema, Rust signature, wire field, published byte, or identity moves. The change is one
sentence of prose about which records read which bytes, which is why this record carries no
migration, no compatibility evidence to prove, and no suite to re-run.

## Impact

- Synthesis identities affected: none
- Verification identities affected: none
- Plan, takes, or package identities affected: none
- Consumers and commands affected: none
- Fakes and shared suites affected: none
- Fixtures and schemas affected: none
- Existing cached artifacts affected: none
- Published packages or accepted takes affected: none

**Nothing is granted or withdrawn by the correction.** All eleven pairs
`e1-s2-evidence-provenance-reconciliation-v3` carries cite
`e1-s1-provisional-contract-baseline-v13`, which `…-v14` superseded on 2026-08-30, and a
superseded record is not checked — so those pairs suppress nothing whatever reading justified
them. What the correction removes is a reader's reason to believe a suppression was examined
against bytes nobody had read. The reading v3 does give, of why each of the eleven files moved,
is unaffected and stays the record of it.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: not applicable; no fake, fixture, or
  shared suite is touched
- Migration procedure: none. No document, cache entry, plan, take, or package is expressed under
  anything this record changes
- Rollback procedure: revert this record and its two index rows as a unit. Nothing durable is
  written, so a revert restores the prior state exactly — including the incorrect sentence
- Compatibility evidence: the records themselves.
  `e1-s2-evidence-provenance-reconciliation-v3` §Scope and decision states the five-round reading
  in its own words; `E1-S2-INTERFACE-CHANGE-002` §Identification numbers six rounds; the four
  moved files were established by comparing each against the commit that added v3, not inferred
  from the round that named them
- Mapped tests and qualification rerun: none. No byte any Rust, Python, schema, or audio check
  reads moves here, and `E1-S2-INTERFACE-CHANGE-002` §Delivery and recovery remains the record of
  those runs. Re-running them would produce a second claim about bytes this record did not move
- Walking skeleton result: not run, and not required. Every class in
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change procedure that reruns the
  skeleton changes an interface; this one changes none
- Evidence provenance: no accepted or proposed record pins
  `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md` or this file, so amending it moves no pin.
  Adding the two rows below to `docs/INDEX.md` does move a file
  `e1-s1-provisional-contract-baseline-v14` pins and `…-v15` re-pins; v15 is `Proposed`, so its
  table is re-pinned from those bytes by
  `python3 scripts/check-evidence-provenance.py --write`, which `repin_refusal` permits only
  while a record declares `Proposed`. **`python3 scripts/check-evidence-provenance.py` exits 1 as
  this record is written, with one mismatch: v14's pin of `docs/INDEX.md`.** Accepting v15
  supersedes v14 and clears it. That state is reported rather than rounded to "exits zero",
  which is the defect this record corrects, in miniature

## Approval

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate for one signatory. Two rows rather than
`E1-S2-INTERFACE-CHANGE-002`'s ten, because no contract, version, refusal, or test is under
decision here.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that an accepted and signed architecture record is corrected from outside, in a successor, as its own §Approval requires, with no predecessor edited; and accept that the same class of defect stays answered by the standing obligation in `e1-s2-evidence-provenance-reconciliation-v3` §Open findings rather than by a mechanism proposed here | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept that no contract, version, identity, published schema, or durable byte moves, and that every technical conclusion `E1-S2-INTERFACE-CHANGE-002` records is untouched by this correction | Pending |

- Effective version and date: no version moves. `lesson 3.1`, `plan 2.0`, `e1-s2-v1`, and
  `e1.tts-executor.2.0` are unchanged by this record
