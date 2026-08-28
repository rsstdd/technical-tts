# E1-S1 Provisional Contract Baseline Evidence v8

- Status: Proposed
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
6. The fake and Python worker implement all six ADR-0001 §10.2 methods, with
   health reporting readiness and model residency truthfully.
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
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `559593d42d5888649a76e3ecbbd12b091c2359132070cc83626bda719bcb3137` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `b8fcc9b18b53008b3c32e9387634b0d23ec81c74e8098f99d8eb78b6fbb4f3c9` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |

The E1-S1 change record, provisional baseline, and `TEST-DATA-MANIFEST.md`
moved for this audit: the first two record the completed boundaries, and the
manifest records both shared fixtures' new checksums. The manifest also differs
from the digest v5, v6, and v7 each pin for a movement that
predates this audit — the `e1-s1-takes-unusable-lesson-id-v1` row — which is
accounted in
[`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md).
All three digests here are recalculated from current bytes. The other three
agree with v7.

The changed implementation records are pinned separately:

| Record | SHA-256 |
|---|---|
| `crates/study-tts-runtime/src/worker_protocol.rs` | `18b7aacc7ab1ae2680efb68c61e1adce2f17191731b8c2c111479407f2fbebcb` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `9dc3257d7f82ec30a52d86a991fe2426f69ee72155ea43b57b23b3c486e20892` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `e6044c426b652e8699a8c613f9a1b6f1a722e680d4d622cc22cdb0caea1e90fa` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `fe12a3ff542a6bb8721aea1c4e63a32090ebab0dbf8e4d95fb891114a9c931ac` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e889d30b83cbc4e62c8b779c6fed4082c357facab6bcfa65141044` |
| `schemas/worker-protocol-v1.schema.json` | `d1e5f3fad5747c64129a77a8bb04f4d8d45c987ea402bbfe9315e5fd69a31f7b` |
| `worker/bundle-manifest.json` | `8dc54edaa80bf9cae3e3800bf33bec4d3fa4bb1d9738b163abf044f0e9c58b3c` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `da7baa5c48d6038c3537e6414614de9beedcdf2098abd74d5a70d105814b4c98` |
| `worker/study_tts_worker/worker.py` | `ce0d7fe4d24e6accc18ebe3d42ed4d3095093e8487653511241af06a4750eadf` |
| `worker/tests/test_protocol.py` | `405e9c41787b6784374146b695e166ff2b9de5828ba259826e7078f99149a6fd` |
| `worker/tests/test_worker.py` | `0ff9c1d124583ba04639f7a20d618e8d0bc733dd3a6cb784194e964457848c21` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef3bc1c1338a62e1126300cc0bb97a89d0a89c4d6dcfb7c88025d9` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `1631f74eb29d954376cc50327d6ab590ac072ab3be96ff5bce19a119ce3f1b83` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `1125ad302012532a693a12b9734c3db5b6539ad4f86fa1f53fbdad932b9f0792` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `2402f2c25436bb7e62dc6999f8f2740358e4f820dad343e359661e3a89eb955c` |

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-28, cargo 1.97.1, CPython 3.12.3, and
FFmpeg 6.1.1 on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 275 tests,
  including `t3_e1_both_protocol_ends_decide_the_committed_cases_alike` over 31
  shared cases and the schema's active-ID boundary decisions,
  `t4_e1_fake_worker_passes_shared_protocol_contract` over all six methods,
  `t1_e1_generated_lesson_includes_the_stable_schema_uri`, the four typed
  malformed-lock reason tests,
  `t1_e1_a_lockfile_fault_no_line_carries_names_no_line` over the three
  whole-file lock faults,
  `t3_e0_registered_fixture_checksums_match_test_data_manifest` over the moved
  fixtures, and
  `t1_e0_governed_remedy_mappings_are_exhaustive` over the four worker-bundle
  repairs.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 41
  tests, including the three `RequestIdentityCeilingTests` cases, the
  bounded-refusal case, the shared active-ID subprocess session, the shared
  Unicode case, the health response, and the process-level redaction regression
  for sentinel lesson text and voice paths.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21 tests.
- `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` — pass, 11
  tests.
- `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `taplo fmt --check`,
  `python3 scripts/check-rust-conventions.py`, Python 3.12 `compileall` over
  `worker/study_tts_worker`, and `git diff --check` — pass.
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
  relative links were reviewed, and `git diff --check` passed.
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

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending review of the thirteenth-audit amendments | |
| Engineering owner | engineering owner | Pending review of the thirteenth-audit remediation | |
| Project owner | project owner | Approved the supplemental provenance reconciliation | 2026-08-28 |
| Worker/runtime owner | T-WORKER | Pending reference-machine reproduction of the bundle hash | |
| Affected-track reviewers | T-RUNTIME | Pending confirmation of the cache-impact account | |
