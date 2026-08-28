# E1-S1 Provisional Contract Baseline Evidence v1

## Scope and decision

This record re-pins the controlled-record digests after
[`../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S1-INTERFACE-CHANGE-001.md),
which names this file in return. It supersedes
`../../g0/e0-s4/e0-s4-provisional-contract-baseline-v2.md` **only for the table
below**; that record's account of the E0-S4 remediation stands, and neither it
nor the v1 beneath it is amended, per `../../../README.md`.

Why each document moved is in the pinned change record, not restated here: this
record freezes that document by digest, and a second copy of its reasoning could
disagree with the copy the digest froze.

Filed under `g1/` because E1-S1 feeds G1, in the same shape as `../../g0/e0-s3/`
— a story-level record under the gate it feeds, not a gate acceptance. G1
acceptance is unchanged. The interface freeze stays deferred to G1.

## Acceptance criterion

Stated before any result, because a bar set afterwards cannot fail. Accepted
when all four hold:

1. Every document the superseded table pinned is re-pinned at its current
   committed bytes, with no row silently dropped.
2. The E1-S1 change record is pinned alongside them, so it cannot be edited
   without invalidating this evidence.
3. Every check `AGENTS.md` and `.github/workflows/ci.yml` require passes on the
   recorded environment, including the walking skeleton against real FFmpeg.
4. The three E0-S4 Delivery Plan acceptance names still exist and still pass.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `877a76587d6fa50f1dcaaccfade3bd075b9eaa8dd0f7f9e9f9a152cd65540a7e` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `a2903664f1d4fb3d7ee743c887346720c2ef4449bad17ffcb5b1b8d1bcbdd9db` |
| `docs/architecture/WALKING-SKELETON.md` | `c893b3c5a2af6b6e9fc0c31fcda9be74f8d50f9346cca74e3736679d6537b51c` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `887893fe3436489335b96e7f5eac8503380760a5952da55bff7c1a3c5760b9c8` |

Criteria 1 and 2: four of the five E0-S4 rows moved.
`docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` did not, and is carried at its
unchanged digest rather than dropped, so this table stands alone as the complete
controlled set. `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` is the new
sixth row.

Three of the six moved again during the E1-S1 audit, and the digests above are
the ones after those edits. `docs/architecture/WALKING-SKELETON.md` moved twice:
once to record the three ceilings E1-S1 introduced, so each constant's mirror
has the other side it claims, and again to record the two worker-frame ceilings
the audit added. `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` gained §What
the E1-S1 audit closed, which is that audit's own record of what moved and why.
`docs/testing/TEST-DATA-MANIFEST.md` gained the eight contract fixtures that
prove every published format's schema and its parser agree.

Two of the six moved once more when the audit's two open items were closed, and
the digests above are the ones after those edits.
`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` gained §The two the audit left
open, recording that the worker frames now carry typed digest identities and
that `validate_package`'s accepted manifest layouts are held against the
published schema. `docs/architecture/WALKING-SKELETON.md` gained the other side
of the new `ManifestLayout` mirror, so that type's coupling comment names a
paragraph that names it back. The four remaining rows are carried at unchanged
digests, verified against the current bytes rather than copied forward.

## Verification run

Criterion 3. Ubuntu 24.04 under WSL2 on 2026-08-27, cargo 1.97.1, FFmpeg 6.1.1
on `PATH`:

- `cargo test --workspace --all-targets --locked --offline` — pass, 247 tests.
- `cargo test --workspace --doc --locked --offline` — pass, 13 doctests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline`
  — pass, 35 tests with real FFmpeg and ffprobe.
- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline`
  — pass, 6 tests.
- `python3 -m unittest discover -s tests` in `worker/` — pass, 22 tests, which
  include the subprocess tests for stdout isolation, offline application, and
  survival of a hostile frame.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass, no warnings.
- `cargo fmt --all -- --check`, `python3 scripts/check-rust-conventions.py`,
  `taplo fmt --check`, `cargo deny --offline check`, `cargo doc --offline
  --workspace --no-deps --locked`, and `git diff --check` — pass, with no
  warnings.

Criterion 4: unchanged and passing within the runs above —
`t4_e0_every_provisional_seam_has_a_fake`,
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension`,
`t4_e0_walking_skeleton_uses_only_published_seams`.

## Deviations and limitations

- **Real-model qualification was not rerun**, and this is a single-machine
  result with no reference-machine run behind it. Both need the governed model,
  weights, and voice roots that
  `../../../../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps outside Git
  and CI; no synthesis path in this change reaches a model.
  `.github/workflows/qualification.yml` is where an operator runs them.
- The change record's §Impact of the two deliberately incomplete inputs records
  the two unresolved ADR-0001 §12.5 inputs and what they will move.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | T-CORE | Adopted as the E1-S1 controlled-record baseline | 2026-08-27 |
| Engineering owner | engineering owner | Pending review of `E1-S1-INTERFACE-CHANGE-001` | |
| Affected-track reviewers | T-WORKER, T-AUDIO, T-RUNTIME | Deferred to the G1 fake/real parity review | |
