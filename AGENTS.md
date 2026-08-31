# AGENTS.md

A Rust workspace for a local-first WSL2 CLI that converts reviewed technical lessons into long-form study-guide audio via a resource-governed Chatterbox worker pool, an in-process Rust ASR verifier, and FFmpeg.

These instructions apply to the whole repository. A nested `AGENTS.md` in a subdirectory overrides only for that tree.

**State.** E0-S0 walking skeleton is complete: two-segment fixture load/validate, provisional render plan, deterministic fake-tone synthesizer, cached-WAV reuse, Rust PCM+silence assembly, real FFmpeg M4A, minimal private-preview manifest. E0-S3 qualification is complete under accepted ADR-0002's constrained-development performance waiver; the disposable Chatterbox spike is evidence, not a product worker. E0-S4 provisional seams and the E1-S1 contract baseline are complete: seven published versioned schemas under `schemas/` generated from the Rust types, canonical serialization, the BLAKE3 synthesis and verification identities, the mechanically derived worker-bundle identity with the environment precondition ADR-0001-D006 now carries (superseding ADR-0001-D004, whose check stands while its cost band is retaken on CPU time), the locked Python worker environment, an executable protocol fake, and split pull-request and reference-machine workflows. E1-S2 is **implemented, with its interface change accepted and its story evidence still open**: the lesson document is at `3.1` with required `speakers`, ADR-0001 §8.1's `learning_objectives` and `source` records, closed `role`, `style`, and `review_status` vocabularies each with their own absent and unrecognized refusals, §13.2's recall-prompt response interval enforced at both ends, a `speakers` object that binds one name twice refused rather than resolved by keeping the last binding, a build that resolves the voice profile of every speaker a segment names before planning so the ADR-0001 §12.5 conditioning input is a real value in every cache key — a *declared* speaker no segment uses is deliberately not resolved, because nothing is synthesized from it, and `evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md` §Deviations records why no `UnusedSpeaker` refusal was added — a render plan that carries display text for the package writer without letting it reach a cache key, and every refusal the lesson module raises arriving as a `LessonDiagnostic` naming its document, segment, and JSON Pointer. `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md` records the second half of that and is **`Accepted`, signed 2026-08-30**, covering six rounds of correction including the render plan's move to `2.0`, six new vocabulary refusals, the repeated-speaker refusal, and a recorded source content hash that is not a digest refused as its own invariant rather than as a shape error; its §Approval records the decision each role made and the date it was signed. Its provenance reconciliation under `evidence/gates/g1/e1-s2/` is `e1-s2-evidence-provenance-reconciliation-v3`, `Accepted` and superseding v2, because the suppression it grants takes effect only while accepted. The **story record stays `Proposed` by design until G1 runs**, so the story itself is not accepted: an interface record accepts the contract change once its rows are signed, and never accepts the story. E1-S3 is **in progress**: `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` is `Accepted`, signed 2026-08-30, moving the plan document to `3.0`, `TtsExecutor` to `e1.tts-executor.3.0`, and the worker frames to `e1.worker.2.0` with the published schema at `schemas/worker-protocol-v2.schema.json`; a capacity-one `WorkerTtsExecutor` drives a persistent child over that protocol, and the cache's identity gate now compares the conditioning artifact the worker reports rather than one echoed from the request. `docs/architecture/E1-S3-INTERFACE-CHANGE-002.md` is `Accepted`, signed 2026-08-31, moving `CACHE_SCHEMA_VERSION` to `2.0` with ADR-0001 §13.4's padding and ramp counts recorded on every entry and checked on reuse, which invalidated every cache key and plan hash. The product worker now **loads the real Chatterbox backend and renders**: one model load per lifetime, output contained inside the assigned staging root by a descriptor held across generation, and edge conditioning applied and recorded. Three gates run before a worker can start, all inside `WorkerConfiguration::for_bundle`, the only constructor that is *given* the governed roots — the bundle identity is derived and its interpreter proved, the pinned model revision's four artifacts are hashed against `model_gate`'s declared SHA-256 digests, and every voice profile beneath the governed root passes the consent, rights, scope, and checksum gate, because the worker deserializes all of them during `initialize` rather than only the one a request names, and a profile directory whose name is not UTF-8 refuses the whole root rather than being skipped, because Python reads that name through `surrogateescape` and would still load it. The other constructor, `for_protocol_fake`, refuses an environment naming either governed-root variable, so it cannot be pointed at the real worker over a root nothing gated; that pair, not a sole constructor, is what makes the gating unavoidable. `shutdown` observes a voluntary exit without reaping it, so the process group is still signallable and a descendant started after the last enumeration is contained rather than left behind; a descendant that calls `setsid()` in that window is reached by neither half, which is an owner-approved deviation from ADR-0001 §10.3 under `ADR-0001-D008` and closes in E5-S4, not a property this build claims. Product CLI commands, hardened recovery, and the complete output package are **not** implemented. The **story record stays `Proposed` by design until G1**, as E1-S2's does. Do not describe planned commands, schemas, workers, or audio behavior as present until they exist and are verified.

