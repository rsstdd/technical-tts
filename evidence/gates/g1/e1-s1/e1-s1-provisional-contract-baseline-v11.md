# E1-S1 Provisional Contract Baseline Evidence v11

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v10`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v10`, SHA-256
`1ea38e35a31ce64ad9b0d8ee705acec70b72fb8d3caadb9932d2bb1f1cf6fcf3`, for its
controlled-record table and verification run. V10 remains the immutable record
of the bytes it read, and everything it concluded stands.

This record closes two remediation scopes. The eighteenth E1-S1 audit revised
four controlled records and added two; the nineteenth approves audits 15–16,
approves `ADR-0001-D005`, restores CI's authoritative 60-second T4 deadline,
and bounds the fake-worker contract driver. `evidence/README.md` §Provenance is
the obligation being discharged: the changed records are re-pinned here rather
than any accepted predecessor being edited.

## What the eighteenth audit changed

The audit is recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`
§What the eighteenth audit closed. It closed no defect in the build. It answered
a review finding that two governance statements stood wider than what supported
them:

1. The fifteenth audit's decision to retain `e1.worker.1.0` across a
   required-field change was argued inline in
   `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`, a document that
   describes itself as mirroring
   `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`. A mirror cannot
   grant an exception to the document it mirrors. The decision now lives in
   `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md`,
   which audit 18 introduced as Proposed. Audit 19 approves that record under
   the role-specific decisions below.
2. `PROVISIONAL-CONTRACT-BASELINE.md` §Amendment rules claimed the change
   classes were "enforced by" `assess_successor` plus
   `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`.
   That test reads `fixtures/contracts/e0-s4-contract-*.json` and nothing else,
   so it could not have observed the fifteenth audit's own change. The claim was
   narrowed to what each mechanism reaches, and
   `t3_e1_published_schema_required_fields_match_the_recorded_surface` was added
   to cover the gap it named.

## What the nineteenth audit changed

The audit is recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`
§What the nineteenth audit closed.

- T-CORE accepts the typed initialization identities and refusal of incomplete
  legacy success frames within the unreleased `e1.worker.1.0` baseline and
  `1.1` extension. Engineering accepts schema, parser, worker, fake, fixture,
  and bounded-harness parity.
- T-WORKER accepts the fail-closed product worker and the developer-machine
  bundle hash. The reference machine must reproduce that hash before G1.
- T-RUNTIME accepts that old plan and cache entries remain valid only under
  their producing identities; this change does not reuse, delete, or re-key
  them. T-AUDIO accepts that no audio behavior or bytes changed, so no listening
  evidence is required.
- `.github/workflows/ci.yml` again uses the 60-second T4 execution deadline
  fixed by `DELIVERY-PLAN.md`. Neither the plan nor
  `docs/architecture/WALKING-SKELETON.md` changed.
- `crates/study-tts-testkit/tests/worker_contract.rs` now supervises its direct
  fake child with a test-local two-second deadline, short bounded polling,
  timeout kill and reap, and best-effort `Drop` cleanup. No runtime process API
  or dependency was added.

## Acceptance criterion

This record is accepted when all of the following hold:

1. Audit 18's required-field surface is explicit, fails when the recorded and
   generated surfaces disagree, and all governed records it moved are pinned.
2. Audits 15–16 have separate T-CORE, engineering, T-WORKER, T-RUNTIME, and
   T-AUDIO decisions, and `ADR-0001-D005` is approved under its bounded G1
   expiry.
3. CI's ordinary T4 command uses exactly the Delivery Plan's 60-second
   deadline, without changing that plan or the walking-skeleton contract.
4. The fake-worker contract driver returns `ErrorKind::TimedOut` after two
   seconds, kills and reaps the direct hung child, and cleans up on write failure
   or panic; the regression completes within the T4 budget.
5. The targeted contract suite, full offline workspace suite, formatting,
   Clippy, Rust conventions, schemas, docs, Python worker tests, script tests,
   dependency policy, Markdown links, diff hygiene, and evidence provenance
   checks pass.
6. Two independent worker-bundle hash computations agree with v10, and the
   record states that hosted CI and reference-machine qualification were not
   run locally.
7. No public Rust API, wire field, schema version, dependency, product-worker
   behavior, cache identity, or audio byte changed in audit 19.

## Verification run

