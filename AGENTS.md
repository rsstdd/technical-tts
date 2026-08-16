# AGENTS.md

## Repository

A Rust workspace for a local-first WSL2 CLI that converts reviewed technical lessons into long-form study-guide audio through a resource-governed Chatterbox worker pool, an in-process Rust ASR verifier, and FFmpeg.

These instructions apply to the entire repository. A more specific `AGENTS.md` within a subdirectory governs work in that directory.

The repository has completed the E0-S0 tested walking skeleton. The four-crate Rust workspace now loads and validates a two-segment lesson fixture, derives a provisional render plan, uses a deterministic fake tone synthesizer, validates and reuses cached WAVs, assembles PCM and silence in Rust, invokes real FFmpeg for M4A, and writes a minimal private-preview manifest. Product CLI commands, production schemas, Chatterbox, hardened recovery, and the complete output package remain unimplemented. `docs/adr/ADR-0001-production-rust-study-guide-tts.md` is authoritative for architecture, scope, and production invariants. `DELIVERY-PLAN.md` is authoritative for milestone scope, backlog order, tests, evidence, and sign-offs. Do not describe planned commands, schemas, worker behavior, or audio behavior as implemented before they are present and verified.

## Rules

- Read the relevant implementation, tests, and documentation before editing.
- Read `docs/adr/ADR-0001-production-rust-study-guide-tts.md` before making an architectural or cross-cutting change.
- Follow established repository patterns unless the task explicitly changes them.
- Make the smallest coherent change that satisfies the request.
- Preserve unrelated changes. Do not reformat, rename, or reorganize unrelated code.
- Check for an appropriate existing extension point before adding a new abstraction.
- Create a new file when it gives the code clearer ownership or follows the repository structure.
- For behavioral changes, add or update tests proportional to the risk.
- Document non-obvious constraints, coupling, and decisions. Do not write comments that merely restate the code.
- Prefer fixing the underlying cause. If a compatibility workaround is necessary, explain why.
- Do not hide incomplete behavior behind placeholders, silent fallbacks, or stubbed implementations.
- Do not claim that a check passed unless it was run successfully.
- If verification cannot be run, state what remains unverified and why.
- Use Ubuntu 24.04 under WSL2 for development and initial runtime verification.
- Keep the repository, Rust target directory, Python environment, model files, caches, fixtures, and working data in the WSL2 Linux filesystem, not under `/mnt/c`.

## Autonomy and approval

For requests to explain, review, diagnose, or plan:

- Inspect the relevant files and report the result.
- Do not modify files unless the request also asks for changes.

For requests to build, change, or fix:

- Make the requested local changes.
- Run relevant, non-destructive verification without asking first.
- Do not pause for approval between routine implementation steps.

Ask before:

- deleting or overwriting user data;
- deleting cache or quarantined artifacts, including through a prune command without `--dry-run`;
- discarding uncommitted changes;
- changing branches or rewriting Git history;
- committing, pushing, merging, or opening a pull request;
- deploying, publishing, or signing a release;
- downloading or executing unpinned model artifacts;
- enabling network access during rendering;
- adding a voice clone or changing its consent scope;
- introducing a paid dependency or service;
- making a material expansion beyond the requested scope.

## Architectural invariants

These constraints must remain true unless the task explicitly updates the controlling ADR:

