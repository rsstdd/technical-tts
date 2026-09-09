# E2-S4 Interface Change 003 — Observability crosses the frozen Rust ports

## Identification

- Record ID: `E2-S4-INTERFACE-CHANGE-003`
- Status: **Proposed.** No row in §Approval is signed.
- Contract owners: T-RUNTIME (`job_state`) and T-WORKER (`tts_executor`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CLI, T-RUNTIME, T-WORKER
- Accepted authority: ADR-0001 §14 and `DELIVERY-PLAN.md` E2-S4 require stage events, partial run
  reports, model-load duration, peak RAM, and open handles.

## Version and compatibility

### `job_state` — `1.0` → `2.0`, breaking

`JobRepository` gains two required methods:

- `record_stage` appends a job-correlated `BuildStage` after the stage it names is durably reached;
- `retain_run_report` atomically replaces `jobs/<job-id>/run-report.json` for complete and partial
  attempts.

Every implementation must make a deliberate durability choice, so default no-op methods would be
false compatibility. The filesystem adapter, in-memory fake, interrupting fake, and recording
wrapper all implement both methods, and the shared port tests observe the calls.

This moves the Rust port only. `job.json` remains schema `1.0`; no field or state changes.
`events.ndjson` remains an internal journal at `JOB_EVENT_SCHEMA_VERSION`. Adding the new
`JobEventKind` variants is read-compatible because the envelope and prior variants are unchanged.

### `tts_executor` — `e1.tts-executor.3.0` → `3.1`, compatible extension

`TtsExecutor::process_measurements` is a defaulted method returning `ExecutorMeasurements`. Existing
in-process implementations continue to compile and honestly report `NoWorkerProcess`; the product
worker overrides it with model-load elapsed time and bounded `/proc` samples. The method changes no
request, response, backend descriptor, worker frame, or synthesis behavior.

The minor version is required even though implementations need no edit: the frozen public surface
gained a callable capability. The version is not a synthesis input and moves no cache key.

## Identity and durable-state effect

- Synthesis, cache, verification, and plan identities: unchanged.
- `job.json`, worker protocol, cache entries, and package identity: unchanged by this record.
- New local diagnostics: stage-event variants and `run-report.json`, governed by
  `E2-S4-INTERFACE-CHANGE-001` and `-002`.
- Migration: none. Existing job events remain readable. Implementations of the old Rust port must
  be updated at compile time; there is no durable `job_state` version field to rewrite.

## Verification

- `run_job_repository_contract_scenario` exercises both required repository methods through the
  fake and filesystem implementations.
- `t4_e2_a_build_records_one_event_for_every_stage_it_reached` and
  `t4_e2_a_failed_synthesis_retains_its_segment_and_event` exercise the event path.
- The executor contract runs against the fake and product worker; the default is exercised by the
  in-process fake, and worker measurements are covered by the worker executor tests.

## Approval

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept the two frozen Rust-port moves without changing the accepted G1 charter before signature | |
| Contract owner (T-RUNTIME) | Accept `job_state` `2.0` and the two required repository methods | |
| Contract owner (T-WORKER) | Accept `tts_executor` `3.1` as a compatible defaulted extension | |
| Affected track (T-CLI) | Accept the report and event capabilities exposed through those ports | |
| Engineering owner | Accept that no identity or durable schema moves under this record | |
| Effective version and date | `job_state` `2.0` and `tts_executor` `3.1`, on signature | |