**Sources of truth (conflict order).** Newest accepted ADR that explicitly supersedes → `docs/adr/ADR-0001-production-rust-study-guide-tts.md` (architecture, scope, production invariants) → `DELIVERY-PLAN.md` (milestones, backlog, tests, evidence, sign-off) → this file → nested `AGENTS.md`. Proposed ADRs do not authorize scope.

## Operating rules

- Read the relevant implementation, tests, and docs before editing. Read ADR-0001 before any architectural or cross-cutting change.
- Smallest coherent change. Follow established patterns unless the task changes them. Preserve unrelated code; do not reformat, rename, or reorganize it.
- Prefer an existing extension point over a new abstraction. New file only when it clarifies ownership or matches repo structure.
- Behavioral change ⇒ tests proportional to risk. Document non-obvious constraints and coupling; no comments that restate code.
- Fix the cause. If a workaround is required, say why. No placeholders, silent fallbacks, or stubbed "complete" behavior.
- Do not claim a check passed unless it ran successfully. If verification cannot run, state what is unverified and why.
- Dev/runtime: Ubuntu 24.04 under WSL2. Keep repo, `target/`, Python env, models, caches, fixtures, and working data on the Linux filesystem — never under `/mnt/c`.

### Rust style (2024 edition)

Source of truth: The Rust Style Guide, 2024 style edition. Upstream wins on conflict.

- Emit rustfmt-default code. After edits run `cargo fmt` (or `rustfmt --edition 2024`); before done run `cargo fmt --check` when a shell exists.
- No `rustfmt.toml` options, `#[rustfmt::skip]`, or hand alignment unless asked.
- 4-space indent, no tabs outside strings, max 100 cols, no trailing whitespace, 0–1 blank line between items/statements.
- Block indent, never visual indent. Trailing comma on every broken comma-list; none on single-line lists.
- Version-sort where sorting is required (`u8`, `u16`, `u32`, `u64`, `u128`, `usize`).
- One attribute per line; exactly one `#[derive(...)]` per item, order preserved.
- Imports: version-sort within groups; do not merge or reorder groups; attribute starts a new group. Nested import ⇒ multi-line form.
- Names: UpperCamelCase types/variants; snake_case fields/fns/modules; SCREAMING_SNAKE_CASE consts/immutable statics.

## Autonomy

**Explain / review / diagnose / plan:** inspect and report. Do not modify files unless the request also asks for changes.

**Build / change / fix:** make the local change. Run relevant non-destructive verification without asking. Do not pause between routine implementation steps.

**Ask first:**

- delete or overwrite user data; delete cache or quarantine (including prune without `--dry-run`); discard uncommitted work
- change branches or rewrite history; commit, push, merge, or open a PR
- deploy, publish, or sign a release
- download or execute unpinned model artifacts; enable network during rendering
- add a voice clone or change consent scope
- introduce a paid dependency or service
- expand materially beyond the requested scope

