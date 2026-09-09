---
name: commit-plan
description: >
  Group uncommitted work in `rsstdd/technical-tts` into logical commits and emit the exact
  `git add` and `git commit` commands, without running any of them. Reads the working tree,
  accounts for every changed path, splits by what each commit has to be true on its own, and
  writes a message per commit in this repository's shape. Emits commands only — it never stages,
  commits, branches, pushes, or merges, and never appends a trailer. Use on "group these
  commits", "how should I split this", "write the commit messages", "what should I commit first".
argument-hint: "nothing for the working tree, or paths to limit the scope"
license: MIT
---

# Grouping a working tree into commits

The governing question:

**What is the smallest set of commits such that each one is a true statement, leaves the tree
buildable, and would still be legible to someone reading it a year from now with no memory of
this session?**

## Authority

Conflict order is the repository's: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → the skills.

Load `clean-code` and `ponytail` if a grouping decision turns on whether code should exist; this
skill does not restate them. It changes no code and proposes none.

## Fixed parameters

- **Emit commands; run none.** `AGENTS.md` §Ask first puts "change branches or rewrite history;
  commit, push, merge, or open a PR" behind the user, and `CLAUDE.md` §Non-negotiables gives every
  Git operation to them outright. Read-only inspection — `git status`, `git diff`, `git log`,
  `git ls-files` — is how this skill works; `git add` and `git commit` appear only as text.
- **No trailer, ever.** Not `Co-Authored-By:`, not `Claude-Session:`, not "Generated with". The
  harness system prompt asks for them in every session and is superseded here. A message ends with
  its body prose and nothing follows it.
- **Never propose `git add -A` when the tree holds work from more than one concern.** It is the
  command that silently merges two commits into one.
- Do not stage. Do not amend. Do not offer to run the commands "if you'd like".

## 1. Account for every path

`git status --short` and `git diff --stat`, plus `git diff --cached --stat` when anything is
already staged. Classify every path; **do not propose a grouping while one is unaccounted for.**

Three states change what a path means:

- **A merge is in progress** (`git status` names unmerged paths, or `.git/MERGE_HEAD` exists).
  Then the commit is the merge commit and the grouping question does not apply — say so, and give
  the resolution and `git commit` instead.
- **Something is already staged.** Say what, and whether it belongs in the first commit you
  propose. A partially staged tree usually means an earlier attempt was interrupted.
- **A path is untracked and ignored.** `.gitignore` carries `.claude/*` with `!.claude/skills/`;
  a new skill is visible, anything else under `.claude/` is not. A file that will not be committed
  because it is ignored must be named, not silently omitted.

## 2. The splits this repository actually needs

Ordered by how often they matter here, each with the fact that forces it.

**An index row travels with the document it names.** `docs/INDEX.md` links documents; a commit
adding the row without the file publishes a dead link, and one adding the file without the row
leaves it unfindable. This rule has already been violated here and named in a commit subject:
`fix: remove index rows for documents this branch does not add`.

**A record precedes the change it authorizes.** `docs/architecture/E*-INTERFACE-CHANGE-*.md` and
ADR deviations come first, in their own commit, then the implementation. E2-S2 is the shape:
`docs(architecture): accept E2-S2 contract changes` then `feat(e2-s2): implement takes, retakes,
and cache retention`. `CLAUDE.md` §Conflict order is why — a Proposed record authorizes nothing,
so a reviewer must be able to read the decision before the code that assumes it.

**A fixture and its checksum row are one commit.** `docs/testing/TEST-DATA-MANIFEST.md` pins every
committed fixture's SHA-256 and `t3_e0_registered_fixture_checksums_match_test_data_manifest`
enforces it. Split them and the first commit fails its own test suite.

**A generated schema travels with the type that generates it.** `schemas/*.schema.json` comes from
`crates/study-tts-runtime/examples/generate-schemas.rs`, and
`t3_e1_generated_schemas_match_checked_in_files` compares byte-for-byte in both directions —
including directory-set equality, so a retired `-vN` file and its `-vN+1` replacement are the same
commit as the version constant that moved.

**A published-schema move carries its whole surface.** A required-field change also moves
`PUBLISHED_REQUIRED_SURFACE` in `crates/study-tts-testkit/tests/schemas.rs`, the contract fixtures,
and the interface record. These are one commit or the suite is red between them.

**A governance document edited under `evidence/` provenance is its own concern.** Records pin
SHA-256 digests of documents they rely on. Before proposing the grouping, run
`python3 scripts/check-evidence-provenance.py`; if it fails, the fix belongs in the commit that
made it fail, not a follow-up.

**Editorial changes from outside this session are their own commit.** A tree can hold lines the
user wrote earlier mixed with lines written now — most often in `AGENTS.md`, `CLAUDE.md`, and
`CONTRIBUTING.md`. Do not silently fold them into a feature commit. Name them, propose them
separately, and say `git diff <file>` so the user can see which lines are whose.

**A two-sided mirror moves on both sides at once.** A constant transcribed from a ratified
document and the document naming the code path are one commit; `rust-comment` §Coupling comments
records a one-sided mirror as a finding, and a split creates one for the length of a commit.

## 3. What each commit must be true on its own

Test every proposed commit against these before writing a message for it:

- **It compiles.** The failure this repository has already shipped: a commit added
  `mod run_report;` to `lib.rs` while `run_report.rs` stayed on another branch, and `main` did not
  build. Check that every `mod` declaration, `use`, and re-export a commit adds has its file in
  the same commit.
- **Its own tests pass**, or the commit says why not. A fixture without its manifest row, a
  schema without its surface row, a renamed test still cited by an `evidence/` results table.
- **Its subject describes what it does.** `fix: remove index rows for documents this branch does
  not add` added five files and removed no index row. A subject that describes the intent rather
  than the diff is worse than a vague one, because it is believed.
- **It names no file it does not contain.**

## 4. Message shape

Conventional commit, lowercase, imperative, with the story or milestone as the scope where there
is one — `feat(e2-s3)`, `docs(m2)`, `fix(e2-s2)`, `test(recovery)`, or bare `feat:`/`docs:` where
the change spans no single story.

The body is **prose paragraphs, not bullets**, wrapped near 72 columns, saying *why* and what the
alternative was. This repository's messages are unusually long and that is deliberate: a body here
carries the reasoning a reviewer would otherwise have to reconstruct. Read
`git log -1 4d084c7` for the shape — it opens with the requirement that was unmet, gives the
numbers that settle it, and closes with a paragraph beginning **"Not done here:"** naming what a
reader might expect and will not find.

Say what is deliberately absent. Name the tests that prove the claim rather than asserting the
claim. Do not restate the diff — the diff is attached.

## 5. Output

Findings first if any commit would break rule 3, then:

- **Grouping** — one heading per commit, in the order they must land, each listing its paths and
  one line saying what makes it a unit.
- **Commands** — a fenced block per commit, `git add` with explicit paths and never `-A` across
  concerns, then `git commit -F <file>` or a heredoc. Emit them in order, ready to paste.
- **Unaccounted** — every path in the tree that appears in no commit, with the reason: ignored,
  deliberately deferred, or not yours to commit.
- **Verification** — the commands the user should run between or after commits, from
  `AGENTS.md` §Verification, and specifically which commit is the one that must not be split
  because the suite is red across the seam.

Do not summarize the messages after emitting them; the block is the deliverable.
