# E1-S1 Provisional Contract Baseline Evidence v4

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v3`, SHA-256
`5ccbb88b702193b7f5dac5a24cb6a984d5af8801db2bee7bd9bc39b56c077fa9`, **for its
controlled-record table, its verification run, and the worker-bundle hash it
reports**. It exists because two of the six documents v3 pinned have moved since
it was written, and `../../../README.md` makes an accepted report immutable: a
record edited after adoption is a record nothing can be checked against.

Nothing in v3 was wrong. Like v2 and unlike v1, it is superseded because the
bytes it pinned moved and because a new run replaces the one it reports, not
because it misstated anything. Its account of what the third audit closed, its
criteria, and its supersession of v2 all stand. So does the bundle hash it
recorded: that value is the immutable record of the bundle v3 measured, not a
claim about the bundle today.

Why each document moved is in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
§What the fourth audit closed and §What the fifth audit closed, which name this
file in return; a second copy of that reasoning here could disagree with the
copy this record's digest froze.

Filed under `g1/` for the reason v1, v2, and v3 were: E1-S1 feeds G1, and this is
a story-level record under the gate it feeds rather than a gate acceptance. G1
acceptance is unchanged. The interface freeze stays deferred to G1.

## Acceptance criterion

Stated before any result, because a bar set afterwards cannot fail. Accepted
when all five hold:

1. Every document the superseded table pinned is re-pinned at its current
   committed bytes, with no row silently dropped.
2. The E1-S1 change record is pinned alongside them, so it cannot be edited
   without invalidating this evidence.
3. Every check `AGENTS.md` and `.github/workflows/ci.yml` require passes on the
   recorded environment, including the walking skeleton against real FFmpeg.
4. The three E0-S4 Delivery Plan acceptance names still exist and still pass.
5. Every check this record reports as passing was **run** on the recorded
   environment, and every check that was not run is named in §Deviations and
   limitations with the reason.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `364259126bdca16b3831406f7aafced35579eab5b71e02e54b3560b26876d803` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `1eaca00abe695c2ea08e9642b9f2fb6a9dfb55f4679eb99b4f14a34b8748b7ae` |

Criteria 1 and 2: two of the six rows moved.

- `E1-S1-INTERFACE-CHANGE-001.md` gained §What the fourth audit closed, which
  records three worker-tree repairs and the bundle identity they moved, and
  §What the fifth audit closed, which records two published-schema narrowings
  and the fixture-type correction below.
- `TEST-DATA-MANIFEST.md` retypes the four single-frame `e0-s4-worker-*.json`
  fixtures from `Worker NDJSON` to `Worker frame JSON`. They are one JSON
  document each and correctly named `.json`; the label they carried is the one
  the two genuine multi-frame `.ndjson` fixtures carry, so a reader could not
  tell the two formats apart by the column that exists to say which is which.
  No checksum, path, rights record, retention, owner, or status cell moved, and
  no row was added or dropped: the manifest still registers exactly the 33
  fixtures on disk, one row each.

`PROVISIONAL-CONTRACT-BASELINE.md`, `E0-S4-INTERFACE-CHANGE-001.md`,
`WALKING-SKELETON.md`, and `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` are carried
at unchanged digests, verified against the current bytes rather than copied
forward.

`worker/bundle-manifest.json` is again **not** among them, and again because
nothing it declares changed as a declaration. Its input set, import roots, and
Python identity are what v3 recorded; one file the input set names changed
bytes, which moves the hash rather than the manifest. See §The worker-bundle
hash.

`docs/operations/WORKER-ENVIRONMENT.md` is **not** in this table, for the reason
v2 and v3 gave: the set of controlled records is the one E0-S4 established and
E1-S1 carried forward, and widening it is a change to what is controlled rather
than to what those records say. It moved again since v3 — its current SHA-256 is
`d4496d3a280a0073cf22716cd748f54e88c8a1bfeaec12ee4eb2dcb91da6383c`, recorded
here so a reader can tell whether it moved without that being a pinned claim.

## Verification run

Criterion 3. Ubuntu 24.04 under WSL2 on 2026-08-28, cargo 1.97.1, FFmpeg 6.1.1
on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 269 tests.
- `cargo test --offline --workspace --doc --locked` — pass, 7 doctests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline`
  — pass, 35 tests with real FFmpeg and ffprobe.
- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline`
  — pass, 6 tests.
- `cargo test -p study-tts-testkit --test worker_contract --locked --offline`
  — pass, 3 tests, including the shared protocol cases both implementations
  must decide alike.
- `cargo test -p study-tts-testkit --test schemas --locked --offline` — pass,
  12 tests, including the two the fifth audit added.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 34
  tests.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo doc --offline --workspace --no-deps --locked`,
  and `git diff --check` — pass, with no warnings.
- `cargo deny --offline check` — `advisories ok, bans ok, licenses ok, sources
  ok`, with one `warning[duplicate]: found 2 duplicate entries for crate
  'cpufeatures'`, unchanged from v2 and v3.

The doctest count fell from v3's 14 to 7 and the all-targets count rose from 254
to 269. Both moved for work outside the two audits this record carries, so
neither is attributed here; the numbers are reported as run rather than
reconciled against v3's.

