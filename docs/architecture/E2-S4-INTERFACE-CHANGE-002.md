# E2-S4 Interface Change 002 — The package publishes its run report, and the manifest checksums it

## Identification

- Record ID: `E2-S4-INTERFACE-CHANGE-002`
- Status: **Proposed.** No row in §Approval is signed.
- Contract owner: T-AUDIO, which owns both contracts this record moves
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUDIO, T-CLI, T-RUNTIME
- Accepted ADR, if architectural: ADR-0001 §14, whose required-measurement list names "assembly and
  encoding durations". This record implements that line; it changes no architecture.

## Version and compatibility

**Two contracts move for one reason**, as `E0-S4-INTERFACE-CHANGE-001` carried
`cache-publication` and `package-writer` together. Both are T-AUDIO's.

- Contract ID: `package_writer`
- Old version: `e0.package-writer.2.0`
- New version: `e0.package-writer.3.0`
- Compatibility class: **breaking**. `PreparedPackageWriter::write` returns
  [`PackageWriteOutcome`] where it returned `PackagePublication`. Every implementation changes
  signature, so nothing compiles against the old shape by accident.
- Required/defaulted fields: one — `PackageWriteOutcome::sealed`, an `Option<RunReport>`. `Some`
  where this call assembled, encoded, and published; `None` where it selected a package an earlier
  build produced. Absence is not "no report": a reused package keeps the one sealed beside it.
- Unknown-field behavior: not applicable. This is a Rust API, not a serialized format; nothing here
  reaches the wire.
- Wire or Rust representation changed: **Rust only.** No published schema, manifest field, or
  durable document carries `PACKAGE_WRITER_CONTRACT_VERSION` or the new types.

- Contract ID: `manifest`
- Old version: `2.0-skeleton` (`MANIFEST_SCHEMA_VERSION` `2.0`)
- New version: `3.0-skeleton` (`3.0`); `schemas/manifest-v2.schema.json` retired for `v3`
- Compatibility class: **breaking**. `artifacts` gains a required `run_report` entry.
- Unknown-field behavior: refused. `deny_unknown_fields` on every stored shape.
- Unknown-version behavior: refused at the version, before any field is decoded.

**`2.0-skeleton` stays readable and can never be reused.** `parse_stored_manifest`'s final arm
returns an error rather than a non-match, so a layout it does not name makes reconciliation *fail*
on a workspace holding such a package. `2.0-skeleton` is on `main`, so those workspaces exist. A
frozen `StoredManifestV2` and its own six-name artifact list read it; every other stored shape is
shared, because none of them moved. It carries no `JsonSchema`: it is read and never published, and
giving it one would put a second definition of a format into the generated schemas.

**Reuse now requires the whole recorded artifact set, and that is a fix this change forced.** Before
it, `validate_package` compared the plan hash, the tool profiles, the renderer, and the take
selection — none of which the run report touches, because no tool produces it. A `2.0-skeleton`
package would therefore have been **reused** by a build that publishes seven artifacts, and the
manifest a consumer read would have checksummed a report that package never held.
`t4_e2_the_previous_layout_is_read_and_rebuilt_rather_than_refused` is what found it, and
`records_every_artifact` is what closes it.

**Why the return type moved rather than the publication growing.** `PackagePublication` derives
`Eq`, and two tests compare it whole: `provisional_contracts.rs:374` against the fake writer and
`:456` inside `t4_e1_the_real_package_writer_passes_the_shared_contract`, both asserting that a
second write selects the package the first published. The second write takes the early-return reuse
branch and performs no assembly and no encoding, so a document that moved with the build would make
every reused package unequal to the one it reuses. `E2-S3-INTERFACE-CHANGE-001` §G-A recorded the same
trap when it kept a join finding off this type; this record follows it.

**Why `sealed` is an option and not a report.** The package's report describes the package; the
build's report describes the build. On a fresh write they are one document. On a reuse they are two,
and the caller needs its own — an earlier design returned the package's report on both paths, and
two tests caught it: the reused write reported the first build's durations, and a warm build claimed
two segments synthesized when it had synthesized none.

**Why the durations are `Measured` and not numbers.** A build that reused a package spent nothing on
it, which is a different statement from spending no time. `Unavailable::ReusedFromCache` says so, and
`PackageTimings::reused()` is the one constructor the writer's reuse branch and the fake share.

## Identity effect

**None.** `PACKAGE_WRITER_CONTRACT_VERSION` appears in exactly two places in the source — its
declaration in `crates/study-tts-runtime/src/package_port.rs` and its re-export in `lib.rs` — and
nowhere in `schemas/`, `fixtures/`, or `evidence/`. It reaches no cache key, no synthesis key, no
manifest field, and no durable document. The durable identity that gates package reuse is
`manifest::CURRENT_MANIFEST_LAYOUT_VERSION` together with the recorded artifact set, the
tool-profile comparison, and `text_renderer_version` — the first of which this record adds, for the
reason §Version and compatibility gives.

`E1-S4-INTERFACE-CHANGE-001` reached the same conclusion when it moved this constant from `1.0` to
`2.0`, and `docs/INDEX.md` records it: "No synthesis, verification, or cache identity moves."

## Impact

- Synthesis identities affected: none
- Verification identities affected: none
- Plan, takes, or package identities affected: none
- Consumers and commands affected: one — `pipeline::render_attempt`, which uses the sealed report
  where there is one and its own where there is not. No CLI command reads a package writer directly.
- Fakes and shared suites affected: `FakePackageWriter`, `RecordingPreparedPackageWriter`, and
  `run_package_writer_contract_scenario`, which now returns `[PackageWriteOutcome; 2]`
