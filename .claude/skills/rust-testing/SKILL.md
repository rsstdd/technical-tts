---
name: rust-testing
description: >
  How tests are written in the `technical-tts` Rust workspace: TDD order, the
  `t<tier>_<epic>_<behavior>` naming and tier budgets, where a test and its helpers live,
  deterministic offline setup, table-driven cases, Drop-guard cleanup, exact-variant error
  assertions, and when a test dependency is worth proposing (here, usually not). Use whenever
  writing, reviewing, refactoring, or designing a Rust test. Triggers on "rust test",
  "cargo test", "integration test", "test layout", "fixture", "fake worker", and on
  "tokio test", "loom", "proptest", "rstest", "insta", "nextest" — none of which are in the
  tree yet.
argument-hint: "[lite|full|exhaustive]"
license: MIT
---

# Rust tests in this workspace

A test is documentation of intended behavior that a machine can check. It earns its runtime by
pinning one externally meaningful invariant, deterministically, inside its tier's budget.

## Authority

This skill is the craft layer only. `docs/testing/TEST-STRATEGY.md` owns tiers, budgets, naming,
the offline policy, and the required suites; `crates/AGENTS.md` owns TDD order and crate
boundaries. Conflict order is unchanged — newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → this skill. Flag a genuine conflict; never resolve one
silently.

Also load `clean-code` (style), `rust-comment` (comments and rustdoc), `rust-review` (severity
scale), and `ponytail` — which decides whether a test, helper, or dependency should exist at all.

## Decide where the check belongs, in this order

1. **The type system.** An invariant a `NonZeroU32`, private field, sealed trait, or newtype makes
   unrepresentable needs no test. Push it to the compiler first.
2. **A `#[cfg(test)] mod tests` beside the code.** Private state transitions and internal
   invariants. Default home for T1/T2.
3. **`tests/<domain>.rs` in the owning crate.** The public boundary, one file per functional
   domain — `worker_contract.rs`, `schemas.rs`, `walking_skeleton.rs`.
4. **A doc test.** Public API usage a reader of the docs needs. It is an example first, a test
   second.
5. **Nothing yet.** Lock-free interleavings, generated inputs, and snapshots need a crate this
   workspace does not have — see *Not available here*.

## Binding in this workspace

- **TDD.** Write the failing test first, then the minimum change, then refactor green.
- **Names.** `t<tier>_<epic>_<behavior>`, for example `t1_e0_duplicate_segment_id_is_rejected`.
  The `<behavior>` half states the externally meaningful outcome, never the function called.
- **Tier and budget.** State the tier (T1–T6) for every new test and stay inside its budget.
  T1–T4 run offline after dependency restoration and never download a model.
- **Shared helpers live in `study-tts-testkit`** — fixtures, fake worker, fault injection, audio
  helpers. Production crates depend on it only from tests. A `tests/common/mod.rs` is for helpers
  that genuinely must not leave one crate; a bare `tests/common.rs` is never right, since Cargo
  builds it as its own test binary.
- **Runner of record:** `cargo test --workspace --all-targets --locked`, clean under
  `cargo clippy --all-targets`. Report what actually ran.
- **Completion** is `cargo test --workspace` including `walking_skeleton`, not unit tests passing.
  It needs `ffmpeg`/`ffprobe` on `PATH`; if they are absent, say so and name what is unverified.
- **Never weaken a test to make it pass**, and never weaken a validation, containment, checksum,
  consent, offline, or recovery control to make one pass. A flaky test is a defect: quarantine
  needs an owner, expiry, issue, and unaffected-gate analysis.

## Writing the test

- **Deterministic setup.** No network, no clock reading, no ambient environment. Inject time and
  randomness; nothing outside the assigned temporary root.
- **No shared mutable state.** Tests run concurrently, so never mutate an env var or a global.
  Needing to is a design smell in the code under test, not a reason for `serial_test`.
- **`Result<(), Box<dyn Error>>` return** so setup propagates with `?`. Reserve `unwrap`/`expect`
  for the assertion itself, and give `expect` a message that names the invariant.
- **Assert the exact error variant.** Match the enum, never `.is_err()` — a new variant must break
  the test. Pair every `#[should_panic]` with `expected = "..."`.
- **Table-driven over near-identical functions**, with the case in the failure message:
  ```rust
  #[test]
  fn t1_e0_square_matches_reference_values() {
      const CASES: [(i32, i32); 2] = [(2, 4), (3, 9)];

      for (input, expected) in CASES {
          assert_eq!(square(input), expected, "input {input}");
      }
  }
  ```
- **Cleanup by `Drop`, never by trailing statements** — an assertion failure skips them. Use RAII
  or a small guard struct for temp roots, locks, and spawned children.
- **Fake the boundary, not the logic.** Take `impl Read + Write` or a trait rather than a `File`,
  socket, or child process, and drive it with `Cursor<Vec<u8>>` or the testkit fake worker.
- **`Debug` + `PartialEq` on domain types** so `assert_eq!` prints a usable diff.
- **Arrange / Act / Assert**, separated by blank lines. Gate feature-dependent tests with
  `#[cfg(feature = "...")]` so `--no-default-features` still builds warning-free.

## Not available here

**Not async yet.** ADR-0001 §12 lists `tokio` and `async-trait` and specifies an
`#[async_trait] TtsExecutor`, so async is ratified architecture that has not landed — a
sequencing rule, not a ban. Until the worker pool ships, do not add `tokio` or `#[tokio::test]`
to a crate with no async code; when it ships, the rules to write here are test flavors,
`tokio::time::pause`/`advance` instead of real sleeping, and `timeout` around anything that can
hang. No `rstest`, `proptest`, `quickcheck`, `insta`, `loom`, `serial_test`, or `cargo-nextest`
either. Each is a dependency under `AGENTS.md` ("add
only when they remove more risk than they add") and `ponytail`: name the risk it removes and the
hand-written alternative, then let the user decide — never add one silently to satisfy this
skill. Until then a `const` table covers matrices, a seeded loop covers properties (T2 needs no
crate), and a checked-in golden covers snapshots.

## Reporting

Say the tier, where the test lives, and which invariant it pins — one line, in the change summary.
No separate strategy table.

## Intensity

| Level | Behavior |
| :--- | :--- |
| **lite** | Colocated unit tests, correct names and tiers, deterministic setup. |
| **full** | Everything under *Binding* and *Writing the test*. Default. |
| **exhaustive** | Adds hand-written property and boundary coverage, failure-path and cancellation cases, and an explicit dependency proposal where a missing tool would genuinely pay for itself. |
