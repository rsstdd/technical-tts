# E1-S1 Provisional Contract Baseline Evidence v9

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v8`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v8`, SHA-256
`c31db66ecae87aa3a31986f51f13dc1630bf21d50051248cc568fbc0ee539546`, for its
controlled-record table and verification run. V8 remains the immutable record of
the bytes it read, and everything it concluded about the tenth through
sixteenth audits stands unchanged: this record adds one audit and re-measures,
it does not revisit a decision.

**Superseding rather than amending.** V8 carries `- Status: Accepted`, so
`evidence/README.md`'s "Never overwrite an accepted report" applies without
qualification, and this record supersedes it rather than correcting it in place.
V8's status is also inconsistent with its own §Review table, which records four
Pending decisions, and with `evidence/README.md`'s rule that "a table containing
a Pending or Proposed decision is not accepted." That inconsistency is recorded
under §Deviations and limitations rather than repaired: v8 is immutable, and
this record's acceptance retires it from checking, so the inconsistency has no
further effect. This record's own §Review table is completed, which is what
keeps the same defect from recurring here.

The seventeenth audit, recorded in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md),
closed no defect. It answers a review of the E1-S1 baseline against
`rust-review` and `ponytail`, and every item is scope, legibility, or governance
debt rather than behavior:

- **The environment check is authorized rather than assumed.** Comparing the
  installed environment against `worker/requirements.lock` is a precondition on
  returning a bundle identity, not one of the inputs ADR-0001 §12.5 names.
  [`ADR-0001-D004`](../../../../docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md)
  records the gap it closes, its measured cost, the alternatives, and a
  rollback. The project owner approved it on 2026-08-29, so the check is
  governed scope rather than an unrecorded extension of §12.5.
- **The bundle module was split along that boundary.** `worker_bundle.rs` was
  4,119 lines covering both the §12.5 identity and the D004 precondition; the
  precondition is now `worker_environment.rs`, reached through exactly two
  crate-private functions so no probe or lockfile type crosses the boundary.
  The split deletes nothing and is deliberately line-neutral.
- **The runtime probe is a Python file.** Its ~180 lines moved from a `concat!`
  of Rust string literals to `runtime_probe.py`, loaded by `include_str!`. The
  executable code is byte-identical, verified line by line before the move.
- **Sixteen tests were misfiled by tier**, at T1 while resolving or spawning an
  interpreter, which `DELIVERY-PLAN.md` §3.2 places at T4. All sixteen are
  renamed. None is named in `DELIVERY-PLAN.md`; the two that are keep their
  names.
- **CI reports tier duration**, which `DELIVERY-PLAN.md` §3.3 requires and
  nothing implemented.

**No wire contract, schema, error variant, refusal message, or worker-bundle
input moved.** §Worker-bundle hash below is the evidence: the verified identity
is the same value v8 recorded.

Filed under `g1/` because E1-S1 feeds G1. This is story evidence, not G1
acceptance; the interface freeze remains deferred to that gate.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. V8's eleven criteria carry
forward unchanged and were re-run, not re-argued. This record adds five, and is
accepted when all sixteen hold:

12. The verified worker-bundle hash is reproduced and is **identical** to the
    value v8 recorded. A refactor that moved it would be a refactor that changed
    the product.
13. `runtime_probe.py`'s executable lines are byte-identical to the Rust string
    literals they replace, compared mechanically rather than by reading, and the
    file compiles under CPython 3.12.
14. Every test in `worker_environment` carries a `t4_` prefix, no renamed name
    appears in `DELIVERY-PLAN.md`, and no superseded evidence record was edited
    to follow a rename.
15. The environment check's cost is measured on the recorded environment and
    stated as a number, with what it is paid per.
16. The full Rust and Python suites, formatting, conventions, Clippy, doctests,
    `cargo doc`, `cargo deny`, and the provenance check pass, with the schema
    regeneration proved idempotent and every check not run named under
    §Deviations and limitations.

## Controlled records

Every row v8 pinned is checked again here, with none dropped. Rows this audit
did not move carry v8's digest recalculated from current bytes.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `581c22ad07a0152eaa50c6f3cb25dc64654e3d3dffc9998a19c3b280563662c4` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `d2738f9a03e84d74b70f92c66aac31b1e5e2dbf36e263def2e059f2518b383f3` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |

`E1-S1-INTERFACE-CHANGE-001.md` moves for the seventeenth audit's own record.
The other five agree with v8. `TEST-DATA-MANIFEST.md` still differs from the
digest v5, v6, and v7 each pin, for a movement that predates this audit and is
accounted in
[`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md).

The changed implementation records are pinned separately:

| Record | SHA-256 |
|---|---|
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `b0454bc2cd2351f806e7e896eb093e93c603b3bdd6d74e8ace312537e7b92cef` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `eda72a02e6053da8e0860cabcb9ac8a39a592d8661f501598241393ea0e28b6d` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `5f402386284f44175a32b6933859d0868225b7630bfbc02f97dc659119c08a42` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `d6785d52d6714f53247d4b036d6cc4f021cf50a4b0e7a8546d786488e0d27bf0` |
| `crates/study-tts-testkit/src/json_schema.rs` | `97ba85b3fb0a057a088d634faab5d8a0cdf5c717bf9029f564f54d008572dbed` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `ebcf0783ad1b24b7e12ce6c1f665762c3eadde1a37583fada0d174cff14d464e` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e889d30b83cbc4e62c8b779c6fed4082c357facab6bcfa65141044` |
| `schemas/worker-protocol-v1.schema.json` | `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` |
| `worker/AGENTS.md` | `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` |
| `worker/bundle-manifest.json` | `2135f785f47f6e9bc21ef6e9d95e8b67b990c7f689c9f32c01aace55a0dd46a4` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `da7baa5c48d6038c3537e6414614de9beedcdf2098abd74d5a70d105814b4c98` |
| `worker/study_tts_worker/worker.py` | `0777f9b16a41e1c2db00c445229c04b48328bae7fafc6001174846aca0fc8bbf` |
| `worker/tests/test_protocol.py` | `405e9c41787b6784374146b695e166ff2b9de5828ba259826e7078f99149a6fd` |
| `worker/tests/test_worker.py` | `682f2d24c7db45bc0bac90aa4d37de72238f456203b8f2b1a06c3fa6b5aa7113` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef3bc1c1338a62e1126300cc0bb97a89d0a89c4d6dcfb7c88025d9` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `d2e102a091a9610056d8378fa6e9ada7f294d47a4df3cb76c473ac9de8345fc6` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `9ee3fd43ac856b2a48154d1a7c18736cb3c147d72677eaaf8332aeca4b218d32` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `fca66abe1a0cfaef9e95d8c5792a48e03d4d98c6bc2676134ec4d81fcee55afc` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `50f8684a38a10a6c87dea9d1c3eb4cb189a13517a2231e2af1d4019aa08821bb` |
| `docs/INDEX.md` | `3976251caa9cd37ed3b2e53e3b1b7fa42d26a744e27a82a49cd71d80e3a443e7` |
| `.github/workflows/ci.yml` | `46d04e26233013a37cf5abe960d0854bfe265280fea1213b3e3556a0c0212b79` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |

Five rows are new: the module the split created, the extracted probe script, the
deviation record that authorizes the check, the ADR index that lists it, and the
two workflow files this audit changed. `worker_bundle.rs`, `lib.rs`, and
`WORKER-ENVIRONMENT.md` move for the split and the references that follow it.
**No `worker/` file and no `schemas/` file moved**, which is why the bundle hash
below is unchanged.

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-29, cargo 1.97.1, CPython 3.12.3, and
FFmpeg 6.1.1 on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, **283**
  tests. V8 recorded 284; the difference is
  `t4_e1_the_runtime_probe_script_compiles_as_python`, deleted rather than
  moved because `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter`
  runs the same script on a real interpreter and a real `.dist-info` and so
  strictly subsumes it, and because `.github/workflows/ci.yml` now compiles the
  file directly. No other test was removed, disabled, or weakened.
- The run includes every test v8 named, at the tier prefixes this audit
  corrected: `t4_e1_a_lockfile_fault_no_line_carries_names_no_line` over the
  three whole-file lock faults,
  `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` over valid
  wheel scripts, bounded first-fault reporting, malformed digests, modified and
  missing files, absent `RECORD`, control-bearing and absolute paths, and
  site-package symlink escape,
  `t4_e1_runtime_probe_diagnostics_cannot_emit_terminal_controls`,
  `t4_e1_an_environment_that_is_not_the_locked_one_is_refused`, and
  `t1_e1_startup_module_names_display_as_they_serialize`, which stays at T1
  because it spawns nothing.
- The same locked workspace run passed all ten E1-S1 acceptance tests named in
  `DELIVERY-PLAN.md`, none of which this audit renamed.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 42
  tests, unchanged by this audit.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21 tests.
- `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` — pass, 11
  tests.
- `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `taplo fmt --check`,
  `python3 scripts/check-rust-conventions.py`, Python 3.12 `compileall` over
  `worker/study_tts_worker` **and `crates/study-tts-runtime/src/runtime_probe.py`**,
  and `git diff --check main` — pass.
- `python3 scripts/check-evidence-provenance.py` — pass, no unaccounted
  mismatch.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`, with
  the existing allowed duplicate-`cpufeatures` warning.
- `cargo doc --offline --workspace --no-deps --locked` — pass, no warnings.
- Schema regeneration proved idempotent by digest rather than by `git diff`,
  which cannot distinguish drift from an uncommitted change on a dirty tree:
  `schemas/worker-protocol-v1.schema.json` hashed
  `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` before and
  after `cargo run --example generate-schemas`.
- The probe extraction was verified mechanically: the 139 executable lines of
  `runtime_probe.py`, with its docstring and `#` comments removed, compare equal
  to the 139 unescaped Rust string literals they replace.
- The tier-duration report was rehearsed against the built test binaries. T1
  0.531 s, T2 0.029 s, T3 0.060 s, T4 3.504 s, against §3.2 budgets of 30 s,
  2 m, 30 s, and 5 m. Every tier is inside its budget, and the distribution is
  the reason the rename mattered: T4 now carries the subprocess work that was
  billing against T1's 30 seconds.

## Worker-bundle hash

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`
returned the same value on every run against the restored locked environment:

```text
6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02
```

**This is v8's value, unchanged, and that is the result rather than an
incidental fact.** The seventeenth audit moved Rust modules, a Python file
inside `crates/`, two workflow files, and documentation. None is listed in
`worker/bundle-manifest.json`. A refactor of the code that derives an identity
must not move the identity, and this is the check that says it did not.
Input paths and the derivation are unchanged, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`.

No cache entry is invalidated, re-keyed, or deleted by this audit.

### Measured cost of the environment precondition

Five consecutive runs on the developer environment, timed end to end:

| Measure | Value |
|---|---|
| Wall time, whole `verified_hash` | 3.43, 3.47, 3.52, 3.60, 3.62 s |
| Of which `RECORD` digest verification | 1.50 s |
| Bytes read for those digests | 1,263 MiB across 31,704 files |

The cost is paid once per build, not once per segment: the bundle identity is
one input to every cache key, derived once and reused. At that scale it is
affordable as written, and no memoization is proposed.
`.github/workflows/qualification.yml` now times the step on every qualification
run, which is what would make a drift from these numbers visible.

V8 recorded 1.58 GB across 43,828 files for the fourteenth audit's run. The
difference is the environment, not the check: that run inspected a virtualenv
carrying more tolerated extras. Both are developer-machine figures.

## Deviations and limitations

- **V8's status and its review table disagree, and this record does not repair
  it.** V8 carries `- Status: Accepted` while its §Review table records four
  Pending decisions, which `evidence/README.md` says "is not accepted". V8 is
  immutable either way, and this record's acceptance retires it from provenance
  checking, so the disagreement is preserved rather than resolved — which is
  what supersession is for. Nothing downstream reads it.
- **The reviews below were recorded by one person holding every role.**
  `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
  project and requires each approval to name its role and accepted risk
  separately, which is why the five rows differ rather than repeating one
  decision. They are not five independent readings of this change.
- The fifteenth- and sixteenth-audit reviews that v8 left open are **not**
  granted here. This record's §Review covers the seventeenth audit; the earlier
  initialization corrections carry forward as v8 recorded them.
- This was a developer-machine run, not the protected reference-machine
  qualification workflow, which was not dispatched. The reference machine must
  reproduce the bundle hash before G1.
- **The tier-duration step and the qualification timing were not run in CI.**
  The tier loop was rehearsed locally against the same prebuilt binaries and its
  numbers are recorded above, but its `$GITHUB_STEP_SUMMARY` output is only
  observable in a real run, and `qualification.yml` needs the self-hosted
  runner.
- Three ADRs — `ADR-0003`, `ADR-0004`, `ADR-0005` — carry a trailing-blank-line
  change in the working tree that this audit did not make and did not revert.
  None is pinned by an unsuperseded evidence record, so none affects provenance.
- Markdown Prettier was not available in this workspace. Markdown structure and
  relative links were reviewed, and `git diff --check main` passed.
- Real-model qualification, ASR, listening, and reference-machine measurements
  were not run. This change reaches no speech backend and changes no audio
  bytes.
- Everything v8 recorded under its own §Deviations and limitations that this
  audit did not touch remains true, including the absence of a durable check
  tying a routing-row name to `docs/governance/ROUTING-TABLES.md`.

## Review

Ross Todd holds every role below.
`docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
project and requires each approval to name its role and accepted risk
separately, which is why each row records a different acceptance.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept that the seventeenth audit changes no contract: no wire shape, schema, error variant, or refusal message moved | 2026-08-29 |
| Engineering owner | Ross Todd | Accept the module split, the probe extraction, and the tier corrections, on the unchanged bundle hash and the 283-test suite as the evidence they moved nothing | 2026-08-29 |
| Project owner | Ross Todd | Accept this record, and approve `ADR-0001-D004` separately in that record | 2026-08-29 |
| Worker/runtime owner | Ross Todd for T-WORKER | Accept the unchanged bundle hash as reproduced on the developer environment, and the risk of accepting before the reference machine reproduces it, which §Deviations keeps open as a G1 prerequisite | 2026-08-29 |
| Affected-track reviewers | Ross Todd for T-RUNTIME | Accept the `worker_bundle`/`worker_environment` boundary and its two crate-private entry points | 2026-08-29 |