- Fixtures and schemas affected: `schemas/manifest-v2.schema.json` is retired and
  `manifest-v3.schema.json` published; `PUBLISHED_REQUIRED_SURFACE` records the new surface under
  `manifest 3.0`. The run report's own move to `2.0` is the first amendment to
  `E2-S4-INTERFACE-CHANGE-001`, which owns that contract.
- Existing cached artifacts affected: **none.** No cache entry records either contract version, and
  a rebuilt package re-encodes without re-synthesizing a segment.
- Published packages or accepted takes affected: **none are migrated, rewritten, or deleted.** A
  `2.0-skeleton` package stays readable and validates; the next build of the same lesson writes a new
  generation beside it, because it records six artifacts where this build publishes seven. That is
  the disposition `E1-S4-INTERFACE-CHANGE-001` set for the two layouts it demoted.

## Why the package holds a document ADR-0001 §12.1 does not list

§12.1's `output/` tree names six artifacts, `manifest.json`, and a `quality-report.json` nothing
writes yet. It does not name a run report, and it places that tree under `jobs/<job-id>/output/`
while this build publishes to `previews/<lesson-id>/packages/<manifest-blake3>/`.

That tree is neither exhaustive nor current. `docs/architecture/WALKING-SKELETON.md` is what governs
the published package in practice, and its step 15 moves from six package files to seven here.
`quality-report.json` shows §12.1 already envisages a report-shaped file inside the package, so this
follows its intent rather than contradicting its list — but the list does not name the run report,
and a reviewer checking §12.1 deserves that stated rather than discovered.

**The report had to go inside the package.** The manifest can only checksum a document that already
exists, and the manifest is written inside `write`; sealing the report into the staging directory is
what lets the rename publish both or neither. A report written to the job directory first would be
durable before the package existed, so a crash between them would leave a report describing a
package that never appeared.

## The reference runs one way only

The manifest names the report. The report names no manifest. `DELIVERY-PLAN.md`'s M2 acceptance
requires human approval "without a checksum cycle", and the same discipline binds any pair where one
document checksums the other.

It is prevented by construction rather than by assertion: `RunReport` has no field that can hold a
manifest digest, and the exhaustive destructure in
`t1_e2_run_report_units_and_missing_values_follow_schema` is what proves the field list. A test
scanning the published bytes for the word would prove less than the type already does.

## What is measured, and what is deliberately not

`assembly_micros` spans `assembly::assemble`, this binary's own PCM work. `encode_micros` sums both
`export::encode` calls, the M4A and the MP3, each derived independently from the master.

Both are **supervisor** measurements. Assembly is Rust in this process and encoding is FFmpeg
running under it, so neither can be read against `docs/perf/BUDGETS.md`, whose figures describe the
Python worker.

**Two spans sit outside both on purpose, and the pair does not account for the write.**
`export::normalize_master`, the loudness pass E2-S3 added, runs over the master before either
encode; the three `ffprobe` validations prove the outputs rather than produce them. A third
`normalize_micros` field is the obvious next question and is refused here: ADR-0001 §14 names two
durations, and adding a third uninvited is scope this record cannot justify.
`docs/observability/RUN-REPORT-FIELDS.md` states the exclusion so a reader does not read the pair as
a full account.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: **yes**, in that order, as
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change procedure step 4 requires.
  `FakePackageWriter` and `run_package_writer_contract_scenario` moved first, then
  `PreparedFileSystemPackageWriter`, then `pipeline.rs`.
- Migration procedure: **none required.** No durable artifact records this contract version, so
  there is nothing in either layout to migrate. In-tree callers change signature and the compiler
  finds every one.
- Rollback procedure: revert the commit. Nothing durable was written under the new contract, so a
  rollback leaves no artifact behind that the old code would misread.
- Compatibility evidence: `t4_e1_the_real_package_writer_passes_the_shared_contract` and the fake's
  equivalent both assert the reuse equality that motivated the shape, and each now also asserts the
  timings differ where the publications do not — the first write observed, the second reused.
- Mapped tests and qualification rerun: package changes map to E1-S4/E2-S3 per §Change procedure.
- Walking skeleton result: recorded at approval.

## Open questions

**G-C — whether the loudness pass earns its own duration. Answered 2026-09-09: yes.**
`normalize_micros` joins the two ADR-0001 §14 names, on a line the set can be drawn on without
arbitrariness: **measure what produces bytes, not what validates them.** Assembly produces the
master, normalization rewrites it — two FFmpeg passes, measure then apply, neither skippable — and
encoding produces each lossy output; the three `ffprobe` calls produce nothing and stay outside. The
three durations now account for every byte-producing span of the package write.

The excluding argument was also wrong on its own terms. It offered `wall_micros` minus the two
published timings as answer enough; `wall_micros` spans the whole build up to the seal and synthesis
dominates it, so that subtraction yields synthesis plus everything else rather than the packaging
residue. `docs/observability/RUN-REPORT-FIELDS.md` carried the same false claim and is corrected
with this.

## Approval

| Role | Decision | Signature |
|---|---|---|
| Contract owner (T-AUDIO) | Accept `e0.package-writer.3.0`, the `PackageWriteOutcome` return shape, and `manifest` `3.0-skeleton` with its seventh artifact | |
| Engineering owner | Accept the empty migration, on the evidence that no durable artifact records this contract version | |
| Affected-track reviewers (T-CLI, T-RUNTIME) | Accept the two supervisor timings and the stated exclusions | |
| Effective version and date | `e0.package-writer.3.0` and `manifest` `3.0-skeleton`, on signature | |

## Amendments

| Date | Amendment | Approval |
|---|---|---|
