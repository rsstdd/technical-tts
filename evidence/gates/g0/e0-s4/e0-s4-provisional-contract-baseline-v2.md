# E0-S4 Provisional Contract Baseline Evidence v2

## Scope and decision

This record refreshes E0-S4 evidence after the Rust-review remediation in
`E0-S4-INTERFACE-CHANGE-001`. It supersedes the v1 record only for the current
provisional cache and package Rust APIs. The v1 record and its checksums remain
unchanged as historical provenance.

The cache-publication and package-writer contracts are provisional `1.0`
interfaces, not the G1 freeze. Stabilization remains deferred until the real
Chatterbox worker and real master-first package path pass the same suites.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `b6a61564cd6642b0db8c0bd0435bff047b89082462c8ad76e6922abad51e7bd6` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/WALKING-SKELETON.md` | `229c177a8a815c4130f9973677a1b274d3e6dd63350ffe36df8fbb344012e232` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `6e68540ad601cab17457eb75781b299e1d80e44dabd41d31bbc968d91adc0e41` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `9daded5d8420b8a2a852e3a2fa64abb1251cfcdeb5a4e59395a2f41e520ece11` |

The test-data manifest and fixture bytes did not change during remediation.

## Remediation evidence

- Executor validation now precedes package-tool preflight, workspace creation,
  job ownership, cache resolution, and package reconciliation.
- Same-version descriptors with changed represented semantics are refused.
- `ValidatedCachedArtifact` is opaque outside the runtime crate; package writing
  rechecks plan order and managed-cache containment.
- Package preflight returns a prepared writer, so the package fake runs without
  FFmpeg or ffprobe while the filesystem adapter retains real tool inspection.
- The in-memory job fake retains current state and complete replacement history.

## Verification run

On Ubuntu 24.04 under WSL2 on 2026-08-26:

- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline`
  — pass, 6 tests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline`
  — pass, 34 tests with real FFmpeg and ffprobe.
- `cargo test --workspace --all-targets --locked --offline` — pass, 162 tests.
- `cargo test --workspace --doc --locked --offline` — pass, 5 doctests.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
  — pass.
- `cargo fmt --all -- --check`, the Rust convention checker, and
  `git diff --check` — pass.

The three Delivery Plan acceptance names remain unchanged and pass:

- `t4_e0_every_provisional_seam_has_a_fake`
- `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`
- `t4_e0_walking_skeleton_uses_only_published_seams`
