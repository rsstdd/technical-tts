# Run-report fields — what each number means

The human side of `crates/study-tts-runtime/src/run_report.rs`. `ReportField::semantics` is the
machine side and names this document in return; `MeasurementUnit::suffix` owns the unit spellings
below. A change to either without the other is a one-sided mirror, which
`.claude/skills/rust-comment/SKILL.md` §Coupling comments records as a finding.

`DELIVERY-PLAN.md` E2-S4 task 4 requires each field to declare its unit, clock or sampling source,
measured process, aggregation, missing-value semantics, and whether it is exact, sampled, or
approximate under WSL2. The first five are properties of the field rather than of a run, so they
are written here and in the `match` this document mirrors, not into every published document. Only
the sixth — whether a value was observable at all — varies per run, which is why `Measured` is the
only piece of this vocabulary that reaches the wire.

## The fields

| Field | Unit | Clock or source | Process | Aggregation | Fidelity |
|---|---|---|---|---|---|
| `wall_micros` | microseconds | monotonic elapsed | supervisor | total | exact |
| `model_load_micros` | microseconds | monotonic elapsed | worker | total | exact |
| `assembly_micros` | microseconds | monotonic elapsed | **supervisor** | total | exact |
| `normalize_micros` | microseconds | monotonic elapsed | **supervisor** | total | exact |
| `encode_micros` | microseconds | monotonic elapsed | **supervisor** | total | exact |
| `synthesis.wall_micros` | microseconds | monotonic elapsed | worker | total | exact |
| `synthesis.audio_frames` | frames at 24 000 Hz | frame count | worker | total | exact |
| `synthesis.segments_synthesized_count` | count | structural | worker | total | exact |
| `synthesis.aggregate_real_time_factor_milli` | ratio × 1 000 | derived | worker | **aggregate** | exact |
| `synthesis.worst_segment.real_time_factor_milli` | ratio × 1 000 | derived | worker | **worst segment** | exact |
| `synthesis.segment_audio_minimum_frames` | frames | frame count | worker | minimum | exact |
| `synthesis.segment_audio_maximum_frames` | frames | frame count | worker | maximum | exact |
| `resources.worker_restarts_count` | count | structural | worker | total | exact |
| `resources.peak_resident_kib` | kibibytes | `/proc/<pid>/status` | worker | maximum | **approximate** |
| `resources.open_handles_count` | count | `/proc/<pid>/fd` | worker | **point in time** | **approximate** |
| `segments[].synthesis_wall_micros` | microseconds | monotonic elapsed | worker | **segment** | exact |
| `segments[].audio_frames` | frames | frame count | worker | **segment** | exact |
| `segments[].retry_count` | count | structural | worker | **segment** | exact |

Every number's unit is repeated in its own name — `_micros`, `_frames`, `_count`, `_kib`,
`_milli` — following the convention `pause_after_ms` and `total_frames` already set.
`t3_e2_every_published_run_report_number_names_its_unit` walks the generated schema and enforces
it, so a future field cannot leave its unit to a doc comment. Three names are exempt and the test
lists them: `value`, which takes its unit from the field holding it, and `build_attempt` and
`take`, which are identifiers spelled this way in documents that already exist.

## Complete, or as far as it got

`completion` is `complete` or `incomplete`, and it is the field that makes every other one
readable. An incomplete report's totals cover the work that happened before a failure, not the work
the lesson asked for, and nothing else in the document distinguishes the two: a build that stopped
after two of thirty-four segments publishes the same shape as one that finished.

