# AGENTS.md — Rust Workspace

## Codebase

This is the four-crate Rust workspace for a local-first WSL2 CLI that converts reviewed technical lessons into long-form study-guide audio: `study-tts-core` owns durable domain decisions, `study-tts-runtime` owns the render pipeline through FFmpeg, `study-tts-cli` is the binary (currently a stub; product commands begin at G1), and `study-tts-testkit` holds shared test helpers plus the end-to-end suite.

The root `AGENTS.md` governs the whole repository; this file adds the rules and routing for work inside `crates/`.

## Rules

- Load `.claude/skills/rust-review/SKILL.md`, `.claude/skills/clean-code/SKILL.md`, and `.claude/skills/rust-comment/SKILL.md` before writing, generating, or editing anything in this tree. The review standard governs generation, not only review: writing to it costs less than being refactored to it. Before reporting done, run its review sections and severity scale against your own diff.
- Test-driven development: write the failing test before the production change. Name tests `t<tier>_e<epic>_<behavior_sentence>` (for example `t1_e0_duplicate_segment_id_is_rejected`); the tier definitions are in `docs/testing/TEST-STRATEGY.md`. The test is the documentation of intended behavior and lets the change be validated without a human in the loop.
- One-sentence `///` doc comment on every public type, function, and module. Cheap grounding context for whoever touches nearby code next; repetition over abstraction.
- Document non-trivial cross-file and code-to-document coupling in the code, on both sides, at the point of coupling. Grep is the practical discovery tool; anything only implied by git history or convention is invisible. The load-bearing example: `study-tts-core/src/release.rs` (`REQUIRED_PRODUCTION_GATES`) must mirror `docs/governance/RELEASE-PROFILES.md` §3 — a comment at each end must name the other.
- Naming conventions, applied by default: crates `study-tts-<area>`; one module per domain concern (`lesson`, `plan`, `cache`, `manifest`); error enums with one distinct variant per violated invariant so tests can assert the exact failure; serde enums in `snake_case` string form (`private_preview`); gate and evidence identifiers copied literally from the governance docs, never paraphrased.
- When adding something new, imitate the best existing example rather than inventing a shape: validation with distinct error variants → `study-tts-core/src/lesson.rs`; deterministic identity/hashing → `study-tts-core/src/plan.rs`; an external-tool boundary with preflight → `study-tts-runtime/src/tools.rs`; an end-to-end behavior test → `study-tts-testkit/tests/walking_skeleton.rs`.
- End-to-end verification after any pipeline-visible change: run `cargo test --workspace` including the `walking_skeleton` suite, which exercises the real path — fixture lesson in, cache reuse, PCM assembly, real FFmpeg M4A out, manifest written. Unit tests passing is not completion; the skeleton suite passing is. It requires `ffmpeg`/`ffprobe` on `PATH`; if they are absent, say so and state what remains unverified.
- The quality bar, stated so it gets enforced: rendering is offline (no network egress); outputs are deterministic (cache hits byte-identical, plans stable for identical inputs); every manifest carries `release_status`, and nothing but a passed-gate path may claim `production`; invalid input is rejected before work starts, never patched over; `cargo fmt --check` and `cargo clippy --all-targets` are clean; unimplemented behavior is never described or stubbed as implemented.

## Routing table

| When looking for | Look in |
|---|---|
| Architecture, scope, production invariants | `docs/adr/ADR-0001-production-rust-study-guide-tts.md` (authoritative) |
| Engineering principles and enforcement vocabulary | `PRINCIPLES.md` |
| Milestone scope, backlog order, evidence items | `DELIVERY-PLAN.md` |
| Release profiles and production gates | `docs/governance/RELEASE-PROFILES.md` ↔ `study-tts-core/src/release.rs` (must agree) |
| Ratified deviations from an ADR | `docs/adr/deviations/` |
| Test tiers, strategy, fixture manifest | `docs/testing/` |
| Decision, work, failure, and artifact routing | `docs/governance/ROUTING-TABLES.md` |
| Evidence filing rules | `evidence/README.md` |
| Lesson schema and validation | `study-tts-core/src/lesson.rs` |
| Render planning and cache identity | `study-tts-core/src/plan.rs` |
| Release status and gate identifiers | `study-tts-core/src/release.rs` |
| Pipeline orchestration (build, validate, publish) | `study-tts-runtime/src/pipeline.rs` |
| WAV cache validation and reuse | `study-tts-runtime/src/cache.rs` |
| Synthesis (deterministic fake tone for E0) | `study-tts-runtime/src/synthesis.rs` |
| PCM and silence assembly | `study-tts-runtime/src/assembly.rs` |
| FFmpeg invocation and M4A export | `study-tts-runtime/src/export.rs` |
| External-tool preflight (`ffmpeg`, `ffprobe`) | `study-tts-runtime/src/tools.rs` |
| Output manifest shape | `study-tts-runtime/src/manifest.rs` |
| End-to-end walking-skeleton tests | `study-tts-testkit/tests/walking_skeleton.rs` |
| Safe lesson/audio/pronunciation fixtures | `fixtures/` |
| Documentation index | `docs/INDEX.md` |