## Architectural invariants

Hold unless the task explicitly updates the controlling ADR.

- **Rust owns durable decisions.** Validation, planning, cache identity, job state, recovery, audio validation, manifests, CLI — Rust application, not the worker.
- **One production TTS backend.** Standard Chatterbox only. No Nano, Kokoro-82M ONNX, Qwen3-TTS, Turbo, Dia, or a backend collection without measured evidence and a new ADR.
- **Replaceable worker boundary.** Inference in a configurable pool of persistent children behind the versioned NDJSON protocol. One request per process; async Rust executor leases individually synchronized clients. Model-specific fields stay out of lesson/planning domain.
- **Resource-governed concurrency.** Pool size and per-worker threads must fit measured aggregate-RAM and WSL-visible physical-core budgets. Cap PyTorch and native numerical threads. Drain and unload the TTS pool before ASR.
- **Offline rendering.** No network after install. Hosted/remote path needs an explicit ADR.
- **Post-render verification.** Validate the lesson before workers. Structurally validate and atomically cache synthesized audio before ASR. After selected audio is rendered, verify cached segments with the pinned in-process `whisper-rs` stack. Missing or stale verification must never invoke Chatterbox.
- **Filesystem is authoritative.** Atomic JSON, checksummed artifacts, per-job locking own v1 state. No SQLite, DB queue, or distributed coordination without an ADR extension threshold.
- **Separate identities.** Every speech-affecting input belongs in the synthesis key (including mechanically derived worker-bundle hash). ASR deps, decoder controls, conversion, expected patterns, normalizer, thresholds belong in a verification key. Display-only metadata belongs in neither.
- **Explicit take selection.** Versioned `<lesson-stem>.takes.json` is the production selection source, including take zero. Reject stale base keys; propagate selected keys and checksums into plan and manifest; accepted takes and published manifests are prune roots.
- **No blind cache trust.** Verify artifact manifest, checksum, WAV structure, sample format, and quality checks before reuse.
- **Managed-path containment.** Canonicalize all paths; reject traversal and symlink escape; confine worker writes to the assigned staging root.
- **No shell command construction.** Workers, FFmpeg, ffprobe: executable + discrete checked arguments. ASR is in-process Rust, not a CLI.
- **Canonical master first.** Assemble and validate one lossless master WAV. Derive M4A and MP3 independently from that master, never from another lossy export.
- **Reviewed text only.** Never send raw Markdown to TTS, invent unsupported technical claims in deterministic compilation, or promote an unreviewed lesson to production.
- **Bounded failure.** Bound retries, timeouts, message/segment sizes, disk, process lifetime. Preserve valid completed work; keep terminal failures visible.
- **Voice consent is data integrity.** Clone requires consent record, reference checksum, permitted-use scope, and build audit event. Public-figure cloning is prohibited.

## Repository structure

Approved target. Create incrementally. Absent paths are planned, not missing work, unless the active phase requires them.

| Path                        | Responsibility                                                                                                                    |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `Cargo.toml`                | Workspace + shared dependency policy. Commit `Cargo.lock`. Keep the workspace small.                                              |
| `crates/study-tts-cli/`     | Executable, commands, config, diagnostics. Thin orchestration. Human + JSON output.                                               |
| `crates/study-tts-core/`    | Lesson types, normalization, planning, cache keys. No Python, model SDK, CUDA, or FFmpeg bindings.                                |
| `crates/study-tts-runtime/` | FS state, worker pool, in-process ASR, PCM, process control, FFmpeg adapter. Containment, atomic publish, recovery, verification. |
| `crates/study-tts-testkit/` | Fixtures, fake worker, fault injection, audio helpers. Production crates depend on it only from tests.                            |
| `worker/`                   | One production backend adapter + locked Python env. stdout = protocol only; stderr = diagnostics.                                 |
| `schemas/`                  | Versioned lesson, worker, takes, verification, job, manifest JSON Schemas. Must match Rust types and fixtures.                    |
| `fixtures/`                 | Reviewed lessons, pronunciation cases, deterministic audio. Small, licensed, non-sensitive, stable.                               |
| `docs/adr/`                 | Durable decisions. ADRs supersede explicitly; do not silently contradict.                                                         |
| `docs/operations/`          | Install, diagnostics, recovery, prune, rollback, release. Exercise on clean Ubuntu 24.04 / WSL2.                                  |
| `data/`                     | Local models, voices, cache, jobs, staging, outputs. Do not commit. Published output and quarantine need explicit deletion.       |
| `target/`, `.venv/`         | Generated. Do not edit or commit.                                                                                                 |

