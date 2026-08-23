# CLAUDE.md

Substance lives in [`AGENTS.md`](AGENTS.md) for the whole repository and
[`crates/AGENTS.md`](crates/AGENTS.md) for the Rust workspace. Read the relevant one before
changing behavior. This file records only what must load first, so it stays short and does not
drift from them.

## Required skills

| When | Load |
|---|---|
| Writing, generating, or editing **any Rust** in this workspace | `clean-code` **and** `rust-review` |
| Reviewing a diff, file, module tree, or crate | `rust-review`, with `clean-code` |
| Writing or editing any other code | `clean-code` |

Load them **before the first edit, not after**. `rust-review` is the standard the code will be
judged against; writing to it costs less than refactoring to it. Before reporting done, apply
that skill's own severity scale to your own output.

`.claude/skills/react-review/SKILL.md` is held for a frontend that ADR-0001 has not authorized.
Its presence is not permission to add one.

## Conflict order

Newest accepted ADR that explicitly supersedes → `docs/adr/ADR-0001-production-rust-study-guide-tts.md`
→ `DELIVERY-PLAN.md` → `AGENTS.md` → `crates/AGENTS.md` → `PRINCIPLES.md` → the skills above.

A Proposed ADR authorizes nothing. Flag a genuine conflict; never resolve one silently.

## Non-negotiables

- Never commit, push, branch, merge, or open a pull request. The user performs all git operations.
- Never weaken validation, containment, rights, checksum, consent, offline, or recovery controls
  to make a test pass.
- Do not claim a check passed unless it ran. State what is unverified and why.
