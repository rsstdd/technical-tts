# E1-S1 Provisional Contract Baseline Evidence v3

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v2`, SHA-256
`bf3c7aedfd4c480ad7d8a650f6e5cbef5efdb12220e7b500d1c27ae33f7bf7b1`, **for its
controlled-record table and its verification run**. It exists because a third
audit of E1-S1 found seven open findings, and running the resulting restore
procedure end to end found two more, whose remediation moves three of the
six pinned documents, and `../../../README.md` makes an accepted report
immutable: a record edited after adoption is a record nothing can be checked
against.

Nothing in v2 was wrong. Unlike v1, it is superseded because the bytes it pinned
moved and because a new run replaces the one it reports, not because it
misstated anything. Its account of what the second audit closed, its criteria,
and its supersession of v1 all stand.

Why each document moved is in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
§What the third audit closed, which names this file in return; a second copy of
that reasoning here could disagree with the copy this record's digest froze.

Filed under `g1/` for the reason v1 and v2 were: E1-S1 feeds G1, and this is a
story-level record under the gate it feeds rather than a gate acceptance. G1
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
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `a1a2b11cece66f745eeb2d8b0e548e242d28bf737a0db9b4f1d8252267edd8ad` |
| `docs/architecture/WALKING-SKELETON.md` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `2c6261efa11cf6034fbc1c3210ab507cfdbd6c932174dc51980d1972562140f4` |

Criteria 1 and 2: three of the six rows moved.

- `E1-S1-INTERFACE-CHANGE-001.md` gained §What the third audit closed, and its
  §Version and compatibility field list now names the `take` that
  `SynthesisRequest` gained.
- `WALKING-SKELETON.md` gained one provisional resource ceiling: selections per
  takes document, which is the lesson's own segment ceiling applied at the
  boundary that had none.
- `TEST-DATA-MANIFEST.md` re-pins `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson`,
  which gained the `zero-threads` case both protocol ends must refuse alike.

`worker/bundle-manifest.json` is deliberately **not** among them. Running the
restore found the probe disagreeing with its declared `platform_tag`, and the
declaration was the correct side: the probe moved instead, so no declared bundle
input changed and the hash below is the one the manifest already described.

`PROVISIONAL-CONTRACT-BASELINE.md`, `E0-S4-INTERFACE-CHANGE-001.md`, and
`INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` are carried at unchanged digests,
verified against the current bytes rather than copied forward.

`docs/operations/WORKER-ENVIRONMENT.md` is **not** in this table, for the reason
v2 gave: the set of controlled records is the one E0-S4 established and E1-S1
carried forward, and widening it is a change to what is controlled rather than
to what those records say. It moved substantially again — the governed install
is now a VCS install and the startup-hook rule is new — and its current SHA-256
is `342e9d2b4c9de853a27b52f6feb9af6e8b12a6d381c368380d1f4737db5612b3`, recorded
here so a reader can tell whether it moved again without that being a pinned
claim.

## Verification run

Criterion 3. Ubuntu 24.04 under WSL2 on 2026-08-27, cargo 1.97.1, FFmpeg 6.1.1
on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 254 tests.
- `cargo test --offline --workspace --doc --locked` — pass, 14 doctests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline`
  — pass, 35 tests with real FFmpeg and ffprobe.
- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline`
  — pass, 6 tests.
- `cargo test -p study-tts-testkit --test worker_contract --locked --offline`
  — pass, 3 tests, including the shared protocol cases both implementations
  must decide alike, which now carry `zero-threads`.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 31
  tests.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo doc --offline --workspace --no-deps --locked`,
  and `git diff --check` — pass, with no warnings.
- `cargo deny --offline check` — `advisories ok, bans ok, licenses ok, sources
  ok`, with one `warning[duplicate]: found 2 duplicate entries for crate
  'cpufeatures'`, unchanged from v2.

The six new tests are inside the counts above:
`t1_e1_a_synthesis_request_carries_the_take_its_cache_key_names`,
`t1_e1_a_selection_naming_an_identity_no_lesson_can_carry_is_refused`,
`t1_e1_takes_selection_ceiling_accepts_the_boundary_and_is_the_lesson_ceiling`,
`t1_e1_two_installs_canonicalizing_alike_are_refused_rather_than_collapsed`,
`t1_e1_a_startup_hook_the_lockfile_does_not_account_for_is_refused`, and
`t1_e1_the_interpreter_is_probed_where_it_is_attached_not_where_it_resolves`.

## The worker-bundle hash

**Read for the first time in this project, on 2026-08-28:**

```text
2cf29b055d54e673709326d9e0318423eb487da5902ca82c109a8d70d6b6afc7
```