# Formatting rules

Source of truth: The Rust Style Guide, Rust 2024 style edition. Upstream wins on any conflict.

## 0. Operating rules

- Emit code that already matches default rustfmt; do not defer layout to a later pass.
- Run `cargo fmt` (or `rustfmt --edition 2024`) after editing, and `cargo fmt --check` before declaring completion, when a shell exists.
- After editing any `Cargo.toml`, run `taplo fmt` (configured by the repo-root `.taplo.toml`); CI enforces `taplo fmt --check` and `cargo deny check`.
- No `rustfmt.toml` options, `#[rustfmt::skip]`, or hand alignment unless asked, because non-default style raises review cost.
- Do not reformat untouched code.
- *small* = fits on one line within the width limit and uses simple names, not nested sub-expressions.

## 1. Whitespace and width

- 4 spaces per level; no tabs outside string literals; every indent a multiple of 4.
- Max width 100 characters. No trailing whitespace.
- Zero or one blank line between items and statements; never two.
- Block indent, never visual indent, because it shrinks diffs and prevents rightward drift:

  ```rust
  f(
      foo,
      bar,
  )
  ```

  not

  ```rust
  f(foo,
    bar)
  ```

- Trailing comma on every comma-separated list followed by a newline; none on single-line lists.

## 2. Sorting

Version-sort wherever sorting is required: compare alternating digit and non-digit runs, digits by numeric value, `_` immediately after lowercase. Correct: `u8`, `u16`, `u32`, `u64`, `u128`, `usize`. Lexicographic (`u128`, `u16`) is wrong.

## 3. Comments and doc comments

- Prefer `//` over `/* */`; prefer `///` over `/** */`; reserve `//!` and `/*! */` for module/crate docs.
- One space after `//`. Single-line block comment: one space inside each sigil. Multi-line: newline after the opening sigil and before the closing sigil.
- Own line where possible; one space before a trailing comment.
- Complete sentences, capital letter, terminal period. Inline block comments may be unpunctuated notes.
- Whole-line comments: 80 columns, counting indentation and sigils. Code lines keep the 100-column limit; a comment is held to the stricter one so it stays readable beside a diff.
- Doc comments precede attributes.
- No comment on a brace line; none inside a function signature.
- This section owns comment mechanics. `.claude/skills/rust-comment/SKILL.md` owns content — what a comment must say, which rustdoc sections are required, and how debt markers are written.

## 4. Attributes

One per line at item indentation. Inner attributes (`#![...]`) indent to the item interior; prefer outer. Argument lists format like function calls. Spaces around `=` in `#[foo = 42]`. Exactly one derive per item, preserving the order of derived names.

```rust
#[repr(C)]
#[derive(Clone, Debug)]
#[long_multi_line_attribute(
    split,
    across,
    lines,
)]
struct CRepr {
    x: f32,
    y: f32,
}
```

## 5. Items

Order: `extern crate` first, alphabetical → `use` statements → `mod foo;` declarations → everything else. Version-sort each group. Never move a `#[macro_use]` module declaration, because that can change semantics.

### Imports

- One line where possible; no spaces inside braces. Prefer several single-line imports to one multi-line import; never split or merge lists by default.
- When breaking: after `{`, before `}`, block-indent names, trailing comma.
- A group is a run of consecutive import lines; version-sort within, never merge or reorder groups; an attribute starts a new group.
- In a list: `self`/`super` first, nested groups and globs last, recursively.
- Normalize `use a::self;` → `use a;`, `use a::{};` → nothing, `use a::{b};` → `use a::b;`.
- Any nested import forces the multi-line form even if it would fit; each nested import on its own line, non-nested names packed onto as few lines as possible:

  ```rust
  use a::b::{
      x, y, z,
      u::{...},
      w::{...},
  };
  ```

