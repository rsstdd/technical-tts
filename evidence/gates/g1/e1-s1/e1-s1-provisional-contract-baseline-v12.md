# E1-S1 Provisional Contract Baseline Evidence v12

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v11`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v11`, SHA-256
`f1c2300d5934a75502939ecb4beb41f3dfc089b806861dc6be0efaf1354253ef`, for its
controlled-record table and verification run. V11 remains the immutable record
of the bytes it read, and everything it concluded stands.

This record closes one remediation scope: the twentieth E1-S1 audit, which
answers three review findings against the worker-environment precondition
`ADR-0001-D004` authorizes. `evidence/README.md` §Provenance is the obligation
being discharged — the nine changed records are re-pinned here rather than any
accepted predecessor being edited.

## What the twentieth audit changed

The audit is recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`
§What the twentieth audit closed. Each finding was a control that read as
enforced while a specific action passed it.

1. **The probe ran under `python -I`, which is not isolation from the thing it
   observes.** `site.main` still executes every `.pth` file and imports
   `sitecustomize` before the script runs, so the startup code the probe exists
   to report got to edit the report. Reproduced against a real interpreter: one
   `.pth` line replacing `json.dumps` made the probe answer `integrity_faults:
   []` and `path_hooks: []` for an environment holding a modified module and an
   unowned hook. The probe is bootstrapped with `-I -S`, makes every observation
   with the standard library, and imports `packaging` last from a site directory
   it has checked resolves inside a prefix `site` itself would search. `-S` also
   skips `site.venv`, so the script repeats that function's prefix half — and
   nothing else of `site.main`.
2. **Installed files were checked against their adjacent, mutable `RECORD`.**
   Editing a module and the `RECORD` line pinning it is one action, after which
   the distribution agrees with itself. `worker/bundle-manifest.json` moves to
   layout `1.2`, adding a required `record_digests` that declares, per locked
   distribution, a digest over the `RECORD` claims the check rests on. The
   manifest is a declared bundle input, so what the lock may have installed is
   now part of what the identity describes.
3. **`ln -sfn "${QUALIFIED_WORKER_VENV}" worker/.venv` does not replace an
   existing real directory.** It creates a link inside it, exits `0`, and the
   version check on the next line runs the stale interpreter already there.
   Reproduced in a scratch tree. The documented step now guards on a non-symlink
   destination and attaches with `ln -sfnT`, which
   `.github/workflows/qualification.yml` already used.

## Acceptance criterion

This record is accepted when all of the following hold:

1. The probe's report is unchanged by hostile interpreter startup code, proven
   against a real interpreter carrying exactly that code, and the proof fails
   when `-S` is removed.
2. A module edited together with its own `RECORD` line is refused, proven
   against the reference machine's restored environment and not only against a
   fake probe answer, and the observation that refusal rests on is pinned by a
   test driving a real interpreter.
3. The interpreter attach step refuses a non-symlink `worker/.venv` instead of
   silently linking inside it.
4. The `-S` bootstrap reports the same runtime identity, path hooks, startup
   modules, and absence of integrity faults for the reference environment as
   `-I` did, so the flag closes a hole without narrowing the observation.
5. Formatting, Clippy, Rust conventions, the full offline workspace suite,
   doctests, documentation, published schemas, Python worker tests, script
   tests, dependency policy, Markdown links, diff hygiene, and evidence
   provenance all pass.
6. Two independent worker-bundle hash computations agree, and the record states
   which reproductions were not run locally.
7. No public Rust API signature, wire field, published schema, worker protocol
   version, dependency, product-worker behavior, or audio byte changed.

## Verification run

Run on the branch working tree at the time of writing, on the ADR-0002
reference machine: WSL2, Ubuntu 24.04, CPython 3.12.3, with `worker/.venv`
attached to the qualified environment.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Red regression, startup code | `cargo test --offline --locked -p study-tts-runtime t4_e1_interpreter_startup_code`, with `-S` removed from the probe invocation | Failed: `integrity_faults` came back `[]` where the modified module was expected. Restoring `-S` made it pass |
| Red regression, `RECORD` authentication | Edited `audioread/exceptions.py` and its `RECORD` line together in the restored environment, then ran `worker-bundle-hash` | The probe reported `integrity_faults: []`, and `verified_hash` refused with `ModifiedDistributionRecord { distribution: "audioread" }`. Both files restored to their original digests afterwards, and the identity returned to `f9711a21…` |
| Red regression, digest derivation | The probe made to hash a constant instead of the `RECORD` rows | `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` failed with `a rewritten RECORD row must move the digest the manifest pins`. Restoring the derivation made it pass |
| Red regression, declaration comparison | One character of the declared `torch` digest changed in the manifest, keeping the spelling canonical | Refused with `ModifiedDistributionRecord { distribution: "torch" }`; restoring the manifest restored the hash |
| Reproduced defect, `ln -sfn` | Scratch tree with a real `worker/.venv` directory holding a stale interpreter | `ln -sfn` exited `0`, created `worker/.venv/qualified`, and `worker/.venv/bin/python` printed the stale interpreter's output. `ln -sfnT` refused with `cannot overwrite directory` |
| Tests | `/usr/bin/time -f 'elapsed=%e seconds' cargo test --offline --workspace --all-targets --locked` | 287 passed, 0 failed, 0 ignored; 6.13 seconds wall time |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Doctests | `cargo test --offline --workspace --doc --locked` | Passed |
| Documentation | `cargo doc --offline --workspace --no-deps --locked` | Passed without warnings |
| Published schemas current | `cargo run --offline --locked -p study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | No diff |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Probe compiles | `python3 -m compileall -q -f crates/study-tts-runtime/src/runtime_probe.py` | Clean |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 42 passed |
| Repository scripts | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | 11 passed |
| Qualification scripts | `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'` | 21 passed |
| Dependency policy | `cargo deny check` | Advisories, bans, licenses, and sources passed |
| Markdown links | Repository-wide relative-link scan over `*.md` | Clean |
| Diff hygiene | `git diff --check` | Clean |
| Worker bundle identity, five runs | `cargo run --offline --locked -p study-tts-runtime --example worker-bundle-hash` | All five returned `f9711a21f3e046d53c7c617e9308893c9c0240badec0d3656487fe2796c6dc2a` in 3.43–3.52 s, inside the 3.43–3.62 s `ADR-0001-D004` records |
| Provenance | `python3 scripts/check-evidence-provenance.py` | Clean after the controlled-record hashes below were pinned |