## Routing

| Looking for                     | Start                                                                     | Source of truth                                                                         |
| ------------------------------- | ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Architecture, scope, boundaries | `docs/adr/ADR-0001-production-rust-study-guide-tts.md`                    | Newest accepted ADR that explicitly supersedes                                          |
| Delivery sequence, acceptance   | `DELIVERY-PLAN.md`                                                        | Approved backlog + ADR traceability                                                     |
| Engineering principles          | `PRINCIPLES.md`                                                           | Enforcement mechanism named per principle; ratified ADRs win on conflict                |
| Doc map                         | `docs/INDEX.md`                                                           | Index → controlling accepted decision or execution record                               |
| Governance / gates              | `docs/governance/PROJECT-EXECUTION-CHARTER.md`                            | ADR + Delivery Plan win on conflict                                                     |
| Capability ownership            | `docs/governance/MILESTONE-CAPABILITY-MATRIX.md`                          | One owner, approver, validation route, first required gate                              |
| Decisions, failures, artifacts  | `docs/governance/ROUTING-TABLES.md`                                       | Named owner, record, deadline, blocking rule                                            |
| Requirement traceability        | `docs/governance/TRACEABILITY-MATRIX.md`                                  | ADR requirement → story, validation, gate                                               |
| Risks / descope                 | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md`                          | Ratified only; proposals do not change scope                                            |
| Rights / data / artifacts       | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`                          | Approved rights records + ADR-0004                                                      |
| Interface freeze                | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`                  | G1 freeze + approved versioned changes                                                  |
| GitHub backlog                  | `docs/governance/GITHUB-PROJECT-PLAYBOOK.md`                              | Project status + repo acceptance evidence                                               |
| Chatterbox qualification        | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md`          | Accepted development baseline + scoped waiver; full-box performance required before G3  |
| Audio formats / loudness        | `docs/adr/ADR-0003-production-audio-quality-profile.md`                   | Proposed until thresholds, codecs, frozen refs approved                                 |
| Voice consent / watermark       | `docs/adr/ADR-0004-voice-content-and-retention-policy.md`                 | Proposed until rights records + retention approved                                      |
| ASR verification                | `docs/adr/ADR-0005-asr-calibration-and-release-control.md`                | Proposed until identities, corpus, decoder, patterns, gates measured                    |
| Domain / planning               | `crates/study-tts-core/`                                                  | Rust types + checked-in schemas                                                         |
| Worker contract                 | `schemas/worker-protocol-v2.schema.json`, `worker/`                       | Versioned protocol + shared contract tests                                              |
| Configuration                   | `crates/study-tts-cli/`                                                   | Parsed types + documented precedence                                                    |
| Job / cache / recovery          | `crates/study-tts-runtime/`                                               | Runtime, manifest schemas, recovery tests                                               |
| External-process safety         | `crates/study-tts-runtime/`                                               | Pool + FFmpeg adapters + containment tests; ASR in-process                              |
| Test patterns                   | `crates/study-tts-testkit/`, colocated tests                              | Fake-worker, property, contract, recovery tests                                         |
| Code review standard            | `.claude/skills/rust-review/SKILL.md`, `.claude/skills/ponytail/SKILL.md` | This file + `PRINCIPLES.md`; review reports, never edits unless asked                   |
| Code style rules                | `.claude/skills/clean-code/SKILL.md`, `.claude/skills/ponytail/SKILL.md`  | Binding on all code; this file, `PRINCIPLES.md`, and accepted ADRs win on conflict      |
| Production Rust craft standard  | `.claude/skills/rust-production/SKILL.md`                                 | Process, durability, determinism, and schema-evolution rules, each citing the module in this tree that proves it |
| Rust test standard              | `.claude/skills/rust-testing/SKILL.md`                                   | `docs/testing/TEST-STRATEGY.md` owns tiers and budgets; the skill owns how a test is written |
| Comment content standard        | `.claude/skills/rust-comment/SKILL.md`                                    | Binding on all Rust; `crates/AGENTS.md` §3 owns comment mechanics                       |
| TDD / tiers                     | `docs/testing/TEST-STRATEGY.md`                                           | Delivery Plan named tests + tier policy                                                 |
| Qualification evidence          | `docs/testing/EVIDENCE-AND-QUALIFICATION.md`                              | Immutable reports + governed raw artifacts                                              |
| Test-data provenance            | `docs/testing/TEST-DATA-MANIFEST.md`                                      | IDs, checksums, rights, sensitivity, retention, owner                                   |
| Threat model                    | `docs/security/THREAT-MODEL.md`                                           | Trust boundaries, controls, residual risk                                               |
| Operations                      | `docs/operations/`                                                        | Exercised runbooks + `study-tts doctor`                                                 |
| Production diagnostics          | Local `events.ndjson`, `job.json`, `manifest.json`, `quality-report.json` | Redacted local artifacts; do not upload source text or voice-reference paths by default |

