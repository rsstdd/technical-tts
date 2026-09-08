---
name: issue-plan-review
description: >
  Review, critique, and amend an implementation plan already posted as a comment on a
  `rsstdd/technical-tts` issue, then post the review as a new follow-up comment that amends the
  earlier plan only where the evidence justifies it. Re-derives the criteria set independently,
  verifies every path, symbol, test name, version, and record the plan cites, and grades findings
  Blocking / Major / Minor / Optional. Planning only — it writes no production code and never
  edits the original comment. Use on "review the plan on #N", "critique the implementation plan",
  "amend the plan comment", "is this plan right".
argument-hint: "<issue number or URL>"
license: MIT
---

# Reviewing a plan posted on an issue

The prior plan is a proposal by a fallible author working from a tree that has since moved. Judge
it against the repository, not against its own reasoning. Two failures are equally bad: passing a
plan that will strand a cache entry or retain a version across a breaking change, and inventing
findings so the review looks like work.

## Authority

Same conflict order as everything here: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → the skills. A **Proposed** ADR, interface-change record, or
evidence report authorizes nothing.

Load `issue-plan` first — it owns how the criteria set is assembled, what counts as evidence, the
consequences the repository charges for, and the verification order. This skill does not restate
them; it adds what changes when a plan already exists. Load `ponytail` and `clean-code` always,
and `rust-review`, `rust-comment`, `rust-production`, `rust-testing` when the plan touches
`crates/`. `rust-review`'s severity scale is the one used below.

Planning only. Never edit the original planning comment, the issue body, or a checkbox. Never
commit, branch, push, merge, or open a pull request.

## 1. Identify exactly which comment is under review

```bash
gh issue view <N> --comments
gh api repos/rsstdd/technical-tts/issues/<N>/comments \
  --jq '.[] | "\(.id) \(.user.login) \(.created_at) \(.html_url)"'
```

Name the plan comment by URL in the review. If several plans or amendments exist, the newest one
is the subject and the older ones are context — say which you are amending, and do not re-litigate
a point a later comment already corrected.

## 2. Re-derive before you read the plan's evidence

Assemble the criteria set and prove each one against the tree per `issue-plan` §1–§2 **without**
using the plan as the discovery map. Then compare. A plan's file list is a hypothesis about where
the behavior lives; the frequent failure here is a plan that inherited a stale map and never
checked it.

Produce the reassessment independently: `| Criterion | Source | Status | Evidence | Implication for the plan |`.

## 3. Verify what the plan cites, mechanically

Every citation is checkable, so check it rather than reading it charitably.

| Claim in the plan | How it is checked |
|---|---|
| A path or symbol | `rg 'fn resume_preview' crates/`; a missing or renamed symbol is a Major finding, not a typo |
| A named test | It exists, is not `#[ignore]`d, and its body asserts the outcome its name claims |
| A test list | Matches `DELIVERY-PLAN.md` §Story; the plan must update that list in the same commit |
| "No identity moves" | Read the key inputs: `SYNTHESIS_IDENTITY_VERSION`, `CACHE_SCHEMA_VERSION`, plan hash, verification, package and transaction identity. Silence is not proof |
| A schema change | The version move matches the change class, and the `PUBLISHED_REQUIRED_SURFACE` row in `crates/study-tts-testkit/tests/schemas.rs` moves with any required field |
| An ADR section | Still says what the plan says, after its accepted amendments and `docs/adr/deviations/` |
| A record or deviation | `Accepted` and signed, not Proposed, not spent (`ADR-0001-D005` is spent), not superseded — `evidence/README.md` §supersession names the record in force |
| A gate or milestone claim | `DELIVERY-PLAN.md`'s acceptance line for that gate, not the plan's paraphrase |
| A command | Exists in `AGENTS.md` §Commands and its behavior is implemented; product commands are authoritative only once the referenced behavior exists |

## 4. Classify every material assertion

**Correct / Correct but incomplete / Incorrect / Unnecessary / Over-scoped / Under-specified /
Blocked by prerequisite / In conflict with repository guidance.** Then grade:

- **Blocking** — executing as written violates something the repository does not trade away:
  weakening validation, containment, rights, checksum, consent, offline, or recovery to pass a
  test; a breaking contract change that retains its version after the G1 freeze; planning against
  a Proposed record; a required field entering a published schema with no version move and no
  `PUBLISHED_REQUIRED_SURFACE` edit; an identity move that strands a cache entry or a governed
  package without saying so; an architectural change with no accepted ADR amendment first;
  closing a speech-affecting story on automated checks with no human listening review.
- **Major** — a criterion ends up unproved: an unaddressed task or named test, a test that pins the
  implementation instead of the outcome, a mock standing in for FFmpeg, the real filesystem, or a
  fault at a write boundary, a missing interface-change record or evidence record the story owes.
- **Minor** — executable but wrong in detail: stale path, wrong crate, wrong tier, missing
  verification command, a step with no criterion.
- **Optional** — a real improvement that is not required. Style preferences live here or nowhere.

Omit empty sections. Do not promote taste into Blocking. If the plan is sound, say so in a
paragraph and post a short review — `ponytail` applies to the review as much as to the code, and
an amendment that only re-words a correct step is noise.

## 5. Amend only what the analysis condemns

Say plainly which parts stand: "Steps 1–3 stand as written." Restate a step only where it changes.

Plan diff: `| Area | Existing plan | Critique | Amendment |`, limited to substantive differences —
ownership, contract and version, files and symbols, error semantics, durability or containment
behavior, identity effect, tests, order, verification, scope.

Amended steps take the `issue-plan` §5 item 7 shape — criteria addressed, why the step exists,
repository evidence, files and symbols, the change, the test written first, verification,
completion condition, step-specific risks. Every amendment states why the existing plan was
insufficient; every step maps to a criterion; every open criterion maps to verification. Prefer
what the tree already does over anything invented in review.

Amended test plan: `| Test | Tier | Criterion | Behavior proven |`, names as
`t<tier>_<epic>_<behavior>`, T1–T4 offline, real boundary where the semantics are the boundary's.

Verification sequence: the fixed order in `issue-plan` §4, plus the `AGENTS.md` §Verification row
for the change class and any command the issue names, ending on the walking skeleton.

## 6. Comment shape

Write to the scratchpad, then post. Sections: assessment (sound / sound with amendments /
materially incomplete / architecturally incorrect / unsafe, with the one-paragraph reason);
governing guidance; criteria reassessment table; findings by severity; plan diff; amended steps;
amended test plan; verification sequence; scope (required / out of scope / follow-up candidates);
revised execution checklist as `- [ ]` items runnable top to bottom.

Open the comment by naming the plan comment it reviews, by URL, and stating that it amends that
plan where the two conflict and leaves it standing elsewhere. No scratch work, no speculation, and
no claim that a check passed unless it ran.

## 7. Post it

```bash
gh issue comment <N> --body-file <scratchpad>/issue-<N>-plan-review.md
gh api repos/rsstdd/technical-tts/issues/<N>/comments --jq '.[-1].html_url'
```

The original comment stays untouched. The task is not done until the new comment exists on the
right issue and its URL is returned.