**The `-S` bootstrap was checked against the reference environment for
narrowing, not only for correctness.** It reports the same runtime identity
`cpython 3.12.3 (cp312, manylinux_2_39_x86_64)`, the same single path hook
`distutils-precedence.pth` owned by `setuptools`, the same `sitecustomize`
resolving to digest `Q9gRJdkjdrGmnVOnESagQcyaGNgIDpLeoKKuI74Tix4`, sixty-four
installed distributions, and no integrity fault.

Hosted CI and the protected reference-machine qualification workflow were not
run. Real-model qualification, ASR, and listening were not run; this audit
changes no audio behavior or bytes.

## The identity that moved

`worker/bundle-manifest.json` is a declared bundle input and its bytes changed,
so the worker-bundle identity moves from
`6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02` to
`f9711a21f3e046d53c7c617e9308893c9c0240badec0d3656487fe2796c6dc2a`. That is the
intended shape of the control rather than a side effect: what the lock is
allowed to have installed is now part of what the identity describes, and
changing it moves every cache key where a reviewer sees it.
`WORKER_BUNDLE_IDENTITY_VERSION` stays at `e1-s1-v4`, because the derivation is
unchanged and ADR-0001 §12.5's input list gains nothing.

The fifty-six declarations were generated on this machine from its own restored
environment by the command in `docs/operations/WORKER-ENVIRONMENT.md` §Declaring
what the lock installed. Old plan and cache entries remain valid only under the
identities that produced them; this change does not reuse, delete, or re-key
them.

## Controlled records

