# E0-S4 Provisional Contract Baseline

## Status

E0-S4 is implemented as a provisional engineering baseline. It lets T-CORE,
T-WORKER, T-AUDIO, and T-RUNTIME proceed against public fakes and shared
scenarios without claiming the interfaces are production `1.0` contracts.
There is no migration promise before G1. G1 freezes these interfaces only after
the real Chatterbox worker and the real package path pass the same shared suites.

This record is the documentation side of the contract mirrors in
`crates/study-tts-core/src/contract.rs`,
`crates/study-tts-runtime/src/synthesis.rs`,
`crates/study-tts-runtime/src/worker_protocol.rs`,
`crates/study-tts-runtime/src/cache_port.rs`,
`crates/study-tts-runtime/src/package_port.rs`, and
`crates/study-tts-runtime/src/job_repository.rs`. Those modules name this
record or the governing change-control document in return.

## Baseline inventory

| Contract ID / version | Owner and public representation | Consumers | Fake, fixtures, and unchanged shared suite | Identity effect | Stabilization story |
|---|---|---|---|---|---|
| `tts_executor` / `e1.tts-executor.1.0` | T-WORKER; `study_tts_runtime::TtsExecutor`, `BackendDescriptor`, `SynthesisRequest`, `SynthesisReport`, `BackendError` | Preview orchestration; E1-S3 worker pool | `study_tts_testkit::FakeTtsExecutor`; `run_tts_executor_contract_scenario`; descriptor fixtures under `fixtures/contracts/` | every `BackendDescriptor` field but `contract_version` and `max_text_bytes` affects every synthesis key; backend and worker identities in a report become artifact provenance | E1-S1/E1-S3; frozen at G1 only after the capacity-one Chatterbox adapter passes the suite |
| `worker_frames` / baseline `e0.worker.0.1`; declared optional extension `e0.worker.0.2` | T-WORKER; strict `WorkerRequestFrame` and `WorkerResponseFrame` in `study-tts-runtime/src/worker_protocol.rs` | Future Rust worker client and executable worker | `fake-ndjson-worker`; valid, malformed, incompatible-version, and compatible-extension NDJSON fixtures; `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson`, the committed decisions both the Rust and the Python end must make alike; `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`, `t3_e1_both_protocol_ends_decide_the_committed_cases_alike`, and frame-boundary tests | Executable protocol interpretation is a worker-bundle input and therefore synthesis-affecting | E1-S1/E1-S3; security and real-worker protocol suites must pass before G1 |
| `cache_publication` / `e0.cache-publication.1.0` | T-AUDIO; `CachePublisher`, `CacheResolveRequest`, `StagedAudioProducer`, and opaque `ValidatedCachedArtifact` with read-only accessors | Preview orchestration, PCM assembly, manifest writer | `FakeCachePublisher`; `run_cache_contract_scenario`; deterministic worker WAV | Cache-layout or acceptance changes affect reusable artifacts; speech-affecting acceptance changes require synthesis-identity review | E1-S3, E2-S1, E2-S2, and E4 cache/recovery/prune work; frozen after fake and filesystem adapters pass at G1 |
| `package_writer` / `e0.package-writer.1.0` | T-AUDIO; `PackageWriter::preflight`, `PackagePreflightRequest`, `PreparedPackageWriter`, prepare/write requests, and `PackagePublication` | Preview orchestration and provisional job state | `FakePackageWriter`; `run_package_writer_contract_scenario` without external tools; real FFmpeg walking-skeleton fixture | Package tool/profile or assembly changes affect package identity; they do not silently reuse a package selected under different tool identities | E1-S4 and E2-S3; real master-first package path must pass the shared suite before G1 |
| `job_state` / `e0.job-state.0.1` | T-CORE/T-RUNTIME; `ProvisionalJobSnapshot` plus `JobRepository` and `JobOwnership` | Preview orchestration and later E2 recovery | `InMemoryJobRepository`; `run_job_repository_contract_scenario`; strict snapshot written as `job.json` | Selected package ID and manifest digest are durable state; this snapshot does not define synthesis or verification keys | E2-S1, E4-S4, and E5 recovery; complete state machine and resume semantics remain deferred |

The executor is object-safe through a boxed `Future`, takes `&self`, validates
before work, and exposes capacity. The synchronous `build_preview` entry point
uses a provisional internal blocking bridge; concurrent dispatch belongs to the
E1 executor and does not require an API change.