- **Rust owns durable decisions.** Lesson validation, planning, cache identity, job state, recovery, audio validation, manifests, and CLI behavior belong to the Rust application.
- **One production TTS backend.** Use the standard Chatterbox model. Do not add Chatterbox Nano, Kokoro-82M ONNX, Qwen3-TTS, Chatterbox Turbo, Dia, or a backend collection without measured evidence and a new decision record.
- **Replaceable worker boundary.** Model inference runs in a configurable pool of persistent child processes behind the versioned newline-delimited JSON protocol. Each process accepts one request at a time; the asynchronous Rust executor leases individually synchronized clients. Model-specific fields must not enter the lesson or planning domain.
- **Resource-governed concurrency.** Worker-pool size and per-worker threads must satisfy both measured aggregate-RAM and WSL-visible physical-core budgets. Explicitly limit PyTorch and native numerical-library threads. Drain and unload the TTS pool before ASR.
- **Offline rendering.** Normal rendering performs no network access after installation. A future hosted or remote path requires an explicit ADR.
- **Post-render verification.** Validate the lesson before starting workers. Structurally validate and atomically cache synthesized audio before ASR. After all selected audio is rendered, verify cached segments with the pinned in-process `whisper-rs` stack. Missing or stale verification must never invoke Chatterbox.
- **Filesystem is authoritative.** Atomic JSON documents, checksummed artifacts, and per-job locking own version 1 state. Do not introduce SQLite, a database queue, or distributed coordination without satisfying an ADR extension threshold.
- **Separate identities.** Every speech-affecting input belongs in the synthesis key, including the mechanically derived worker-bundle hash. ASR dependencies, decoder controls, input conversion, expected patterns, normalizer, and thresholds belong in a separate verification key. Display-only metadata belongs in neither.
- **Explicit take selection.** A versioned `<lesson-stem>.takes.json` is the production selection source of truth, including for take zero. Reject stale base keys, propagate selected keys and checksums into plan and manifest, and treat accepted takes and published manifests as prune roots.
- **No blind cache trust.** Verify the artifact manifest, checksum, WAV structure, sample format, and quality checks before reusing cached audio.
- **Managed-path containment.** Canonicalize all paths, reject traversal and symlink escape, and confine worker writes to the assigned staging root.
- **No shell command construction.** Invoke workers, FFmpeg, and ffprobe with an executable plus discrete checked arguments. ASR is an in-process Rust dependency, not an external CLI.
- **Canonical master first.** Assemble and validate one lossless master WAV. Derive M4A and MP3 independently from that master, never from another lossy export.
- **Reviewed text only.** Never send raw Markdown directly to TTS, add unsupported technical claims during deterministic compilation, or promote an unreviewed lesson to a production build.
- **Bounded failure behavior.** Retries, timeouts, message sizes, segment sizes, disk use, and process lifetime must be bounded. Preserve valid completed work and keep terminal failures visible.
- **Voice consent is data integrity.** A cloned voice requires a consent record, reference checksum, permitted-use scope, and build audit event. Public-figure cloning is prohibited.

## Repository structure

The following is the approved target structure. Create it incrementally; absent paths are planned, not evidence of missing work unless the active phase requires them.

| Path | Responsibility | Notes |
|---|---|---|
| `Cargo.toml` | Rust workspace definition and shared dependency policy | Commit `Cargo.lock`; keep the initial workspace small. |
| `crates/study-tts-cli/` | Executable, commands, configuration, and user diagnostics | Keep orchestration thin; support human-readable and JSON output. |
| `crates/study-tts-core/` | Lesson types, normalization, render planning, and cache keys | Must not depend on Python, a model SDK, CUDA, or FFmpeg bindings. |
| `crates/study-tts-runtime/` | Filesystem state, worker pool, in-process ASR, PCM handling, process control, and FFmpeg adapter | Own containment, atomic publication, recovery, verification, and external-process boundaries. |
| `crates/study-tts-testkit/` | Fixtures, fake worker, fault injection, and audio test helpers | Production crates must not depend on it outside tests. |
| `worker/` | Selected backend adapter and locked Python environment | One production backend only; stdout is protocol-only and stderr carries diagnostics. |
| `schemas/` | Versioned lesson, worker, takes, verification, job, and manifest JSON Schemas | Checked-in schemas must remain aligned with Rust types and fixtures. |
| `fixtures/` | Reviewed lessons, pronunciation cases, and deterministic audio fixtures | Keep fixtures small, licensed, non-sensitive, and stable. |
| `docs/adr/` | Durable architecture decisions after repository bootstrap | ADRs supersede earlier decisions explicitly; do not silently contradict them. |
| `docs/operations/` | Installation, diagnostics, recovery, pruning, rollback, and release runbooks | Commands must be exercised on clean Ubuntu 24.04 under WSL2. |
| `data/` | Local models, voices, cache, jobs, staging, and generated outputs | Runtime data; do not commit it. Published output and quarantine require explicit deletion. |
| `target/`, `.venv/` | Generated Rust and Python build environments | Do not edit or commit manually. |

## Routing table