## Commands

Run from the repo root on Ubuntu 24.04 / WSL2. Check `Cargo.toml`, worker lockfiles, scripts, and CI before extending this table.

Current tree: E1-S1 tested contract baseline + non-product status executable. Product commands below are authoritative **only** when the referenced behavior, files, and targets exist. Do not invent options. If a command fails because the phase is not implemented, report the exact gap.

| Purpose             | Command                                                                                        |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| System tools        | `gcc --version && cmake --version && python3 --version && ffmpeg -version && ffprobe -version` |
| Fetch               | `cargo fetch --locked`                                                                         |
| Build               | `cargo build --workspace --locked`                                                             |
| Type-check          | `cargo check --workspace --all-targets --locked`                                               |
| One test            | `cargo test --workspace <test_name>`                                                           |
| Tests               | `cargo test --workspace --all-targets --locked`                                                |
| Lint                | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`                |
| Rust conventions    | `python3 scripts/check-rust-conventions.py`                                                    |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py`                                                 |
| Fmt check           | `cargo fmt --all -- --check`                                                                   |
| Fmt apply           | `cargo fmt --all`                                                                              |
| CLI help            | `cargo run -p study-tts-cli -- --help`                                                         |
| Doctor              | `cargo run -p study-tts-cli -- doctor`                                                         |
| Validate lesson     | `cargo run -p study-tts-cli -- lesson validate <lesson.json>`                                  |
| Worker tests        | Environment restored per `docs/operations/WORKER-ENVIRONMENT.md`; do not invent a runner.       |
| Schemas / generated | Checked-in repo task or CI once created; no ad-hoc generator.                                  |

## Verification

Fastest relevant check first. Broader checks after it passes. PR CI must not download model weights or require a GPU.

Spoken-output changes: automated checks are necessary but not sufficient. Listen to the changed segment in context, or state that human listening remains unverified.

