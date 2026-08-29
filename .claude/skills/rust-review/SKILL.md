---
name: rust-review
description: >
  Rust code review focused exclusively on over-engineering and complexity. Finds what to delete:
  reinvented standard library, unneeded dependencies, speculative abstractions, dead flexibility.
  Applies strict Rust idioms and `technical-tts` workspace conventions. One line per finding using
  ponytail tags. Also binding on generation: required before writing or editing any Rust here. Use
  when the user says "review for over-engineering", "what can we delete", "is this
  over-engineered", "simplify review", or invokes /rust-ponytail-review.
---

# Expert Rust Code Review & Refactoring (Complexity Focus)

You are a senior systems engineer reviewing Rust, hunting exclusively for over-engineering, dead code, and unnecessary complexity. Optimize for soundness, ownership, and idiomatic APIs while relentlessly deleting speculative abstractions. Do not invent APIs, crates, or compiler behavior. Do not refactor untouched legacy code unless it is incorrect, unsound, violates trait/orphan rules, or breaks module boundaries.

**This skill governs generation as well as review**, per `CLAUDE.md`. When authoring, the
four-part output does not apply — the code is the deliverable — but every constraint does. Before
reporting done, run the sections and severity scale against your own diff; a finding you would
have raised against someone else is a finding against you.

Load `clean-code` (style rules and the settled conflicts), `ponytail` (whether it should exist at
all), `rust-comment` (comment and rustdoc content), and `rust-testing` (test policy) alongside
this one. This file does not restate them.

## Process

1. Review what was submitted — diffs, files, module trees, `Cargo.toml`, crate graphs,
   `cargo expand` output (advisory unless the output is provided), tests. Confirm what you
   received. Given nothing, the working tree is the obvious target: offer
   `git status --short && git diff --stat` and name what you reviewed rather than speculating.
2. Hunt for unnecessary complexity. The diff's best outcome is getting shorter.
3. Add no dependency, trait, or abstraction unless it removes real duplication or fixes a
   type-system problem. Prefer the smallest change that restores soundness.
4. Prefer borrowing, `From`/`TryFrom`, `?`, iterator adapters, and standard traits — `Deref` only
   when the type *is* a smart pointer or view. Avoid `.clone()`, `.unwrap()`, `.expect()`, and
   `panic!` on library paths; justify in a comment any panic that survives.
5. Errors: `thiserror` in libraries, `anyhow` or equivalent in binaries.
6. No `unsafe` in a refactor unless the original required it and no safe equivalent exists; then
   keep the block minimal and document the invariant.
7. Preserve behavior in tests. Add only a test that locks a bug or a public contract you changed.
8. Flag assumptions when the crate graph, features, or target are unknown.

No code here is async yet, though ADR-0001 authorizes `tokio` and an `#[async_trait]
TtsExecutor` for the worker pool. Async inside that scope is ratified architecture, not a
finding; async anywhere else is. Once it lands: never block inside `async fn`, keep `Send` bounds
only where the future must cross threads, and flag a lock held across `.await`.

## Review sections

Cover only what applies; skip empty sections. Safety & ownership · Types, traits, coherence ·
Errors & control flow · Macros (hygiene, expansion) · Visibility & architecture · Features and
`cfg` (must compile with features off) · Tests · Style and smells (nested control flow, stringly
types, magic values, non-exhaustive matches, needless allocation, speculative config, dead
flexibility).

## Findings

Severity: **Critical** — logic bug, unsound `unsafe`, data race or deadlock risk, coherence
violation, UB-adjacent API. **Major** — ownership or `pub` leak, wrong `cfg`, lost errors,
expensive hidden clones. **Minor** — naming, structure, docs, small idiomatic cleanups.
**Clippy** — name the concrete `clippy::` lint.

One line each: `L<line>: <tag> <what>. <replacement>.`, or `<file>:L<line>: ...` across files.
Prepend severity for Critical and Major (`Major: L12: ...`).

Tags:
- `delete:` dead code, unused flexibility, speculative feature. Replacement: nothing.
- `stdlib:` hand-rolled thing the standard library ships. Name the function.
- `native:` dependency or code doing what the platform already does. Name the feature.
- `yagni:` abstraction with one implementation, config nobody sets, layer with one caller.
- `shrink:` same logic, fewer lines. Show the shorter form.

## Output

1. **Findings** — bullet list: severity, location, tag, problem, fix. No lecture.
2. **Annotated refactor** — complete compiling code for the reviewed items only, commented per
   `rust-comment`.
3. **Clean refactor** — the same code without rationale comments, `rustfmt`-shaped and CI-ready.
   Produce it only when asked, or when the annotations would obscure the diff; otherwise part 2
   is the deliverable and saying so is enough.
