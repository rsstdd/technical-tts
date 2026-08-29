---
name: clean-code
description: Clean Code (Robert C. Martin) rules as applied in this repository, with the conflict order against binding repo conventions and the settled conflicts that must not be re-litigated. REQUIRED before writing, generating, editing, or reviewing any code here, and whenever a Clean Code rule appears to conflict with a repo convention.
---

# Clean Code in this repository

These rules govern all code written or edited here. They are style and structure guidance,
not authority: **`AGENTS.md`, `crates/AGENTS.md`, `PRINCIPLES.md`, and the accepted ADRs win
on any genuine conflict.** Where they conflict, apply Clean Code inside the repo convention's
frame and say so in the summary — never resolve the conflict silently.

Conflict order: newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md`, then
nested `AGENTS.md` → `PRINCIPLES.md` → these rules. `ponytail` decides whether the code should
exist at all; `rust-comment` owns comment content; `rust-testing` owns Rust test policy.

## Known conflicts, already settled

Do not re-litigate these. Flag anything new.

| Clean Code rule | Repo convention that wins | Why |
|---|---|---|
| One assert per test | Loop-over-variants tests with several assertions | Test names are copied character for character from `DELIVERY-PLAN.md`; splitting a named test breaks traceability. Keep one *behavior* per test and put the failing case in the assertion message. |
| Prefer polymorphism to if/else | Exhaustive `enum` + `match` for domain rules | Serde parse-rejection of unknown values is load-bearing for fail-closed gating; a trait object cannot refuse an unknown variant at the boundary. Traits stay for real seams (`SegmentSynthesizer`), not closed rule sets. |
| Avoid comments; explain in code | Two-sided coupling comments to governance docs | A mirror between code and a ratified policy cannot be expressed in code. Both ends must name each other. |
| Prefer polymorphism / non-static methods / no base class knows its derivatives | Free functions, associated functions, and enums | Rust has no inheritance. Read the OO rules as "one thing owns one behavior", not as a demand for a type hierarchy. |

## General

- Follow standard conventions. Keep it simple: simpler is always better.
- Boy scout rule: leave the code cleaner than you found it — within the change's scope. Do not
  reformat, rename, or reorganize unrelated code (`AGENTS.md` operating rules).
- Always find the root cause. No workaround without saying why.

## Design

- Keep configurable data at high levels. Prevent over-configurability.
- Inject what a unit depends on; follow the Law of Demeter.
- Separate multi-threading code from the logic it runs.
- Prefer polymorphism to if/else or switch — subject to the settled conflicts above.

## Understandability

- Be consistent: one idea, one spelling, one shape.
- Use explanatory variables. Encapsulate boundary conditions.
- Prefer dedicated value objects to bare primitives — a `VoiceUse` enum, not a `&str` compared
  ad hoc at each call site. Scope compared by string equality is scope that stops being enforced.
- Avoid logical dependency between methods. Avoid negative conditionals.

## Names

- Descriptive, unambiguous, pronounceable, searchable, meaningfully distinct.
- Named constants over magic numbers (`BLAKE3_HEX_LENGTH`, not `64`).
- No encodings, no type prefixes, no Hungarian notation.

## Functions

- Small. Do one thing. Descriptive name. Few arguments. No side effects. No flag arguments —
  a boolean parameter that selects behavior means two functions.

## Types and data

- Hide internal structure. Prefer plain data where there is no behavior; avoid hybrids.
- Small, doing one thing, with few fields.
- Prefer many functions to one function taking a parameter that selects behavior.

## Comments

- Explain yourself in code first.
- Comments carry intent, clarification, consequence, or warning — never a restatement of the
  code, never a closing-brace label, never commented-out code.
- In this repo a comment is also the right tool for: why an ordering is load-bearing, why a
  deviation is proportionate, and what a code↔document mirror is bound to.

## Source structure

- Separate concepts vertically; keep related code vertically dense.
- Declare variables close to use. Keep dependent and similar functions close.
- Functions read downward: callers above callees.
- No horizontal alignment. Use whitespace to associate, not to decorate. Do not break
  indentation. Widths belong to `AGENTS.md` (100 columns; 80 for whole-line comments).

## Tests

- Readable, fast, independent, repeatable. One concept per test.
- **A test must not re-derive the implementation.** Expected values are a table a reviewer reads
  against the controlling document, not a second copy of the code under test — a copied
  `matches!` passes for any policy, including a wrong one.
- Prefer an exhaustive `match` in the expectation table so a new enum variant is a compile error
  in the test rather than an untested one.
- A test that needs an external binary for a claim that does not require one is not independent.

## Smells to refuse

Rigidity, fragility, immobility, needless complexity, needless repetition, opacity — plus the
unrequested abstractions, boilerplate, and cleverness that `ponytail` refuses.

In this repo, "opacity" specifically includes returning a catch-all error variant where the
failing subsystem is known and could be named.
