---
name: issue-plan
description: >
  Turn a GitHub issue in `rsstdd/technical-tts` into an implementation-ready plan posted as a
  comment on that issue. Assembles the binding criteria set from the issue's Tasks and named
  tests, the `DELIVERY-PLAN.md` story and gate acceptance, ADR-0001 and its accepted amendments
  and deviations; proves each one satisfied or unsatisfied against the tree; and names the
  interface, identity, evidence, and verification consequences before any code is written.
  Planning only — it writes no production code. Use on "plan issue #N", "analyze the issue",
  "what is left on E2-S4", "implementation plan for the story".
argument-hint: "<issue number or URL>"
license: MIT
---

# Planning a story from its issue

A plan here is a proof obligation, not a summary. For every criterion, either the tree already
satisfies it and the plan cites the behavior that proves it, or the plan names the smallest
observable change that makes it true and the test that will fail until it does.

## Authority

Conflict order is the repository's: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → the skills. A **Proposed** ADR or interface-change record
authorizes nothing; plan against what is accepted and signed, and say so where a story depends on
a Proposed record. Flag a genuine conflict in the plan; never resolve one silently.

Load before planning: `ponytail` and `clean-code` always; `rust-review`, `rust-comment`,
`rust-production`, and `rust-testing` when the plan touches `crates/`; `worker/AGENTS.md` when it
touches the Python worker. Route every document question through `docs/INDEX.md` rather than
guessing which file governs.

Planning only: no edit to `crates/`, `worker/`, `schemas/`, `fixtures/`, or `docs/`. Never commit,
branch, push, merge, or open a pull request. Never edit the issue body or tick a checkbox.

## 1. Assemble the criteria set

The issue is an index, not the specification. A story issue carries **Tasks** checkboxes and a
**Tests** list; the acceptance statement lives elsewhere. Collect all of:

| Source | What it contributes |
|---|---|
| `gh issue view <N> --comments` | Tasks, named tests, parent, dependencies, blockers, corrections in comments |
| Parent epic and dependency issues | Work this issue must not duplicate |
| `DELIVERY-PLAN.md` §Story E*-S* | Numbered tasks, the named-test list of record, gate, evidence requirement, interface record |
| The epic's `**Acceptance:**` line and the gate's `**G* acceptance:**` / `**M* acceptance:**` | The conjuncts the story is judged against |
| ADR-0001 sections the story cites | The invariant; the plan quotes the section number |
| `docs/architecture/E*-INTERFACE-CHANGE-*.md`, `docs/adr/deviations/ADR-0001-D0*.md` | Amendments that change what the invariant now requires; check `Accepted` and signed |
| `docs/governance/` routed policy | Rights, freeze, risk register, project rules for the affected surface |

Every named test in the Delivery Plan list is itself a criterion. Missing, renamed, or
`#[ignore]`d tests are unsatisfied criteria, not bookkeeping.

## 2. Prove each criterion against the tree

The tree is the source of truth for state; the issue is the source of truth for intent.

- An unchecked box proves nothing. A checked box proves nothing. Find the behavior.
- A named test proves its criterion only if it exists **and** asserts the outcome its name claims —
  read the body, don't match the name. `rg 't4_e2_interrupt_before_rename' crates/`.
- A published contract is proved by `schemas/`, the versioned constant in `crates/`, and the
  `PUBLISHED_REQUIRED_SURFACE` row in `crates/study-tts-testkit/tests/schemas.rs` — not by a type
  that looks right.
- Cite file, symbol, and behavior: `crates/study-tts-runtime/src/pipeline.rs::resume_preview`, not
  "the job module".
- Prefer the existing abstraction — `managed::` containment, the durable publish path, the
  testkit fake worker, `ContractDescriptor` — over any parallel mechanism.

Classify each criterion **Satisfied / Partially satisfied / Unsatisfied / Ambiguous / Blocked**.
Ambiguous and Blocked are legitimate outcomes; guessing is not. Do not invent work for a criterion
already satisfied, and do not plan work a dependency issue owns.

## 3. Name the consequences the repository charges for

A plan that omits these is incomplete, because the work cannot land without them:

- **Interface class.** Compatible patch / compatible extension / breaking contract / architectural
  / emergency containment, per `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`. The G1
  interfaces are frozen by `docs/architecture/G1-FREEZE-CHARTER.md` and `ADR-0001-D005` is spent:
  a breaking change moves the major version, no exceptions. An architectural change needs an
  accepted ADR amendment **before** implementation.
