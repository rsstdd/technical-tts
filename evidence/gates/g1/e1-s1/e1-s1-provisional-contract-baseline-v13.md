# E1-S1 Provisional Contract Baseline Evidence v13

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v12`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v12`, SHA-256
`f75b03dbc23bdd597281c5a031e040172370b747195d95d7d8dfbb2ccf2f77f9`, for its controlled-record table and
verification run. V12 remains the immutable record of the bytes it read.

**Unlike v11 and v12, this record does not say that everything its predecessor
concluded stands**, because two things it concluded do not. Saying so is most of
why this record exists. It has three reasons:

1. to **correct v12's controlled-record table**, which pinned six records at
   digests that were already stale, and its §Verification run, which recorded a
   clean provenance check that does not pass;
2. to **narrow a scope statement v10 introduced** and v11 and v12 carried
   forward without reading it against the record that made it; and
3. to record the **twenty-first audit**, which closes two controls that read as
   enforced while a specific input or record walked past them.

### V12's controlled-record table pinned bytes that were already stale

V12 §Controlled records states that "every row v11 pinned is checked again here,
with none dropped" and that the unmoved rows "are re-pinned at digests
reproduced from the current bytes". Six were not:

| Record | V12 pinned | The bytes at v12's own commit |
|---|---|---|
| `crates/study-tts-runtime/src/worker_bundle.rs` | `a2e9294c80f4…` | `ebade17f66ca…` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `941ff4df3623…` | `9d94ed6cc323…` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `d6785d52d671…` | `22aa6c6a38fa…` |
| `crates/study-tts-testkit/src/json_schema.rs` | `97ba85b3fb0a…` | `a6c7f0760387…` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `8e29fe95300b…` | `c9cffad3e858…` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `4aa9246845dd…` | `3ae6fe5e2f05…` |

Each pinned digest is a real earlier state of that file — the bytes before one
of the four commits audit 20 landed — so the table was assembled from a tree
that had moved under it before the record was committed, not from invented
values. That is the ordinary way this fails, and it is why the check exists.

**The consequential half is the verification row, not the table.** V12
§Verification run records:

> | Provenance | `python3 scripts/check-evidence-provenance.py` | Clean after the controlled-record hashes below were pinned |

That row is false. Run against v12's own commit the command exits `1`. A stale
pin is a recoverable bookkeeping error; a verification row asserting a check
passed when it did not is the thing every other row in that table is trusted on,
and it is the same defect v10 was written to correct in v9 — a claim about a
diff that was never read in full.

All six are re-pinned below from current bytes, and this record's own provenance
row was written after the command exited `0`, not before.

### The seventeenth-audit statement has been overclaimed since v10

`e1-s1-provisional-contract-baseline-v10` §Scope and decision reads:

> V9 remains the immutable record of the bytes it read. Everything it concluded
> about the seventeenth audit stands.

The second sentence is too broad, and v10 itself is what shows it. Its section
§Commit `9b66fd4` repaired a control that was weaker than its own document
establishes that `check_startup_modules_are_accounted` accepted a startup module
owned by *any* installed distribution while
`docs/operations/WORKER-ENVIRONMENT.md` had required a **locked** owner since the
fourteenth audit, that the seventeenth audit reviewed that exact function and
did not find it, and that "a reviewer of this record should treat the
seventeenth audit's coverage of `worker_environment` as verified for structure
and unverified for document conformance". V10's own §Review carries that
narrower acceptance in its worker/runtime-owner row.

So one conclusion does not stand: **v9's finding that the seventeenth audit
"closed no defect" and that every item in it was "scope, legibility, or
governance debt rather than behavior"**. The audit left a live defect in place —
a control weaker than its ratified document — which `9b66fd4` repaired.

What does stand is everything else v9 concluded about that audit, and it is
worth naming so this correction is not read as wider than it is: no contract
moved; no wire shape, schema, error variant, or refusal message moved; the
worker-bundle hash did not move, which was the evidence that a refactor of the
code deriving an identity had not moved the identity; and the module split
changed no behavior.

V11 and v12 each then wrote that their predecessor's conclusions stood, without
distinguishing this one. Neither statement was checked against v10's body. The
overclaim is corrected here at its source rather than repeated a fourth time.

### What the twenty-first audit changed

