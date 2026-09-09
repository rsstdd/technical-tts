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

**One published schema is added.** It was first drafted at `1.0-skeleton`; the amendments below
fold all E2-S4 fields into the still-unmerged contract at `2.0-skeleton`.

- Contract ID: `run-report`
- Old version: none
- New version: `2.0`, layout `2.0-skeleton`
- Compatibility class: **new contract**. Nothing consumed it before, so nothing can break.
- Required fields: fifteen at the root, listed in `PUBLISHED_REQUIRED_SURFACE` under
  `run-report 2.0`
- Unknown-field behavior: refused. `#[serde(deny_unknown_fields)]` on every struct and on both
  `Measured` variants; no `#[serde(other)]` anywhere.
- Unknown-version behavior: refused **at the parse**, not compared downstream. `schema_version` is
  a `RunReportLayout` newtype whose `Deserialize` accepts only `2.0-skeleton` and whose published
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
| **I-6** | `manifest` schema | **Moved, by `E2-S4-INTERFACE-CHANGE-002`** | Predicted here and made there: the manifest now checksums the sealed report, a **Breaking contract** move to `3.0-skeleton`. The run report joins the package as its seventh artifact, and reuse gained a check on the recorded artifact set so a six-artifact package cannot stand in for one holding seven |

The pipeline now writes partial reports under the job and seals complete reports into packages.
The package manifest checksums the sealed bytes and package validation joins the report back to the
manifest before reuse.

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

### What the stage events add, and what stays out of them

Task 1's stages are new `JobEventKind` variants and nothing else. An internally tagged enum reads
every line whose tag it knows, so a log written before these existed still parses and the version
above stays put. **Adding a field to the `JobEvent` envelope would have been the same mistake as a
version bump**: `deny_unknown_fields` plus a missing required field fails every old line, and
because `validate_event_file` runs before each append, that failure would take every later state
change with it — a job directory that could no longer record that it had advanced.

**ADR-0001 §14's `stage` field is the tag discriminant.** `JobEventKind` is `tag = "kind"`, so
every line already writes the stage's own name; a second `stage` field would be that name repeated.
Renaming the tag to `stage` was not available — it would break every existing line for a spelling.

**Events carry ordering and identity; the run report carries measurements.** `PackageAssembled` and
`PackageEncoded` name a boundary and carry no duration, because a duration in both documents is two
numbers for one fact that can drift apart. `SegmentSynthesized` is the exception ADR-0001 §14 names
directly, pairing `segment_id` with `duration_ms`.

**No package stage can be recorded from inside the writer.** Everything `package_port::write`
stages is non-authoritative until its final rename, so the three package events are appended after
it returns, describing work a crash can no longer discard. That is ADR-0001 §12.3 step 5 applied to
a stage rather than a state.

