# Engineering Principles

**Scope**: `technical-tts`, a local-first Rust system that converts reviewed technical lessons into long-form study-guide audio through a resource-governed synthesis worker, an in-process ASR verifier, and FFmpeg.
**Purpose**: keep the system correct, deterministic, rights-clean, listenable, and auditable as agents implement more of it.

These principles bind the whole repository. `docs/adr/ADR-0001-production-rust-study-guide-tts.md` remains authoritative for architecture; where a principle and a ratified ADR conflict, the ADR wins and this file must be amended in the same pull request.

Crate and process layout referenced throughout:

| Component | Responsibility | Depends on |
| --- | --- | --- |
| `study-tts-core` | Lesson validation, render planning, cache identity, gate rules, manifest meaning, verification thresholds | nothing in-workspace |
| `study-tts-runtime` | Orchestration, worker supervision, cache and quarantine I/O, FFmpeg invocation, ASR verification, job state, synthesis adapters (E0 deterministic tone) | `core` |
| `study-tts-cli` | Argument parsing, presentation, exit codes, operator affordances | `core`, `runtime` |
| `study-tts-testkit` | Fixtures, harnesses, the end-to-end walking-skeleton suite, fault injection | `core`, `runtime` |
| Python worker | Model loading and synthesis only, over a versioned protocol, as a supervised child process | nothing; decides nothing |

## Non-negotiables

Rules protecting listeners, rights holders, data integrity, and the audit trail. Violations require automated or runtime controls plus review; review alone is insufficient.

1. **Rust is the authority.** Lesson validation, planning, cache identity, job state, recovery, audio validation, manifests, and release decisions live in the Rust application. The Python worker and external tools execute; they never decide (P1, P8).
2. **External input is untrusted.** Every lesson file, worker payload, cached artifact, model file, and tool output is validated at the boundary before use, and no artifact is trusted because this system wrote it earlier — identity and checksums are reverified on read (P2, P4).
3. **Requested is not rendered.** A synthesis call that returned is not proof of valid speech: audio is verified against the script and against acoustic invariants after rendering, failed attempts are quarantined with their diagnostics, and only verified artifacts enter the cache, a track, or a package (P4, P9, P11).
4. **Nothing claims `production` without passed gates.** `release_status` is derived from the gate evidence in `docs/governance/RELEASE-PROFILES.md`, a preview is never silently promoted, and unimplemented behavior is never described as implemented in a manifest, a changelog, or a status line (P5, P17, P18).
5. **Rights, consent, and privacy precede synthesis and publication.** No voice without a consent record, no model without a pinned checksum, no network egress during rendering, and no source text, voice audio, model weights, or secrets in logs, issues, evidence bundles, or diagnostics (P7).

All other principles bind equally; the five above are the minimum that must never be review-only.

## Enforcement vocabulary

Every binding rule names its mechanism, because a rule without a mechanism is documentation rather than policy.

- **Static:** lint, dependency, or build-time structural check (`cargo fmt`, `taplo fmt`, `clippy -D warnings`, `cargo deny`, crate dependency direction, `unsafe_code = "forbid"` in the workspace lint table).
- **Types:** invalid states are harder to express. Types neither validate runtime input nor prove correctness.
- **Test:** automated test of observable behavior or contract, named `t<tier>_e<epic>_<behavior>` per `docs/testing/TEST-STRATEGY.md`.
- **Bench:** a committed measurement with a recorded budget — segments per minute, peak resident memory, worker restart cost — run against a pinned baseline per `docs/perf/BUDGETS.md`.
- **Runtime:** production code validates, checksums, quarantines, contains, or refuses the operation.
- **Evidence:** a committed, checksummed record under `evidence/` per `evidence/README.md`, written before the result it supports is claimed.
- **Listen:** a recorded human listening pass over a sampled set of rendered segments, logged as evidence with the sample definition, because some defects — prosody collapse, wrong emphasis, an unnatural speaker handoff — are audible and not yet machine-detectable.
- **Review:** a reviewer judges context automation cannot reliably judge, routed per `docs/governance/ROUTING-TABLES.md`.

Review is valid, but review-only is convention rather than guarantee. Exceptions apply only where a rule states a path, and each must be narrow, explained beside the suppression or in an ADR deviation record (`docs/adr/deviations/`), and covered by a test.

## Principles

**1. Domain rules have one authoritative implementation.**

- Rules defining lesson validity, render planning, cache identity, verification thresholds, release gates, and manifest meaning live in `study-tts-core` with framework-independent tests that touch no filesystem, subprocess, or model. `study-tts-runtime` orchestrates and `study-tts-cli` presents; neither reimplements a core decision, and the worker process implements none.
- **Prevents:** Duplicated rules that drift apart, a worker that quietly decides segment eligibility, a CLI that recomputes a cache key slightly differently, and domain behavior coupled to tokio, FFmpeg, or a vendor model.
- **Enforced by:** Static crate dependency direction (`core` ← `runtime` ← `cli`); Test of core behavior without I/O; Review for inline reimplementation.

