# E1-S1 Provisional Contract Baseline Evidence v5

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v4`, SHA-256
`a50c7b92fada2ede621008a674ee12671b3df064a57dbc1b89d8efbc4c8dba9b`, for its
controlled-record table, verification run, and worker-bundle hash. V4 remains
the immutable record of the bytes and identity it measured.

The full-tree audit recorded in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
§What the sixth audit closed found that JSON Schema's ECMAScript `$` anchor
admits a final line terminator. The Rust parsers do not. The published patterns
now carry an absolute-end guard, and the test-only schema validator no longer
treats malformed primitive subschemas or malformed `type` arrays as
permissive.

Filed under `g1/` because E1-S1 feeds G1. This is story evidence, not G1
acceptance; the interface freeze remains deferred to that gate.

## Acceptance criterion

Accepted when all five hold:

1. Every row in v4's controlled-record table is checked again, with none
   silently dropped.
2. The E1-S1 change record pins the sixth audit's compatibility, cache, and
   worker-identity impact.
3. The repository's complete offline Rust suite, Python worker suite, schema
   checks, formatting, conventions, documentation, and dependency policy pass
   on the recorded environment.
4. An independent Draft 2020-12 validator rejects representative trailing-line-
   terminator values the old schemas accepted.
5. Every check not run, or run with a limitation, is named under §Deviations
   and limitations.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `18d36edcbba3406ed35d1765411859434c826eee99ff884dfa4f851a5a9d547b` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `1eaca00abe695c2ea08e9642b9f2fb6a9dfb55f4679eb99b4f14a34b8748b7ae` |

Only the E1-S1 change record moved. The other five digests were recalculated
from current bytes and agree with v4.

`docs/operations/WORKER-ENVIRONMENT.md` remains outside the controlled table for
the reason v2 through v4 record. Its current SHA-256 is
`d4496d3a280a0073cf22716cd748f54e88c8a1bfeaec12ee4eb2dcb91da6383c`.

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-28, cargo 1.97.1, FFmpeg 6.1.1 on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 269 tests.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `cargo test -p study-tts-testkit --test schemas --locked` — pass, 12 tests.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 34
  tests.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21 tests.
- `cargo check --workspace --all-targets --locked --offline` and
  `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo doc --offline --workspace --no-deps --locked`,
  and `git diff --check` — pass, no warnings.
- Schema regeneration through
  `cargo run --offline --locked --package study-tts-runtime --example generate-schemas`
  followed by the generated-schema test — pass.
- Python 3.12 `compileall` over `worker/study_tts_worker` — pass.
- PyYAML 6.0.1 parsed both workflow files — pass.
- Python `jsonschema` 4.10.3 checked every generated schema against its
  metaschema and independently rejected `lesson_id: "a\n"` and
  `language: "en\n"` under `lesson-v1` — pass.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`, with
  the existing duplicate-`cpufeatures` warning.

## Worker-bundle hash

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`
returned the same value twice against the restored locked environment:

```text
8f11f9edc75096688b0d2f17ceed5eec767c32cb42aefa6c4f346ff68df844c5
```

It supersedes v4's
`f9a0c8f25e322aa7eeb34382a45dd702be72df7b33e476543c0907a0728e9ec4`.
The only bundle input whose bytes moved is
`schemas/worker-protocol-v0.schema.json`; its digest patterns gained the same
absolute-end guard as the other published schemas. The input set, runtime
identity, lockfile, and derivation are unchanged.

## Deviations and limitations

- This was a developer-machine run, not the protected reference-machine
  qualification workflow. The reference machine must reproduce the bundle hash
  before G1.
- The self-hosted qualification workflow was parsed but not dispatched. Its
  environment approval, runner-group restriction, and fixed interpreter link
  remain external configuration that a local run cannot prove.
- `cargo deny --offline check` could not acquire the advisory database lock
  because the managed environment exposes that path read-only. The ordinary
  `cargo deny check` used the existing database and passed; no dependency moved
  during this audit.
- Python `jsonschema` is an independently installed development tool, not a
  locked project dependency or CI gate. Its result supplements the locked Rust
  schema suite; it does not replace it.
- Real-model qualification, ASR, listening, and reference-machine measurements
  were not run. This change reaches no speech backend and changes no audio
  bytes.
- T-CORE must ratify the compatible-patch classification. T-RUNTIME must review
  the cache-impact account; existing entries are retained and simply stop
  matching the new worker identity.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of §What the sixth audit closed | |
| Engineering owner | engineering owner | Pending review of the remediation and this record | |
| Worker/runtime owner | T-WORKER | Pending reference-machine reproduction of the bundle hash | |
| Affected-track reviewers | T-RUNTIME, T-AUDIO | Pending cache-impact review; audio behavior unchanged | |