Run on the branch working tree at the time of writing, on WSL2 / Ubuntu 24.04.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Compile-time red regression | `cargo test --offline --locked -p study-tts-testkit --test worker_contract --no-run`, after adding the regression and before the guard | Failed only because `FakeWorkerChild` and `FAKE_SESSION_DEADLINE` did not exist; the old unbounded `hang` path was not executed |
| Targeted worker contract | `cargo test --offline --locked -p study-tts-testkit --test worker_contract` | 4 passed in 2.01 seconds, including bounded timeout and `/proc` reap confirmation |
| Tests | `/usr/bin/time -f 'elapsed=%e seconds' cargo test --offline --workspace --all-targets --locked` | 285 passed, 0 failed, 0 ignored; 7.04 seconds wall time |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Doctests | `cargo test --offline --workspace --doc --locked` | Passed |
| Documentation | `cargo doc --offline --workspace --no-deps --locked` | Passed without warnings |
| Published schemas current | `cargo run --offline --locked -p study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | No diff |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 42 passed |
| Repository scripts | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | 11 passed |
| Qualification scripts | `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'` | 21 passed |
| Dependency policy | `cargo deny check` | Advisories, bans, licenses, and sources passed; the existing `cpufeatures` duplicate-version warning remains |
| Markdown links | Repository-wide relative-link scan over `*.md` | Clean |
| Diff hygiene | `git diff --check` | Clean |
| Worker bundle identity, twice | Two consecutive `cargo run --offline --locked -p study-tts-runtime --example worker-bundle-hash` runs | Both returned `6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`, unchanged |
| Provenance | `python3 scripts/check-evidence-provenance.py` | Clean after the final controlled-record hashes below were pinned |

Audit 18's test was also verified to fail, not merely to pass: removing the
fifteenth audit's `voice_profile_hashes` requirement from the recorded surface
produced
`/$defs/WorkerInitializationIdentities: recorded [...], published [...]`
naming the document, version, pointer, and field. Restoring the row made the
targeted test green again.

Hosted CI and the protected reference-machine qualification workflow were not
run locally. Real-model qualification, ASR, and listening were also not run;
audit 19 changes no audio behavior or bytes.

## Controlled records

Every row v10 pinned is checked again here, with none dropped. Seven moved; the
rest are re-pinned at digests reproduced from the current bytes.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae916a5cc7ea4d94aacc288b6d2ac96b252a04e8adb2944cf2c95d` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `5e4c86b8f8804bc178a75e53e4f4208383baa2c68254797219d3a8b73cd702c2` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `cf458da1d5330ca0c7258005aaa354d55e410798dec0617856a0f2257633ef15` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `e20732d22714ad6597e86f7ce3fc9b52a99de8d4776d264f97d08933851f08a7` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `2609ae05b35d5ecc67b43b8f11dc04b0f80bfcdf70e23b54f2c72c731c4bef5e` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `28e4b2128ee1632f735cb9b5dc66c46ab5734936b338ed034e04bcee01b24816` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `d6785d52d6714f53247d4b036d6cc4f021cf50a4b0e7a8546d786488e0d27bf0` |
| `crates/study-tts-testkit/src/json_schema.rs` | `97ba85b3fb0a057a088d634faab5d8a0cdf5c717bf9029f564f54d008572dbed` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `8e29fe95300ba85b3bdb50da0e0a405718c3b22f4eabf431e55bab80fe347bb3` |
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
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `05aabe1d5f72208a77284fcdb7c3ff4e27d8cd55a8364df420232c262228ae2d` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `9ee3fd43ac856b2a48154d1a7c18736cb3c147d72677eaaf8332aeca4b218d32` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `fca66abe1a0cfaef9e95d8c5792a48e03d4d98c6bc2676134ec4d81fcee55afc` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `50f8684a38a10a6c87dea9d1c3eb4cb189a13517a2231e2af1d4019aa08821bb` |
| `docs/INDEX.md` | `ff014a14da41a38824f8d5becc8c6d17492be469d7cbf76e867d2ee4d9ab6f8e` |
| `.github/workflows/ci.yml` | `ff80cf2ec76731ab805c5ee6d5dad13c61b423359aac5f156508077be757cda3` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `a561d78d628eba447d7013589f141a58fbc31118f0142955c710e78c90bcf8cf` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4453d8a0b64324366902e0a87e9f1a756587fa22673fff8a57571` |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317a67d487ef9203d18f99e928659c49165740a9da64dbd11dce68d` |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | `84ed5903193a95a4e8056cb6a7ae07f4ea17ca729f2f67846ec6bd26fe081957` |

The rows audits 18–19 changed, and the two audit 18 added:

| Record | What changed |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | Audit 18 narrowed §Amendment rules and named `ADR-0001-D005`; audit 19 points the stale Proposed evidence statement to accepted v11 |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | Audit index rows 18–19 and both audit sections appended; audit 19 records the role-specific decisions without rewriting earlier pending statements |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | The mechanization list went from two entries to three, each stating what it reaches, and names the new test in return |
| `crates/study-tts-testkit/tests/schemas.rs` | `PUBLISHED_REQUIRED_SURFACE`, `required_surface`, and `t3_e1_published_schema_required_fields_match_the_recorded_surface` added. This file is the home its module documentation already claimed: "the rule in `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes is enforced on real documents" |
| `crates/study-tts-testkit/tests/worker_contract.rs` | Test-local bounded child guard and `t4_e1_fake_worker_contract_deadline_kills_and_reaps_a_hung_worker` added |
| `.github/workflows/ci.yml` | T4 execution deadline restored from 120 to 60 seconds |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | Added by audit 18 and approved by audit 19 with distinct engineering- and project-owner decisions |
| `docs/governance/TRACEABILITY-MATRIX.md` | Approved `ADR-0001-D005` mapped to E1-S1, its schema-surface test, v11 evidence, and G1 expiry |
| `docs/INDEX.md` | Current E1-S1 evidence routed to v11 and `ADR-0001-D005` listed as approved |

