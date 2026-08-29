# E1-S1 Provisional Contract Baseline Evidence v8

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v7`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v7`, SHA-256
`8d794746b0638369ecd300c4d8993c3c02a7c261877045c9955e1855849707cc`, for its
controlled-record table, verification run, and worker-bundle hash. V7 remains
the immutable record of the bytes it read.

The tenth through thirteenth audits recorded in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
closed two unbounded correlation identities and an unbounded refusal message.
Both ends now require every correlation ID, including `active_request_id`, to
be nonempty ASCII of at most 256 bytes; the published schema carries the same
rules on requests and cancellation responses, and every failure frame bounds
its diagnostic prose where the frame is built. The shared decision fixture
carries the runtime/schema boundaries, and the shared subprocess session
echoes the exact active-ID ceiling through both executables. The protocol is
classified and owner-approved as breaking, and moves to `e1.worker.1.0` with
extension `e1.worker.1.1`; the old major is refused. The same audits add
ADR-0001's sixth method, `health`, and close E1-S1's generated-lesson `$schema`
requirement through `AuthoredLesson::new`. Lockfile refusals now carry a typed
reason for the invariant that failed instead of describing every malformed
lock as a non-exact pin, and name a `WorkerLockfileLocus` rather than a line
number, so the three faults that are the file's — bytes that are not UTF-8, an
absent required directive, a missing governed pin — no longer render as a line
the lock does not have. The tenth audit also repaired two references that
pointed at nothing: every `WorkerBundleError` named a failure-routing row that
`docs/governance/ROUTING-TABLES.md` does not contain, and
`LESSON_SCHEMA_VERSION` documented itself with an intra-doc link to a field the
eighth audit had made private.

The thirteenth audit closes the refusal-boundary confidentiality defect: the
parser no longer embeds a rejected method, protocol version, unknown or
duplicate field name, numeric value, or interpreter exception text in a
failure frame. Refusals retain the invariant, schema-owned path, and derived
bound an operator can act on. A real-worker subprocess regression sends
sentinel lesson text and a voice-reference path through the formerly unsafe
branches, asserts neither channel reproduces them, and confirms the worker
continues through shutdown.

The fifteenth audit corrects initialization itself. A successful
`initialized` frame now carries the closed, typed
`WorkerInitializationIdentities` record: required model and tokenizer
revisions, worker-bundle hash, and at least one voice-profile hash. Missing,
malformed, unknown, and empty identity data are refused by the Rust boundary
and the generated schema describes the same required wire shape. The product
worker no longer reports successful initialization after loading nothing; both
`initialize` and `synthesize` fail nonrecoverably with
`initialization_failed` until E1-S3. The deterministic fake instead reports a
loaded synthetic backend, complete exact identities, and consistent ready and
model-loaded health. This required-response correction remains in the same
pre-G1 `e1.worker.1.0`/`1.1` baseline, whose evidence is still Proposed and
offers no migration promise.

The sixteenth audit closes the remaining fake-only identity inconsistency that
the fifteenth test masked by requesting the fake's own fixed bundle hash. The
fake now refuses any different requested worker-bundle identity with
nonrecoverable `initialization_failed`, while successful initialization and
synthesis report the same model, tokenizer or codec, bundle, and voice-profile
identities. The shared contract test covers both paths and validates every
response against the published schema. This changes no product worker or wire
shape and moves no worker-bundle manifest input.

**V7 recorded a bundle-hash claim that was already false when it was written.**
Its §Identity and compatibility impact states that "the worker bundle does not
contain `study-tts-core`, so its v6 hash remains valid." The first half is
true and the conclusion does not follow: `worker/study_tts_worker/protocol.py`
and `schemas/worker-protocol-v0.schema.json` are declared bundle inputs and had
already moved for the tenth audit when v7 was recorded, so
`9ef560e8f884f50dc23bd0bc88d41aff88ff58d8077fbe283adb0f297361108e` did not
describe the worktree v7 was written against. Per `evidence/README.md` v7 is
superseded rather than corrected; §Worker-bundle hash below carries the value
that holds now. What v7 concluded about its *own* change stands: schema
metadata and CLI status text reach no bundle input, and neither moved the hash.

