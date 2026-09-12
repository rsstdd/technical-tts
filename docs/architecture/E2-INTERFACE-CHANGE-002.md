# E2 Interface Change 002 — Where a tool is installed stops being an identity

## Identification

- Record ID: `E2-INTERFACE-CHANGE-002`
- Status: **Accepted 2026-09-12.** Every row in §Approval is signed.
- Contract owner: T-RUNTIME (`transaction_identity`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUDIO (package reuse), T-RUNTIME
- Accepted ADR, if architectural: **not architectural.** ADR-0001 §12.5 fixes what a *synthesis*
  identity hashes and this touches none of it. No accepted document names the transaction identity's
  inputs.
- Governing issue: #92, which `E2-INTERFACE-CHANGE-001` §Open questions **G-A** delegated and issue
  #82 task 4 owned

## Version and compatibility

- Contract ID: `transaction_identity`
- Old version: `0.3-skeleton-transaction`
- New version: `0.4-skeleton-transaction`
- Compatibility class: **compatible.** No published schema, no durable document, and no field
  changes. The constant separates concurrent staging directories and is written into no artifact.
- What left: `ffmpeg_executable` and `ffprobe_executable`, the two `resolved_executable` paths.
- What stayed: `ffmpeg_version`, `ffprobe_version`, `lesson_id`, `plan_hash`, every argument-profile
  digest, and `text_renderer_version`.

Reuse changes with it, in the same direction and for the same reason:
`manifest::tools_match` no longer compares the recorded `resolved_executable` against the live tool.
`version` is the surviving comparison.

## What a recorded executable is for, decided

**The path is provenance. The version is identity.**

An absolute path is a poor identity proxy in both directions, and the failure is symmetric:

- the **same** path holds a **different** binary after an upgrade — caught by `version`, not by the
  path;
- the **same** binary at a **different** path invalidated every staging transaction and every
  package, for no difference a consumer could observe.

`version` is not a bare number. The manifest records
`"ffmpeg version 6.1.1-3ubuntu5 Copyright (c) 2000-2023 the FFmpeg developers"` — the tool's own
name and its distro build string. `resolved_executable` adds only the install location, which is
exactly the operator-specific part and exactly what changes between a distro package, a container,
and a second machine running the identical build.

This is the question `E2-INTERFACE-CHANGE-001` answered once already, for arguments: a recorded path
naming where the build ran made two builds of one lesson differ, and the answer was to stop
recording it that way. The executable path is the same mistake one field over.

**`resolved_executable` is not removed from the manifest.** It remains a required field of published
`manifest 4.0`, listed in `PUBLISHED_REQUIRED_SURFACE`, and a manifest missing it is still refused —
`tools_match` names it without comparing it, in the idiom `parse_stored_manifest` already uses. What
the field records and what reuse reads are now two different questions, which is the point.

## Identity effect

| # | Identity | Verdict | Where it is enforced |
|---|---|---|---|
| **I-1** | `transaction_identity` | **Moves** | `preview.rs::transaction_identity`. In-flight staging transactions strand and are rebuilt |
| **I-2** | Synthesis and cache keys | **Do not move** | Nothing here reaches a synthesis request or a cache key; ADR-0001 §12.5's inputs are untouched |
| **I-3** | `plan_hash` | **Does not move** | No plan field is added or reinterpreted |
| **I-4** | Package identity | **Does not move** | No manifest field changes value. A package's digest is the digest of its own manifest, and the manifest still records the same executable string it always did |
| **I-5** | Reuse of an existing package | **Widens** | `tools_match` compares one field fewer. Every package reusable before this remains reusable; some that were not now are |
| **I-6** | Verification, takes, `job_state` | **Do not move** | Not reached |

**I-5 is the row that matters, and its direction is what makes this cheap.** A reuse comparison that
is removed cannot invalidate anything: no published package is stranded, nothing rebuilds, and no
cache entry is orphaned. G-A anticipated that changing this field would "rebuild every existing
package" — true of *changing* the comparison, false of *removing* it, and that distinction is why
the decision landed where it did.

## Impact

- **Existing artifacts:** none migrated, rewritten, or deleted. Every published manifest keeps the
  absolute path it recorded, and every one stays readable and reusable.
- **In-flight work:** a staging transaction begun under `0.3` is not resumed under `0.4`. Staging is
  transient by construction and nothing durable is keyed by it, which is why this is the cheapest
  identity in the tree to move.
- **Security, rights, and privacy:** no control is waived. The change *narrows* what an identity
  reads; it does not widen what a document records. Whether the manifest should keep recording an
  absolute path is deliberately **not decided here** — see §Deliberately deferred.
- **Determinism:** two machines running the same FFmpeg build now produce the same transaction
  identity for one plan, which they did not before. That is a strengthening of determinism, not a
  weakening.

## Deliberately deferred

**What the manifest records is not changed and is not decided.** Replacing the absolute path with a
basename or a placeholder is a semantic change with no field change, which
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes makes a **Breaking
contract**: `manifest` `5.0-skeleton`, a `v5` schema, a `PUBLISHED_REQUIRED_SURFACE` re-key, and a
fifth frozen decoder immediately after `E2-INTERFACE-CHANGE-001` §Open questions **G-B** observed
that decoders are accumulating.

That price is worth paying if the publication and distribution definition `DELIVERY-PLAN.md` §11
owes before G3 says a manifest leaves the machine that made it. §11 does not exist yet. Issue #92
carries the deferral and issue #82 task 6 carries the obligation to record the outcome there.

**The ordering is the point:** with the path feeding no identity and no comparison, changing what is
recorded is now a pure provenance edit whose only cost is the version move. Before this record it
would have moved a transaction identity and a reuse comparison as well.

## Delivery and recovery

One commit. `TRANSACTION_IDENTITY_VERSION`, the struct, its construction, and the reuse comparison
are red across every seam between them: a build that hashes a struct it no longer fills does not
compile, and a reuse comparison left reading a field the identity dropped would pass its own tests
while disagreeing with the identity about what a toolchain is.

Recovery needs nothing. An operator with a half-finished staging directory from `0.3` sees it
abandoned and a new one created, which is the normal path for any identity move and is what
`preview::transaction_stage` already does.

## What this signature moves in the freeze charter

`docs/architecture/G1-FREEZE-CHARTER.md` §The inventory is derived claims every `*_IDENTITY_VERSION`
constant declared in `crates/*/src/*.rs` appears below "as a frozen row, or in §Deliberately not
frozen with the reason", and states the cost of the gap: "A charter that freezes a subset while
reading as complete leaves the remainder unfrozen and nothing reports it."

**Four such constants exist and only three are listed.** `SYNTHESIS_IDENTITY_VERSION`,
`VERIFICATION_IDENTITY_VERSION`, and `WORKER_BUNDLE_IDENTITY_VERSION` are frozen rows;
`TRANSACTION_IDENTITY_VERSION` is in neither list. That was true before this change and is not
caused by it — the constant has been absent since the charter was accepted 2026-09-02.

**On signature this record adds it to §Deliberately not frozen, not to the frozen rows.** An earlier
draft of this section proposed the frozen list and left the choice open as G-A. Reading the
charter's own criterion settles it the other way.

§Deliberately not frozen admits a constant that is read by one module and never by a consumer.
`JOB_LOCK_SCHEMA_VERSION` is the closest parallel and the charter states its reason in terms that
transfer without adjustment: "The strict record inside `build.lock`, read by one module and never by
a consumer. Its version exists so a record from another build is refused as `IncompatibleJobLock`
rather than misread."

`transaction_identity` is that shape exactly. It is read only by `preview.rs`, it is written into no
artifact — no manifest, package, cache entry, or report records it — and its version exists so a
staging directory from another build is abandoned rather than resumed into. Moving it refuses
in-flight work; it does not break a consumer, because it has none.

The argument for freezing was that moving it strands work. That is true and is not the criterion:
moving `JOB_LOCK_SCHEMA_VERSION` strands a lock the same way, and the charter lists it as not frozen
regardless. What the frozen list protects is a surface someone outside the build depends on, and
nothing outside this build can see a transaction identity.

So the row records the constant, its current value, and why it is not frozen — which keeps §The
inventory is derived's promise that every such constant appears in one list or the other, without
claiming a compatibility surface that does not exist.

## Open questions

None. **G-A is resolved above**, in the §Deliberately not frozen direction, rather than carried
forward: a record that names the charter row it moves should not also leave open which row that is.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Contract owner (T-RUNTIME) | Accept `transaction_identity` `0.4-skeleton-transaction` and that the two tool paths leave it while both tool versions stay |Accepted — Ross Todd, 2026-09-12 |
| Affected track (T-AUDIO) | Accept that package reuse compares one field fewer, that this widens reuse rather than narrowing it, and that no published package is stranded or rebuilt |Accepted — Ross Todd, 2026-09-12 |
| Engineering owner | Accept that in-flight staging transactions strand, and that `resolved_executable` remains a required published field that nothing reads |Accepted — Ross Todd, 2026-09-12 |
| Project owner | Accept the `transaction_identity` row entering the charter's **§Deliberately not frozen** list at `0.4`, closing a gap open since 2026-09-02, and that it names no compatibility surface because it has no consumer |Accepted — Ross Todd, 2026-09-12 |

- Effective version and date: `transaction_identity` `0.4-skeleton-transaction`, effective 2026-09-12.

## Amendments

| Date | Amendment | Approval |
|---|---|---|
