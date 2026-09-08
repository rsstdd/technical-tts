---
name: rust-production
description: >
  The OS-facing craft rules for this workspace, each one codified from a module that already
  proves it: supervising an external process (deadlines, concurrent pipe capture, process-group
  kill, containment), publishing durable state (sync → RENAME_NOREPLACE → sync parent, advisory
  locks, path containment, cache validation), keeping identity deterministic (canonical bytes,
  ordered maps, no floats), and evolving a schema or frame (major refused, older minor accepted,
  unknown fields rejected). REQUIRED before writing, generating, or editing any Rust here, and
  used to review a diff that spawns a process, writes durable state, computes an identity, or
  changes a published format.
---

# Production Rust in this workspace

`crates/AGENTS.md` says to imitate the best existing example rather than invent a shape. This
file is that instruction applied to the parts that touch the operating system, where inventing a
shape costs a corrupted cache, a hung render, or an identity that silently changes. Every rule
below is already implemented and tested in this tree; the citation is the specification.

## Authority

Conflict order is unchanged — newest accepted ADR that explicitly supersedes →
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` → `DELIVERY-PLAN.md` → `AGENTS.md` →
`crates/AGENTS.md` → `PRINCIPLES.md` → this skill. Flag a genuine conflict; never resolve one
silently.

Siblings own what this file does not restate: `clean-code` (style catalogue), `ponytail` (whether
it should exist), `rust-comment` (comment and rustdoc content), `rust-review` (review conduct and
the severity scale), `rust-testing` (test policy). This file states the *authoring* rule; where
`rust-review` states the matching review finding, it is not repeated here.

## External process boundary

Canonical: `study-tts-runtime/src/process.rs`, `tools.rs`, `worker_environment.rs`.

- **Preflight, never lazy discovery.** Resolve and identify every external binary before any work
  begins, so a build that will fail for a missing encoder says so before it synthesizes anything
  — `tools::inspect`. Record the binary that actually ran, not the one requested.
- **Every spawn carries a named deadline** whose constant mirrors the document that sets it, and
  the wait never busy-spins (`PROCESS_POLL_INTERVAL`). A spawn with no deadline is a hang waiting
  for CI.
- **Capture stdout and stderr concurrently, on their own threads** — `spawn_capture_pipe`. Reading
  one pipe to completion before the other deadlocks as soon as the unread pipe fills its buffer.
  Bound what you retain (`TOOL_OUTPUT_LIMIT_BYTES`); an unbounded capture is a memory bug on a
  chatty tool.
- **Spawn into its own process group** (`configure_process_group` → `process_group(0)`), kill the
  *group*, then **prove the tree is gone** before reporting success — killing the direct child
  leaves its grandchildren holding your files. `process_identity_is_live` checks PID *and*
  `/proc` start time, because a reused PID is a different process.
- **The caller keeps error ownership.** Supervision reports what happened; it does not collapse a
  launch failure, a nonzero exit, and a timeout into one variant.

## Durability and crash-consistency

Canonical: `durable.rs`, `locking.rs`, `managed.rs`, `cache.rs`.

- **A rename that makes data authoritative is preceded by a file sync and followed by a
  parent-directory sync.** ADR-0001 §12.3. A rename alone is not durable: the entry survives and
  the contents do not. The ordering is proven by a test, never merely present —
  `t4_e0_durable_json_replacement_flushes_file_then_rename_then_parent` and
  `t4_e0_directory_publication_flushes_files_before_rename_and_parent` assert the *sequence* through
  a crash-injection seam.
- **Publish with `RENAME_NOREPLACE`**, so a concurrent winner is reported (`DestinationExists`)
  rather than silently overwritten.
- **Every path beneath an owned root goes through the central resolver** — `managed::subdirectory`,
  `directory_candidate`, `leaf`. Lexical validation is not containment: a validated component list
  still follows a symlink planted at any level, and the build then reads and writes outside the
  root it was given. Containment is only as good as its least careful call site.
- **Ownership is an advisory lock plus process identity** — `acquire_job_lock` records PID and
  `/proc` start time so a stale record is auditable and PID reuse is never mistaken for a live
  owner.
- **Validate before reuse.** A cache entry is reused only after its recorded metadata is checked
  against this build's canonical format and its audio re-hashed to the digest the artifact
  records. Quarantine a partial transaction; a corrupt published entry is a refusal that names the
  entry and its remedy owner, never a silent repair that could hide tampering.

## Determinism and identity

Canonical: `study-tts-core/src/canonical.rs`, `digest.rs`, `identity.rs`, `plan.rs`,
`verification.rs`.

- **An identity hashes canonical bytes, never serializer output.** `canonical_bytes` /
  `canonical_digest` own the byte form so field order and `serde` defaults cannot move an
  identity. Artifact checksums hash raw bytes instead.
- **Ordered maps only.** This workspace has zero `HashMap` and 55 `BTreeMap` in crate sources; a
  `HashMap` anywhere an ordering can reach an output or a hash is a defect, not a preference.
- **No floating point in an identity.** ADR-0001 §12.5 requires none, and admitting one adds an
  encoding choice with no right answer.
- **Digest spelling is validated separately from digest meaning** — `digest.rs` checks textual
  form; the owning domain module assigns what it identifies.
- **Synthesis and verification keys stay separate** (`identity.rs` vs `verification.rs`), so an
  ASR change cannot invalidate synthesized audio.
- **What feeds a hash is recorded, not implied.** A tool version, a bundle digest, or a format
  choice that changes the output belongs *in* the recorded inputs.

## API and schema evolution

Canonical: `schema.rs`, `contract.rs`, `worker_protocol.rs`.

- **`SchemaVersion::accepted_by` is the whole rule**: a major above this build is refused, an
  older minor is accepted. Do not hand-roll a second version comparison anywhere.
- **A version policy and the module enforcing it name each other.**
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` ↔ `schema.rs`, per the two-sided
  coupling rule in `rust-comment`. A one-sided mirror drifts.
- **A project-owned format rejects unknown fields** — `#[serde(deny_unknown_fields, rename_all =
  "snake_case")]` on every frame. Tool *output* is the only exemption and `rust-review` records
  it; a format this project defines never qualifies, whoever wrote the file.
- **Enforce the size ceiling before decoding** (`MAX_WORKER_FRAME_BYTES`,
  `MAX_WORKER_REQUEST_ID_BYTES`). A parser that allocates first is a denial-of-service surface on
  a local pipe.
- **Identity fields parse as value objects at the boundary**, so a frame naming a bundle or voice
  profile that is not a digest is refused at the frame, not downstream at the cache.
- **A rule both sides enforce names the other side.** `worker_protocol.rs` ↔
  `worker/study_tts_worker/protocol.py`.

## Not settled here

Named so they are visible, with no rule attached — do not infer one from this file:

- Supply chain: `cargo-deny` / `cargo-audit`, license gating outside release, MSRV pinning.
- Performance method: `docs/perf/BUDGETS.md` sets budgets; nothing states how to measure before
  optimizing, or what earns a benchmark.
- Observability: structured logging, error-context chains, and the exit-code taxonomy behind
  `--json`.
- Debugging method for a nondeterministic failure, which `docs/testing/TEST-STRATEGY.md` calls a
  defect without saying how to chase one.

Proposing a rule for any of these is a conversation with the user, not an edit to this file.
