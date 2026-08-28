# E1-S1 Provisional Contract Baseline Evidence v6

- Status: Proposed
- Supersedes: `e1-s1-provisional-contract-baseline-v5`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v5`, SHA-256
`eff542fda465a9e7074df440c23faf9ad3066c827fc05b266b35fcc897802d6e`, for its
controlled-record table, verification run, and worker-bundle hash. V5 remains
the immutable record of the bytes and identity it measured.

The seventh audit recorded in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
closed the version-only Python lock: package sources and artifact kinds are now
explicit, every index-supplied pin is bound to one SHA-256 artifact digest, and
the restore has been exercised from a hash-verified wheelhouse without index
access. The governed Chatterbox source remains bound to its recorded Git commit
and is force-reinstalled by the documented command.

Filed under `g1/` because E1-S1 feeds G1. This is story evidence, not G1
acceptance; the interface freeze remains deferred to that gate.

## Acceptance criterion

Accepted when all five hold:

1. Every row in v5's controlled-record table is checked again, with none
   silently dropped.
2. Every index-supplied worker distribution is bound to the selected reference-
   ABI artifact and the two package sources are explicit.
3. A fresh CPython 3.12.3 environment restores those artifacts offline, with no
   dependency resolution or build-isolation fetch.
4. The Rust identity boundary refuses an unhashed pin or implicit source and the
   complete Rust and Python suites pass.
5. Every check not run, or run with a limitation, is named under §Deviations
   and limitations.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `20ef0fcba49cfbfc21678acd10e06b66ac5b93999fcc8391b8a77e3cdecaca50` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `1eaca00abe695c2ea08e9642b9f2fb6a9dfb55f4679eb99b4f14a34b8748b7ae` |

Only the E1-S1 change record moved. The other five digests were recalculated
from current bytes and agree with v5.

Two implementation records outside that controlled table are pinned here
because they are the inputs this audit changed:

| Record | SHA-256 |
|---|---|
| `docs/operations/WORKER-ENVIRONMENT.md` | `2687c8f4a1fa52984e67dc572a38721a12308f1a9f28408d25fd9a7669a5036c` |
| `worker/requirements.lock` | `6596698ae92e805c608be07503d318b2a749255203b8c2a4aa177355b2579e3a` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `cf92deb45adc906e442829c494aa7af0c193eba0d7e9b979876df1cc35148c22` |

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-28, cargo 1.97.1, CPython 3.12.3, `pip`
24.0, and FFmpeg 6.1.1 on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 271 tests.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `cargo test --offline --locked -p study-tts-runtime worker_bundle` — pass,
  29 tests, including the two new artifact-lock refusal tests.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 34
  tests.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21 tests.
- `cargo check --workspace --all-targets --locked --offline` and
  `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo doc --offline --workspace --no-deps --locked`,
  Python 3.12 `compileall` over `worker/study_tts_worker`, and
  `git diff --check` — pass.
- Every relative Markdown link in the two changed documents and this evidence
  record resolves to a repository path — pass.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`, with
  the existing duplicate-`cpufeatures` warning.
- Every index artifact was acquired with `pip download --isolated --no-deps`
  from the two recorded indexes. A fresh environment then installed the
  resulting wheelhouse with `--no-index --require-hashes --no-deps
  --force-reinstall --no-build-isolation` — pass. `torch==2.6.0+cpu`,
  `torchaudio==2.6.0+cpu`, and the sdist-built `s3tokenizer==0.1.7` imported.

## Worker-bundle hash

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`
returned the same value twice against the restored locked environment:

```text
9ef560e8f884f50dc23bd0bc88d41aff88ff58d8077fbe283adb0f297361108e
```

It supersedes v5's
`8f11f9edc75096688b0d2f17ceed5eec767c32cb42aefa6c4f346ff68df844c5`.
The lockfile bytes moved and the lock-validity definition now includes source,
artifact-kind, and artifact-hash requirements, so
`WORKER_BUNDLE_IDENTITY_VERSION` moved from `e1-s1-v2` to `e1-s1-v3`.

## Deviations and limitations

- This was a developer-machine run, not the protected reference-machine
  qualification workflow. The reference machine must reproduce the bundle
  hash before G1.
- The self-hosted qualification workflow was not dispatched. Its environment
  approval, runner-group restriction, and fixed interpreter link remain
  external configuration that a local run cannot prove.
- The disposable offline restore did not install the governed Chatterbox tree,
  whose model root is intentionally outside the repository. The already
  attached qualified environment carries that exact Git revision; the PEP 610
  provenance gate passed when the bundle hash was read.
- `pip check` in the disposable environment reports `pre-commit` absent because
  `s3tokenizer` publishes development tooling as an install dependency. That
  tooling is deliberately excluded from the worker-loaded set and synthesis
  identity; the runtime imports passed. The attached qualification environment
  includes it and passes `pip check`.
- Markdown Prettier was not available in this workspace (`pnpm exec prettier`
  reported no package). Markdown structure and relative links were reviewed,
  and `git diff --check` passed.
- Real-model qualification, ASR, listening, and reference-machine measurements
  were not run. This change reaches no speech backend and changes no audio
  bytes.
- T-CORE must ratify the worker-identity-version change. T-RUNTIME must review
  the cache-impact account; existing entries are retained and simply stop
  matching the new worker identity.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of §What the seventh audit closed | |
| Engineering owner | engineering owner | Pending review of the remediation and this record | |
| Worker/runtime owner | T-WORKER | Pending reference-machine reproduction of the bundle hash | |
| Affected-track reviewers | T-RUNTIME, T-AUDIO | Pending cache-impact review; audio behavior unchanged | |