Every row v11 pinned is checked again here, with none dropped. Nine moved; the
rest are re-pinned at digests reproduced from the current bytes.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae916a5cc7ea4d94aacc288b6d2ac96b252a04e8adb2944cf2c95d` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `dc33eb951cd99316b62e6cc557f32da94ff3926e41a170c3fd90eb7c1489d8ab` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `cf458da1d5330ca0c7258005aaa354d55e410798dec0617856a0f2257633ef15` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `a2e9294c80f4f3f0a899c7d6c4af4aa103cafa879f6e00db789dee9bab0d02a5` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `941ff4df362376f6a48b04f5c30e6779247055a4e7bab7d38278a6d1cdb6e83e` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `7b49ab56a27056d56a5bd49df0f5c3ce07cc14ea54f6a7981f18f6ac9b2640c1` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `d6785d52d6714f53247d4b036d6cc4f021cf50a4b0e7a8546d786488e0d27bf0` |
| `crates/study-tts-testkit/src/json_schema.rs` | `97ba85b3fb0a057a088d634faab5d8a0cdf5c717bf9029f564f54d008572dbed` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `8e29fe95300ba85b3bdb50da0e0a405718c3b22f4eabf431e55bab80fe347bb3` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e889d30b83cbc4e62c8b779c6fed4082c357facab6bcfa65141044` |
| `schemas/worker-protocol-v1.schema.json` | `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` |
| `worker/AGENTS.md` | `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` |
| `worker/bundle-manifest.json` | `cba489f1d3922c64997fb65c0930a0d8bbb2d542903c435e88b24c0d65fcb2a9` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `da7baa5c48d6038c3537e6414614de9beedcdf2098abd74d5a70d105814b4c98` |
| `worker/study_tts_worker/worker.py` | `0777f9b16a41e1c2db00c445229c04b48328bae7fafc6001174846aca0fc8bbf` |
| `worker/tests/test_protocol.py` | `405e9c41787b6784374146b695e166ff2b9de5828ba259826e7078f99149a6fd` |
| `worker/tests/test_worker.py` | `682f2d24c7db45bc0bac90aa4d37de72238f456203b8f2b1a06c3fa6b5aa7113` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef3bc1c1338a62e1126300cc0bb97a89d0a89c4d6dcfb7c88025d9` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `f8ff5796dde3712c2f270ffadcc6e151320e4a7cb128a8c7d562367716f01556` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `9ee3fd43ac856b2a48154d1a7c18736cb3c147d72677eaaf8332aeca4b218d32` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `4aa9246845dda733361746a0ab563402d0036b9cecf1837db0b2fbf763785cc2` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `b85b819c29dd3fab9d45c3c5704007df9aa8af3a4930410c1da50d776909a6de` |
| `docs/INDEX.md` | `facfb7886c9f25703aa72798cf0366b494e901b2021afc94e4a42e99e7d2095b` |
| `.github/workflows/ci.yml` | `ff80cf2ec76731ab805c5ee6d5dad13c61b423359aac5f156508077be757cda3` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `a561d78d628eba447d7013589f141a58fbc31118f0142955c710e78c90bcf8cf` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4453d8a0b64324366902e0a87e9f1a756587fa22673fff8a57571` |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317a67d487ef9203d18f99e928659c49165740a9da64dbd11dce68d` |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | `84ed5903193a95a4e8056cb6a7ae07f4ea17ca729f2f67846ec6bd26fe081957` |

The rows audit 20 changed:

| Record | What changed |
|---|---|
| `crates/study-tts-runtime/src/runtime_probe.py` | Bootstrapped under `-S`: the prefix half of `site.venv` repeated, site directories validated against the prefixes `site` searches, distributions and startup modules resolved over an explicit path, PEP 503 canonicalization spelled out, per-distribution `record_digest` reported, and `packaging` imported last |
| `crates/study-tts-runtime/src/worker_environment.rs` | `-S` added to the probe invocation, `record_digest` added to the reported distribution, `check_records_match_their_declarations` added, two T4 tests added — one driving a real interpreter carrying a hostile `.pth` — the real-interpreter `RECORD` test extended to pin the digest derivation, and the exhaustive mismatch table lifted into one shared `fault_name` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | Manifest layout `1.2` with a required `record_digests`, `DeclaredDistributionRecord`, the `1.1` decoder, and the layout-ladder test extended one step |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `EnvironmentMismatch::UndeclaredDistributionRecord` and `::ModifiedDistributionRecord` added; no existing variant or message changed |
| `worker/bundle-manifest.json` | `schema_version` `1.1`→`1.2` and fifty-six machine-generated `record_digests` |
| `docs/operations/WORKER-ENVIRONMENT.md` | §Declaring what the lock installed added with its regeneration command; the `-S` rationale, the corrected attach step, the manifest field table, and the canonicalization and artifact-hash cross-references updated |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | The authorized comparison names the manifest declaration, two alternatives added, the test count and the moved identity corrected |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | Audit index row 20 and §What the twentieth audit closed appended |
| `docs/INDEX.md` | Current E1-S1 evidence routed to v12 |

Every other row v11 pinned is carried at an unchanged digest. No row is added
and none is dropped.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no
others. Each is a record pinning
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` at the digest it
carried before the eighteenth audit revised the mechanization list, and is
carried forward from v11 unchanged.