**2. External contracts are decoded once and evolved deliberately.**

- Every lesson file, schema payload, worker protocol message, cached artifact header, manifest, and external tool output is parsed at the boundary into an internal type, with malformed input rejected before any work starts, and every format carrying a version field handled by an explicit migration tested in both directions.
- **Prevents:** Silent coercion, half-processed invalid lessons, worker payloads trusted by construction, manifests unreadable by the release that wrote them, and pipelines broken by an added field.
- **Enforced by:** Runtime validation with distinct error variants carrying file, location, and expected shape (the `lesson.rs` pattern); Test of valid, missing, malformed, boundary, and portability cases; Types for the decoded result.

**3. Synthesis backends are interchangeable behind one identity-carrying interface.**

- The deterministic tone synthesizer, Chatterbox, and any future backend sit behind one interface whose identity — model, revision, voice, parameters, seed, ABI revision — participates in cache identity, and genuine backend differences are typed capabilities rather than conditionals scattered through the pipeline.
- **Prevents:** Cache hits across different voices, fictional uniformity between backends, a model upgrade served from stale audio, and the E0 fake synthesizer leaking assumptions into production paths.
- **Enforced by:** Types for synthesizer identity; Test that any identity change misses the cache (`t1_e0_synthesizer_identity_participates_in_the_cache_key`); Review for backend conditionals outside adapters.

**4. Provenance is explicit wherever an artifact is reused or published.**

- Every cached WAV, assembled track, and package carries enough identity — content hashes, input hashes, synthesizer identity, tool versions, verification results, `release_status` — to answer "what produced this, from what, under what authority" without consulting git history or CI logs.
- **Prevents:** Stale or foreign artifacts reused as current, manifests that cannot support an audit, packages whose provenance depends on a CI run that has since expired, and quarantined output mistaken for good output.
- **Enforced by:** Types for manifest and cache metadata; Runtime checksum verification on every read; Test of metadata-mismatch rejection; Evidence for gate-relevant claims.

**5. Every pipeline stage defines its complete failure behavior before it is built.**

- Before implementing a stage, define what happens on invalid input, tool absence, tool failure, worker crash, protocol violation, partial output, corruption, interruption, and retry exhaustion — and which failures are recoverable, which quarantine, and which block publication, per the failure-routing table in `docs/governance/ROUTING-TABLES.md`.
- **Prevents:** Silent fallbacks, destructive retries, half-written cache entries visible to a later run, lost diagnostics, and failures that surface three stages downstream as an unrelated assembly error.
- **Enforced by:** Types for stage outcomes; Test of each defined failure, including injected interruption between write and rename (the preflight and quarantine suites); Review against the failure-routing table.

**6. Determinism is a product feature, not a test convenience.**

- Identical inputs produce identical plans and byte-identical cached audio across runs, machines, and worker-pool sizes; nondeterminism — model sampling, wall clock, temporary paths, completion order, environment — is injected at explicit seams and characterized where it cannot be removed.
- **Prevents:** Unreproducible bugs, cache identity that lies, evidence that cannot be re-derived, an overnight render that differs from its own retry, and flaky suites agents learn to ignore.
- **Enforced by:** Test with fixed seeds and controlled inputs at varying parallelism (`t1_e0_plan_is_stable_for_identical_inputs`, byte-identical cache-hit tests); Evidence characterizing residual nondeterminism; Review of new nondeterministic seams.

**7. Security and privacy controls do not depend on good behavior.**

- Rendering runs offline, the worker is contained and its process tree terminated on protocol failure, model artifacts are pinned by checksum and verified before execution, and source text, voice recordings, model weights, and secrets never enter the repository, CI logs, evidence bundles, or diagnostic archives.
- **Prevents:** Data exfiltration during rendering, supply-chain surprises from an unpinned model pull, unauditable voice use, and private lesson content leaking through an error path or a stack trace.
- **Enforced by:** Runtime egress denial and containment; Static dependency and license policy (`cargo deny`); CI Test running the T4 suite in a no-network namespace; Review under `docs/security/THREAT-MODEL.md`.

**8. The worker is a subordinate process behind a versioned protocol and a supervised lifecycle.**

