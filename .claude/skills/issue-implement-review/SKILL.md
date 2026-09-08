---
name: issue-implement-review
description: >
  Acceptance-driven implementation review for `rsstdd/technical-tts`: given uncommitted work, a
  branch, or a pull request, decide whether the implementation satisfies every criterion in its
  governing issue, at the correct enforcement boundary, under the repository's binding architecture.
  Reconstructs the contract from the issue and the accepted records first, then compares it against
  the real code — green CI, passing tests, preserved behavior, and a small diff prove nothing on
  their own. Reviews and reports; writes no production code and never commits. Use on "review this
  work", "does this satisfy the issue", "is this PR ready", "audit the branch before merge".
argument-hint: "<PR number, branch, or nothing for the working tree>"
license: MIT
---

# Implementation review — does the work satisfy its issue?

The governing question, and the only one this skill answers:

**Does the authoritative working-tree, branch-integration, or merge-result implementation satisfy
every criterion at the correct boundary under the repository's binding architecture, with the
smallest production-grade Rust that does it?**

## Authority

Conflict order is the repository's: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → the skills. A **Proposed** ADR, interface-change record, or
evidence record authorizes nothing; `CLAUDE.md` §Conflict order says so. Flag a genuine conflict;
never resolve one silently.

Load before judging any implementation: `ponytail` and `clean-code` always; `rust-review`,
`rust-comment`, `rust-production`, and `rust-testing` when the change touches `crates/`;
`worker/AGENTS.md` when it touches the Python worker. Route every document question through
`docs/INDEX.md` rather than guessing which file governs.

**Where this sits beside `rust-review`.** That skill is scoped to over-engineering and complexity
and says so: "Correctness bugs, security holes, and performance are out of scope unless caused by
an abstraction — route them to a normal review pass." This is that pass. Its `delete:` / `yagni:` /
`shrink:` findings belong here too, folded in under §Findings rather than reported twice.

## Fixed parameters here

- **Non-mutating.** No edit to tracked files, no commit, branch, push, merge, or pull request —
  `CLAUDE.md` §Non-negotiables gives every Git operation to the user. No `--write`, no
  `cargo fmt` without `--check`, no regeneration of a tracked artifact.
- Temporary untracked output is allowed when a check needs it, in a scratch directory, cleaned up
  before the report.
- Review the work; do not fix it. Offer the remedy in the report and stop.
- Never weaken a validation, containment, rights, checksum, consent, offline, or recovery control
  to make a check pass, and never recommend that a submission do so.

## 1. Establish the authoritative state

**Working tree** — staged, unstaged, and untracked against `HEAD`. Untracked files matter here:
`fixtures/`, `schemas/`, and `evidence/` additions are part of the change and carry their own gates.

**Branch** — against its intended *current* base, not the fork point. Separate a feature defect
from base drift; say which. `git merge-base --is-ancestor origin/main HEAD` answers whether the
branch has the base it will merge into.

**Pull request** — the **merge result against the current base** is authoritative; the head diff
is secondary and shows scope and provenance. `gh pr view <N> --json mergeable,mergeStateStatus`
says whether it integrates, and `git merge-tree --write-tree <base> <head>` produces the merged
tree without touching the working tree or writing a commit. Branch-local CI is never proof of
merge-result correctness. If integration state cannot be inspected, integration-dependent
conclusions are **Unverified**, not passed.

## 2. Assemble the criteria set

`issue-plan` §1 owns this and is not restated: the issue is an index, not the specification, and
the criteria come from the issue's Tasks and named tests, the `DELIVERY-PLAN.md` story with its
gate and epic acceptance lines, ADR-0001's cited sections, and the accepted interface-change and
deviation records that amend them. Assemble it the same way, independently, before reading the
diff — a criteria set derived from the implementation will agree with the implementation.

Two things this repository makes criteria that a reader may take for bookkeeping:

- **Every named test in the Delivery Plan list is a criterion.** Missing, renamed, or `#[ignore]`d
  is unsatisfied. The names are contracts copied character for character; `grep DELIVERY-PLAN.md`
  before calling a rename harmless, and check `evidence/` too — a record citing a test name in its
  results table breaks when the test moves.
- **The owed records are criteria.** A story that moves a published contract owes
  `docs/architecture/E*-INTERFACE-CHANGE-*.md`, a `docs/INDEX.md` row, and — where the Delivery
  Plan names one — an `evidence/` record. Absent, they are violated criteria, not follow-ups.

Grade each criterion **Satisfied**, **Partially satisfied**, **Violated**, or **Unverified**.
`Partially satisfied` is only for a multi-obligation criterion where one obligation holds and
another does not; it never softens a failed criterion, and the failed obligation is still Blocking.
`Unverified` is for what the evidence cannot decide without inventing facts — never a polite
`Satisfied`.

## 3. Account for every changed file

Classify all of them, by role — `scoped`, `supporting`, `unrelated`, `generated` — and by status —
`clear`, `suspicious`. Trace the behavioural hunks; still classify the mechanical ones. Do not
conclude while any hunk is unaccounted for.

Watch for what rides along: unrelated cleanup mixed into scoped work, an obsolete compatibility
path kept alive, machinery for a caller that does not exist, and a doc comment updated to describe
something the code no longer does.

## 4. The invariant tests

**Specification fidelity.** Read the governing rule, read the implementation, compare their
semantics *directly*, and only then read the tests. Tests are evidence, not specification. None of
these is proof: the suite passes; behavior was preserved; the code already existed; both ends of a
mirror name each other; a guard exists somewhere; the old and new states are each independently
valid; branch CI is green. "Behavior preserved" answers *did I break it*, never *was it right* —
`check_startup_modules_are_accounted` survived a refactor that proved behavior unchanged while
accepting a module owned by any installed distribution, where
`docs/operations/WORKER-ENVIRONMENT.md` had required a **locked** owner since the fourteenth audit,
and both existing tests agreed with the weaker rule.