| When looking for | Start here | Source of truth |
|---|---|---|
| Architecture, scope, and boundaries | `docs/adr/ADR-0001-production-rust-study-guide-tts.md` | Accepted ADRs, with the newest explicit superseding decision controlling |
| Delivery sequence, milestones, and story acceptance | `DELIVERY-PLAN.md` | Approved backlog and its traceability to ADR requirements |
| Complete documentation routing | `docs/INDEX.md` | Index routes to the controlling accepted decision or execution record |
| Governance, approvals, and gate ownership | `docs/governance/PROJECT-EXECUTION-CHARTER.md` | Accepted ADR and Delivery Plan remain authoritative when a conflict exists |
| Capability ownership and approval | `docs/governance/MILESTONE-CAPABILITY-MATRIX.md` | One delivery owner, approver, validation route, and first required gate per capability |
| Decisions, failures, work, and artifacts | `docs/governance/ROUTING-TABLES.md` | Named owner, record, deadline, and blocking rule |
| Requirement traceability | `docs/governance/TRACEABILITY-MATRIX.md` | ADR requirement to delivering story, validation, and gate |
| Risks, questions, and descope | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | Ratified decisions only; proposed entries do not authorize scope changes |
| Rights, data, and artifact handling | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | Individual approved rights records and ADR-0004 |
| Interface freeze and contract changes | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | G1 freeze record and approved versioned changes |
| GitHub backlog operation | `docs/governance/GITHUB-PROJECT-PLAYBOOK.md` | GitHub Project status plus repository acceptance evidence |
| Chatterbox qualification | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | Proposed until pinned revisions and G0 measurements are approved |
| Audio formats and loudness | `docs/adr/ADR-0003-production-audio-quality-profile.md` | Proposed until thresholds, codecs, and frozen references are calibrated and approved |
| Voice consent and watermark policy | `docs/adr/ADR-0004-voice-content-and-retention-policy.md` | Proposed until individual rights records and retention decisions are approved |
| ASR verification | `docs/adr/ADR-0005-asr-calibration-and-release-control.md` | Proposed until exact identities, corpus, decoder, patterns, and gates are measured and approved |
| Domain model and render planning | `crates/study-tts-core/` | Rust types plus checked-in schemas |
| Worker contract | `schemas/worker-v1.schema.json` and `worker/` | Versioned protocol schema and shared contract tests |
| Configuration | `crates/study-tts-cli/` | Parsed configuration types and documented precedence |
| Job state, cache, and recovery | `crates/study-tts-runtime/` | Runtime implementation, manifest schemas, and recovery tests |
| External process safety | `crates/study-tts-runtime/` | Worker-pool and FFmpeg adapters plus containment tests; ASR uses the in-process verifier |
| Testing patterns | `crates/study-tts-testkit/` and colocated tests | Representative fake-worker, property, contract, and recovery tests |
| TDD and test tiers | `docs/testing/TEST-STRATEGY.md` | Delivery Plan named tests and tier policy |
| Qualification and evidence | `docs/testing/EVIDENCE-AND-QUALIFICATION.md` | Immutable evidence reports and governed raw artifacts |
| Test data provenance | `docs/testing/TEST-DATA-MANIFEST.md` | Stable IDs, checksums, rights, sensitivity, retention, and owner |
| Threat model | `docs/security/THREAT-MODEL.md` | Trust boundaries, controls, validation, and residual risks |
| Operations and diagnostics | `docs/operations/` | Exercised runbooks and `study-tts doctor` behavior |
| Production diagnostics | Local `events.ndjson`, `job.json`, `manifest.json`, and `quality-report.json` | Redacted local artifacts; do not upload source text or voice-reference paths by default |

## Commands

Run commands inside Ubuntu 24.04 under WSL2 from the repository root. Check `Cargo.toml`, worker lock files, repository scripts, and CI before changing or extending this table.

The current repository contains the E0-S0 library walking skeleton and a non-product status executable. Product commands below become authoritative only when their referenced behavior, files, and targets exist.

| Purpose | Command |
|---|---|
| Verify required system tools | `gcc --version && cmake --version && python3 --version && ffmpeg -version && ffprobe -version` |
| Fetch Rust dependencies | `cargo fetch --locked` |
| Build workspace | `cargo build --workspace --locked` |
| Type-check workspace | `cargo check --workspace --all-targets --locked` |
| Run one Rust test | `cargo test --workspace <test_name>` |
| Run Rust tests | `cargo test --workspace --all-targets --locked` |
| Lint | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| Check formatting | `cargo fmt --all -- --check` |
| Apply formatting | `cargo fmt --all` |
| Inspect CLI commands | `cargo run -p study-tts-cli -- --help` |
| Diagnose local environment | `cargo run -p study-tts-cli -- doctor` |
| Validate a lesson | `cargo run -p study-tts-cli -- lesson validate <lesson.json>` |
| Run worker tests | Use the command defined by `worker/pyproject.toml` and its lockfile; do not infer a Python test runner before it is configured. |
| Validate schemas and generated files | Use the checked-in repository task or CI command once created; do not replace it with an ad hoc generator. |