- **Interface-change record.** Anything past a compatible patch needs one from
  `docs/templates/INTERFACE-CHANGE-TEMPLATE.md`, plus a `docs/INDEX.md` row. Name it in the plan.
- **Identity effect.** State, per identity, whether it moves: synthesis, verification, plan hash,
  cache schema, takes, package/transaction. Say what is invalidated, what is stranded, and what
  must be rebuilt or requalified. "No identity moves" is a claim requiring the same evidence.
- **Evidence.** If the Delivery Plan story names an evidence record, the plan says which template
  under `docs/templates/` and which `evidence/gates/g*/` path, and that its acceptance is a
  signature, not a passing suite.
- **Listening.** Any change to spoken output is unfinished without a human listening review
  against `docs/operations/PREVIEW-REVIEW-CHECKLIST.md`. Automated checks are necessary and never
  sufficient; the plan states the listen that remains owed.
- **Security and rights.** Containment, redaction, offline, checksum, consent, and license effects
  where the touched path has them. Never plan to weaken one to make a test pass.

## 4. Design verification before implementation

Every open criterion gets an explicit proof. Name tests as `t<tier>_<epic>_<behavior>` with the
tier stated and the behavior as an externally meaningful outcome; place them per `rust-testing`
and `docs/testing/TEST-STRATEGY.md`. T1–T4 stay offline and download no model. Use the real
boundary where correctness depends on it — real FFmpeg, real filesystem, fault injection at the
write boundary — and the testkit fake worker only where the worker is not the behavior.

Identify the **first failing test**: the smallest one that fails now for the right reason, what
observable behavior it captures, and what makes it pass. No test invented only to be first.

The verification order is fixed and belongs in the plan verbatim, narrowest first:

```bash
cargo test --workspace <test_name>                     # the target behavior
cargo fmt --all -- --check
python3 scripts/check-rust-conventions.py
python3 scripts/check-evidence-provenance.py           # when evidence or docs moved
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline -p study-tts-testkit --test walking_skeleton --locked
```

Add the row `AGENTS.md` §Verification requires for this change class, and any command the issue
names. State what each proves. The walking skeleton must stay green in every plan.

## 5. Report shape

Write the plan to the scratchpad, then post it. Sections, in order:

1. **Executive summary** — outcome, completion state, remaining work, dependency and gate status,
   whether it fits one branch, material ambiguities, top risks.
2. **Governing requirements** — ADR sections, amendments and their acceptance status, Delivery Plan
   story, routed policy, skills, dependency issues.
3. **Current behavior** — the execution and data flow the story touches, by file and symbol.
4. **Criteria matrix** — `| Criterion | Source | Status | Evidence | Remaining work | Verification |`.
5. **Findings by crate or surface** — `study-tts-core`, `-runtime`, `-cli`, `-testkit`, `worker/`,
   `schemas/`, `docs/`.
6. **Interface, identity, and evidence consequences** — §3, as a table.
7. **Steps** — each with the criteria addressed, current state, exact files and symbols,
   behavior and error semantics, the test written first, verification, completion condition, and
   only the risks specific to it. No production patches; enough detail that the implementer needs
   no second discovery pass.
8. **Test plan** — `| Test | Tier | Criterion | Behavior proven |`.
9. **Verification plan** — the commands above plus the change-class row, and what each proves.
10. **Risks and edge cases** — failure mode, mitigation, verification.
11. **Scope** — required / explicitly out of scope / follow-up candidates. Findings that are not
    this issue's work become follow-up candidates, never scope creep.
12. **Execution checklist** — ordered `- [ ]` items a coding agent can run top to bottom, starting
    with the first failing test.
13. **Definition of done** — the story-close checklist of
    `docs/governance/GITHUB-PROJECT-PLAYBOOK.md` made concrete for this issue.

State uncertainty as uncertainty. No scratch work, no unsupported speculation, no claim that a
check passed unless it ran — a plan asserts what will be run, not what was.

## 6. Post it

```bash
gh issue comment <N> --body-file <scratchpad>/issue-<N>-plan.md
```

Then verify the comment landed on the right issue and return its URL:
`gh issue view <N> --comments | tail -5`. The task is not done until the comment exists.