`e1-s1-evidence-provenance-reconciliation-v1` §Open findings already records
that the unapproved `-v5`/`-v6`/`-v7` chain is accounted for rather than
retired, and that the rows should be dropped once that chain is approved or
superseded. This audit adds no new class of drift.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-evidence-provenance-reconciliation-v1` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |

## Deviations and limitations

- **The declaration is not authentication against the locked artifact.** Nothing
  this build can ask the interpreter reports the artifact a distribution came
  from, so `record_digests` is an independently generated manifest rather than a
  re-derivation from the wheel. It binds the installed tree to a Git-tracked,
  hash-carrying statement made outside the environment; it does not prove that
  statement was itself derived from the locked wheel bytes. Closing that would
  require retaining the verified wheelhouse, which
  [Acquire the hashed artifacts, minus the governed distribution](../../../../docs/operations/WORKER-ENVIRONMENT.md#acquire-the-hashed-artifacts-minus-the-governed-distribution)
  builds and discards.
- **The digest excludes `.dist-info` rows.** `INSTALLER`, `REQUESTED`, and
  `direct_url.json` move with the command that installed rather than with
  anything the worker imports, and pinning them would make a correct restore
  read as tampering — which trains an operator to regenerate on every mismatch.
  Those files are still verified per-file against `RECORD`; they are only
  outside the claim the manifest pins. A `METADATA` edited to keep the version
  the lock requires is therefore not detected by this check, and is not detected
  by the version comparison either, because it would agree with it.
- **An author who regenerates the declarations and the manifest in one commit
  still passes**, exactly as `t3_e1_published_schema_required_fields_match_the_recorded_surface`
  does not choose a schema version. What this converts is a change that could
  land silently into one that is explicit, hash-moving, and reviewable where it
  is made. This record claims nothing further for it.
- **`-S` narrows the search path by design.** Distributions reachable only
  through a path a `.pth` file adds are no longer enumerated. That is the
  intended direction — such a hook is already refused by name — but it is a
  behavior change, not only a hardening, and it is recorded here as one.
- Hosted CI and protected reference-machine qualification remain unrun local
  follow-up evidence at their existing gates. `ADR-0001-D005` and its G1 expiry
  are unchanged by this audit.

## Review

Ross Todd holds every role below.
`docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
project and requires each approval to name its role and accepted risk
separately, which is why each row records a different acceptance.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept manifest layout `1.2` as a pre-freeze extension of a project-owned format, with `1.0` and `1.1` still readable and refusing rather than exempting | 2026-08-29 |
| Engineering owner | Ross Todd | Accept the `-S` bootstrap and the `site.venv` prefix repetition it requires, the two added `EnvironmentMismatch` variants, the two added T4 tests, and the 287-test suite with all three red checks reproduced | 2026-08-29 |
| Project owner | Ross Todd | Accept this current record, its three carried-forward accounted provenance rows, and the limitations above — in particular that the declaration is not authentication against the locked artifact | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the worker-bundle identity moving to `f9711a21…6c6dc2a`, reproduced five times on the reference machine, with hosted-CI and protected qualification reproduction still owed before G1 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that old plan and cache entries remain valid only under their producing identities and are not reused, deleted, or re-keyed by this change | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-29 |
