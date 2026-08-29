---
name: rust-comment
description: The commenting and rustdoc standard for this Rust workspace — why-not-what prose, `///` and `//!` doc comments, the `# Examples` / `# Errors` / `# Panics` / `# Safety` sections, two-sided coupling comments to governance documents, and `TODO`/`FIXME`/`#[deprecated]` debt markers. REQUIRED before writing, generating, or editing any Rust here, and used to review the comments in a diff, file, or crate.
---

# Rust comments and doc comments in this workspace

Every comment is code that a compiler cannot check. It earns its line by carrying something the
code cannot: intent, a trade-off, an invariant, a domain rule, a coupling to a ratified document.
Anything else is future noise.

## Authority

`crates/AGENTS.md` §3 owns comment *mechanics* (sigils, spacing, 80-column whole-line comments,
punctuation, doc comments before attributes). This skill owns *content*: what to say, where, and
when a comment must exist. Conflict order is unchanged — newest accepted ADR that explicitly
supersedes → `docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` →
`AGENTS.md` → `crates/AGENTS.md` → `PRINCIPLES.md` → this skill. Flag a genuine conflict; never
resolve one silently.

Load `clean-code` (style and structure) and `rust-review` (the correctness standard and the
severity scale, plus the conduct rules that bind every skill here). This is the comment layer of
both; it adds constraints and changes neither.

## General

- **Explain WHY, not WHAT.** Design trade-offs, safety invariants, domain rules, and the reason
  an ordering or a deviation is load-bearing. A comment that restates the code is a finding, and
  so is one that does not help its reader.
- **Terse and dense.** Legible to a human skimming a diff and to an LLM reading it cold.
- **Delete commented-out code.** Git tracks history; a commented block is noise that bit-rots.
- **An outdated comment is a bug.** Update or delete it in the same commit as the code it
  describes — never in a follow-up.
- **Keep locality.** Item-level prose goes above the item; a note about one expression goes
  inline beside it. A comment far from what it explains stops being maintained.
- Applies to code you write or touch, under the boy scout rule in `AGENTS.md`. It is not a
  mandate to retrofit untouched files.

## Doc comments

- `///` above every public item — type, function, trait, enum, field, const. Not optional here:
  `missing_docs = "warn"` in the workspace lint table plus CI's `-D warnings` makes an
  undocumented public item a merge blocker. One sentence; repetition over abstraction.
- `//!` at the top of a module or `lib.rs` for architecture and scope — what the module owns,
  what it deliberately does not, and which document governs it.
- Idiomatic Markdown. Code blocks are compiled: ```` ```rust ````. Link items with
  ``[`CacheKey`]`` rather than naming them in plain prose.

## rustdoc sections

Use them in this order, and only when they apply.

| Section | When | Content |
|---|---|---|
| `# Examples` | Public API worth showing in use | Real usage, not a toy. Compiled and run as a doctest. |
| `# Errors` | Every public fn returning `Result` | Which failure conditions produce which variant. |
| `# Panics` | Any path that can panic | The condition, and why no caller argument can reach it. |
| `# Safety` | Every `unsafe fn` | The invariants the caller must uphold to avoid UB. |

**`# Errors` names variants, not categories.** Error enums carry one variant per violated
invariant so a test can assert the exact failure; the doc must let a caller map a condition to
that variant. Where the error routes a refusal to a remedy owner per
`docs/governance/ROUTING-TABLES.md`, say who.

**`# Panics` must justify, not just disclose.** See `study-tts-core/src/plan.rs` — it documents
the panic, then argues from the concrete `Serialize` implementations that no argument can reach
it. A bare "panics if serialization fails" is not enough on a library path; `rust-review`
requires the justification.

**Doctests run in CI, not in the local test command.** `.github/workflows/ci.yml` runs
`cargo test --offline --workspace --doc --locked` as its own step, but `AGENTS.md`'s
`cargo test --workspace --all-targets --locked` excludes doctests. Run
`cargo test --workspace --doc` yourself after adding or editing an example, and never claim it
passed otherwise. Keep examples offline and free of external binaries — an example needing
`ffmpeg` belongs in the T4 suite, not in rustdoc. Reach for `no_run` or `ignore` only with a
comment saying why.

**`# Safety` is currently unreachable.** `unsafe_code = "forbid"` is set workspace-wide in the
root `Cargo.toml`: there is no `unsafe` here, and adding it is a Critical finding needing an ADR,
not a `// SAFETY:` block. If an accepted ADR ever lifts the forbid, the rules written back here
are: `// SAFETY:` above every block explaining why the invariants hold at that point; raw-pointer,
layout, non-null, alignment, aliasing, and thread-safety assumptions spelled out; and the block
kept minimal so the comment covers exactly what it justifies.

## Logic and performance

Document what a reader cannot recover from the code:

- Non-obvious allocations, and zero-copy or borrow-based designs that look accidental.
- A hand-placed `#[inline]` — say what measurement or cross-crate call motivated it.
- Complex algorithms, bit manipulation, and tricky conversions (`AsRef`, `Into`, `TryFrom`).
- Determinism-carrying choices. This project's outputs must be byte-identical across rebuilds;
  where an ordering, a hash input, or a serialization shape is load-bearing for that, say so.

## Coupling comments

Any constant, enum, or table transcribed from a ratified document carries a **two-sided**
comment: the code names the document and section, the document names the code path. A one-sided
mirror is a finding. The load-bearing example is
`study-tts-core/src/release.rs::REQUIRED_PRODUCTION_GATES` ↔
`docs/governance/RELEASE-PROFILES.md` §3.

This is the settled exception to "avoid comments; explain in code" recorded in `clean-code`: a
mirror between code and a ratified policy cannot be expressed in code, so both ends must name
each other. `grep` is the discovery tool — anything implied only by git history is invisible.

## Debt and status

- `// TODO(author): action` — a named owner and the action to take. An unowned `TODO` is
  anonymous debt and will not be done.
- `// FIXME: bug` — known-wrong behavior, described so a reader can reproduce it.
- Neither marker is a substitute for a test or a plan item. Work already scoped belongs in
  `DELIVERY-PLAN.md`, referenced by ID from the comment.
- Do not confuse these with `clippy::todo` and `clippy::unimplemented`, both `warn` in the
  workspace lint table: those catch the `todo!()` and `unimplemented!()` *macros*. Shipping a
  stub as if it were implemented is forbidden by `AGENTS.md` regardless of the comment beside it.
- Phasing out an API: `#[deprecated(note = "...")]` carrying the replacement, plus a `///` line
  saying since when and why. The attribute is what callers see; the comment is what a maintainer
  needs.

## Reviewing comments

Run the sections above against your own diff before reporting done, and map what you find onto
the `rust-review` severity scale:

- **Major** — a wrong or stale doc on a public item, an undocumented or unjustified panic on a
  library path, a missing `# Safety`, or a one-sided coupling comment, whose other end then
  drifts silently.
- **Minor** — a missing `# Errors`, a comment that restates the code, an unowned `TODO`.

Two checks are not readable off the diff: `grep` the governing document for the code path before
calling a coupling comment two-sided, and actually run `cargo test --workspace --doc` if an
example changed.