`crates/study-tts-testkit/tests/schemas.rs` and
`docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md`
are pinned here for the first time; v10 pinned neither, so neither has a prior
digest to have moved from. Seven of v10's own rows moved:
`PROVISIONAL-CONTRACT-BASELINE.md`, `E1-S1-INTERFACE-CHANGE-001.md`, and
`INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, plus `worker_contract.rs`,
`.github/workflows/ci.yml`, `TRACEABILITY-MATRIX.md`, and `docs/INDEX.md`.
Every other row v10 pinned is carried at an unchanged digest.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no
others. Each is a record pinning
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` at the digest it
carried before this audit revised the mechanization list.

`e1-s1-evidence-provenance-reconciliation-v1` §Open findings already records
that the unapproved `-v5`/`-v6`/`-v7` chain is accounted for rather than
retired, and that the rows should be dropped once that chain is approved or
superseded. This audit adds no new class of drift; it extends that same finding
to one more document.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-evidence-provenance-reconciliation-v1` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |

## Deviations and limitations

- `ADR-0001-D005` expires at G1. It authorizes only the already-recorded pre-G1
  correction under all five conditions; it creates no post-freeze precedent.
- `t3_e1_published_schema_required_fields_match_the_recorded_surface` does not
  choose a version. An author who edits a schema and `PUBLISHED_REQUIRED_SURFACE`
  in one commit still passes. It converts a change that could land silently into
  one that is explicit and reviewable where it is made, and this record claims
  nothing further for it.
- No wire contract, published schema, error variant, refusal message,
  worker-bundle input, product runtime behavior, or audio byte moved in audit
  19. Hosted CI and protected reference-machine qualification remain unrun
  local follow-up evidence at their existing gates.

## Review

Ross Todd holds every role below.
`docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
project and requires each approval to name its role and accepted risk
separately, which is why each row records a different acceptance.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept the required typed initialization identities and refusal of incomplete legacy success frames within the unreleased `e1.worker.1.0` baseline and `1.1` extension, including approved `ADR-0001-D005` | 2026-08-29 |
| Engineering owner | Ross Todd | Accept schema, parser, product-worker, fake, fixture, and bounded-harness parity; the required-field surface control; and restored 60-second CI deadline on the 285-test suite and both red checks | 2026-08-29 |
| Project owner | Ross Todd | Accept this current record, its three accounted provenance rows, audits 15–19, and the bounded G1-expiring `ADR-0001-D005` deviation | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the fail-closed product worker and developer-machine bundle hash `6b0a3c14…8aa8d02`; require reference-machine reproduction before G1 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that old plan and cache entries remain valid only under their producing identities and are not reused, deleted, or re-keyed by this change | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-29 |
