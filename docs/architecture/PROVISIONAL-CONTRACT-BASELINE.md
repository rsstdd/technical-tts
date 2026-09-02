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
| `tts_executor` / `e1.tts-executor.3.0` | T-WORKER; `study_tts_runtime::TtsExecutor`, `BackendDescriptor`, `SynthesisRequest`, `SynthesisReport`, `BackendError` | Preview orchestration; E1-S3 worker pool | `study_tts_testkit::FakeTtsExecutor`; `run_tts_executor_contract_scenario`; descriptor fixtures under `fixtures/contracts/` | every `BackendDescriptor` field but `contract_version` and `max_text_bytes` affects every synthesis key, as does `SynthesisRequest::voice_conditioning_hash`; backend and worker identities in a report become artifact provenance | E1-S1/E1-S3; frozen at G1 only after the capacity-one Chatterbox adapter passes the suite |
| `worker_frames` / baseline `e1.worker.2.0`; declared optional extension `e1.worker.2.1` | T-WORKER; strict `WorkerRequestFrame` and `WorkerResponseFrame` in `study-tts-runtime/src/worker_protocol.rs` | Future Rust worker client and executable worker | `fake-ndjson-worker`; valid, malformed, incompatible-version, and compatible-extension NDJSON fixtures; `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson`, the committed decisions both the Rust and the Python end must make alike; `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`, `t3_e1_both_protocol_ends_decide_the_committed_cases_alike`, and frame-boundary tests | Executable protocol interpretation is a worker-bundle input and therefore synthesis-affecting | E1-S1/E1-S3; security and real-worker protocol suites must pass before G1 |
| `cache_publication` / `e0.cache-publication.2.0` | T-AUDIO; `CachePublisher`, `CacheResolveRequest`, `StagedAudioProducer`, and opaque `ValidatedCachedArtifact` with read-only accessors | Preview orchestration, PCM assembly, manifest writer | `FakeCachePublisher`; `run_cache_contract_scenario`; deterministic worker WAV | Cache-layout or acceptance changes affect reusable artifacts; speech-affecting acceptance changes require synthesis-identity review | E1-S3, E2-S1, E2-S2, and E4 cache/recovery/prune work; frozen after fake and filesystem adapters pass at G1 |
| `package_writer` / `e0.package-writer.2.0` | T-AUDIO; `PackageWriter::preflight`, `PackagePreflightRequest`, `PreparedPackageWriter`, prepare/write requests, and `PackagePublication` carrying all six artifacts | Preview orchestration and provisional job state | `FakePackageWriter`; `run_package_writer_contract_scenario`, which `t4_e1_the_real_package_writer_passes_the_shared_contract` now also runs `FileSystemPackageWriter` through; real FFmpeg walking-skeleton fixture | Package tool/profile or assembly changes affect package identity; reuse compares the whole recorded argument-profile set *and* the recorded `text_renderer_version`, so a package written before a format existed, or by different transcript/caption/chapter rules, is rebuilt rather than reused | E1-S4 and E2-S3; the real master-first package path passes the shared suite as of E1-S4, and both ends are frozen at G1 |
| `job_state` / `e0.job-state.0.1` | T-CORE/T-RUNTIME; `ProvisionalJobSnapshot` plus `JobRepository` and `JobOwnership` | Preview orchestration and later E2 recovery | `InMemoryJobRepository`; `run_job_repository_contract_scenario`; strict snapshot written as `job.json` | Selected package ID and manifest digest are durable state; this snapshot does not define synthesis or verification keys | E2-S1, E4-S4, and E5 recovery; complete state machine and resume semantics remain deferred |

The executor is object-safe through a boxed `Future`, takes `&self`, validates
before work, and exposes capacity. The synchronous `build_preview` entry point
uses a provisional internal blocking bridge; concurrent dispatch belongs to the
E1 executor and does not require an API change.

The cache port owns the staging destination and accepts only a
`StagedAudioProducer`. Its filesystem adapter retains managed-path containment,
WAV and report validation, checksums, key locking, no-replace publication,
directory synchronization, and collision-free quarantine.

E1-S3 moves the cache-publication contract to `2.0` without changing its Rust
shapes: `resolve` now conditions staged audio before publication, and cache hits
must satisfy the audio-derived conditioning checks recorded in
[`E1-S3-INTERFACE-CHANGE-002.md`](E1-S3-INTERFACE-CHANGE-002.md). The same
`run_cache_contract_scenario` continues to drive `FakeCachePublisher` and
`FileSystemCachePublisher`; `t3_e1_cache_publication_contract_names_the_current_acceptance_semantics`
pins the semantic version shared by those consumers.

E1-S3 also adds a required `containment_failure` field to `BackendError::Timeout`
and **retains** `e1.tts-executor.3.0`.
[`E1-S3-INTERFACE-CHANGE-003.md`](E1-S3-INTERFACE-CHANGE-003.md) records why: a
required field on a type this table names is a **Breaking contract**, and
`ADR-0001-D005` reaches it because `3.0` is the version E1-S3 itself introduced,
so no consumer ever saw the shape being corrected. That is the same test
`E1-S3-INTERFACE-CHANGE-002` applied to `CACHE_SCHEMA_VERSION` and failed, which
is why that constant took the major instead. The record also states that the
classification happened after the field landed rather than before it, which
D005's fifth condition requires.

