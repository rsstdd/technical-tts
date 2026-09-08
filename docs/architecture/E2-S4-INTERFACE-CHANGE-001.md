# E2-S4 Interface Change 001 — The run report becomes a published document

## Identification

- Record ID: `E2-S4-INTERFACE-CHANGE-001`
- Status: **Proposed.** No row in §Approval is signed.
- Contract owner: T-CLI (structured output and run reports)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CLI, T-RUNTIME, T-AUDIO
- Accepted ADR, if architectural: ADR-0001 §3.4 and §14. This record implements those sections; it
  changes no architecture.

`docs/governance/ROUTING-TABLES.md` §Decision routing sends a public-schema question to an
interface-change record before merge, and
`docs/governance/GITHUB-PROJECT-PLAYBOOK.md` states that ADRs and repository documents remain the
system of record for decisions. Issue #17 is the working record.

## Version and compatibility

**One published schema is added; none moves.** `run-report` enters `PUBLISHED_SCHEMAS` at `1.0`,
writing the layout label `1.0-skeleton`. The other seven documents are byte-identical, and
`t3_e1_generated_schemas_match_checked_in_files` proves it: regenerating produced a diff in
`schemas/run-report-v1.schema.json` alone.

- Contract ID: `run-report`
- Old version: none
- New version: `1.0`, layout `1.0-skeleton`
- Compatibility class: **new contract**. Nothing consumed it before, so nothing can break.
- Required fields: eight at the root, listed in `PUBLISHED_REQUIRED_SURFACE` under `run-report 1.0`
- Unknown-field behavior: refused. `#[serde(deny_unknown_fields)]` on every struct and on both
  `Measured` variants; no `#[serde(other)]` anywhere.
- Unknown-version behavior: refused **at the parse**, not compared downstream. `schema_version` is
  a `RunReportLayout` newtype whose `Deserialize` accepts only `1.0-skeleton` and whose published
  schema emits that label as a `const`, so the schema and the parser refuse the same bytes at the
  same field. `fixtures/contracts/e2-s4-run-report-foreign-layout.json` proves both halves.

**Why `-skeleton`.** `MANIFEST_SCHEMA_VERSION`'s own doc gives the reason and names this story:
"E2-S3 and **E2-S4** will break this manifest again, so the label must not claim a stability they
are going to take away." The same applies here in the other direction — E2-S4's remaining steps add
stage durations, per-segment rows, and cache outcomes to this document. The major will say the
change was breaking; the suffix says the layout is still provisional.

**Why publish now rather than after the first emission.** `docs/governance/TRACEABILITY-MATRIX.md`
names "schema" first among the four things E2-S4 owes M2, and `DELIVERY-PLAN.md` E6 measures "from
the versioned run-report fields" — a consumer reading it by field name. That is what separates this
document from `events.ndjson` and `publication.json`, which stay internal journals.

## Identity effect

| # | Identity | Verdict | Where it is enforced |
|---|---|---|---|
| **I-1** | Synthesis and cache keys | **Do not move** | Nothing here reaches a synthesis request or a cache key |
| **I-2** | `plan_hash` | **Does not move** | No plan field is added or reinterpreted |
| **I-3** | Package transaction identity | **Does not move** | `ExportProfiles::identities` is untouched |
| **I-4** | Package identity | **Does not move** | No package artifact changes, so no digest moves |
| **I-5** | Reuse of an existing package | **Unaffected** | `manifest::expected_executions` is untouched, so an existing package still matches |
| **I-6** | `manifest` schema | **Does not move yet** | The manifest will checksum this report, which is a later E2-S4 step and a **Breaking contract** move to `3.0-skeleton`. It is named here so the reader knows it is coming, and is not made here |

**Nothing this step adds is written by a build.** The document is defined and published; no code
path constructs one. That is deliberate: the vocabulary is reviewable in a diff before any
measurement is shaped by whatever proved easy to instrument.

## The real-time factor, and why the report publishes two

This is the decision the record exists for. `DELIVERY-PLAN.md` E2-S4 task 3 asks for "aggregate
RTF" and nothing said what that meant. It is three questions, and two of them are already answered.

| Axis | Resolution | Authority |
|---|---|---|
| Window | Synthesis wall time, not whole-build elapsed | ADR-0001 §3.4 — settled, not chosen |
| Denominator | Generated-audio duration, not master duration | ADR-0001 §3.4 — settled, not chosen |
| Statistic | **Both** aggregate and worst-segment, shaped differently | Open; this record decides it |