| Change                           | Required verification                                                                                                                                                 |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Docs only                        | Rendered Markdown, links, paths, commands, ADR consistency                                                                                                            |
| Core types / deterministic logic | Targeted unit + property tests, schema consistency, `cargo check`, fmt, Clippy                                                                                        |
| Worker protocol / backend        | Contract tests, malformed-message, cancel/timeout, offline check, short real-backend render on named hardware                                                         |
| Worker pool / CPU governance     | Parallel dispatch through `&self` executor, process isolation, thread-limit asserts, RAM/core oversubscription reject, single-worker RTF qualification                |
| ASR                              | Pinned-stack + decoder asserts, ASR-only invalidation, expected-pattern promotion review, seeded defect-class gates, repeated-run stability, segment-order invariance |
| Job / cache / takes / recovery   | Targeted + fault injection at each write boundary, checksums, stale-take reject, prune protection, restart, one-segment invalidation                                  |
| PCM / FFmpeg / export            | Edge pad/ramp + Rust assembly tests, supported-FFmpeg integration, ffprobe, frozen-loudness refs, deterministic timeline, playback of representative exports          |
| User-visible CLI                 | Tests + real WSL2 exercise, including `--json` and exit codes                                                                                                         |
| Security boundary                | Traversal, symlink escape, command injection, oversized input, hostile worker, redaction, child-process cleanup + human review                                        |
| Model / voice / synth params     | Cache-key compatibility, short contract render, blind quality sample, license, consent where applicable                                                               |
| Release config                   | Clean WSL2 install, full CI, `doctor`, 45–60 min soak, recovery demo, SBOM/license, signing, checksum, rollback                                                       |

## Security and data

- Treat Markdown, JSON, worker responses, models, filenames, voice refs, cache entries, and media metadata as untrusted.
- Enforce size, nesting, segment-count, message, duration, retry, timeout, and disk limits at the owning boundary.
- Resolve and validate managed paths before access. Reject absolute paths, traversal, symlink escape, and Windows-mount escape where containment is required.
- Pass process args as discrete checked argv. Never interpolate user data into a shell.
- Pin Rust, Python, model, tokenizer, codec, voice, and ASR revisions. Derive worker-bundle hash from executable worker inputs. Verify model, voice, cache, verification, and published-output checksums.
- Prefer safe tensor formats. Do not load untrusted pickle-style models when a safe format exists.
- Keep rendering offline; test that the worker cannot make network requests.
- No credentials, tokens, personal data, source content, production audio, or voice-reference files in source, fixtures, logs, examples, or diagnostic bundles.
- Log IDs, hashes, timings, states, error classes. Do not log full source, spoken text, or raw voice-reference paths by default.
- Fixtures and mocks stay in test/dev paths. Clear license, no sensitive content.
- Do not weaken validation, containment, checksum, consent, offline, dependency, or license controls to make a test pass.
- Human review required for voice cloning, consent changes, executable model formats, dependency-license changes, release signing, and any externally visible service or permission.

## Coding conventions