E1-S3 moved that quarantine to the layout ADR-0001 §12.6 names —
`quarantine/<job-id>/<segment-id>/take-<take>/attempt-<attempt>-<request-id>-<nonce>/` — in
`quarantine_transaction` in `crates/study-tts-runtime/src/cache.rs`, which names §12.6 in return.
It keeps a nonce as a final path element, because an attempt number and a request identity are
both derived from the plan and therefore repeat exactly when a job is resumed or re-run: without
one, a second failure of the same segment and take would land on the first failure's evidence,
which §12.6 forbids. The request identity itself is derived once, by
`PlannedSegment::request_id` in `crates/study-tts-core/src/plan.rs`, and used by both the executor
that puts it on the `synthesize` frame and the cache that puts it in this path — two spellings
would be one rule until somebody edited one of them.

Artifact fields are
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

[`E1-S2-INTERFACE-CHANGE-001.md`](E1-S2-INTERFACE-CHANGE-001.md) records the
first of those landing, and why it moved the executor contract again to
`e1.tts-executor.2.0`. Resolving voice references makes the conditioning hash a
real value in every cache key, and the cache's own identity gate compares the
key it derived against the one an executor reports; an executor can only make
that comparison meaningful if the request tells it which artifact the key names.
The same record covers the lesson document's move to `2.1` and the
`SYNTHESIS_IDENTITY_VERSION` move to `e1-s2-v1`.

[`E1-S2-INTERFACE-CHANGE-002.md`](E1-S2-INTERFACE-CHANGE-002.md) completes the same
story against `DELIVERY-PLAN.md` E1-S2's tasks: the lesson document moves to
`3.1` with ADR-0001 §8.1's `learning_objectives` and `source`, closed `role` and
`style` vocabularies, and §8.2's recall-prompt response interval, and
`PlannedSegment` carries the display text a package writer needs. No synthesis
identity moves with it — the style spellings that reach a key are unchanged, so
every published cache entry stays reachable. The plan document moves to `2.0`:
`display_text` becomes required and `style` narrows to the closed vocabulary,
which §Change classes calls a **Breaking contract**, and `ADR-0001-D005` does
not reach it because `plan 1.0` was E1-S1's version rather than this story's.
That no `plan.json` has been written makes the migration empty, not the major
optional.

The report's context is not advisory. `crates/study-tts-runtime/src/cache.rs`
recomputes the synthesis key from it and refuses publication when that key is
not the one the plan derived (`AudioError::SynthesizerIdentityMismatch`), then
records the reported identities in the entry's `artifact.json` provenance. A
fake whose descriptor and report disagreed would therefore be unable to publish,
which is what keeps the fake honest about the property the real worker must
hold.

## Wire compatibility and rejection

Every project-owned JSON representation uses strict Serde deserialization and
rejects unknown fields and enum values. Every worker correlation identity,
including a cancellation's `active_request_id`, is nonempty ASCII of at most
`MAX_WORKER_REQUEST_ID_BYTES`, and every request carries one recognized
protocol version. The parser enforces `MAX_WORKER_FRAME_BYTES` before JSON
decoding and accepts exactly one NDJSON object per call. The six methods are
initialize, capabilities, health, synthesize, cancel, and shutdown; health
reports readiness and model-resource residency.

Worker `1.1` is the one demonstrated compatible extension: optional
`trace_context`, default absent, with unknown fields still rejected. A `1.0`
frame carrying that field is refused. No other successor version is inferred as
compatible.

A successful `initialized` response is evidence that the worker loaded a
pinned model revision, tokenizer or codec revision, the requested worker
bundle, and at least one voice profile. Its `identities` field is therefore the
closed `WorkerInitializationIdentities` record, not an arbitrary map: all four
categories are required, revisions and hashes use their checked value types,
unknown fields are refused, and `voice_profile_hashes` cannot be empty. The
E1-S1 product worker refused both `initialize` and `synthesize` with a
nonrecoverable `initialization_failed`, because it had loaded none of those
inputs. E1-S3 is where that stopped being true: the shipped worker loads
Chatterbox once per lifetime, renders through it into the staging root it was
assigned, and reports all four identity categories. The executable fake instead owns a loaded synthetic backend, returns its
complete deterministic identities, and consistently reports `ready: true` and
`model_loaded: true`. It refuses initialization when the requested worker-bundle
hash differs from its fixed deterministic identity, and every successful
synthesis reports the same fixed identities as successful initialization.

This correction completes the same pre-G1 `e1.worker.1.0` baseline and `1.1`
extension. It changes a required response shape, which
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes puts
under **Breaking contract** and would normally answer with a major increment
and a migration procedure. Retaining the version instead is a deviation,
approved in
[`../adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md`](../adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md),
which names this document in return and lists the five conditions such a
correction must meet. The current accepted evidence is
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v11.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v11.md),
which records the distinct owner approvals for that decision.

The reasoning it records is that supervisor, fake, worker, tests, and generated
schema moved together, that no released consumer or durable artifact ever saw
the incomplete success frame, and that the baseline remained provisional until
the accepted v11 review; the alternative would publish an `e1.worker.1.0` this
project never intends anyone to speak.

## Amendment rules before G1

These rules mirror `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`,
which owns them; that document's own list of where they are mechanized is the
authority, and this section does not restate it loosely.

Two of the rules below are mechanized, and it is worth being exact about which,
because an earlier version of this section was not. **A declared descriptor
pair** is checked by
`study-tts-core/src/contract.rs::ContractDescriptor::assess_successor` through
`t3_e0_contract_change_requires_version_or_explicit_compatible_extension` —
that test reads `fixtures/contracts/e0-s4-contract-*.json` and nothing else, so
it proves the classifier is right and says nothing about any schema in this
repository. **The required-field surface of the published schemas** is held by
`t3_e1_published_schema_required_fields_match_the_recorded_surface`, which is
what makes a required-field change to a real document in `schemas/` impossible
to land unremarked. Everything else here — migration, rollback, impact report,
owner approval — is applied by people, and a claim that a test enforces it
would be false.

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