`error_class` names the class of the failure that ended an incomplete build, and is absent from a
complete one. It is a **class and never a message**: `BuildError::class` is a closed vocabulary of
sixteen values, exhaustively matched so a new failure kind cannot report someone else's. A
formatted message was refused rather than trimmed — several failure kinds carry a path, `IoError`
always does, and `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps host paths out of a
published document. A scrubber would have to be right every time; a closed vocabulary cannot be
wrong.

**Where each report lives.** A build that finishes seals its report into the package, beside the
audio it describes, and the manifest records its BLAKE3 — so the report and the package publish
under one rename and are immutable together. The same document is written to
`jobs/<job-id>/run-report.json` as well, because a failed attempt leaves one there and a later
resume that wrote nothing would leave that stale report describing a build that did finish.

An incomplete report is written to `jobs/<job-id>/run-report.json`
through the same fsync → rename → fsync-parent ordering ADR-0001 §12.3 requires of every durable
replacement. Writing it is the last thing a failing build does, and its own failure is deliberately
discarded: a report that cannot be written must not replace the error that made it worth writing.

A build that fails *before* its attempt is opened writes nothing. There is no attempt to report,
and the job directory is not yet guaranteed to exist — the same boundary a gate refusal sits behind,
for the same reason.

## The segment rows, and what they do not add up to

`DELIVERY-PLAN.md` E2-S4 task 2 requires five facts per segment: synthesis
duration, audio duration, cache outcome, retry count, and take. They are one row
per planned segment in `segments`, ordered as the plan renders them.

**`segment` is its own aggregation, not a total.** The run's totals already sum
these rows, so a reader who adds them again double-counts the build. That is the
whole reason the column exists.

**A reused segment contributes to no total.** `synthesis.wall_micros`,
`synthesis.audio_frames`, the two extremes, and both real-time factors are
computed only over segments whose `cache_outcome` is `synthesized` — audio the
worker produced during *this* build. A reused segment is real audio in the
master and no work by this run, and counting it would report a real-time factor
for synthesis that never happened. This is not a marginal case: the second build
of any lesson is entirely reuse, so an error here would live in the common path
and show only in the first run of anything.

`fixtures/contracts/e2-s4-run-report-valid.json` is shaped to make that
falsifiable — three rows, one of them reused and the longest of the three, with
`segment_audio_maximum_frames` naming a shorter one. A reader who takes the
largest row as the largest audio the worker generated gets a different answer
than the document gives.

**`retry_count` is structural.** E5-S3 owns retry, timeout, and lifecycle, and
no code path retries a segment, so the zero is this build's shape rather than an
observation — the same reading `resources.worker_restarts_count` carries, for
the same reason and with the same clock.

## The package timings, and what they leave out

`assembly_micros` spans `assembly::assemble` — this binary's own PCM work, writing the canonical
master from the resolved cache artifacts. `encode_micros` sums both `export::encode` calls, the M4A
and the MP3, each derived independently from that master.

**They are supervisor figures, not worker ones.** Assembly is Rust in this process and encoding is
FFmpeg running under it, so neither can be read against `docs/perf/BUDGETS.md`, whose numbers
describe the Python worker holding Torch.

**The line is production, not validation.** `assembly::assemble` produces the master,
`export::normalize_master` rewrites it — two FFmpeg passes, measure then apply, both unconditional —
and `export::encode` produces each lossy output. The three `ffprobe` calls are outside all of them
because they prove the outputs rather than produce them. So the three durations account for every
byte-producing span of the package write and nothing else.

ADR-0001 §14 names two of the three. The loudness pass is here anyway because E2-S3 added it after
that list was written and it is plausibly the largest single span in the path;
`docs/architecture/E2-S4-INTERFACE-CHANGE-002.md` §G-C records the decision.

**`wall_micros` cannot be used to recover any of them.** It spans the whole build up to the seal and
synthesis dominates it — the M2 acceptance render spent 1,988 seconds, nearly all of it in the
worker, against seconds of packaging. Subtracting these three from it leaves synthesis and
everything else, not a residue worth reading.

**Both are absent when a build selects a package an earlier one produced.** The writer returns
before assembling or encoding anything, so both read
`{ "observation": "unavailable", "reason": "reused_from_cache" }` — the same reason a reused segment
carries, for the same reason.

## When the two process figures are sampled

Both are read from the live worker in `render_attempt`, **once the last segment has resolved and
before assembly begins**. `docs/architecture/E2-S4-INTERFACE-CHANGE-001.md` §G-B asked for this
statement, and it is the answer.

That point is chosen twice over. `/proc/<pid>/status` stops answering the moment the worker exits,
so it is the last moment either figure is available without depending on a caller that may already
have shut the backend down. And `VmHWM` is a high-water mark the kernel only ever raises, so it is
also the first moment the reading is the run's true peak rather than a partial one.

**The two figures are not the same kind of number, and the table says so.** `peak_resident_kib` is
a `maximum`: whenever it is sampled before exit, the answer covers the whole run. `open_handles_count`
is `point in time`, because `/proc/<pid>/fd` lists what is open at the instant it is read and the
kernel keeps no high-water mark for descriptors. Its meaning is exactly "what the worker held when
synthesis finished" — a reader who treats it as a run maximum is reading a guarantee nothing makes.

`model_load_micros` is separate from both, and separate from every real-time factor on purpose:
ADR-0001 §3.4 excludes one-time installation and model download from the ratio, so a slow start
shows up here instead of moving the number `docs/perf/BUDGETS.md` is written against. It spans the
backend's spawn, its `initialize` exchange, and its capabilities exchange — everything a build pays
once before any synthesis.

## The real-time factor, and why there are two of them

**The ratio itself is not a choice this project gets to make.**
`docs/adr/ADR-0001-production-rust-study-guide-tts.md` §3.4 defines it as "synthesis wall time
divided by generated-audio duration, excluding one-time installation and model download". That
fixes both the window — synthesis, not the whole build — and the denominator — generated audio,
not the master, whose inter-segment pauses no synthesis produced.

**What was open is the statistic, and both answers are published.** `DELIVERY-PLAN.md` E2-S4 task 3
asks for an *aggregate* RTF. `docs/perf/BUDGETS.md` registers `14.9804` as the baseline for
"Chatterbox single-worker CPU RTF", and that number is the **worst of ten** single-utterance runs
(`scripts/qualification/chatterbox_spike.py:1089`). They are different statistics of the same
ADR-defined ratio and cannot be one number. The report carries both, and the `Aggregation` column
above is what stops a reader collapsing them.

**The worst is shaped differently from the aggregate on purpose.**
`evidence/gates/g0/e0-s3/e0-s3-g0-requalification-torch-2-10-0-v1.md`, accepted 2026-09-02, fits
this backend as `RTF = fixed / audio + marginal` — roughly eight seconds of fixed cost per take
plus a marginal rate — and concludes that "a single RTF number is not meaningful for this backend
unless the utterance length is stated beside it". So the worst ratio is a fact about one utterance
and is published as a `worst_segment` object carrying that utterance's `audio_frames`, while the
aggregate is a property of the run and sits beside the run's own minimum and maximum. A reader who
sees two structurally identical numbers averages them; a reader who sees an object and a scalar
does not.

**Nothing else in this document is a ratio.** Whole-build elapsed time is published as
`wall_micros`, undivided. A reader who wants it per second of audio can divide, but the document
never offers a second number called a real-time factor that a `<= 6.0` gate does not answer to.

**No floating point.** Both operands are exact integers — elapsed microseconds and audio frames at
a fixed sample rate — so the quotient is exact rational arithmetic and the gate is checkable as
`6_000` in integers. This is a property of these two operands, not a general prohibition:
`schemas/manifest-v2.schema.json` already publishes `StoredJoin.loudness_ratio` as a float, because
its inputs are RMS values with no exact integer form.

## What is deliberately absent

- **VRAM.** ADR-0001 §14 lists "peak RAM and VRAM where the operating environment exposes them
  reliably", and ADR-0002 pins a CPU-only backend. A `vram_kib` field would be permanently
  unavailable, which is a field whose only value is "not applicable".
- **A mean segment length.** It is `audio_frames / segments_synthesized_count`, both published
  exactly. A stored mean is a second copy of a derivable fact that can disagree with its inputs.
- **Any text.** No field here can hold source text, spoken text, or a voice-reference path, which
  is how `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access is satisfied —
  structurally, rather than by a scrubber that has to be right every time.