### Functions

`[pub] [unsafe] [extern ["ABI"]] fn name(args) -> Ret`. When the signature overruns: break after `(` and before `)`, one argument per block-indented line, trailing comma.

### Structs, unions, enums, tuple structs

- `struct Name {` on one line; fields indented once with trailing commas; `}` unindented on its own line.
- Pull a field type to its own indented line only when it does not fit.
- Prefer `struct Foo;` to an empty struct; if unavoidable, `struct Foo {}` or `struct Foo();` with no interior space.
- Tuple struct on one line when possible: no spaces around parens or semicolon, no trailing comma. Beyond a few fields, use named fields. Broken form: one field per line with a trailing comma.
- Enum variants one per block-indented line, each formatted as a struct (without the keyword), a tuple struct, or a bare identifier. A small struct variant goes on one line with interior spaces and no trailing comma. If any struct variant is multi-line, make all of them multi-line.

### Traits and impls

- Block-indent items; empty trait or impl on one line including braces.
- Bounds: space after `:` not before; spaces around each `+`.
- Prefer not to break bounds; prefer a `where` clause. If bounds must break, every bound on its own block-indented line, break before each `+`, `{` on its own line.
- Avoid breaking an impl signature. If a non-inherent impl must break, break before `for`, block-indent the type, `{` on its own line.

### Generics and where

- Generics on one line; break other parts of the declaration first, and prefer a `where` clause over a broken generics clause.
- No space before/after `<` or before `>`; space after `>` only before a word or `{`, not before `(`; space after each comma; no single-line trailing comma. Broken form: one parameter per block-indented line, break after `<` and before `>`, trailing comma.
- Spaces around `=` in associated-type bindings: `<T: Example<Item = u32>>`.
- Prefer single-letter generic parameter names.
- `where` on the same line as a preceding closing bracket, otherwise on a new line at item indentation; each component on its own block-indented line with a trailing comma unless the clause ends in `;`; a following block or assignment starts on a new line. Prefer an inline bound when the clause is very short. Break a `+`-laden component before each `+` with block-indented continuations.

```rust
fn function<T, U>(args)
where
    T: Bound,
    U: AnotherBound,
{
    body
}
```

### Type aliases, associated types, extern items

Alias on one line when it fits, otherwise break before `=` and block-indent the right-hand side; a trailing `where` follows the broken `=`, while a preceding `where` formats normally with `=` left unindented after the last clause. Format associated types like aliases, with a space after `:` in a bound. Always name the ABI: `extern "C" fn foo`, never `extern fn foo`.

## 6. Statements

- `let`: space after `:` and around `=`, none before `;`. One line if possible; else break after `=` and block-indent; else also break after `:`. A multi-line expression keeps its first line on the `=` line if it fits there, otherwise moves entirely to block-indented lines.
- Semicolon-terminate every expression in statement position unless it ends with a block or supplies a block's value. Semicolon void-typed calls even when the value could propagate.
- Statement-position macros: parentheses or square brackets, terminating semicolon, no spaces around the name, `!`, delimiters, or `;`.

### let-else

One line only when the whole statement is small, the else block holds one single-line expression with no statements and no comments, and the part before `else` fits on one line:

```rust
let Some(1) = opt else { return };
```

Otherwise never break between `else` and `{`, always break before `}`, indent `}` to the `let` and the contained block one step further. Keep `else {` on the initializer line when the initializer is single-line and fits, or when a multi-line initializer ends with closing delimiters alone on a line at the `let` indentation level; otherwise put `else` on the next line at the `let` indentation level.

## 7. Expressions

### Blocks and closures

- Newline after `{` and before `}` unless another rule permits one line. Keyword (`unsafe`, `async`) on the brace line, one space. Empty block is `{}`. A block attribute goes on its own line before the block.
- Single-line block only in expression position (or an `unsafe` block in statement position), with one single-line expression, no statements, no comments, and spaces inside the braces.
- Closures: no space before the first `|` unless a keyword such as `move` precedes it, one space after the second `|`, parameter syntax as in a function definition with types elided where possible. Omit braces unless there is a return type, statements, comments, or a multi-line control-flow body.