`cargo run --offline --locked --package study-tts-runtime --example worker-bundle-hash`,
run twice with the same result, and twice again after the qualified environment
was moved out of the checkout to `<checkout-parent>/study-tts-qualified-worker-venv`.
**The value did not move when the environment did**, which is the property the
identity is supposed to have: it describes the declared bundle, not where the
machine keeps its interpreter. It is the value
[`../../../../docs/operations/WORKER-ENVIRONMENT.md`](../../../../docs/operations/WORKER-ENVIRONMENT.md)
§Reading the current identity requires be recorded in any qualification evidence
taken with this worker, and it is reported here because it passed the whole gate
rather than because a command printed something:

| Gate | What it saw |
|---|---|
| Declared inputs complete | Every `.py` beneath `worker/study_tts_worker` declared; the five required inputs present |
| Runtime identity | `cpython 3.12.3 (cp312, manylinux_2_39_x86_64)`, matching `worker/bundle-manifest.json` field for field |
| Environment against the lock | All 56 pins installed at their pinned versions; 8 unpinned distributions tolerated |
| Governed provenance | `chatterbox-tts` PEP 610 `vcs_info.commit_id` `eb90621fa748f341a5b768aed0c0c12fc561894b`, equal to the commit the lock records |
| Startup hooks | One, `distutils-precedence.pth`, owned by the pinned `setuptools` |

The environment was restored by the procedure in that document, including the
`git+file://…@<commit>` install that replaced the directory install. Before that
reinstall the same command refused with
`EnvironmentMismatch::WithoutRecordedRevision`, which is the finding working
rather than an obstacle to it.

Relocating the environment exercised two more gaps in that procedure, both now
recorded in it: `.gitignore` matched `/worker/.venv/` with a trailing slash, so
the link the procedure tells an operator to create was untracked rather than
ignored and could have carried a machine-local absolute path into Git; and a
virtualenv does not survive a bare `mv`, because every console script in `bin/`
keeps an absolute shebang while the interpreter and this probe go on working.

Criterion 4: unchanged and passing within the runs above —
`t4_e0_every_provisional_seam_has_a_fake`,
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension`,
`t4_e0_walking_skeleton_uses_only_published_seams`.

## Deviations and limitations

Criterion 5. Every item below was **not run**, and why.

- **This is not the reference machine.** The environment now sits where
  `docs/operations/WORKER-ENVIRONMENT.md` §Restoring the environment requires,
  at `<checkout-parent>/study-tts-qualified-worker-venv` with `worker/.venv`
  linked to it, so nothing qualified remains inside the checkout for
  `git clean -ffdx` to remove. It is still a developer machine and a hand-run
  rather than a workflow. **The reference machine must still read the hash
  before G1**, and it must agree with the value above; a different answer means
  the two environments differ in a synthesis-key input.
- **Only three of the seven `EnvironmentMismatch` faults were observed against a
  real environment**, and only one deliberately:
  `WithoutRecordedRevision` before the governed reinstall, and the passing paths
  for the pins and the startup hook. `Absent`, `Version`, `FromIndex`,
  `FromAnotherRevision`, `AmbiguousDistribution`, `UnownedPathHook`, and
  `UnlockedPathHook` are still proved against a scripted stand-in interpreter,
  because reaching them for real means damaging a restored environment.
- **The hash covers what the manifest declares and not this machine.** Both
  fixes found while running the procedure — the interpreter resolution and the
  platform tag — are in `crates/`, which no manifest input names, so neither
  moves the value above. Every cache key still moves relative to `main`, for the
  reason §What this moves records: `worker/study_tts_worker/protocol.py` and
  `schemas/worker-protocol-v0.schema.json` changed bytes.
- **Real-model qualification and listening were not rerun**, and this remains a
  single-machine result with no reference-machine run behind it, for the reasons
  v2 records: both need the governed model, weights, and voice roots that
  `../../../../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps outside Git
  and CI.
- The classification of the two schema narrowings in §What the third audit
  closed as a **compatible patch** is **T-CORE's to ratify**, exactly as the
  earlier narrowings are. It is recorded as an open decision rather than counted
  as accepted.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of the compatible-patch classification in `E1-S1-INTERFACE-CHANGE-001` §What the third audit closed | |
| Engineering owner | engineering owner | Pending review of the third-audit remediation | |
| Worker/runtime owner | T-WORKER | Pending the reference-machine bundle-hash run named above; the governed reinstall and a developer-machine run are done | |
| Affected-track reviewers | T-AUDIO, T-RUNTIME | Deferred to the G1 fake/real parity review | |