ADR-0001 §3.4 reads: "CPU real-time factor no greater than `6.0`, measured as synthesis wall time
divided by generated-audio duration, excluding one-time installation and model download." Under
`CLAUDE.md` §Conflict order that binds, so the first two rows were never this story's to choose.

**The statistic was genuinely open, and the two available answers disagree.**
`docs/perf/BUDGETS.md` registers `14.9804` as the baseline. That figure is the **worst of ten**
single-utterance runs: `scripts/qualification/chatterbox_spike.py` opens `time.perf_counter()`
around one `model.generate()` call, divides by that call's own audio, and takes `max`. The delivery
plan asks for an aggregate. Those are different statistics of one ratio and can never be one
number, so the report publishes both and `Aggregation` keeps them apart.

**They are shaped differently on purpose.**
`evidence/gates/g0/e0-s3/e0-s3-g0-requalification-torch-2-10-0-v1.md`, accepted 2026-09-02, fits
this backend as `RTF = fixed / audio + marginal` — about eight seconds of fixed per-take cost plus
a marginal rate — and concludes that "a single RTF number is not meaningful for this backend unless
the utterance length is stated beside it". The gate outcome turns on it: that record measures a
3.08-second utterance failing `<= 6.0` and a 13.64-second utterance passing, on one machine with
one backend. So the worst is published as a `worst_segment` **object** carrying its own
`audio_frames`, while the aggregate is a scalar beside the run's minimum and maximum. Two
structurally identical numbers would invite a reader to average them.

**Nothing else is called a real-time factor.** Whole-build elapsed time is `wall_micros`,
undivided. `evidence/gates/g3/m2-acceptance-lesson/m2-acceptance-lesson-render-v1.md` already had
to spend a paragraph explaining that its end-to-end `6.43` is not ADR-0002's `14.9804`; this
document does not create a second occasion for that.

**Comparability is claimed narrowly.** The worst-segment row is the closest thing to the ratified
baseline, and it is still not the same measurement: the qualification ran three Torch intra-op
threads while `worker/launcher.json` sets `threads: 4`, inside an `unshare --user --map-root-user
--net` namespace this build does not reproduce. Nothing this report emits lifts ADR-0002's
performance waiver, whose expiry requires that protocol rerun as written.

**No floating point, and not for the reason a reader may assume.**
`schemas/manifest-v2.schema.json` already publishes `StoredJoin.loudness_ratio` and `.rate_ratio` as
`"format": "float"`, in a document whose BLAKE3 names the package directory — so "checksummed
therefore integers" is not the rule. `rust-production`'s prohibition scopes to ADR-0001 §12.5's
synthesis-identity inputs. Integers are right here on their own merits: both operands are exact
integers, so the ratio is exact rational arithmetic and ADR-0001's `<= 6.0` gate is checkable
against `6_000` without a platform-dependent float.

## The freeze charter's delegated question, answered

`docs/architecture/G1-FREEZE-CHARTER.md` §Deliberately not frozen withholds exactly one question
for this story: whether the `events.ndjson` line becomes a published schema.

**Answer: no.** `JOB_EVENT_SCHEMA_VERSION` does not move and the event line stays an internal
diagnostic journal, as `publication.json` is. The run report is the artifact a consumer reads and
the manifest will checksum, so that is where the contract belongs. Two mechanical facts support it:
`job_events.rs`'s `validate_event_file` refuses a foreign `schema_version` as
`MalformedJobEventLog`, so any bump rejects every existing log for no gain; and the two documents
have different readers, so one contract would over-promise the diagnostic and under-serve the
report.

## What the document carries, and what it deliberately does not

`docs/observability/RUN-REPORT-FIELDS.md` is the field-by-field statement and names
`ReportField::semantics` in return. Three omissions are decisions rather than gaps:

- **No VRAM.** ADR-0001 §14 says "peak RAM and VRAM where the operating environment exposes them
  reliably"; ADR-0002 pins a CPU-only backend. A `vram_kib` field would be permanently unavailable.
  Named here because an implementer reading §14 alone would add it.