## Missing is never zero

`peak_resident_kib`, `open_handles_count`, the two ratios, and the segment-length extremes are
`Measured`, a two-variant enum on the wire:

```json
{ "observation": "observed", "value": 6291456 }
{ "observation": "unavailable", "reason": "not_exposed_by_environment" }
```

A document cannot say "missing" and "zero" with the same bytes, and it cannot carry both a value
and a reason — `fixtures/contracts/e2-s4-run-report-unavailable-carries-a-value.json` is refused by
the published schema at `/resources/peak_resident_kib` for exactly that. The reasons are closed:

| Reason | When |
|---|---|
| `not_exposed_by_environment` | ADR-0001 §14's "where the operating environment exposes them reliably", as its negative case |
| `stage_not_reached` | The build ended before the stage that would have measured this |
| `no_audio_generated` | A ratio whose denominator is zero, or an extreme over a run that generated nothing. Distinct from a ratio of zero, which would claim synthesis took no time |
| `reused_from_cache` | A cache entry supplied the segment and this build's worker never synthesized it. Distinct from a duration of zero, which would claim the worker produced the audio instantly |
| `no_worker_process` | The executor ran synthesis with no separate worker process, so there was nothing to sample — or it had one and the process was already gone when the sample was asked for. Distinct from `not_exposed_by_environment`: there, the platform withheld a counter; here, the platform would have answered and no process existed to ask about |

## The layout label is gated, not just recorded

`schema_version` is a `RunReportLayout`, not a string. Its `Deserialize` accepts only
`1.0-skeleton` and its published schema emits that value as a `const`, so a document written by a
future layout is refused where it is parsed rather than compared somewhere downstream.
`fixtures/contracts/e2-s4-run-report-foreign-layout.json` is a complete and otherwise valid report
declaring a layout this build does not read; the schema and the parser both refuse it at
`/schema_version`.

## Related

- `docs/architecture/E2-S4-INTERFACE-CHANGE-001.md` — the record that classifies this document
- `docs/adr/ADR-0001-production-rust-study-guide-tts.md` §3.4, §14
- `docs/perf/BUDGETS.md` — the ratified baseline this report's worst-segment row is comparable to,
  and the thread-budget caveat that keeps the comparison narrow
- `crates/study-tts-runtime/src/run_report.rs` — `ReportField::semantics`, which this mirrors
