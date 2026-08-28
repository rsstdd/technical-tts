# E1-S1 Provisional Contract Baseline Evidence v7

- Status: Proposed
- Supersedes: `e1-s1-provisional-contract-baseline-v6`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v6`, SHA-256
`370936dc3c65311bebe728d0982302296f5a4e22bbbf82106e3a65fd5b4e56d0`, for its
controlled-record table and verification run. V6 remains the immutable record
of the worker environment and bundle identity it measured.

The eighth and ninth audits recorded in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
closed the lesson-generation and four-crate replacement gaps. The
current-version Rust constructor supplies both schema metadata fields, and
external callers cannot construct an `AuthoredLesson` while omitting them.
Compatible older documents remain readable. The CLI crate now reports the
tested E1-S1 baseline instead of identifying itself as the E0-S0 placeholder.

Filed under `g1/` because E1-S1 feeds G1. This is story evidence, not G1
acceptance; the interface freeze remains deferred to that gate.

## Acceptance criterion

Accepted when all six hold:

1. Every row in v6's controlled-record table is checked again, with none
   silently dropped.
2. A programmatically created current lesson serializes `schema_version: "1.1"`
   and the stable lesson schema URI under `$schema`.
3. External Rust callers cannot bypass that constructor by supplying schema
   metadata through a struct literal.
4. Compatible earlier lesson documents remain readable and generated schemas
   remain byte-identical to their checked-in files.
5. The real CLI process reports the E1-S1 baseline, identifies E1-S5 as the
   product-command owner, exits successfully, and writes nothing to stderr.
6. The complete Rust suite, formatting, conventions, check, Clippy, and
   doctests pass on the recorded environment.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `aa6b825796245d025a1fbbfa5ab3a7665639e68685845adf8cee69b5042a08d5` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `1eaca00abe695c2ea08e9642b9f2fb6a9dfb55f4679eb99b4f14a34b8748b7ae` |

Only the E1-S1 change record moved. The other five digests were recalculated
from current bytes and agree with v6.

The changed implementation records are pinned separately:

| Record | SHA-256 |
|---|---|
| `crates/study-tts-core/src/lesson.rs` | `88b781ad94239230c44b634058108435a0ddd3002b8827d8ba42f4793408d29d` |
| `crates/study-tts-cli/Cargo.toml` | `21261662615efc22a9aac91f395288cc94d9123b7798630bbd7892c1abb17ce9` |
| `crates/study-tts-cli/src/main.rs` | `0eb7c9dc5a3c7a47c3c28f77ce61b966c41c42f31c86e3950dffe76b69dac70e` |
| `crates/study-tts-cli/tests/status.rs` | `868fbd423437a3be938a3b7322eb6ad91b122135461839b6b7b8cc8a149aa954` |

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-28, cargo 1.97.1, and FFmpeg 6.1.1 on
`PATH`:

- The new test was written first and failed to compile because
  `AuthoredLesson::new` did not exist.
- `cargo test --offline --locked -p study-tts-core` — pass, 82 unit tests, one
  example unit test, and 7 doctests.
- `cargo test --offline --locked -p study-tts-testkit --test schemas` — pass,
  12 tests, including generated-schema byte equality and compatible-minor
  acceptance.
- The CLI process test was written first and failed because the binary still
  reported the E0-S0 walking skeleton and G1 command boundary.
- `cargo test --offline --locked -p study-tts-cli --test status` — pass, one
  process-level contract test.
- `cargo test --offline --workspace --all-targets --locked` — pass, 273 tests.
- `cargo check --offline --workspace --all-targets --locked` and
  `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings`
  — pass, no warnings.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `cargo fmt --all -- --check`, `taplo fmt --check`,
  `python3 scripts/check-rust-conventions.py`, and `git diff --check` — pass.
- `cargo deny check` — advisories, bans, licenses, and sources pass, with the
  existing duplicate-`cpufeatures` warning.

## Identity and compatibility impact

No identity moves. Schema metadata is display/tooling metadata rather than a
synthesis or verification input, the checked-in lesson examples already carry
the same URI, and no generated schema bytes changed. The worker bundle does not
contain `study-tts-core`, so its v6 hash remains valid.

The CLI status text is informational and reaches no lesson, plan, worker,
cache, verification, or package boundary. It moves no durable identity.

The Rust construction API narrows: `schema` and `schema_version` are private,
and new documents use `AuthoredLesson::new`. JSON deserialization and
validation retain the `1.0` compatible-default behavior, so no stored document
needs migration.

## Deviations and limitations

- The product lesson-scaffold command is not implemented at E1-S1; E1-S5 owns
  the user-visible CLI. This change establishes the construction boundary that
  command must use, but there is no authoring command to exercise yet.
- This record and its two predecessors are unapproved, so none is retired by
  its successor: an unapproved superseding record has no effect. Their five
  stale pins are therefore carried as rows in
  [`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md)
  §Accounted provenance mismatches, which names what moved under each and why
  the conclusion stands. Approving this chain is what lets those rows be
  dropped for supersession metadata.
- Two of those five are this record's own: `docs/testing/TEST-DATA-MANIFEST.md`
  and `crates/study-tts-core/src/lesson.rs` both moved after this audit read
  them, for the takes lesson-ID fixture and a `language` doc paragraph
  respectively. The pins above stay as written; they name the bytes this audit
  ran against.
- Real-model qualification, ASR, listening, and reference-machine measurements
  were not run. This change reaches no speech backend and changes no audio.
- T-CORE must ratify the pre-G1 Rust API narrowing and this evidence record.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of §What the eighth and ninth audits closed | |
| Engineering owner | engineering owner | Pending review of the remediation and this record | |
| Affected-track reviewers | T-RUNTIME, T-AUDIO | No identity or audio impact; pending confirmation | |