The audit is recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`
§What the twenty-first audit closed.

1. **An unhashable frame method crashed the parser instead of being refused.**
   `read_request` evaluated `method not in _REQUEST_FRAMES` — a `dict` — before
   establishing that `method` is a string, so a frame carrying `"method": []` or
   `"method": {}` raised `TypeError: unhashable type` out of the NDJSON
   boundary where every other malformed frame is answered with a correlated
   `FrameError`. Closed by an `isinstance` guard ahead of the membership test.
2. **Any accepted record could excuse its own provenance mismatch.**
   `accounted_mismatches` read the `## Accounted provenance mismatches` section
   of every accepted record, while its own docstring and `evidence/README.md`
   §Provenance both restrict that to an accepted **reconciliation** record. V12
   carried three such rows and they were taking effect. Closed by restricting
   the scan to records carrying `reconciliation` as a hyphen-separated word in
   their record ID, with the convention now stated in `evidence/README.md` and
   named back from the script.

The `isinstance` guard added beside it for `protocol_version` closes no crash
and is not counted as one: `ACCEPTED_PROTOCOL_VERSIONS` is a `tuple`, whose `in`
compares by equality and never hashes, so that field was already refused
cleanly. It is symmetry, and §Verification run records the characterization that
establishes it.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. This record is accepted when
all of the following hold:

1. Every controlled record v12 pinned is checked again here from current bytes,
   with none dropped, and the six v12 pinned wrongly are named as wrong rather
   than quietly corrected.
2. `python3 scripts/check-evidence-provenance.py` exits `0`, and this record's
   provenance row is written after that run rather than before it.
3. The seventeenth-audit conclusion that does not stand is named exactly, and
   the conclusions that do stand are preserved rather than swept aside with it.
4. Each of the twenty-first audit's two closed defects is reproduced against the
   code that carried it, and any change that closes no defect is recorded as
   closing none.
5. No accepted predecessor is edited. Every correction is made by supersession.
6. Formatting, Clippy, Rust conventions, the full offline workspace suite,
   doctests, documentation, published schemas, Python worker tests, script
   tests, qualification script tests, dependency policy, Markdown links, diff
   hygiene, and evidence provenance all pass.
7. Five worker-bundle hash computations agree, and the record states which
   reproductions were not run locally.
8. No public Rust API signature, wire field, published schema, worker protocol
   version, dependency, or audio byte changed.

## Verification run

Run on the branch working tree at the time of writing, on the ADR-0002
reference machine: WSL2, Ubuntu 24.04, CPython 3.12.3, with `worker/.venv`
attached to the qualified environment.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Red regression, unhashable method | The `isinstance(method, str)` guard removed, `python3 -m unittest discover --start-directory worker/tests` | Failed with two errors, `TypeError: unhashable type: 'list'` and `'dict'`, raised out of `read_request`. Restoring the guard returned 44 passed |
| Characterization, `protocol_version` | The `isinstance(version, str)` guard removed, a frame carrying `"protocol_version": []` fed to `read_request` | Refused as `frame protocol version is unsupported` carrying `request_id` `req-1`. A clean refusal, so the guard closes no crash and is recorded as symmetry |
| Red regression, provenance accounting | The reconciliation restriction removed, `python3 -m unittest discover -s scripts/tests` and then the repository check re-run | `test_an_ordinary_accepted_record_cannot_suppress_a_mismatch` failed, and the repository reported 0 mismatches for `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` where the restriction reports 3 |
| Row-necessity sweep | Each row in `e1-s1-evidence-provenance-reconciliation-v2` §Accounted provenance mismatches removed in turn, the check re-run | Over v1's 29 rows: 26 removals produced exactly one refusal naming that row; the three citing `e1-s1-provisional-contract-baseline-v7` produced none, because `-v8` is accepted and retires `-v7` by metadata. Those three are dropped, and the sweep re-run over the 28 carried rows produced exactly one naming refusal for every one |
| Tests | `/usr/bin/time -f 'elapsed=%e seconds' cargo test --offline --workspace --all-targets --locked` | 287 passed, 0 failed, 0 ignored; 6.15 seconds wall time |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Doctests | `cargo test --offline --workspace --doc --locked` | 7 passed |
| Documentation | `cargo doc --offline --workspace --no-deps --locked` | Passed without warnings |
| Published schemas current | `cargo run --offline --locked -p study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | No diff |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Probe compiles | `python3 -m compileall -q -f crates/study-tts-runtime/src/runtime_probe.py` | Clean |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 44 passed |
| Repository scripts | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | 12 passed |
| Qualification scripts | `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'` | 21 passed |
| Dependency policy | `cargo deny check` | Advisories, bans, licenses, and sources passed |
| Markdown links | Repository-wide relative-link scan over `*.md` | Clean |
| Diff hygiene | `git diff --check` | Clean |
| Worker bundle identity, five runs | `target/debug/examples/worker-bundle-hash` | All five returned `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` in 3.41–3.50 s, inside the 3.43–3.62 s `ADR-0001-D004` records |
| Provenance | `python3 scripts/check-evidence-provenance.py` | Clean, exit `0`. Run after the table below was pinned, and the row written from that run |