- **No mean segment length.** Derivable exactly from two published fields, and a stored copy of a
  derivable fact can disagree with its inputs — the reasoning `E2-S3-INTERFACE-CHANGE-001` §Open
  questions used to keep the join verdict out of the manifest.
- **No text of any kind.** `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access is
  satisfied structurally: no type here has a field that source text, spoken text, or a
  voice-reference path could land in.

**Task 4 is discharged by a catalogue, not by the wire.** Five of the six semantics ADR-0001 §14
and task 4 require — unit, clock, process, aggregation, fidelity — are constants of the field, so
they live in `ReportField::semantics` and its mirrored document. Writing them into every report
would repeat the same constants per run and could not be wrong at runtime. Only the sixth,
whether a value was observable, varies; that is `Measured`, and it is the only part of the
vocabulary on the wire.

## Published API added

| Item | Kind | Why it is public |
|---|---|---|
| `RunReport`, `SynthesisTotals`, `RunResources`, `WorstSegment` | structs | The document `schemars` derives the published schema from |
| `Measured`, `Unavailable` | enums | The one dynamic semantic, and the closed reasons a value can be absent |
| `RunReportLayout` | newtype | Gates the layout label at the deserialization boundary |
| `ReportField`, `FieldSemantics` | enum, struct | Task 4's catalogue, mirrored by `docs/observability/RUN-REPORT-FIELDS.md` |
| `MeasurementUnit`, `MeasurementClock`, `MeasuredProcess`, `Aggregation`, `Fidelity` | enums | The closed vocabularies those semantics range over |
| `milli_real_time_factor` | function | ADR-0001 §3.4's ratio, in integers |
| `RUN_REPORT_SCHEMA_STEM`, `RUN_REPORT_SCHEMA_VERSION` | constants | The catalogue entry, and the testkit's contract tests |

All are additive. No published item is removed, renamed, or retyped.

## Impact

- **Synthesis identities affected:** none. See I-1.
- **Existing artifacts:** none. No package, cache entry, job document, or event log is read,
  written, migrated, or invalidated.
- **Security, rights, and privacy:** no control is waived. The document is structurally incapable
  of carrying the three things the rights policy excludes.
- **Determinism:** the report is a function of what a build measured. Its ratio is integer
  arithmetic, so it is byte-identical across rebuilds given identical inputs.
- **Recovery:** nothing yet writes the document, so there is nothing to recover. Durable
  finalization is a later E2-S4 step.
- **Tests:** `t1_e2_run_report_units_and_missing_values_follow_schema` proves the vocabulary is
  total and that only the two ratios are ratios;
  `t3_e2_every_published_run_report_number_names_its_unit` proves every published number names its
  unit; and the run report joins the six contract tests every published format already answers to.

## Open questions

**G-A — the advisory join finding, carried forward from E2-S3.**
`E2-S3-INTERFACE-CHANGE-001.md` §Open questions assigns `JoinTolerance::Outside` a destination in
this story's run report. This step defines the document but adds no field for it. The finding is
derivable from the manifest's recorded joins, so the field belongs with the step that populates the
report from a real build rather than with the one that defines its vocabulary. **Still open, still
assigned here.**

**G-B — which process the peak-resident sample names.** The worker holds Torch and is the
interesting number, but `Fidelity::Approximate` is doing work the record cannot yet quantify:
`/proc/<pid>/status` `VmHWM` is unreadable once the process exits, so the sample must be taken
before shutdown and its representativeness depends on when. The step that measures it owes a
statement of when the sample is taken.

## Approval

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept a new published document at `1.0-skeleton` whose layout later E2-S4 steps will break | |
| Contract owner (T-CLI) | Accept `run-report` as the eighth published schema, and the six required-surface rows recorded for it | |
| Affected track (T-RUNTIME) | Accept that no identity, package, cache entry, or existing schema moves, and that the manifest's move to `3.0-skeleton` is named but not made here | |
| Affected track (T-AUDIO) | Accept that the report publishes both an aggregate and a worst-segment real-time factor, and claims comparability to `docs/perf/BUDGETS.md`'s baseline only narrowly | |
| Engineering owner | Accept the answer to the freeze charter's delegated question: `events.ndjson` stays unpublished and `JOB_EVENT_SCHEMA_VERSION` does not move | |
| Effective version and date | `run-report` `1.0` added; no existing schema version moves | |

## Amendments

| Date | Amendment | Approval |
|---|---|---|
