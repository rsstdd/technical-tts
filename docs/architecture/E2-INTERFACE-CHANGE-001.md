# E2 Interface Change 001 — A recorded argument stops naming the directory it ran in

## Identification

- Record ID: `E2-INTERFACE-CHANGE-001`
- Status: **Accepted 2026-09-09.** Every row in §Approval is signed.
- Contract owner: T-AUDIO (`manifest`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUDIO, T-RUNTIME
- Accepted ADR, if architectural: **not architectural.** No accepted document names the manifest's
  recorded arguments. The two that reach nearest are
  `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access, whose "only identifiers,
  checksums, redacted paths" rule is written for the repository rather than for a published
  package, and `docs/operations/REFERENCE-ENVIRONMENT.md` §Root verification, which keeps raw
  absolute paths in the private environment record and redacts them from the committed one. This
  change extends that established treatment to a durable artifact; it does not inherit a rule that
  already covered it. Issue #82 is the finding, and it is not a `DELIVERY-PLAN.md` story: it names
  no gate, no evidence record, and no named test of record.

## Version and compatibility

- Contract ID: `manifest`
- Old version: `3.0-skeleton` (`MANIFEST_SCHEMA_VERSION` `3.0`)
- New version: `4.0-skeleton` (`4.0`); `schemas/manifest-v3.schema.json` retired for `v4`
- Compatibility class: **breaking**. No field is added, removed, or retyped — a recorded argument
  stops naming the staging root it ran in, which §Change classes calls a *semantic change* and
  answers with a major exactly as it answers a required field.
- Required/defaulted fields: **unchanged.** Not one field is added, removed, retyped, or given a
  default. That is what makes the version the only signal a reader has.
- Unknown-field behavior: refused. `deny_unknown_fields` on every stored shape.
- Unknown-version behavior: refused at the version, before any field is decoded.
- Wire or Rust representation changed: **wire values only.** `ExecutionRecord.arguments` becomes an
  owned `Vec<String>` because the substituted strings do not outlive the borrow they replaced;
  no serialized type, field name, or JSON shape moves.

**The version selects the rule; it does not prove compliance.** Nothing in the document's shape
distinguishes a manifest whose `tools.executions[].arguments[]` names `/home/<user>/…` from one
naming `{staging}/lesson.wav`. A `4.0-skeleton` reader therefore refuses any absolute recorded
argument before treating the document as redacted. The version tells the reader which rule to
enforce; untrusted contents still have to satisfy it. The published schema states the rule:
`arguments` carries a `description` naming the `{staging}` substitution, so a consumer reads what
changed rather than inferring it from a version number. `parse_stored_manifest` enforces it.

**One decoder reads both layouts.** `3.0-skeleton` holds exactly the fields `4.0-skeleton` holds, so
`parse_stored_manifest` decodes it through `StoredManifest` rather than a frozen copy that would
duplicate every field to say nothing new. This differs from `2.0-skeleton`, whose artifact set
genuinely differs and which keeps `StoredManifestV2`.

**`3.0-skeleton` is readable and can never be reused, and that is not automatic here.** Every
earlier demotion in this tree became unreusable for free, because its artifact set or profile set
differed. This one does not: the shapes are identical, so `records_every_artifact` and `tools_match`
both pass. `PackageRecord::redacted_arguments` is the one field that separates them, and without it
`current.json` would keep selecting a manifest naming the operator's home directory — closing the
finding for new packages only, which is not closing it.
`t4_e2_an_unredacted_layout_is_read_and_rebuilt_rather_than_refused` is the proof.

## What a recorded argument is for, amended

`export.rs`'s `ToolExecution` documented the rule this record changes: *"The arguments as they were
passed, not as they were composed: a manifest that records an intended command line rather than the
executed one cannot be used to reproduce a build."*

That rule stands, with one substitution. The argument **vector** is the provenance; the staging
directory it happened to run in is not. It carries a username and a private filesystem layout that
a consumer of the package has no use for — and it is stale as well as private, because
`preview::publish_transaction` renames that directory into the package, so an unredacted manifest
names a path that no longer exists.

Everything else survives: the flags, the codec settings, the measured loudness values, the order.
`t4_e2_a_recorded_argument_names_no_path_outside_the_package` asserts both halves, so redaction
cannot quietly become deletion. The amendment is written into the doc comment itself, not only
here.

The alternative — recording the profile identity and dropping the vector — was refused. It would
satisfy privacy by removing the thing the field exists for, and
`argument_profile_blake3` already publishes that identity beside the vector.

## Identity effect

| Identity | Verdict |
|---|---|
| Synthesis, verification, plan, takes, cache | **None move.** No key input is touched |
| Package | **Moves.** The manifest's bytes change, and its digest names the package directory |
| Transaction | **Does not move.** `transaction_identity` keys on `argument_profile_blake3`, not concrete arguments |
| Reuse | Gains one comparison: a build that redacts cannot reuse a package that did not |

`tools_match` compares recorded profile digests rather than argument text, so redacting the text
moves no reuse decision by itself.

## The second-order consequence, confirmed and bounded

Issue #82 asked whether randomized names in recorded arguments mean two builds of one lesson never
share a package identity, and asked for a re-render to confirm. Reading answered it: `encode`
staged through `Builder::new().prefix("lesson-")` with no `.rand_bytes(0)`, while
`normalize_master` had carried one since E2-S3 pinned it. **E2-S3 pinned one staging name and not
the other.** `encode` now matches it; nothing collides, because the job lock makes the staging
directory single-writer and the two formats' suffixes differ.

**Two of the three causes are closed and the third is deliberate.** With the staging name pinned
and the root redacted, two builds of one lesson in two workspaces produce byte-identical manifests
**except** for `artifacts.run_report.blake3` — the sealed report carries this build's elapsed times,
which the current E2-S4 implementation began checksumming. Package identity therefore remains
per-build under the still-Proposed `E2-S4-INTERFACE-CHANGE-002`, not because of this defect.
`t4_e1_only_the_run_report_makes_two_builds_of_one_lesson_differ` asserts exactly that, and would
fail again if either closed cause returned.

Whether a package should be content-addressed at all is a question the Proposed
`E2-S4-INTERFACE-CHANGE-002` addresses; this record does not decide it.

## Impact

- Synthesis identities affected: **none.** No key input is read, written, or reordered.
- Verification identities affected: **none.** No ASR input is touched.
- Plan, takes, or package identities affected: **package only**, per §Identity effect. New
  packages carry a redacted manifest and therefore a new digest; old packages stay immutable and
  are rebuilt rather than reused. Plan hash and take selection do not move.
- Consumers and commands affected: none. No CLI command reads a recorded argument.
- Fakes and shared suites affected: none. `FakePackageWriter` records no executions, so
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change procedure step 4 has nothing to
  move before its consumers — the fake could not observe this change if it tried.
- Fixtures and schemas affected: `manifest-v3.schema.json` retired, `manifest-v4.schema.json`
  published, `PUBLISHED_REQUIRED_SURFACE` re-keyed to `manifest 4.0`. The two manifest fixtures —
  `e1-s1-manifest-malformed-digests.json` and `e1-s1-manifest-uppercase-cache-key.json` — pin
  digest-*spelling* refusals at `1.0-skeleton` and are untouched, so
  `docs/testing/TEST-DATA-MANIFEST.md` does not move. Their pointers reach
  `/tools/executions/0/argument_profile_blake3`, the profile digest beside the vector, and never
  the vector this change rewrites.
- Existing cached artifacts affected: **none.** No cache entry records a manifest layout, and a
  rebuilt package re-encodes without re-synthesizing a segment.
- Published packages or accepted takes affected: **none migrated, rewritten, or deleted.** A
  `3.0-skeleton` package stays readable and validates; the next build of the same lesson writes a
  new generation beside it. That is the disposition `E1-S4-INTERFACE-CHANGE-001` set for the two
  layouts it demoted and the Proposed `E2-S4-INTERFACE-CHANGE-002` describes for
  `2.0-skeleton`.
- Listening: **not owed.** The staged file is renamed to its destination after FFmpeg writes it, so
  the published bytes do not depend on the staged name, and no synthesis, loudness, or assembly
  input changes. The issue's own package review records that container metadata carries nothing
  beyond `encoder=Lavf60.16.100`.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: **not applicable**, for the reason
  §Impact gives — no fake and no shared contract suite records a tool execution.
- Migration procedure: none. No durable artifact records this layout other than the manifest, whose
  version selects its decoder.
- Rollback procedure: revert. A `4.0-skeleton` package is refused by version before any field is
  decoded, so older code leaves it unread rather than misreading it. Nothing written under this
  change is readable-but-wrong to the code it replaces.
- Compatibility evidence: `t4_e2_an_unredacted_layout_is_read_and_rebuilt_rather_than_refused`,
  `t4_e2_a_current_manifest_refuses_an_absolute_tool_argument`, plus the `0.1`, `0.2`, and `2.0`
  cases that already pass unchanged.
- Mapped tests and qualification rerun: the E1-S4 and E2-S3 package tests, per §Change procedure.
  **No requalification.** Qualification measures the worker, and no synthesis input moves.
- Walking skeleton result: recorded at approval.

## What this signature moves in the freeze charter, and what it does not

`docs/architecture/G1-FREEZE-CHARTER.md`'s `manifest` row moves from `2.0-skeleton` to
`4.0-skeleton`, effective on this record's acceptance date, and its §Status records the amendment
as it records `E2-S1-INTERFACE-CHANGE-001`'s.

**The charter skips `3.0-skeleton`, and that is the accurate entry rather than a gap.** The charter
records the version in force, and `3.0-skeleton` never was: `E2-S4-INTERFACE-CHANGE-002` introduced
it and remains Proposed, so no signature ever made it effective. It was implemented, merged, and
superseded without being authorized. The row therefore moves `2.0-skeleton` → `4.0-skeleton` in one
step, and a reader who looks for a `3.0` acceptance will correctly find none.

**This signature accepts the `4.0-skeleton` layout entire, including two things it did not
argue for.** A layout cannot be half-accepted. `build_attempt` and the `run_report` artifact came
from `E2-S4-INTERFACE-CHANGE-002`, whose reasoning no signature had reviewed; accepting `4.0`
accepts the document that carries them. Stated plainly because it is a real widening of what the
contract owner is signing, and because the alternative — refusing to move the row while the code
emits the version — leaves the charter describing a manifest nobody writes.

**Two things this signature does not move.** `package_writer` stays at `e0.package-writer.2.0`:
`E2-S4-INTERFACE-CHANGE-002` owns that contract, it is a Rust API rather than the manifest, and it
is still Proposed. And no `run-report` row is added, because `E2-S4-INTERFACE-CHANGE-001` owns that
contract and is also still Proposed.

That second one leaves a gap the charter's own §The inventory is derived rule creates and this
record cannot close: the rule promises that every `*_SCHEMA_VERSION` constant in `crates/*/src/*.rs`
appears in the charter as a frozen row or in §Deliberately not frozen with a reason, and
`RUN_REPORT_SCHEMA_VERSION` appears in neither. Freezing it at a version no signature authorizes
would be the wrong repair. It is E2-S4's to close, not this record's — noted here because it was
found here.

## Open questions

**G-A — whether `tools.<tool>.resolved_executable` stays absolute.** It is `/usr/bin/ffmpeg`, a
system path rather than a home directory, so it carries far less. It is also both a
`transaction_identity` input (`preview.rs:751,753`) and a reuse comparison (`manifest.rs:1389`), so
changing it strands in-flight staging transactions *and* rebuilds every existing package. Issue #82
task 4 owns it; it is deliberately not decided here.

**G-B — three frozen decoders now accumulate.** Whether a layout older than two majors may be
refused rather than read deserves a decision before a fourth arrives.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate although one person signed them all.

The T-AUDIO row carries the widening §What this signature moves in the freeze charter states: it
accepts the `4.0-skeleton` document entire, `build_attempt` and the `run_report` artifact included,
though `E2-S4-INTERFACE-CHANGE-002` argued for those and is not itself signed by this.

| Role | Decision sought | Status |
|---|---|---|
| Contract owner (T-AUDIO) | Accept `manifest` `4.0-skeleton` and the amended meaning of a recorded argument, including the `3.0-skeleton` fields the layout inherits | Accepted — Ross Todd, 2026-09-09 |
| Engineering owner | Accept the empty migration, and that every existing published package is superseded and rebuilt while no cache entry is stranded | Accepted — Ross Todd, 2026-09-09 |
| Affected-track reviewer (T-RUNTIME) | Accept the pinned encode staging name and the bounded identity claim: two builds differ only by the sealed run report | Accepted — Ross Todd, 2026-09-09 |
| Affected-track reviewer (T-AUDIO) | Accept that `3.0-skeleton` stays readable, is never reusable, and needs no frozen decoder | Accepted — Ross Todd, 2026-09-09 |

- Effective version and date: `manifest` `4.0-skeleton`, effective 2026-09-09.

## Amendments

| Date | Amendment | Approval |
|---|---|---|