### Literals

- Struct literal: space before `{`; space after `:` not before; small → one line with interior spaces and no trailing comma; else one field per block-indented line with trailing comma. `..expr` never takes a trailing comma and no space after `..`.
- Never break inside `()`, even past the width limit.
- Tuple and array literals: no space inside delimiters, comma-space between elements, broken form one per block-indented line with trailing comma and newlines after `[`/`(` and before `]`/`)`. Tuple struct literals add no space before `(`. Square brackets for `vec!` and similar. Repeating initializer: space after `;` only, break after `;` not before.
- Qualify enum literals with the enum name unless the variant is in the prelude.
- Never mix digit case within one hex literal; keep one case project-wide.

### Operators

- No space between a unary operator and its operand; one space after `&mut`; never break between them.
- Spaces around every binary operator, including `=`, `+=`, and `as`.
- Parenthesize liberally for precedence; never signal precedence with spacing; never auto-insert or auto-remove parentheses. Prefer dereferencing to referencing in comparisons: `*t op u`, not `t op &u`.
- Breaking: block-indent continuations, one sub-expression per line, break after assignment operators and before all others, and prefer breaking at an assignment operator.
- Break before `as`, never after; in a cast chain, if one break makes the rest fit, leave the remaining types on that line, otherwise break before each `as`.

### Calls, indexing, ranges

- No space between callee and `(`, after `(`, before `)`, or before a comma; one space after a comma. Prefer not to break the callee expression.
- Nullary calls are always `func()` on one line, even past the width limit.
- Break a call when it is not small, would overrun, or has a multi-line argument or callee: one argument per block-indented line, break after `(` and before `)`, trailing comma.
- No spaces around `.` or indexing brackets; never break between the target and `[`. A broken index goes block-indented with newlines after `[` and before `]`.
- No spaces in ranges (`0..10`, `x..=y`, `foo..`); break before the operator and block-indent; parenthesize compound bounds: `0..(x - 10)`.
- Format patterns like their corresponding expressions.

### Chains (field accesses, method calls, `?`)

One line when small; otherwise one element per line, breaking before `.` and after `?`, every continuation block-indented. Combine the first two lines when the last line of the first element plus its indentation is no wider than the second line's indentation, recursively. If any element is multi-line, that element and all later ones get their own lines. Prefer a fully multi-line chain with one-line elements over a mix:

```rust
// Preferred.
self.pre_comment
    .as_ref()
    .map_or(false, |comment| comment.starts_with("//"))
```

### Macro uses

A macro parseable as another construct formats as that construct (`foo!(a, b, c)` as a call). For format-string macros: format string on its own line; arguments before it packed on one line if they fit, arguments after it likewise; otherwise one per line as with a function call. Apply these rules only to language and standard-library macros; assume nothing for third-party macros.

```rust
assert_eq!(
    x, y,
    "x and y were not equal, see {}",
    reason,
);
```

### Control flow

- No parentheses around `if`/`while` conditions; interior parentheses that aid arithmetic or logical reading are fine.
- Keyword, clauses, and `{` on one line when they fit. `} else {` on one line with single spaces around `else`.
- If the control line breaks: break after `=` in a `let` sub-expression, before `in` in a `for`, block-indent the continuation, and move `{` to its own unindented line.
- Exception: when the initial clause spans multiple lines, ends with closing delimiters alone on a line, and that line is not indented past the first line of the expression, keep `{` on that line after a space.
- Single-line let-chain only with exactly two clauses, a literal or identifier (optionally unary-prefixed) on the left, and a single-line `let` on the right.
- Single-line `if else` only in expression position, with exactly one `else`, and only when small.

### Match

