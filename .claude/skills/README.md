# Repository skills

Eleven skills live here, one directory each, all invocable as `/<name>`. This file routes: it says
which skill loads when and whether it binds. Each `SKILL.md` frontmatter stays authoritative for
what that skill covers, so nothing here restates a skill's content and nothing here can drift from
it.

`CLAUDE.md` §Required skills is the binding table; `AGENTS.md` §Coding conventions carries the same
rule for an agent that reads that file first. Both win over this index.

## Binding standards — load before the first edit, not after

| Skill | Loads when | Owns |
|---|---|---|
| [`clean-code`](clean-code/SKILL.md) | Any code, any language | Clean Code as applied here, and the settled conflicts against repo convention |
| [`ponytail`](ponytail/SKILL.md) | Any code, and before auditing or editing existing Rust | Whether the code should exist at all: YAGNI, reuse, stdlib before a dependency |
| [`rust-review`](rust-review/SKILL.md) | Any Rust, written or reviewed | The severity scale the code is judged against, and what to delete |
| [`rust-comment`](rust-comment/SKILL.md) | Any Rust | Why-not-what prose, rustdoc sections, two-sided coupling comments, debt markers |
| [`rust-production`](rust-production/SKILL.md) | Rust that spawns a process, writes durable state, computes an identity, or changes a published format | Subprocess supervision, durable publication, determinism, schema evolution |
| [`rust-testing`](rust-testing/SKILL.md) | Any Rust test, which under TDD is nearly every Rust change | TDD order, `t<tier>_<epic>_<behavior>` naming, tier budgets, test placement, offline determinism |

These are standards, not advice. They are not architectural authority: the conflict order is
newest accepted ADR that explicitly supersedes → `docs/adr/ADR-0001-*` → `DELIVERY-PLAN.md` →
`AGENTS.md` → `crates/AGENTS.md` → `PRINCIPLES.md` → these skills. Flag a genuine conflict; never
resolve one silently.

## Issue workflow — invoked explicitly, one per stage

| Skill | Invoke when | Produces | Touches code |
|---|---|---|---|
| [`issue-plan`](issue-plan/SKILL.md) | A story needs a plan before work starts | An implementation plan posted as an issue comment | No |
| [`issue-plan-review`](issue-plan-review/SKILL.md) | A plan comment exists and needs auditing | A critique and amended plan posted as a follow-up comment | No |
| [`issue-implement`](issue-implement/SKILL.md) | The plan is settled and the story is being built | A verified, reviewed, minimal diff and an evidence report | Yes |
| [`issue-implement-review`](issue-implement-review/SKILL.md) | Work exists — a tree, a branch, or a PR — and must be judged against its issue before merge | Criteria graded Satisfied / Partially satisfied / Violated / Unverified, findings by root cause, and one merge verdict | No |

They compose in that order and share one criteria model: `issue-plan` §1 owns how a criteria set is
assembled from the issue's tasks and named tests, the `DELIVERY-PLAN.md` story and gate acceptance,
and ADR-0001 with its accepted amendments and deviations. The other three defer to it rather than
restating it. All four load the binding standards above; none of them commits, branches, pushes,
merges, or opens a pull request.

`issue-implement-review` closes the loop and is the one that may be pointed at work it did not
plan. It re-derives the criteria set independently — a set read off the implementation agrees with
the implementation — and it is the correctness pass `rust-review` routes to, that skill being
scoped to over-engineering alone.

## Working tree — invoked explicitly, changes nothing

| Skill | Invoke when | Produces | Touches code |
|---|---|---|---|
| [`commit-plan`](commit-plan/SKILL.md) | Uncommitted work needs splitting into commits | A grouping, a message per commit, and the `git add`/`git commit` commands as text | No |

It emits commands and runs none of them: `AGENTS.md` §Ask first puts committing behind the user
and `CLAUDE.md` §Non-negotiables gives them every Git operation outright. Its splits are the ones
this tree enforces mechanically — an index row with the document it names, a fixture with its
checksum row, a generated schema with the type that generates it, a record before the change it
authorizes.

## Adding a skill

Match the frontmatter `name` to the directory name, keep it as short as the six standards are, cite
the file in this tree that proves each rule rather than arguing it, and add a row above. A skill
that binds also needs a row in `CLAUDE.md` §Required skills; a workflow skill does not.
