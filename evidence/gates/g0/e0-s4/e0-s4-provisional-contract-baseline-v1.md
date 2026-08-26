# E0-S4 Provisional Contract Baseline Evidence v1

## Scope and decision

This record captures the implemented provisional seam baseline required by
E0-S4. It is engineering evidence, not the G1 interface freeze and not approval
of a production worker, production schema, complete package, or E2 recovery
state machine.

The baseline permits track work against versioned fakes. Stabilization remains
deferred until the real Chatterbox worker and real master-first package path run
the unchanged shared contract scenarios at G1.

## Controlled records

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `9defb41bb9f099325d493c25ad037d334421ca30874d49c1ded3b491a81f1cbf` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `6e68540ad601cab17457eb75781b299e1d80e44dabd41d31bbc968d91adc0e41` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `9daded5d8420b8a2a852e3a2fa64abb1251cfcdeb5a4e59395a2f41e520ece11` |

The test-data manifest checksum above is the E0-S4 fixture-manifest baseline.
Earlier E0-S2 and E0-S3 evidence files and the historical checksums they cite
remain unchanged as provenance of their own reviews; this record supersedes
none of them.

## Implemented seams and evidence

| Contract | Implementation evidence | Fake and shared scenario |
|---|---|---|
| TTS executor | `study_tts_runtime::TtsExecutor`; object-safe boxed future, `&self`, descriptor, capacity, pre-work validation, typed errors | `FakeTtsExecutor`; `run_tts_executor_contract_scenario` |
| Worker frames | Strict request/response enums, required version/request ID, 1 MiB ceiling, baseline and declared minor extension | Executable `fake-ndjson-worker`; registered valid, malformed, incompatible, and extension fixtures |
| Cache publication | `CachePublisher`; staged-audio producer only; filesystem adapter retains validation, containment, checksum, no-replace, lock, sync, and quarantine | `FakeCachePublisher`; `run_cache_contract_scenario` |
| Package writer | `PackageWriter`; pre-work reconciliation and master-first filesystem adapter with real FFmpeg/ffprobe and atomic selection | `FakePackageWriter`; `run_package_writer_contract_scenario` |
| Job state | `ProvisionalJobSnapshot`, `JobRepository`, and ownership guard; atomic strict `job.json` replacement | `InMemoryJobRepository`; `run_job_repository_contract_scenario` |

## Acceptance tests run

On Ubuntu 24.04 under WSL2 on 2026-08-26:

- `cargo test -p study-tts-testkit --test provisional_contracts --locked --offline` — pass, 5 tests.
- `cargo test -p study-tts-testkit --test walking_skeleton --locked --offline` — pass, 34 tests, including real FFmpeg and ffprobe.

The three Delivery Plan acceptance names pass unchanged:

- `t4_e0_every_provisional_seam_has_a_fake`
- `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`
- `t4_e0_walking_skeleton_uses_only_published_seams`

Broader workspace formatting, convention, Clippy, doctest, and complete-suite
results belong in the implementation handoff and do not alter this captured
contract/fixture baseline.