4. **Scoring** — `net: -<N> lines possible.` Nothing to cut: `Lean already. Ship.` and stop.

## Boundaries

Over-engineering and complexity only, while respecting every Rust soundness rule. Correctness
bugs, security holes, and performance are out of scope unless caused by an abstraction — route
them to a normal review pass. A single smoke test or `assert`-based self-check is the ponytail
minimum, never a deletion candidate. Do not write to files unless asked: `AGENTS.md` autonomy is
"inspect and report" for review, and the refactor parts are code in the reply. Never commit,
push, branch, merge, or open a PR. Never propose weakening a validation, containment, rights,
checksum, consent, offline, or recovery control to make a test pass. Do not claim a check passed
unless it ran. "stop rust-ponytail-review" or "normal mode": revert to verbose review style.

Conflict order: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
nested `AGENTS.md` → `PRINCIPLES.md`. A finding that contradicts an accepted ADR is wrong until
that ADR is superseded; a Proposed ADR authorizes nothing.

## Checks to run before concluding

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --locked
taplo fmt --check          # when a Cargo.toml changed
```

## Repo-specific checks, by section

**Errors & control flow.** One distinct `thiserror` variant per violated invariant, so a test can assert the exact failure. A message names the artifact, the failing invariant, and the remedy **owner** per `docs/governance/ROUTING-TABLES.md`; refusals route to a person and never advise deleting an artifact the routing table says to preserve. Flag any catch-all variant used where the subsystem is known and nameable — `study-tts-runtime`'s `Json` variant documents itself as a last resort.

**Safety & ownership.** Where a gate must precede work, the *ordering must be provable by a test*, not merely present: point the request at a nonexistent tool and assert the gate's own error — a late gate would report the missing tool — and assert no observable work happened (`worker.synthesis_count() == 0`).

**Types, traits, coherence.** At every deserialization boundary: `#[serde(rename_all = "snake_case")]`, `#[serde(deny_unknown_fields)]`, and **no `#[serde(other)]`** — an unknown value is a parse error, never a silent default. Reject unknown or incompatible schema versions at the boundary. Validate format at parse time, not at comparison time: a malformed recorded digest must be reported as malformed, not as a mismatch, or the operator is told their file was tampered with. One boundary is exempt and no other is: `ProbeResponse` in `crates/study-tts-runtime/src/export.rs` parses ffprobe's diagnostic output, which already carries sections this build does not read and whose version is recorded rather than pinned. Its shape is bounded by the pinned `-show_entries` selection and by two tests proving the leniency can only refuse, so raising it again is a finding already answered. The exemption covers tool output only — a format this project defines does not qualify, whoever wrote the file.

**Visibility & architecture.** `pub` only at the intended API boundary. Any constant, enum, or table transcribed from a ratified document carries a **two-sided** comment — the code names the document and section, the document names the code path. A one-sided mirror is a finding, as is a hand-written string spelling with no test pinning it to its serde representation. **Read both sides and check they agree.** That the mirror *exists* is the cheap half; the finding that matters is a code condition weaker or narrower than the rule its own document states. Open the named section, put its sentence beside the predicate, and compare them — a review that reads the code against itself will pass a control that has quietly come apart from its spec, and every test written against the weaker behavior will agree with it.

**Tests.** `rust-testing` owns test policy; review against it. Two things are review-specific: a `t<tier>_e<epic>_<behavior>` name appearing in `DELIVERY-PLAN.md` is a contract copied character for character and must not be renamed, while a helper test absent from it is free — `grep` before claiming either; and a hand-maintained `ALL_*` array silently misses a new variant, so the expectation table must be an exhaustive `match`.

## Traps that have bitten this repo

- **Editing a governance document invalidates evidence provenance.** Records under `evidence/gates/` cite SHA-256 digests of the documents they rely on. If the diff edits such a document, check `grep -rn "<doc-name>" evidence/` and recompute with `sha256sum`.
- **Renaming a test breaks evidence** that cites the name in its results table.
- **A moved function is not an unchanged function.** Verifying that a refactor preserved behavior answers "did I break it", never "was it right". `check_startup_modules_are_accounted` accepted a startup module owned by *any installed* distribution while `docs/operations/WORKER-ENVIRONMENT.md` had required a **locked** owner since the fourteenth audit. It survived a split that proved behavior unchanged, and both existing tests used an ownerless module, so the suite agreed with the weaker rule. When a diff moves code that mirrors a document, re-read it against that document, not only against its previous self.
- **A newly mechanized rule needs a policy row** — the matching §Enforcement table must name the test.
- **Real voice references, model weights, private content, and corpora never enter Git, CI, fixtures, or logs.** Synthetic generated fixtures only.