The two new tests are inside the counts above:
`t3_e1_every_published_schema_link_is_constrained_to_its_own_schema` and
`t3_e1_every_published_schema_claims_the_uri_its_documents_name`.

## The worker-bundle hash

**Read on 2026-08-28:**

```text
f9a0c8f25e322aa7eeb34382a45dd702be72df7b33e476543c0907a0728e9ec4
```

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`,
run twice with the same result. It is the value
[`../../../../docs/operations/WORKER-ENVIRONMENT.md`](../../../../docs/operations/WORKER-ENVIRONMENT.md)
§Reading the current identity requires be recorded in any qualification evidence
taken with this worker.

It has moved twice since v3 read
`2cf29b055d54e673709326d9e0318423eb487da5902ca82c109a8d70d6b6afc7`, and both
moves are accounted for:

| Cause | Value after |
|---|---|
| v3, as recorded | `2cf29b055d54e673709326d9e0318423eb487da5902ca82c109a8d70d6b6afc7` |
| §What the fourth audit closed — `protocol.py` and `worker.py` repairs, both declared inputs | `e3f81a79b455ab922aa11b452586b7d27ec8922293111cfe38ff8e3c9f532328` |
| §What the fifth audit closed — `schemas/worker-protocol-v0.schema.json` gained `$id`, and it is a declared input | `f9a0c8f25e322aa7eeb34382a45dd702be72df7b33e476543c0907a0728e9ec4` |

The second row is the value that document already records. The third was
measured directly rather than inferred: the change was reverted, the schema
regenerated, and the hash read again, returning the second row's value exactly,
then restored and read again, returning the first line of this section. That is
what makes `$id` the whole of the difference rather than the most likely
explanation for it.

The gates the hash passed on the current run:

| Gate | What it saw |
|---|---|
| Declared inputs complete | Every `.py` beneath `worker/study_tts_worker` declared; the five required inputs present |
| Runtime identity | Matching `worker/bundle-manifest.json` field for field |
| Environment against the lock | Every pin installed at its pinned version |
| Governed provenance | `chatterbox-tts` PEP 610 `vcs_info.commit_id` equal to the commit the lock records |
| Startup hooks | One, `distutils-precedence.pth`, owned by the pinned `setuptools` |

Criterion 4: unchanged and passing within the runs above —
`t4_e0_every_provisional_seam_has_a_fake`,
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension`,
`t4_e0_walking_skeleton_uses_only_published_seams`.

## Deviations and limitations

Criterion 5. Every item below was **not run**, or was run with a limit worth
naming, and why.

- **This is not the reference machine.** It is a developer machine and a
  hand-run rather than a workflow, exactly as v3 records. **The reference
  machine must still read the hash before G1**, and it must agree with the value
  above; a different answer means the two environments differ in a
  synthesis-key input.
- **The gate table above is thinner than v3's.** v3 read its rows from a
  restore performed and observed end to end, including the pin count, the
  platform tag, and the governed commit id. This run read the same gates through
  the same command on an environment already restored, so the rows record that
  each gate passed rather than the measured values behind them. v3's table is
  the detailed one; nothing here contradicts it.
- **The `EnvironmentMismatch` coverage is unchanged from v3**, and its limit
  with it: most faults are still proved against a scripted stand-in interpreter,
  because reaching them for real means damaging a restored environment. This run
  added no observation of a real fault.
- **Real-model qualification and listening were not rerun**, and this remains a
  single-machine result with no reference-machine run behind it, for the reasons
  v2 and v3 record: both need the governed model, weights, and voice roots that
  `../../../../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps outside Git
  and CI.
- **The all-targets and doctest counts are not reconciled against v3's.** Work
  outside the two audits this record carries moved both, and attributing the
  difference would be a claim this run cannot support. They are reported as run.
- The classification of the two published-schema narrowings in §What the fifth
  audit closed as a **compatible patch** is **T-CORE's to ratify**, exactly as
  the earlier narrowings are. It is recorded as an open decision rather than
  counted as accepted. The same is true of the fourth audit's classification of
  its Python repairs as a compatible repair.
- **No cache-invalidation impact assessment was performed.** Every cache key
  moves relative to `main`, and moves again relative to v3, because the bundle
  hash is a synthesis-key input under ADR-0001 §12.5. Whether any stored take,
  package, or verification result needs migrating rather than re-deriving is
  T-RUNTIME's to determine; nothing here has been re-derived.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of the compatible-patch classification in `E1-S1-INTERFACE-CHANGE-001` §What the fifth audit closed, and of the compatible-repair classification in §What the fourth audit closed | |
| Engineering owner | engineering owner | Pending review of the fourth- and fifth-audit remediation | |
| Worker/runtime owner | T-WORKER | Pending the reference-machine bundle-hash run named above; a developer-machine run against the restored environment is done | |
| Affected-track reviewers | T-AUDIO, T-RUNTIME | Deferred to the G1 fake/real parity review; the cache-invalidation impact named in §Deviations and limitations is T-RUNTIME's | |