- The Rust-to-Python boundary is the least type-safe surface in the system and is therefore the most constrained: the protocol is versioned and negotiated at startup, every message is validated on receipt, the worker holds no durable state, it is restartable at any point without loss, and its process tree is terminated — not merely signalled — on timeout, protocol violation, or shutdown.
- **Prevents:** A hung worker holding a model in memory after the parent exits, protocol skew between a Rust release and a Python environment, a worker that accumulates state a restart would silently discard, and an orphan process that consumes the GPU until the machine is rebooted.
- **Enforced by:** Types for protocol messages with an explicit version field; Runtime handshake rejection on mismatch and process-group termination; Test of crash, hang, malformed message, and mid-segment restart; Review of every protocol change against ADR-0001 §10 (TTS worker: protocol methods and constraints).

**9. Rendered audio is verified against the script and against acoustic invariants.**

- Every rendered segment is checked for the invariants the pipeline depends on — sample rate, channel count, frame count matching the declared duration, absence of clipping, absence of silent or truncated output — and for transcription agreement against the spoken text, where the ASR comparison is normalized against the spoken form rather than the written form, because a protected technical term rendered correctly as speech will not match its written spelling under any ASR.
- **Prevents:** A silent segment assembled into a three-hour track, a truncated final word discovered by a listener, a clipped segment that passes duration checks, and a verifier that fails correct pronunciations of exactly the terms this system exists to narrate.
- **Enforced by:** Runtime per-segment validation and quarantine; Types for the verification outcome, which is never a boolean alone; Test of each invariant with hostile fixtures; Evidence for verification thresholds and their calibration set per ADR-0005; Listen over a sampled set per release.

**10. Pronunciation of protected terms is data, not model luck.**

- Technical terms, acronyms, code identifiers, and units that must be spoken a specific way are declared in a versioned lexicon (seeded by `fixtures/pronunciation/`), the lexicon participates in cache identity because changing it changes the audio, and an unresolved term in a lesson is a validation failure rather than a silent pass to the model.
- **Prevents:** The same acronym spoken three ways in one lesson, a model upgrade silently changing established pronunciations, a lexicon change that fails to invalidate affected cache entries, and pronunciation fixes buried as string replacements in the rendering path.
- **Enforced by:** Types for the resolved lexicon; Runtime rejection of unresolved protected terms at validation; Test that a lexicon revision invalidates dependent cache entries; Review of lexicon changes.

**11. State is separated by authority, lifetime, and transition model.**

- Requested synthesis, observed artifacts, durable job state, cache contents, and quarantine are distinct stores with explicit reconciliation; writes are atomic through write-to-temporary-then-rename; and on checksum or state corruption the system refuses to overwrite and reconciles rather than guessing.
- **Prevents:** Acknowledgements treated as facts, resumption that regenerates completed work, caches that disagree with job state, two concurrent runs corrupting one entry, and corruption papered over by a silent re-render.
- **Enforced by:** Types (distinct state representations rather than one shared map); Runtime refuse-and-reconcile on mismatch; Test of interruption, resumption, concurrency, and conflict; Review for duplicated state.

**12. Style is default and machine-applied; literals are decisions.**

- Code matches default rustfmt (2024 style) and `Cargo.toml` matches taplo, with no custom options and no hand alignment; numbers that encode a decision — sample rates, loudness targets, verification thresholds, retry bounds, timeouts, pool sizes, gate identifiers — are named constants traceable to their governing document.
- **Prevents:** Style debate consuming review, formatting drift between agents, and a threshold that exists only as an unexplained float in a comparison.
- **Enforced by:** Static `cargo fmt --check` and `taplo fmt --check` in CI; Review for unexplained literals; `crates/AGENTS.md` for the full rules.

**13. Boundaries are enforced in the build.**

- Crate dependencies flow one way — `core` depends on no sibling, `runtime` on `core`, `cli` on both, `testkit` on `core` and `runtime` — modules own one concern each, and cross-file or code-to-document coupling is written down on both sides at the point of coupling, including the `release.rs` mirror of `RELEASE-PROFILES.md` §3.
- **Prevents:** Dependency cycles, grep-invisible coupling, falsely generic shared modules that accumulate every unrelated helper, and a documentation mirror that drifts unnoticed for six months.
- **Enforced by:** Static (Cargo makes cycles unbuildable; manifest review guards direction); Test where a document-to-code mirror exists; Review for boundary erosion.

**14. Tests prove behavior at the cheapest reliable tier.**

- Core rules are tested as pure units (T1), invariants as proptest properties, integration through the real pipeline with the real FFmpeg binary (T4), fault injection against a contained worker (T5), and release qualification end-to-end (T6) — preferring deterministic fixtures from `fixtures/`, injected identity, and controlled tools over mocks of things that can run for real.
- **Prevents:** False confidence from a mocked FFmpeg, verification logic proven only against synthetic tones, slow brittle suites, snapshot tests nobody reads, and code agents cannot safely change.
- **Enforced by:** Test in CI at every tier defined by `docs/testing/TEST-STRATEGY.md`; the walking-skeleton suite as the E0 floor; Review for tier-inappropriate tests.