**One later timing sits outside the band and is reported rather than dropped.**
A sixth consecutive run returned the same identity in 0.90 s. Nothing in the
hash path caches, so the most likely cause is the page cache finally holding the
1,263 MiB the per-file comparison reads; it is recorded because a run an order
of magnitude faster than the authorized band is the shape a skipped comparison
would also have, and this record does not claim to have distinguished the two.

Hosted CI and the protected reference-machine qualification workflow were not
run. Real-model qualification, ASR, and listening were not run; this audit
changes no audio behavior or bytes.

## The identity that moved

`worker/study_tts_worker/protocol.py` is a declared input in
`worker/bundle-manifest.json`, so the two added guards move the worker-bundle
identity from
`f9711a21f3e046d53c7c617e9308893c9c0240badec0d3656487fe2796c6dc2a` to
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`.
`WORKER_BUNDLE_IDENTITY_VERSION` stays at `e1-s1-v4`: the derivation and
ADR-0001 §12.5's input list are unchanged, and only the bytes of a declared
input moved.

Old plan and cache entries remain valid only under the identities that produced
them; this change does not reuse, delete, or re-key them.

## Controlled records

Every row v12 pinned is checked again here, with none dropped, and every digest
was reproduced from current bytes rather than copied forward. Three rows are
added: the provenance script and its tests, because this record's second closed
defect rests on them, and `evidence/README.md`, because the convention that
script now enforces is stated there.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `981cb3e232ae916a5cc7ea4d94aacc288b6d2ac96b252a04e8adb2944cf2c95d` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `f5ae16facbca6ce1090e94afcc9e4ca626d26becc9a688261f889d4b94751e0f` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `cf458da1d5330ca0c7258005aaa354d55e410798dec0617856a0f2257633ef15` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `ebade17f66cac6ec290a8f5cebe0a2688960e47e5ea2303d51ca6c077ddc556b` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `9d94ed6cc3238234dbd8c4ac5e59b07d66f07d6ce514b3b88eea4366b8fbfcc1` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `7b49ab56a27056d56a5bd49df0f5c3ce07cc14ea54f6a7981f18f6ac9b2640c1` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `22aa6c6a38facfa28723ca55cfb04ccced3a2476c11d8459da56592317a0783c` |
| `crates/study-tts-testkit/src/json_schema.rs` | `a6c7f0760387af239e905ef730ba2a3eac40a1cca1709dcc0edf1c1d18a5191b` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `c9cffad3e858270444741dd79ed06e0540d6619b93d06ccaf5593459d89085b7` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e889d30b83cbc4e62c8b779c6fed4082c357facab6bcfa65141044` |
| `schemas/worker-protocol-v1.schema.json` | `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` |
| `worker/AGENTS.md` | `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` |
| `worker/bundle-manifest.json` | `cba489f1d3922c64997fb65c0930a0d8bbb2d542903c435e88b24c0d65fcb2a9` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `09678090fa92b77585de6f05adfbc665ed95f8fc45116e5894cd161bbb7dc7e6` |
| `worker/study_tts_worker/worker.py` | `0777f9b16a41e1c2db00c445229c04b48328bae7fafc6001174846aca0fc8bbf` |
| `worker/tests/test_protocol.py` | `e2dfeb70bc0e0be4d9f7e5de26768e95268cb227b41b9fea8c889efaed889178` |
| `worker/tests/test_worker.py` | `682f2d24c7db45bc0bac90aa4d37de72238f456203b8f2b1a06c3fa6b5aa7113` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef3bc1c1338a62e1126300cc0bb97a89d0a89c4d6dcfb7c88025d9` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `f8ff5796dde3712c2f270ffadcc6e151320e4a7cb128a8c7d562367716f01556` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `9ee3fd43ac856b2a48154d1a7c18736cb3c147d72677eaaf8332aeca4b218d32` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `3ae6fe5e2f052f6febf6e41ebd44e21b55cbfb541e24bb3165f127fb01551cba` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `b85b819c29dd3fab9d45c3c5704007df9aa8af3a4930410c1da50d776909a6de` |
| `docs/INDEX.md` | `3d1806b32e6de26fae3ced33550e3f1ab1d40622d3e3c6d074f6322639c99ee6` |
| `.github/workflows/ci.yml` | `ff80cf2ec76731ab805c5ee6d5dad13c61b423359aac5f156508077be757cda3` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `a561d78d628eba447d7013589f141a58fbc31118f0142955c710e78c90bcf8cf` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `2a6667a94ff4453d8a0b64324366902e0a87e9f1a756587fa22673fff8a57571` |
| `crates/study-tts-testkit/tests/schemas.rs` | `caa5f6e1e317a67d487ef9203d18f99e928659c49165740a9da64dbd11dce68d` |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | `84ed5903193a95a4e8056cb6a7ae07f4ea17ca729f2f67846ec6bd26fe081957` |
| `scripts/check-evidence-provenance.py` | `93ed8f9f7d1871ccc310c62f79bbc5b556878b45e6b80e634b5afd071dbf2946` |
| `scripts/tests/test_check_evidence_provenance.py` | `8a4e719d44b4a0ef121a66925681ba40d4084311363ef8f8dcc57642ee970f4a` |
| `evidence/README.md` | `31942b848f0435bdd63711ff1925973feef46aeb6608bee18a54ce17ffebbd7f` |

## Accounted provenance mismatches

This record accounts for nothing, and cannot: the twenty-first audit restricted
that power to reconciliation records, and this is a baseline record. The three
rows v12 carried under this heading were taking effect only through the defect
that audit closed. Two of them describe real mismatches and now live in
`e1-s1-evidence-provenance-reconciliation-v2` §Accounted provenance mismatches,
which supersedes `-v1`. The third named `-v1` itself and is not carried, because
a superseded record is not checked.

## Deviations and limitations

- **The `protocol_version` guard closes no defect.** It is recorded as symmetry
  with the method guard, and §Verification run carries the characterization that
  shows the field was already refused cleanly. No claim is made for it.
- **The 0.90-second bundle-hash run is unexplained.** The identity was identical
  and nothing in the path caches, but this record did not instrument the read to
  prove the per-file comparison ran, so it reports the observation rather than
  an explanation.
- **`declared_superseded_ids` still lets a retired record grant.** It reads
  §Superseded without supersession metadata from any record whose status is
  Accepted, including one an accepted record has since superseded — the same
  defect class the twenty-first audit closed for `accounted_mismatches`. It is
  raised as an open finding in `e1-s1-evidence-provenance-reconciliation-v2` and
  deliberately not closed here: no wrong entry exists today, and `-v2` carries
  the table forward so the answer changes nothing either way.
- **Three accounted provenance rows were dead and were carried for two
  audits.** `e1-s1-evidence-provenance-reconciliation-v1` §Verification reports
  that removing any of its rows produced exactly one refusal; for three of them
  that is not what happens, and `-v2` drops them on the evidence. The sweep
  described there was either not run over those rows or its result was not read.
  This record does not establish which.
- **Six stale pins are corrected; the reviews that rested on them are not
  re-run.** V12's conclusions were reached against the code those digests were
  meant to name, and that code is what this record pins. What went unverified is
  whether v12's reviewer read the post-fix bytes or the pre-fix ones. This
  record cannot establish that retrospectively and does not try.
- **An in-place amendment to `e1-s1-provisional-contract-baseline-v7` was found
  uncommitted in the working tree and reverted**, not carried. It rewrote that
  record's §Identity and compatibility impact to say the v6 worker-bundle hash
  was already invalid. Amending an accepted report is what `evidence/README.md`
  forbids, and `-v7` is in the unapproved chain `-v2` §Open findings already
  routes. The substance was not evaluated here and is not endorsed.
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
| Contract owner | Ross Todd for T-CORE | Accept that two added refusal messages on frames that previously crashed or were already refused move no contract: no wire shape, schema, protocol version, or Rust error variant changed | 2026-08-29 |
| Engineering owner | Ross Todd | Accept both guards and the provenance restriction on the two reproduced red regressions and the unchanged 287-test suite, and accept that the `protocol_version` guard is recorded as closing nothing | 2026-08-29 |
| Project owner | Ross Todd | Accept this record's two corrections against accepted predecessors — v12's stale table and false provenance row, and the seventeenth-audit overclaim v10 introduced — as made by supersession, with no predecessor edited | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the worker-bundle identity moving to `75d56310…9d2bab3`, reproduced five times on the reference machine, and accept the unexplained 0.90-second run as reported rather than resolved, with hosted-CI and protected qualification reproduction still owed before G1 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that old plan and cache entries remain valid only under their producing identities and are not reused, deleted, or re-keyed by this change | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-29 |