- Never break inside the discriminant. Always break after `{` and before `}`. Arms block-indented once, block bodies indented once more.
- Trailing comma on an arm if and only if the body is not a block.
- Never lead a pattern with `|`. Avoid splitting the left-hand side; prefer a block body that keeps the pattern on one line.
- Never break after `=>` without a block body.
- Keep a body on the pattern line when it is a single expression with no line comments and is not a control-flow expression; otherwise use a block. Never use a block for a same-line body unless the block is empty. Never flatten a block holding a single macro call, because the expansion may carry a trailing semicolon.
- Broken pattern: one clause per line at the same indentation, breaking before `|`; pack multiple small clauses per line where they fit. With an `if` guard on a broken pattern, break before `if`, block-indent it, and start the block body on a new line — unless the pattern's last line is narrower than the indent, in which case keep the guard on that line.
- Pattern clause small grammar: a single token, `&small_no_tuple`, `&small`, or a unary tuple `(small_no_tuple,)`. `&&Some(foo)` qualifies; `Foo(4, Bar)` does not.

### Combinable expressions

A call with a single multi-line argument formats as though single-line when the result fits; the same applies to macros, tuple-struct literals, and bracketed lists, recursively. Extend it to a multi-argument call whose last argument is a braced multi-line closure, provided no other argument is a closure and all arguments plus the closure's first line fit on the first line.

```rust
foo(bar(
    an_expr,
    another_expr,
))

foo(first_arg, x, |param| {
    action();
    foo(param)
})
```

## 8. Types and bounds

| Form | Example |
| --- | --- |
| Slice / array | `[T]`, `[u32; 42]` |
| Raw pointer | `*const T`, `*mut T` |
| Reference | `&'a T`, `&mut T` |
| Function type | `unsafe extern "C" fn<'a>(T, U) -> W`, `fn()` |
| Never | `!`, treated as a type name |
| Tuple | `(A, B, C)`, no trailing comma unless a one-tuple |
| Path | `<Baz<T> as SomeTrait>::Foo`, `Foo::Bar<T, U>` |
| Bound sum | `T + T`, `impl T + T` |

No space around parentheses in types. Avoid breaking types; when required, break at the outermost scope, break `[T; expr]` after the `;`, follow the function and generics rules for those forms, and break a `+` sum before every `+` with block-indented continuations. Format a `use<'a, T>` precise-capturing bound like an angle-bracketed trait bound.

## 9. Naming and expression style

- UpperCamelCase: types, enum variants. snake_case: struct fields, functions, methods, locals, macros, modules. SCREAMING_SNAKE_CASE: const and immutable static.
- Reserved words: raw identifier (`r#crate`) or trailing underscore (`crate_`), never a misspelling (`krate`).
- Avoid `#[path]`.
- Prefer expression orientation: `let x = if y { 1 } else { 0 };` over declare-then-assign.
- Spaces around keywords and before an opening brace in `extern crate foo;` and `mod foo { }`; no space before the semicolon. Use `{}` for the full body of a `macro_rules!` definition.

## 10. Cargo.toml

- Same width and indentation as Rust code; every key at column zero; one space around `=`.
- One blank line between a section's last pair and the next header; none between a header and its pairs or between pairs.
- `[package]` first, with `name` then `version` at its top, remaining keys next, `description` last. Version-sort keys in all other sections.
- Bare keys; quote only when the name requires it. Multi-line strings rather than newline escapes.
- Arrays on the key line if they fit; otherwise break after `[`, indent one level, comma after every item including the last, `]` alone at the start of a line.
- Inline tables on the key line if they fit; otherwise promote to their own section.
- `authors` entries as `Full Name <email@address>`, never bare addresses or bare names; `license` a valid SPDX expression (`/` accepted for OR); `homepage` a full URL with scheme.
- Wrap `description` at 80 columns, do not open with the crate name, and put the summary sentence on a line by itself.

## 11. Tie-breakers

Uncovered cases resolve in the style team's priority order:

1. Readability (scan-ability, no misleading formatting, accessibility, legibility in compiler errors, diffs, and grep)
2. Aesthetics (consistency with neighbouring code and other tools)
3. Specifics (diff and merge friendliness, no rightward drift, economy of vertical space)
4. Application (ease of manual and tool application, internal consistency, simplicity of the rule)

## 12. Pre-output checklist

- 4-space indentation, no tabs, no line over 100 chars, whole-line comments within 80 columns including indentation.
- Trailing commas on every broken list and nowhere on single-line lists.
- Imports and `extern crate` version-sorted within groups; groups unchanged.
- One attribute per line, single derive.
- Chains, calls, operators, and types break per Sections 7 and 8.
- Match arms carry trailing commas exactly when the body is not a block.
- Names follow Section 9.
- `cargo fmt --check` passes, or the reason it could not run is stated.