Filed under `g1/` because E1-S1 feeds G1. This is story evidence, not G1
acceptance; the interface freeze remains deferred to that gate.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all eleven
hold:

1. Every row in v7's controlled-record table is checked again, with none
   silently dropped.
2. Both protocol ends refuse either correlation identity past the shared
   ceiling, accept ASCII at it, and refuse the shared 200-character non-ASCII
   case, decided from
   `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` rather than from two
   independently written suites; neither end returns a shortened identity, and
   both executables echo the exact active-ID ceiling inside a readable frame.
3. The published `worker-protocol-v1` schema carries the same ceiling and ASCII
   pattern on every `request_id` and `active_request_id` it describes and
   accepts the at-ceiling active ID while refusing the one past it.
4. `AuthoredLesson::new` is the public generation path and always serializes
   the current `schema_version` and its stable `$schema` URI.
5. `worker_frames` is owner-approved as breaking at `e1.worker.1.0`, the
   previous major is refused, and migration, rollback, and identity impact are
   recorded.
6. The fake and Python worker implement all six ADR-0001 §10.2 methods. The
   product worker refuses initialization and synthesis nonrecoverably until
   E1-S3 and stays unready and unloaded; the fake returns complete typed
   initialization identities and reports its synthetic backend ready and
   loaded, repeats those identities on synthesis, and refuses initialization
   for any requested bundle other than its own. Every fake response validates
   against the published schema.
7. A refusal to a frame at `MAX_WORKER_FRAME_BYTES` is itself under that
   ceiling.
8. Parser refusal diagnostics contain only invariant names, schema-owned paths,
   and derived bounds; a process-level regression proves sentinel lesson text
   and voice paths appear on neither protocol output nor stderr.
9. Every worker-bundle refusal names a routing row that exists in
   `docs/governance/ROUTING-TABLES.md` §Failure routing, hands the operator the
   repair its own fault calls for, and names the exact malformed-lock invariant
   without disclosing the line.
10. The worker-bundle hash is measured against the restored locked environment,
   reproduced, and recorded against the value it replaces.
11. The complete Rust and Python suites, formatting, conventions, Clippy,
   doctests, and `cargo doc` pass without warning on the recorded environment,
   with every check not run or run with a limitation named under §Deviations
   and limitations.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `581c22ad07a0152eaa50c6f3cb25dc64654e3d3dffc9998a19c3b280563662c4` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `9a3ea2006114d82ddf7788b20ced0d0d00452650e422e187c9365b89267128fe` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |

