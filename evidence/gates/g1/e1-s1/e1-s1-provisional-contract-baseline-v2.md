# E1-S1 Provisional Contract Baseline Evidence v2

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v1`, SHA-256
`2e4caf948f5ed9251e3a3de88caeae0bd970244609d82b2da40381c309f98f55`, **for its controlled-record table and its
verification run**. It exists because a second audit of E1-S1 found three
statements in that record that were wrong when it was written, and
`../../../README.md` makes an accepted report immutable: a record edited after
adoption is a record nothing can be checked against.

What v1 got wrong, and what is true:

| v1 said | What ran |
|---|---|
| The walking skeleton passed with 35 tests, while [`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md) said 34 for the same run | 35. The change record is corrected in §Delivery and recovery, which now says so explicitly rather than silently agreeing. |
| `cargo deny --offline check` passed "with no warnings" | It passes, and reports `warning[duplicate]: found 2 duplicate entries for crate 'cpufeatures'`. The four verdicts are `advisories ok, bans ok, licenses ok, sources ok`; the duplicate is a warning the configuration does not deny. |
| Real-model qualification "is where an operator runs them", naming `.github/workflows/qualification.yml` | That workflow states in its own comments that the real-model measurements are **not** invoked from it, and points at `scripts/qualification/README.md`. The workflow attaches the qualified environment and runs the suite on real FFmpeg; the measurements are run by hand. |

Everything else in v1 stands: its account of what E1-S1 changed, its criteria,
and its supersession of `../../g0/e0-s4/e0-s4-provisional-contract-baseline-v2.md`
for the controlled-record table only.

The table below is re-pinned because that second audit's remediation moved four
of the six documents. Why each moved is in
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md)
§What the second audit closed, which names this file in return; a second copy of
that reasoning here could disagree with the copy this record's digest froze.

Filed under `g1/` for the reason v1 was: E1-S1 feeds G1, and this is a
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
   limitations with the reason. This criterion is new in v2, and it is the one
   the three corrections above failed.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `82b8f82b5f207d05e19b2b36e4eb6895d610afac3a8bf20b6a55ae37fe27b220` |
| `docs/architecture/WALKING-SKELETON.md` | `afe75afbe89903b7e17e965da94c5c10c04f6c9c4c4780a684f3e3f873a87a19` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `b0f85b629b4e70da72664413dd9d83ab0d1f7a978fa3da981de878ab27bcb406` |

Criteria 1 and 2: four of the six rows moved.

- `PROVISIONAL-CONTRACT-BASELINE.md` records the shared worker-protocol case
  file in the `worker_frames` row, because that file is now what decides
  whether the two ends of that contract agree.
- `E1-S1-INTERFACE-CHANGE-001.md` gained §What the second audit closed, and its
  two `34`s and its two `ManifestLayout` references are corrected.
- `WALKING-SKELETON.md` gained the other side of the manifest-layout mirror,
  which now names two constants rather than a deleted enum.
- `TEST-DATA-MANIFEST.md` gained four fixtures: the shared worker-protocol
  cases and one malformed-digest document for each of the job, manifest, and
  verification schemas.

`E0-S4-INTERFACE-CHANGE-001.md` and `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`
are carried at unchanged digests, verified against the current bytes rather than
copied forward.

`docs/operations/WORKER-ENVIRONMENT.md` is **not** in this table and is not
added to it. It moved substantially in this remediation — it gained §The
installed environment is checked against the lock and §The launcher is read
closed — but the set of controlled records is the one E0-S4 established and E1-S1
carried forward, and widening it is a change to what is controlled rather than to
what those records say. Its current SHA-256 is
`8529025108d5100870aa49352342dea0b3d9d3b44b5b0334a5abbb9a8ea101b2`, recorded here so a reader can
tell whether it moved again without that being a pinned claim.

## Verification run

Criterion 3. Ubuntu 24.04 under WSL2 on 2026-08-27, cargo 1.97.1, FFmpeg 6.1.1
on `PATH`:

- `cargo test --offline --workspace --all-targets --locked` — pass, 248 tests.
- `cargo test --offline --workspace --doc --locked` — pass, 13 doctests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline`
  — pass, 35 tests with real FFmpeg and ffprobe.
- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline`
  — pass, 6 tests.
- `cargo test -p study-tts-testkit --test worker_contract --locked --offline`
  — pass, 3 tests, including the shared protocol cases both implementations
  must decide alike.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 31
  tests, which include the subprocess tests for stdout isolation, offline
  application, and survival of a hostile frame, and the launcher-shape tests
  that prove a variable outside the allowlist never reaches `os.environ`.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo doc --offline --workspace --no-deps --locked`,
  and `git diff --check` — pass, with no warnings.
- `cargo deny --offline check` — `advisories ok, bans ok, licenses ok, sources
  ok`, with one `warning[duplicate]: found 2 duplicate entries for crate
  'cpufeatures'`. Recorded rather than described as clean, which is what v1 got
  wrong.

Criterion 4: unchanged and passing within the runs above —
`t4_e0_every_provisional_seam_has_a_fake`,
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension`,
`t4_e0_walking_skeleton_uses_only_published_seams`.

## Deviations and limitations

Criterion 5. Every item below was **not run**, and why.

- **The worker-bundle hash was not read on this machine, and the new
  environment check therefore never ran against a real interpreter.**
  `cargo run --package study-tts-runtime --example worker-bundle-hash` refuses
  with `UnreadableRuntimeIdentity` because the attached `worker/.venv` holds
  only `pip` and has no `packaging`. That is the documented correct failure —
  an unrestored environment cannot witness the runtime the manifest claims —
  but it means the four `EnvironmentMismatch` faults are proved against a
  scripted stand-in interpreter here and not against a restored environment.
  **The reference machine must run it before G1**, and the printed hash must be
  recorded in any qualification evidence taken with this worker. Every cache key
  moves with it: four declared bundle inputs changed bytes.
- **Real-model qualification and listening were not rerun**, and this is a
  single-machine result with no reference-machine run behind it. Both need the
  governed model, weights, and voice roots that
  `../../../../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps outside Git
  and CI; no synthesis path in this change reaches a model.
  `scripts/qualification/README.md` is what an operator runs them from.
  `.github/workflows/qualification.yml` attaches the qualified environment and
  runs the suite on real FFmpeg; it states in its own comments that it invokes
  no real-model measurement. v1 named the workflow for both, which is the second
  thing it got wrong.
- The change record's §Impact of the two deliberately incomplete inputs records
  the two unresolved ADR-0001 §12.5 inputs and what they will move.
- The classification of the schema narrowings in §What the second audit closed
  as a **compatible patch** is **T-CORE's to ratify**, exactly as the
  worker-frame typing in §The two the audit left open is. It is recorded as an
  open decision rather than counted as accepted.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Pending ratification of the compatible-patch classification in `E1-S1-INTERFACE-CHANGE-001` §What the second audit closed | |
| Engineering owner | engineering owner | Pending review of the second-audit remediation | |
| Worker/runtime owner | T-WORKER | Pending the reference-machine bundle-hash run named above | |
| Affected-track reviewers | T-AUDIO, T-RUNTIME | Deferred to the G1 fake/real parity review | |