The cache port owns the staging destination and accepts only a
`StagedAudioProducer`. Its filesystem adapter retains managed-path containment,
WAV and report validation, checksums, key locking, no-replace publication,
directory synchronization, and collision-free quarantine. Artifact fields are
not publicly constructible or mutable, and the real package adapter rechecks
plan order and cache containment before assembly. The package adapter owns tool
inspection: preflight returns a prepared writer, while its fake performs no
external process work. The package path retains Rust PCM assembly, canonical
master first, M4A derived from the master, real FFmpeg/ffprobe validation,
manifest checksums, the publication journal, and atomic immutable selection.
The job contract intentionally records only plan, stage, ownership, and
selected-package identity; it does not claim E2 recovery.

The audit remediation in
[`E0-S4-INTERFACE-CHANGE-001.md`](E0-S4-INTERFACE-CHANGE-001.md) records why the
cache and package contracts moved from provisional `0.1` to `1.0` before G1.

[`E1-S1-INTERFACE-CHANGE-001.md`](E1-S1-INTERFACE-CHANGE-001.md) records why the
executor contract moved to `e1.tts-executor.1.0`: `BackendDescriptor` replaced
one opaque `synthesis_identity` string with the complete ADR-0001 §12.5
synthesis-key input set, then gained a declared language set, and
`SynthesisReport` began carrying the `SynthesisContext` the executor actually
used. That record also names the two §12.5 inputs — the voice-conditioning
artifact hash and the backend generation parameters — that are present in the
identity but not resolved to real values until E1-S2 and E1-S3 respectively.

The report's context is not advisory. `crates/study-tts-runtime/src/cache.rs`
recomputes the synthesis key from it and refuses publication when that key is
not the one the plan derived (`AudioError::SynthesizerIdentityMismatch`), then
records the reported identities in the entry's `artifact.json` provenance. A
fake whose descriptor and report disagreed would therefore be unable to publish,
which is what keeps the fake honest about the property the real worker must
hold.

## Wire compatibility and rejection

Every project-owned JSON representation uses strict Serde deserialization and
rejects unknown fields and enum values. Every worker request requires a nonempty
request ID and one recognized protocol version. The parser enforces
`MAX_WORKER_FRAME_BYTES` before JSON decoding and accepts exactly one NDJSON
object per call.

Worker `0.2` is the one demonstrated compatible extension: optional
`trace_context`, default absent, with unknown fields still rejected. A `0.1`
frame carrying that field is refused. No other successor version is inferred as
compatible.

## Amendment rules before G1

These rules mirror
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` and are enforced by
`study-tts-core/src/contract.rs::ContractDescriptor::assess_successor` plus
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension`:

- An unchanged contract retains its version.
- A diagnostic-only compatible patch retains the version and changes no
  durable bytes or behavior.
- An optional compatible extension increments the minor version, states the
  default used by older consumers, and declares unknown-field behavior.
- A required field, semantic change, or incompatible frame change increments
  the major version and supplies migration and rollback.
- An authority or architectural boundary change requires an accepted ADR and a
  major contract version.
- Fakes, fixtures, and shared suites change before consumers; all mapped tests
  rerun before approval.

Use `docs/templates/INTERFACE-CHANGE-TEMPLATE.md` for every amendment.

## Affected-test mapping

| Change | Stories and required validation |
|---|---|
| Executor or worker frames | E1-S1 and E1-S3; shared executor scenario, strict frame fixtures, malformed/size/version tests, fake-worker process tests, worker/security suites, walking skeleton |
| Cache publication | E1-S3, E2-S1, E2-S2, and E4; shared cache scenario, corruption, containment, quarantine, concurrency, recovery, prune-root, and walking-skeleton suites |
| Package writer | E1-S4 and E2-S3; shared package scenario, PCM arithmetic, real FFmpeg/ffprobe, checksum, atomic-selection, failure-preservation, and walking-skeleton suites |
| Job repository/state | E2-S1, E4-S4, and E5 recovery; shared repository scenario, strict record, ownership, write-boundary fault injection, restart/recovery, and walking-skeleton suites |

The permanent E0 acceptance tests are copied exactly from `DELIVERY-PLAN.md`:

- `t4_e0_every_provisional_seam_has_a_fake`
- `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`
- `t4_e0_walking_skeleton_uses_only_published_seams`