The E1-S1 change record and provisional baseline move again for the fifteenth
and sixteenth audits' initialization corrections. `TEST-DATA-MANIFEST.md`
moved earlier in this proposed record to carry both shared fixtures' new
checksums. It also differs from the digest v5, v6, and v7 each pin for a
movement that predates this audit — the `e1-s1-takes-unusable-lesson-id-v1`
row — which is accounted in
[`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md).
All three digests here are recalculated from current bytes. The other three
agree with v7.

The changed implementation records are pinned separately:

| Record | SHA-256 |
|---|---|
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `775f6b62c7207f7a5f572090f29ab9f8bb2228ef86e37fe4d950338c73ba97d4` |
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
| `crates/study-tts-runtime/src/lib.rs` | `f5edbae666dcf7280ac13aaf7329bd964a82014d6f27fb9de2539e3bca7ea14d` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `298c4f7a7ec3cb1ff9fc7d5f366a45aa5bc3446902f4fdf1add06f9a4d7ac2eb` |

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-29, cargo 1.97.1, CPython 3.12.3, and
FFmpeg 6.1.1 on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 284 tests,
  including `t3_e1_both_protocol_ends_decide_the_committed_cases_alike` over 31
  shared cases and the schema's active-ID boundary decisions,
  `t4_e1_fake_worker_passes_shared_protocol_contract` over all six methods,
  every fake response against the published schema, exact initialization
  identities, matching synthesis identities, consistent loaded health, and
  nonrecoverable refusal of a mismatched requested bundle,
  the complete initialized-response parser case plus each missing or malformed
  identity category, unknown identity data, and an empty voice-profile set,
  `t1_e1_generated_lesson_includes_the_stable_schema_uri`, the four typed
  malformed-lock reason tests,
  `t4_e1_a_lockfile_fault_no_line_carries_names_no_line` over the three
  whole-file lock faults,
  `t1_e0_external_tool_supervision_policies_are_pinned` over the distinct
  version-only and worker-environment deadlines,
  `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` over valid
  wheel scripts, bounded first-fault reporting, malformed digests, modified
  and missing files, absent `RECORD`, control-bearing and absolute paths, and
  site-package symlink escape,
  the startup-module boundary cases over closed module names and canonical
  digest spelling,
  `t1_e1_startup_module_names_display_as_they_serialize` over both closed
  module names, and
  `t4_e1_runtime_probe_diagnostics_cannot_emit_terminal_controls`,
  `t3_e0_registered_fixture_checksums_match_test_data_manifest` over the moved
  fixtures, and
  `t1_e0_governed_remedy_mappings_are_exhaustive` over the four worker-bundle
  repairs.
- The same locked workspace run passed all ten E1-S1 acceptance tests named in
  `DELIVERY-PLAN.md`: four schema/version tests, the two canonical/synthesis
  identity tests, the two worker-bundle ownership tests, and the two fake/no-
  download worker tests.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 42
  tests, including the three `RequestIdentityCeilingTests` cases, the
  bounded-refusal case, the shared active-ID subprocess session, the shared
  Unicode case, the health response, the fail-closed initialization followed by
  unready and unloaded health, and the process-level redaction regression for
  sentinel lesson text and voice paths.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21 tests.
- `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` — pass, 11
  tests.
- `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `taplo fmt --check`,
  `python3 scripts/check-rust-conventions.py`, Python 3.12 `compileall` over
  `worker/study_tts_worker`, and `git diff --check main` — pass.
- `python3 scripts/check-evidence-provenance.py` — pass, no unaccounted
  mismatch after the supplemental reconciliation was accepted.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`, with
  the existing allowed duplicate-`cpufeatures` warning.
- `cargo doc --offline --workspace --no-deps --locked` — pass, no warnings.

## Worker-bundle hash

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`
returned the same value twice against the restored locked environment:

```text
5d77a5a6a520466043cb6a67ae805b148104d74d8c91fe85932b31d782d8b0af
```

It replaces the pre-thirteenth-audit value in this proposed v8 draft,
`8339a5b425781965527e299591a445a1c4452ecdbeea6756fa82fd401b8d508a`.
The Python protocol and worker modules are declared bundle inputs and both
moved. Their input paths and the derivation are unchanged, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`.

**Superseded by the fourteenth audit and re-measured on the developer
environment.** That audit moved `worker/bundle-manifest.json` to layout `1.1`
and declared the digest of the `sitecustomize` this Ubuntu interpreter executes.
The manifest is itself a declared bundle input, so the value above no longer
describes the checked-in bundle. The verified bundle now hashes to:

```text
92bd4e442ed1caf2897660d57be580796d4f88a558ad65d45983f66336db16a3
```

`cargo run --offline --locked --package study-tts-runtime --example
worker-bundle-hash` returned that value twice against the restored locked
environment, in 4.62 and 3.78 seconds. The worker-environment probe inspected
1.58 GB across 43,828 site-package files under its new two-minute ceiling. This
is a developer-machine `verified_hash` run, not the protected reference-machine
reproduction; the worker/runtime owner's pending review in §Review therefore
still applies. The input paths and the derivation are unchanged, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`.

**Superseded by the fifteenth audit and re-measured on the developer
environment.** The typed initialization schema and fail-closed product worker
move two declared bundle inputs. The verified bundle now hashes to:

```text
6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02
```

`cargo run --offline --locked --package study-tts-runtime --example
worker-bundle-hash` returned that value twice against the restored locked
environment. This remains a developer-machine result, not the protected
reference-machine reproduction. The input paths and derivation are unchanged,
so `WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`.

The sixteenth audit moves only the deterministic fake, its contract test, and
documentation. None is listed in `worker/bundle-manifest.json`; reproducing the
bundle hash twice therefore retains the value above.

Every plan hash built against this worker identity moves with it. Cache entries
written under earlier bundle identities remain on disk and remain valid under
the identity that produced them; none is reused, deleted, or re-keyed.

## Deviations and limitations

- **Two records this audit moved are pinned by unapproved predecessors.** The
  shared contract-case file and the `lesson.rs` link repair move
  `TEST-DATA-MANIFEST.md` and `lesson.rs`, which v5, v6, and v7 pin at earlier
  digests. Each is accounted in
  [`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md),
  where the difference is named and reproduced byte for byte rather than
  asserted.
- **Nothing mechanically ties a routing-row name to the table it names.**
  `t1_e0_governed_remedy_mappings_are_exhaustive` pins each row name as a
  literal, which is what let `Worker bundle input missing or oversized` survive
  in every worker-bundle refusal although
  `docs/governance/ROUTING-TABLES.md` §Failure routing has never carried that
  row. For this audit every routing-row literal in `crates/` was extracted and
  compared against every table in the document as a one-off: 7 distinct names
  across 34 sites, all of them rows that exist. That was a script run once, not
  a test the suite reruns; a check that reads the document and refuses an
  unknown row is the durable fix and is not written here.
- This was a developer-machine run, not the protected reference-machine
  qualification workflow. The reference machine must reproduce the bundle hash
  before G1.
- The self-hosted qualification workflow was not dispatched.
- Markdown Prettier was not available in this workspace. Markdown structure and
  relative links were reviewed, and `git diff --check main` passed.
- Real-model qualification, ASR, listening, and reference-machine measurements
  were not run. This change reaches no speech backend and changes no audio
  bytes.
- **The owner-approved narrowing has bounded practical impact.** No frame this
  build can produce is refused by the new ceiling — `pipeline.rs` builds the
  longest identity in the workspace at 132 bytes — and both ends live in this
  repository.
- The accepted reconciliation v1 and draft baselines v5-v7 pin earlier bytes
  of `PROVISIONAL-CONTRACT-BASELINE.md`, `TEST-DATA-MANIFEST.md`,
  `WORKER-ENVIRONMENT.md`, and `worker_bundle.rs`.
  [`e1-s1-eleventh-audit-provenance-reconciliation-v1.md`](e1-s1-eleventh-audit-provenance-reconciliation-v1.md)
  now accounts for those seven movements and their compatibility impact. No
  earlier accepted evidence was edited.
- The fifteenth- and sixteenth-audit contract, engineering, worker/runtime, and
  affected-track reviews have not occurred. This Proposed record does not infer
  them from the local test results.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending review of the fifteenth- and sixteenth-audit initialization corrections | |
| Engineering owner | engineering owner | Pending review of the fifteenth- and sixteenth-audit remediations | |
| Project owner | project owner | Approved the supplemental provenance reconciliation | 2026-08-28 |
| Worker/runtime owner | T-WORKER | Pending review and reference-machine reproduction of the bundle hash | |
| Affected-track reviewers | T-RUNTIME | Pending review of the corrected initialization contract and cache-impact account | |