**15. Long-form rendering is budgeted, and the budget is measured.**

- Worker pool size, memory ceiling, retry bounds, timeouts, and disk use for the cache and quarantine carry committed budgets in `docs/perf/BUDGETS.md`, are measured on the reference machine (`docs/operations/REFERENCE-ENVIRONMENT.md`) against a pinned baseline at a lesson length representative of production rather than a two-segment fixture, and a regression beyond the stated tolerance is either fixed or recorded as an accepted deviation.
- **Prevents:** A pipeline correct on a fixture yet unusable on a three-hour lesson, a memory ceiling discovered by the kernel at hour two, and unbounded cache growth on a self-hosted machine.
- **Enforced by:** Runtime resource limits; Bench against pinned baselines at representative length; Evidence for accepted regressions; Review of optimizations that cross an abstraction boundary.

**16. Observability is product behavior, and diagnostics are minimal by construction.**

- Every job, segment, worker invocation, and external command carries stable identifiers and structured `tracing` spans so a failed overnight render is diagnosable from preserved evidence alone, and the diagnostic bundle is assembled from an allowlist of fields rather than filtered afterward, because a denylist eventually misses a field containing lesson text (P7).
- **Prevents:** Incidents whose only evidence is a vanished process, a quarantine directory that records failure without cause, and a support bundle that leaks the source material it was meant to describe.
- **Enforced by:** Runtime structured logging and allowlisted bundling; Test asserting no source text or path appears in a generated bundle; Review of diagnostic usefulness and data minimization.

**17. Configuration expresses deployment policy; code expresses stable behavior.**

- Model roots, voice roots, data directories, worker interpreter and environment, and release profiles are typed configuration validated at startup; model and voice artifacts are referenced by pinned URI and checksum and never fetched implicitly; and the toolchain and dependency set are pinned (`rust-toolchain.toml`, `Cargo.lock`, `deny.toml`, and the pinned Python environment specification).
- **Prevents:** Hidden environment assumptions, a render that changes because a Python package was upgraded outside the lockfile, invalid startup state discovered mid-render, and configuration surface that merely relocates complexity into a schema.
- **Enforced by:** Types and Runtime validation at startup; Static `cargo deny` and locked builds in CI; Test that a mismatched worker environment fails the handshake rather than the render; Review for new configuration surface.

**18. Capability and release claims are enumerated, gated, and measured.**

- `release_status` is computed from gate evidence rather than asserted, each of the twelve production gates from ADR-0001 §18 (mirrored in `study-tts-core/src/release.rs` and `RELEASE-PROFILES.md` §3) names its evidence artifact and its owner, and features are enumerated as implemented, partial, or unimplemented — where a partial feature fails loudly on the unimplemented path rather than returning a plausible artifact.
- **Prevents:** A preview package indistinguishable from a production one, gates satisfied by assertion, criteria written after the result, and a stub that produces a listenable file which is quietly wrong.
- **Enforced by:** Runtime derivation of `release_status` from evidence; Test that a missing or stale gate artifact blocks promotion; Evidence per gate under `evidence/`; Review of every status change.

**19. The repository is operable by agents and auditable by people.**

- `AGENTS.md` files are the authoritative instructions (root for the repository, `crates/AGENTS.md` for Rust work), routing lives in tables rather than tribal knowledge, checks are reproducible without undocumented local state, claims of passing checks are made only after the checks ran, and gated operations — publication, rights decisions, deletion of cache or quarantine content, promotion of `release_status` — require human approval.
- **Prevents:** Instruction drift, fabricated confidence, environment-specific success, and autonomous escalation into irreversible operations.
- **Enforced by:** CI Test on every push and pull request; Evidence rules in `evidence/README.md`; the approval boundaries in the root `AGENTS.md`; human Review for every gate in `docs/governance/ROUTING-TABLES.md`.

**20. Enforcement is proportionate and tested.**

- Automate rules whose violations are mechanically recognizable and consequential; keep the local check set fast enough to run before every commit (`cargo fmt --check`, `taplo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`, `cargo test --workspace`) with failure output that tells the next agent how to correct the problem; and give every stated invariant that can break silently a test that fails when it does.
- **Prevents:** Architectural decay, performative policy nobody can verify, document-to-code mirrors that drift, and build complexity exceeding the value of the control.
- **Enforced by:** Static checks, Types, Test, Bench, Runtime controls, Evidence, and Listen, as specified per principle; Review of the enforcement itself whenever a rule remains review-only.

## Amendment and deviation

A principle is amended by pull request citing the ADR that motivates the change, and the amendment lands in the same commit as the enforcement that implements it. A one-time departure is recorded as a deviation under `docs/adr/deviations/` naming the violated principle, the scope, the compensating control, and the expiry condition. Deviations without an expiry condition are rejected.
