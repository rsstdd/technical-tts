# ADR-0001-D013 — A draft segment is a valid lesson; approval gates synthesis, not validation

- **Status:** Approved
- **Date:** 2026-10-05
- **Controlling ADR and sections:** ADR-0001 §8.2, whose invariant list is the subject, read
  against §9.2's requirement that compilation produce "reviewable draft segments"
- **Requesting story:** E3-S1, for issue #21
- **Owner:** Engineering owner
- **Approver:** Project owner and engineering owner
- **Expiry:** None. This narrows where a ratified invariant is enforced; it does not resolve
  itself with time.

## Approved amendment

ADR-0001 §8.2 lists among a lesson's invariants:

> no segment marked unreviewed can enter a production build

**That is amended to read as what it says, and no more.** `review_status` is enforced at the
boundary where synthesis or a production claim is attempted, **not** at every parse of a lesson
document. A document whose segments are `draft` is a valid lesson `3.1`; it is simply not one this
build will render.

An amendment rather than an interface-change record alone, because §8.2 *is* the ratified
invariant list. A record can describe how an invariant is checked; it cannot move which boundary
owns it. That reading is not new: `ADR-0001-D011` states it for §12.5's enumerated inputs, and the
same logic applies to §8.2's enumerated invariants.

## Why the accepted design cannot be followed

§9.2 requires Markdown compilation to "split material into reviewable **draft** segments", and
`ReviewStatus::Draft` exists in the published lesson vocabulary for exactly that. But
`ValidatedLesson::validate` refuses any segment that is not `Approved`, inline in the same loop as
every structural invariant — so **a compiled draft cannot be a `ValidatedLesson` at all**, and the
two requirements cannot both be satisfied as the code stands.

The implementation is stricter than §8.2's own sentence. §8.2 bars unreviewed material from "a
production build"; the gate bars it from being read as a lesson. Nothing in §8.2 asks for that,
and the gap is visible in the tree's own history: `E1-S5-INTERFACE-CHANGE-001` §Limits records that
`scaffold_lesson` writes `approved` as "a starting value rather than a review", because
`t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton` needed a renderable lesson. That
was defensible for one segment a human then edits. A compiler emitting twenty segments marked
`approved` would assert a review nobody performed, which is the control §8.2 exists to hold.

E3-S1 shipped around the conflict without weakening anything: `compile_source` validates the bytes
it is about to write and tolerates exactly one refusal, `LessonError::UnapprovedSegment`, returning
every other. That keeps the gate intact and stops the compiler claiming its output passed it, but
it is a workaround at one call site. **E3-S2's golden harness and E3-S3's transformation audit both
need to render compiled material**, and they meet the same wall with no call site of their own to
work around it.

## Impact

- **Architecture and authority boundaries:** the approval check moves from the lesson parse
  boundary to the synthesis and publication boundaries. `WorkerConfiguration::for_bundle` and the
  release-claim path are where it belongs: §8.2's sentence says "production build", and those are
  the gates a production build passes.
- **Schemas and interfaces:** none move. `review_status` is already a required field of published
  lesson `3.1` with `draft` in its enum, so no document becomes valid that was invalid, and no
  schema, charter row, or `PUBLISHED_REQUIRED_SURFACE` entry changes. `ValidatedLesson`'s public
  signature is unchanged; what moves is which refusal it raises.
- **Synthesis, verification, cache identities:** none. `review_status` reaches no identity —
  `crates/study-tts-core/src/identity.rs` lists it among the review metadata excluded from
  `SynthesisContext`, and the field-sensitivity table proves it by mutation.
- **Security, rights, and privacy:** **the control is preserved, not relaxed, and this is the
  clause to check on review.** Today an unapproved segment cannot reach synthesis because it
  cannot be parsed. After this amendment it can be parsed and must be refused at the gate instead.
  That refusal is therefore owed a test that fails if it is absent, and the deviation is not
  discharged until one exists: point a build at a lesson whose segments are `draft` and assert the
  gate's own refusal **and** `worker.synthesis_count() == 0`, which is the ordering proof shape
  `rust-review` §Safety and ownership requires. Without it this amendment is a hole rather than a
  relocation.
- **Tests and evidence:** `t3_e2_private_preview_requires_human_approval_record` and
  `t4_e2_content_change_invalidates_prior_approval` are E2-S6's and unaffected — they govern the
  approval *record*, not `review_status`. The existing lesson-gate tests that assert
  `UnapprovedSegment` move to the synthesis boundary with it. `compile_source`'s one-refusal
  tolerance and the comment explaining it are deleted once the gate moves, because the thing it
  worked around is gone.
- **Existing artifacts and migration:** none. No stored lesson, package, cache entry, approval
  record, or takes document changes meaning. A lesson on disk with `draft` segments was already
  refused by this build and will still not render.
- **Schedule and scope:** the relocation is **not** E3-S1's and is not done here. E3-S1 records the
  conflict and works around it at one call site. The story that moves the gate is E3-S2 or E3-S3,
  whichever first needs to render compiled material, and it carries the synthesis-boundary test
  above.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Compiler emits `approved` | Asserts a review nobody performed. It is the control §8.2 exists to hold, and the one thing `CLAUDE.md` §Non-negotiables forbids weakening to make a path work |
| Leave the gate at the parse boundary and let each caller tolerate `UnapprovedSegment` | What E3-S1 did at one call site, and it does not generalise: every future reader of a draft repeats the tolerance, and one that forgets refuses a document §9.2 requires to exist |
| A second entry point, `AuthoredLesson::from_json_unapproved` or similar | Two parse paths for one format, differing by one check. The next reader picks the wrong one, and `clean-code` §Smells calls needless duplication what it is |
| Amend §9.2 instead, so compilation emits approved segments | Inverts the problem: §9.2's "reviewable draft segments" is the requirement that makes human review possible at all, and §8.2's sentence already permits what we want |

## Rollback

Revert the commit that moves the check. `ValidatedLesson::validate` regains the `review_status`
arm, `compile_source` regains its one-refusal tolerance, and no authoritative data is touched in
either direction — `review_status` is authored content, not derived state, and nothing durable is
written differently under either reading.

The deviation itself is reversed by marking this record superseded and restoring §8.2's stricter
reading, which costs nothing stored and only re-breaks §9.2.

## Decision

- [x] Approve through an ADR amendment
- [ ] Reject
- [ ] Defer

**Approved — Ross Todd, project owner and engineering owner, 2026-10-05.**
`docs/governance/PROJECT-EXECUTION-CHARTER.md:39` permits one person to hold both roles for a
personal project and requires each approval to name its role and accepted risk separately:

| Role | Decision | Accepted risk |
|---|---|---|
| Project owner | §8.2's approval invariant is enforced at the synthesis and publication boundaries rather than at every lesson parse | That an unapproved segment is now representable in a parsed lesson, and the refusal depends on a gate rather than on the parser. Accepted **conditionally on the synthesis-boundary test** named in §Impact landing with the relocation |
| Engineering owner | The relocation belongs to the story that first renders compiled material, not to E3-S1, which records the conflict and works around it at one call site | That the workaround stands in the tree until then, and a second reader of a draft lesson could repeat it. Bounded by this record naming the single call site that has it |