Do not invent command options. If a command is uncertain or fails because the workspace has not reached that implementation phase, inspect the relevant manifest or CI configuration and report the exact gap.

## Verification

Choose verification according to the change:

| Change | Required verification |
|---|---|
| Documentation only | Review rendered Markdown, links, paths, commands, and consistency with accepted ADRs |
| Core types or deterministic logic | Targeted unit and property tests, schema consistency, `cargo check`, formatting, and Clippy |
| Worker protocol or backend | Shared contract tests, malformed-message tests, cancellation and timeout tests, offline check, and short real-backend render on named hardware |
| Worker pool or CPU governance | Actual parallel dispatch through the `&self` executor, process isolation, thread-limit assertions, RAM/core oversubscription rejection, and single-worker RTF qualification |
| ASR verification | Exact pinned-stack and decoder assertions, ASR-only invalidation, expected-pattern promotion review, seeded defect-class gates, repeated-run stability, and segment-order invariance |
| Job state, cache, takes, or recovery | Targeted tests plus fault injection at each write boundary and pipeline state, checksum validation, stale-take rejection, prune protection, restart, and one-segment invalidation |
| PCM, FFmpeg, or export behavior | Edge padding/ramp and Rust assembly tests, supported-FFmpeg integration, ffprobe assertions, frozen-loudness-reference checks, deterministic timeline checks, and playback of representative exports |
| User-visible CLI behavior | Relevant tests plus an actual CLI exercise in WSL2, including `--json` and exit-code behavior |
| Security boundary | Traversal, symlink escape, command-injection, oversized-input, hostile-worker, redaction, and child-process cleanup tests plus human review |
| Model, voice, or synthesis parameter | Cache-key compatibility review, short contract render, blind quality sample, license check, and consent check where applicable |
| Release configuration | Clean WSL2 installation, full CI, `doctor`, 45–60 minute soak test, recovery demonstration, SBOM/license review, signing, checksum, and rollback exercise |

Use the fastest relevant check first. Run broader checks after basic verification succeeds. Pull-request CI must not download model weights or require a GPU.

For changes that affect spoken output, automated checks are necessary but insufficient. Listen to the changed segment in context and state explicitly if human listening remains unverified.

## Security and data

- Treat Markdown, JSON, worker responses, model artifacts, filenames, voice references, cache entries, and media metadata as untrusted input.
- Enforce file-size, nesting, segment-count, message-size, duration, retry, timeout, and disk limits at their owning boundaries.
- Resolve and validate managed paths before access. Reject absolute paths, traversal, symlink escape, and Windows-mount escape where the operation requires containment.
- Pass external-process arguments directly. Never interpolate user-controlled data into a shell command.
- Pin Rust, Python, model, tokenizer, codec, voice, and ASR revisions as applicable. Derive the worker-bundle hash from executable worker inputs. Verify model, voice, cache, verification, and published-output checksums.
- Prefer safe tensor formats. Do not load untrusted pickle-style model files when a safe format is available.
- Keep rendering offline and test that the worker cannot make network requests.
- Do not add credentials, tokens, personal data, source content, production audio, or voice-reference files to source, fixtures, logs, examples, or diagnostic bundles.
- Log identifiers, hashes, timings, states, and error classes. Do not log full source text, spoken text, or raw voice-reference paths by default.
- Keep test fixtures and mocks in test or development paths. Use only material with a clear license and no sensitive content.
- Do not weaken validation, containment, checksum, consent, offline, dependency, or license controls to make a test pass.
- Require human review for voice cloning, consent changes, executable model formats, dependency-license changes, release signing, and any externally visible service or permission.

## Coding conventions