**The event log names voice profiles and the run report does not.** ADR-0001 §14 lists
`voice_profile` among an event's fields, so `SegmentSynthesized` carries the profile identity, while
`t4_e2_run_report_excludes_sensitive_fixture_content` asserts no profile name appears in the run
report. Both are correct and the asymmetry is deliberate: E2-S4 task 6 and
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` redact voice-reference **paths**, and a profile
identity is not a path — it is the name of a record a reviewer follows to a consent decision. The
run report is stricter than required because it has no field that needs one.

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
| `RunReport`, `RunReportSegment`, `JoinFinding`, `SynthesisTotals`, `RunResources`, `WorstSegment` | structs | The document `schemars` derives the published schema from |
| `Measured`, `Unavailable`, `CacheOutcome`, `ReportCompletion`, `BuildErrorClass` | enums | Closed observation, cache, completion, and failure states |
| `RunReportLayout` | newtype | Gates the layout label at the deserialization boundary |
| `ReportField`, `FieldSemantics` | enum, struct | Task 4's catalogue, mirrored by `docs/observability/RUN-REPORT-FIELDS.md` |
| `MeasurementUnit`, `MeasurementClock`, `MeasuredProcess`, `Aggregation`, `Fidelity` | enums | The closed vocabularies those semantics range over |
| `milli_real_time_factor` | function | ADR-0001 §3.4's ratio, in integers |
| `RUN_REPORT_SCHEMA_STEM`, `RUN_REPORT_SCHEMA_VERSION` | constants | The catalogue entry, and the testkit's contract tests |

The `2.0` move covers the breaking changes from the first draft: segment audio became `Measured`,
completion and error class became one typed state, and join findings became required.

## Impact

- **Synthesis identities affected:** none. See I-1.
- **Existing artifacts:** no artifact is migrated or deleted. A `2.0-skeleton` manifest remains
  readable but cannot satisfy current reuse because it has no run report.
- **Security, rights, and privacy:** no control is waived. The document is structurally incapable
  of carrying the three things the rights policy excludes.
- **Determinism:** identities and ratios use canonical integer representations. Elapsed and sampled
  measurements naturally vary and participate in no synthesis, cache, or plan identity.
- **Recovery:** partial reports are durably replaced under the job; complete reports publish with
  the immutable package.
- **Bounded growth:** `docs/governance/TRACEABILITY-MATRIX.md` names this beside schema,
  redaction, and reconciliation as what E2-S4 owes M2, and it is the one the other three sections
  do not answer. Two documents are involved and only one of them grows. `run-report.json` is
  *replaced*, never appended, so its size follows the lesson rather than the attempt count: one row
  per planned segment, and join findings capped at `MAX_LESSON_SEGMENTS`. `events.ndjson` does
  append, and this story roughly doubled what an attempt writes to it — the log carried only
  `state_durable` before, and now carries a stage line per segment as well. Measured against
  `fixtures/lessons/m2-durable-publication.json`, the largest committed lesson at 34 segments: a
  `segment_synthesized` line serializes to 250 bytes, `state_durable` to 169, and
  `package_published` to 238, giving **76 lines and 15.5 KiB per attempt, so 528 attempts against
  one job directory** before `MAX_JOB_EVENT_LOG_BYTES` is reached. The ceiling refuses rather than
  truncates: `encode_line` checks the prospective size before any write, so a full log keeps every
  line it already held, which `t4_e2_event_log_limits_are_enforced_before_append` proves. Published
  packages each carry one sealed report and accumulate as generations, which ADR-0001 §12.2's
  cache-prune roots govern rather than this record.
- **Tests:** `t1_e2_run_report_units_and_missing_values_follow_schema` proves the vocabulary is
  total and that only the two ratios are ratios;
  `t3_e2_every_published_run_report_number_names_its_unit` proves every published number names its
  unit; and the run report joins the six contract tests every published format already answers to.

## Open questions

**G-A — the advisory join finding, answered 2026-09-09.** `join_findings` carries the ordered
segment pairs whose manifest evidence is `JoinTolerance::Outside`. The ratios stay only in the
manifest. Package validation derives the same pairs and refuses disagreement, so the advisory
finding cannot drift from its evidence.

**G-B — which process the peak-resident sample names. Answered 2026-09-09.** The worker, which is
the process holding Torch; a supervisor figure would measure this Rust binary and say nothing about
the cost that matters. The sample is taken in `render_attempt` once the last segment has resolved
and before assembly — both the last point `/proc/<pid>/status` still answers and the first at which
`VmHWM`, a mark the kernel only ever raises, is the run's true peak.

The representativeness this question worried about turns out to differ between the two figures, and
the field semantics now carry the difference rather than leaving `Fidelity::Approximate` to imply
it. `peak_resident_kib` is a `Maximum` and covers the whole run wherever it is sampled before exit.
`open_handles_count` is `PointInTime`, a variant this step added: `/proc/<pid>/fd` lists what is
open at the instant it is read and the kernel keeps no high-water mark for descriptors, so its
value means only "what the worker held when synthesis finished". It was previously declared
`Total`, which would have licensed a reader to sum it.

## Approval

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept a new published document at `2.0-skeleton` | |
| Contract owner (T-CLI) | Accept `run-report` as the eighth published schema, and the fifteen required root fields recorded for it | |
| Affected track (T-RUNTIME) | Accept that no identity, package, cache entry, or existing schema moves, and that the manifest's move to `3.0-skeleton` is named but not made here | |
| Affected track (T-AUDIO) | Accept that the report publishes both an aggregate and a worst-segment real-time factor, and claims comparability to `docs/perf/BUDGETS.md`'s baseline only narrowly | |
| Engineering owner | Accept the answer to the freeze charter's delegated question: `events.ndjson` stays unpublished and `JOB_EVENT_SCHEMA_VERSION` does not move | |
| Effective version and date | `run-report` `2.0` added, on signature | |

## Amendments

| Date | Amendment | Approval |
|---|---|---|
| 2026-09-09 | **`run-report` moves to `2.0`, layout `2.0-skeleton`.** The final proposed surface adds per-segment rows, stage durations, `join_findings`, and a closed completion state. `ReportCompletion::Incomplete` contains a typed `BuildErrorClass`; complete reports cannot carry an error and incomplete reports cannot omit one. Failed segment rows use `cache_outcome: failed` and measured-or-unavailable audio and synthesis fields. One move covers the additions because no accepted build has published the earlier draft. `schemas/run-report-v1.schema.json` is retired and `run-report-v2.schema.json` replaces it. The `-skeleton` suffix remains because the layout is still provisional. | |