**Two-sided mirrors.** That the mirror exists is the cheap half. Open the named section, put its
sentence beside the predicate, and compare. A code condition narrower or weaker than the document
it mirrors is the finding that matters, and every test written against it will agree with it.

**Boundary ownership.** For each invariant: which boundary owns it, whether a caller can bypass it,
whether it runs before the protected or irreversible work, whether durable state can reach the
protected state without passing through it. A correct check at the wrong boundary is still a defect.

**Transition correctness.** Old state valid and new state valid do not make the transition legal.
If durable state can be written past the transition API, the in-memory method does not enforce it.

**Ordering.** "The gate runs before the work" is a control-flow claim; presence is not ordering.
The repository's own proof shape: point the request at a nonexistent tool, assert the gate's error
wins — a late gate would report the missing tool — and assert no work happened.

**Evidence independence.** A test that re-derives the implementation agrees with a wrong one
forever. Expected values belong in a table a reviewer reads against the controlling document.

**Minimalism**, in `ponytail`'s order: does it need to exist, does the repository already have it,
does `std`, does the platform, does an existing dependency, can it be smaller — and only then new
machinery.

## 5. Generated artifacts and this repository's traps

Generated output is never its own specification. `schemas/` is generated from the Rust types by
`crates/study-tts-runtime/examples/generate-schemas.rs`; the module header says a hand-written
schema is a second definition of a format. **Do not run the generator to review** — it overwrites
tracked files. The non-mutating proof already exists and is byte-for-byte in both directions:
`t3_e1_generated_schemas_match_checked_in_files`.

Check, when the change touches each:

- **Published schema** — a `PUBLISHED_SCHEMAS` entry, a `PUBLISHED_REQUIRED_SURFACE` row
  per required-field surface, a valid and an invalid contract fixture, and
  `t3_e1_published_schema_required_fields_match_the_recorded_surface`. `schemars` publishes `///`
  into `description`, so a doc comment on a serialized type moves a governed schema.
- **Committed fixture** — one row in `docs/testing/TEST-DATA-MANIFEST.md` with a current SHA-256;
  `t3_e0_registered_fixture_checksums_match_test_data_manifest` enforces it. Synthetic only: no
  real voice reference, model weight, private content, or corpus enters Git, CI, fixtures, or logs.
- **Governance document edited** — records under `evidence/` pin its SHA-256.
  `python3 scripts/check-evidence-provenance.py` is the check; its `--write` form is mutating and
  is not a review command. An accepted record cannot be amended in place: the answer is a
  supersession or a row in an accepted reconciliation record.
- **Newly mechanized rule** — the matching §Enforcement table must name the test that mechanizes it.
- **Frozen contract** — `docs/architecture/G1-FREEZE-CHARTER.md` sets the price of a change;
  §Deliberately not frozen is where a delegated question is answered, and a story that answers one
  must say so there rather than only in code.

## 6. Verification

`AGENTS.md` §Verification is the authority. Run non-mutating forms only:

```bash
cargo fmt --all -- --check
python3 scripts/check-rust-conventions.py
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo test --workspace --doc --locked
python3 scripts/check-evidence-provenance.py
python3 scripts/qualification/check_evidence_citations.py
```

Add what the changed surface requires: the `walking_skeleton` suite for anything pipeline-visible,
and for a worker change `python3 -m compileall -q -f worker/study_tts_worker` and
`python3 -m unittest discover --start-directory worker/tests`. The skeleton needs `ffmpeg` and
`ffprobe` on `PATH`.

Report each as **Passed**, **Failed**, **Not run**, **Blocked by environment**, or
**Not applicable**. A check that could not run leaves its criterion **Unverified**; missing
verification is never success, and no check may be reported as passing unless it actually ran.

## 7. Findings

One finding per **root cause**, not per violated authority. A defect breaking both a criterion and
an ADR is one Blocking finding that cites the criterion first and the record second. One cause
breaking several criteria is one finding when one remedy fixes them all. Split only when the causes
or the remedies are materially independent.

- **Blocking** — any violated criterion, a failed required check, a contradiction with binding
  architecture, unsoundness, or a corruption, containment, deadlock, or merge-result defect.
- **Major** — correctness, boundary, durability, protocol, visibility, governance, or
  over-engineering defect outside the criteria set.
- **Minor** — localized docs, structure, comments, test placement, idiom, unrelated churn.
- **Clippy** — only with the concrete `clippy::` lint named.

Do not inflate severity, and do not report a hypothetical without evidence.

## 8. Report shape

Findings first — no preamble, no praise, no narration of process. Then:

- **Criteria** — every one, graded, with a one-line reason. Name the failed obligation on a
  `Partially satisfied`.
- **What is correct and should stay** — the scoped work a remediation must not undo. Only what is
  actually right; no manufactured praise.
- **Unrelated churn** — only when present.
- **Verification** — what was inspected and what ran, separating head state from merge-result
  state, and local from CI. Name every check not run and why.
- **Conclusion** — exactly one of **Ready to merge**, **Ready after minor corrections**, **Not
  ready to merge**, **Cannot determine from available evidence**. Any violated criterion forces
  *Not ready to merge*.

Finding IDs are stable: `B1`, `M1`, `m1`. Say the boundary, the file, the invariant, and the
smallest correct remedy — "enforce the transition before durable replacement", not "consider
improving validation".