- **Naming:** Use the `study-tts-*` crate prefix, stable lowercase kebab-case CLI names, versioned schemas such as `lesson-v1`, and stable segment IDs that do not depend on mutable display text.
- **Errors:** Use typed internal errors and source-aware user diagnostics. Classify invalid input, missing dependency, incompatible environment, worker failure, audio-quality failure, cancellation, resource exhaustion, integrity failure, and internal error distinctly. Never silently fall back to another model or device.
- **Logging:** Use structured `tracing` events with `job_id`, stage, segment ID, attempt, worker/model identity, duration, and error class where applicable. Keep terminal output concise and reserve worker stdout for protocol messages.
- **Types and schemas:** Represent durable lesson, worker, takes, verification, job, and manifest data with explicit versioned Rust types and JSON Schemas. Reject unknown or incompatible versions at the boundary unless a tested migration exists.
- **Async and processes:** Use the object-safe `TtsExecutor` with `&self`; bound queues and concurrent work. The pool owns individually synchronized worker clients. The parent owns every worker process tree, deadline, cancellation, restart budget, thread budget, and cleanup. Unload the pool before starting ASR.
- **Verification:** Use the pinned in-process `whisper-rs` stack with one model context and an independent decoder state per segment. Compare against the approved expected-ASR lattice. Never learn patterns automatically or let ASR mutate approved text.
- **Filesystem writes:** Stage, flush, validate, and atomically publish authoritative state and cache artifacts. Store verification evidence separately. Use collision-free quarantine paths; do not overwrite or delete invalid artifacts automatically.
- **Audio edges and loudness:** Rust owns silence insertion, PCM concatenation, edge padding, transition ramps, and float-range validation. Use committed frozen voice/style loudness references; unrelated edits must not change unrelated gain decisions.
- **Dependencies:** Add a dependency only when it removes more risk than it introduces. Pin through `Cargo.lock` or the worker lockfile, review licenses and advisories, and avoid duplicate libraries for the same narrow concern.
- **Generated code and schemas:** Produce generated artifacts through a checked-in deterministic command. CI must fail when generated schemas drift from authoritative Rust types.
- **Compatibility:** Support Ubuntu 24.04 under WSL2 first while retaining native Linux compatibility. Native Windows packaging, GPU acceleration, remote execution, and multi-user operation are deferred.
- **Unsafe Rust:** Avoid `unsafe`. Any required use needs a documented invariant, focused tests, and human review.

## Authoritative guides

- Architecture: `docs/adr/ADR-0001-production-rust-study-guide-tts.md`
- Delivery: `DELIVERY-PLAN.md`
- Documentation routing: `docs/INDEX.md`
- Project execution: `docs/governance/PROJECT-EXECUTION-CHARTER.md`
- Contribution workflow: `CONTRIBUTING.md` and `.github/PULL_REQUEST_TEMPLATE.md`
- Code style: repository `rustfmt.toml` and Clippy configuration when created
- Python worker policy: `worker/pyproject.toml` and its lockfile when created
- Operations: `docs/operations/`

## Completion criteria

Before reporting completion:

- Confirm that the requested behavior is implemented rather than represented by a placeholder.
- Review the final diff for accidental or unrelated changes.
- Run the relevant tests and checks from WSL2.
- Exercise changed CLI or audio behavior through the actual interface when possible.
- For speech changes, listen to the affected output in context or state the exact listening check that remains.
- Confirm that no secrets, source text, voice references, temporary diagnostics, generated runtime data, or unlicensed artifacts entered the repository.
- Confirm that no valid cache, quarantine, job, or published output was deleted without approval.
- Update durable documentation when behavior, architecture, setup, schema, cache identity, recovery, or an operator workflow changed.
- Add or supersede an ADR for a change to an architectural invariant or deferred capability.
- Summarize files changed, verification performed, model or media tests performed, and anything that remains unverified.

## Scoped instructions

Keep specialist procedures near the files they govern rather than expanding this root document. Add nested instruction files when those paths exist and local rules become concrete:

```text
AGENTS.md
├── crates/AGENTS.md
│   └── Rust boundaries, errors, schemas, and process adapters
├── worker/AGENTS.md
│   └── Python locking, model installation, protocol, and offline controls
├── fixtures/AGENTS.md
│   └── Fixture licensing, determinism, size, and review rules
└── docs/AGENTS.md
    └── ADR lifecycle, operations runbooks, and documentation verification
```

Do not duplicate a root instruction in a nested file unless the local rule changes or clarifies it.