- **Style.** Load `.claude/skills/clean-code/SKILL.md` and `.claude/skills/ponytail/SKILL.md` before writing or editing any code, and `.claude/skills/rust-review/SKILL.md`, `.claude/skills/rust-comment/SKILL.md`, `.claude/skills/rust-testing/SKILL.md`, and `.claude/skills/rust-production/SKILL.md` as well before any Rust — the review standard governs generation, not just review, so write to it rather than refactoring to it later. They are binding standards for how code is written here, not advice to weigh. They are not architectural authority: this file, `PRINCIPLES.md`, and the accepted ADRs win on any genuine conflict, and the settled conflicts are tabulated in the clean-code file. Flag a new conflict; do not resolve it silently. `CLAUDE.md` carries the same table for agents that read it first.
- **Rust audits and edits.** `.claude/skills/ponytail/SKILL.md` is required before auditing or editing an existing Rust file, not only before writing a new one. `rust-review` finds what is wrong; ponytail finds what should not exist at all, and an audit that skips it leaves the tree longer than it needs to be.
- **Naming.** `study-tts-*` crates; stable lowercase kebab-case CLI names; versioned schemas (`lesson-v1`); segment IDs independent of mutable display text.
- **Errors.** Typed internal errors + source-aware user diagnostics. Distinct classes: invalid input, missing dependency, incompatible environment, worker failure, audio-quality failure, cancellation, resource exhaustion, integrity failure, internal error. Never silently fall back to another model or device.
- **Logging.** Structured `tracing` with `job_id`, stage, segment ID, attempt, worker/model identity, duration, error class. Concise terminal. Worker stdout is protocol-only.
- **Types / schemas.** Durable lesson, worker, takes, verification, job, manifest data as explicit versioned Rust types + JSON Schemas. Reject unknown or incompatible versions at the boundary unless a tested migration exists.
- **Async / processes.** Object-safe `TtsExecutor` with `&self`; bound queues and concurrency. Pool owns individually synchronized clients. Parent owns process tree, deadlines, cancel, restart budget, thread budget, cleanup. Unload pool before ASR.
- **ASR.** Pinned in-process `whisper-rs`: one model context, independent decoder state per segment. Compare against the approved expected-ASR lattice. Never auto-learn patterns or let ASR mutate approved text.
- **Filesystem writes.** Stage, flush, validate, atomically publish. Verification evidence stored separately. Collision-free quarantine. Do not overwrite or auto-delete invalid artifacts.
- **Audio.** Rust owns silence, PCM concat, edge padding, transition ramps, float-range validation. Committed frozen voice/style loudness refs; unrelated edits must not change unrelated gain.
- **Dependencies.** Add only when they remove more risk than they add. Pin via `Cargo.lock` or worker lockfile. Review licenses and advisories. No duplicate libraries for the same narrow concern.
- **Generated artifacts.** Deterministic checked-in command. CI fails on schema drift from authoritative Rust types.
- **Compatibility.** Ubuntu 24.04 / WSL2 first; keep native Linux. Native Windows packaging, GPU accel, remote execution, multi-user are deferred.
- **Unsafe.** Avoid. Any use needs a documented invariant, focused tests, and human review.

## Completion

Before reporting done:

- Requested behavior is implemented, not a placeholder.
- Diff has no accidental or unrelated changes.
- Relevant tests and checks ran from WSL2.
- Changed CLI or audio exercised through the real interface when possible.
- Speech changes: listened in context, or the exact remaining listen check is stated.
- No secrets, source text, voice refs, temp diagnostics, generated runtime data, or unlicensed artifacts entered the repo.
- No valid cache, quarantine, job, or published output deleted without approval.
- Durable docs updated if behavior, architecture, setup, schema, cache identity, recovery, or operator workflow changed.
- ADR added or superseded for an invariant or deferred-capability change.
- Summary: files changed, verification run, model/media tests, anything still unverified.

## Nested instructions

Keep specialist rules next to the files they govern. Add nested files when those paths exist and local rules are concrete:

```text
AGENTS.md
├── crates/AGENTS.md      # Rust boundaries, errors, schemas, process adapters
├── worker/AGENTS.md      # Python lock, model install, protocol, offline
├── fixtures/AGENTS.md    # Licensing, determinism, size, review
└── docs/AGENTS.md        # ADR lifecycle, runbooks, doc verification
```

Do not copy a root rule into a nested file unless the local rule changes or clarifies it.

## Authoritative guides

- Architecture: `docs/adr/ADR-0001-production-rust-study-guide-tts.md`
- Principles: `PRINCIPLES.md`
- Delivery: `DELIVERY-PLAN.md`
- Doc routing: `docs/INDEX.md`
- Execution: `docs/governance/PROJECT-EXECUTION-CHARTER.md`
- Contribution: `CONTRIBUTING.md`, `.github/PULL_REQUEST_TEMPLATE.md`
- Style: rustfmt 2024 defaults + Clippy when configured, plus `.claude/skills/clean-code/SKILL.md`, plus `.claude/skills/ponytail/SKILL.md`, plus `.claude/skills/rust-production/SKILL.md`
- Review: `.claude/skills/rust-review/SKILL.md`, plus `.claude/skills/ponytail/SKILL.md`
- Comments: `.claude/skills/rust-comment/SKILL.md`
- Python worker: `docs/operations/WORKER-ENVIRONMENT.md`, `worker/bundle-manifest.json`, `worker/pyproject.toml`, `worker/requirements.lock`
- Operations: `docs/operations/`
