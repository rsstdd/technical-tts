# CLAUDE.md

Substance lives in [`AGENTS.md`](AGENTS.md) for the whole repository and
[`crates/AGENTS.md`](crates/AGENTS.md) for the Rust workspace. Read the relevant one before
changing behavior. This file records only what must load first, so it stays short and does not
drift from them.

## Required skills

| When | Load |
|---|---|
| Writing, generating, or editing **any Rust** in this workspace | `clean-code`, `ponytail`, `rust-review`, **and** `rust-comment` |
| Auditing or reviewing a diff, file, module tree, or crate | `rust-review` and `ponytail`, with `clean-code` and `rust-comment` |
| Writing or editing any other code | `clean-code` **and** `ponytail` |

Load them **before the first edit, not after**. These are binding standards for code written
here, not advice to weigh: `rust-review` is what the code will be judged against and `ponytail`
is what decides whether it should exist at all, so writing to them costs less than refactoring
to them. Before reporting done, apply `rust-review`'s own severity scale to your own output.

## Conflict order

Newest accepted ADR that explicitly supersedes → `docs/adr/ADR-0001-production-rust-study-guide-tts.md`
→ `DELIVERY-PLAN.md` → `AGENTS.md` → `crates/AGENTS.md` → `PRINCIPLES.md` → the skills above.

A Proposed ADR authorizes nothing. Flag a genuine conflict; never resolve one silently.

## Non-negotiables

- Never commit, push, branch, merge, or open a pull request. The user performs all git operations.
- Never weaken validation, containment, rights, checksum, consent, offline, or recovery controls
  to make a test pass.
- Do not claim a check passed unless it ran. State what is unverified and why.
