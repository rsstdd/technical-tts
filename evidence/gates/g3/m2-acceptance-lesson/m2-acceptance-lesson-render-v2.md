# Evidence Report: m2-acceptance-lesson-render-v2

- Status: **Accepted 2026-09-12** at the M2 gate. Every row in §Review is signed.
- Supersedes: `m2-acceptance-lesson-render-v1`
- Governing story/gate: issue #80, an M2 acceptance gap found by audit and not a
  `DELIVERY-PLAN.md` Version 3 story; gate M2/G3
- Hypothesis or decision: whether the five-minute canonical lesson still renders end to end through
  the real Chatterbox worker after E2-S4 moved the run report to `3.0-skeleton`, and whether the
  package it publishes carries every measure accepted ADR-0002's waiver retains
- Owner: Engineering owner
- Date/time and timezone: rendered 2026-09-11, 08:56:05 to 09:23:13 CEST (UTC+02:00)
- Environment ID: `reference-wsl2-d9d550f06b783405`, per `docs/operations/REFERENCE-ENVIRONMENT.md`

**Why this record exists.** `m2-acceptance-lesson-render-v1` measured the 2026-09-06 render and
said, accurately, that the run report M2 requires did not exist. E2-S4 built it, moved `run-report`
to `3.0` / `3.0-skeleton`, and in doing so moved package identity, because the manifest checksums
the sealed report. The listening disposition v1 carries was taken against a package this build can
no longer produce. So the render was repeated and this record measures the result.

**Accepted at the M2 gate on 2026-09-12**, which is the gate this record serves and the only place
`evidence/README.md` §Accepting a record at its gate allows it to be accepted. §M2 acceptance states
the six conjuncts and what discharges each; `e2-s2-retake-listening-review-v1` is accepted in the
same act and discharges conjunct 3.

No listening session was taken against this package and §Listening does not claim one. What it
records is the project owner's decision of 2026-09-11 that the 2026-09-07 session carries forward,
on the evidence in §Determinism that the audio is byte for byte the audio that session judged. The
one criterion `PREVIEW-REVIEW-CHECKLIST.md` `1.0` added after that session, "Protected terms", is
answered `not applicable` by inspection and shown to be so, rather than left open or quietly counted
as clean.

## Acceptance criteria, stated before the results

| # | Criterion | Where it comes from |
|---|---|---|
| AC5 | Render through the real worker on the reference environment and keep the package | `DELIVERY-PLAN.md:738` M2 acceptance |
| AC6 | A human listening review against the checklist | ADR-0001 §17.5; `PREVIEW-REVIEW-CHECKLIST.md` `1.0` |
| AC7 | Record the measured render time and resulting cache growth | issue #80; `OQ-12` sets no budget |
| AC8 | The package emits a **complete run report** carrying every measure ADR-0002 retains | `DELIVERY-PLAN.md:738`; ADR-0002 `:77`, whose waiver is live |

AC8 is new here. v1 could not answer it and said so; it is the reason this render was taken.

A pass on AC5 is the artifact set under
`previews/m2-durable-publication/packages/<manifest digest>/` with a manifest that validates. AC7
carries no threshold — OQ-12 sets none — so its rows record measurements rather than verdicts.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Lesson rendered | `m2-durable-publication`, lesson schema `3.1`, 34 segments | `fixtures/lessons/m2-durable-publication.json` | `b34ace96a262376a892116a92871d71660721e1855362d4bee6dd5bffda1af81` |
| Loudness references applied | `PROVISIONAL_LOUDNESS_TARGET_LUFS` `-27.0`, `PROVISIONAL_TRUE_PEAK_CEILING_DBTP` `-1.0` | `docs/adr/deviations/ADR-0001-D012-provisional-loudness-and-discontinuity.md` | `5928a42451d6de2ee241bfbed06161469b6d03eb8365d3a950250c2c913015c2` |
| Launcher configuration | schema `1.1`, CPU device, `threads` 4, seed 42 | `worker/launcher.json` | `147385c95b4b46d4e732b58e191b96ba92d56f063a6c0f77153d31d98ac8b905` |
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` | Computed by `WorkerBundle::verified_hash` at launch | — |
| Model | `ResembleAI/chatterbox` at `1b475dffa71fb191cb6d5901215eb6f55635a9b6` | Governed model root; `PINNED_MODEL_REVISION` | — |
| Voice profile | `owner-fallback-v1`, resolved at `VoiceUse::PrivateSynthesis` | Governed voice root | — |
| FFmpeg / ffprobe | `/usr/bin/ffmpeg` and `/usr/bin/ffprobe` | Recorded in the manifest's `tools` object | — |

**All three pinned inputs hash to the values `m2-acceptance-lesson-render-v1` recorded.** That is
load-bearing for §Determinism below: the inputs did not move, so a byte difference in the output
would have to come from the code.

Context references, pinned by nothing here because no result below was measured against their
bytes: `docs/adr/ADR-0001-production-rust-study-guide-tts.md`,
`docs/adr/ADR-0002-model-hardware-voice-format-qualification.md`,
`docs/adr/deviations/ADR-0001-D002-constrained-development-performance-gate.md`, `DELIVERY-PLAN.md`,
`docs/operations/REFERENCE-ENVIRONMENT.md`, `docs/operations/PREVIEW-REVIEW-CHECKLIST.md`,
`docs/architecture/E2-S4-INTERFACE-CHANGE-001.md`, `docs/architecture/E2-INTERFACE-CHANGE-001.md`.

## Procedure

```bash
cargo run --release --package study-tts-testkit --example package-render -- \
  --bundle-root . \
  --model-root data/models/chatterbox \
  --voice-root data/voices \
  --lesson fixtures/lessons/m2-durable-publication.json \
  --output-root data/qualification/m2-package-2026-09-11-085605
```

The same invocation v1 records, against a fresh `--output-root`. `package-render` refuses an
existing one, so the run began with an empty workspace and synthesized all 34 segments: no cache
entry was reused, which makes the cache figure a growth-from-zero measurement.

**The 2026-09-06 package was not touched.** It remains at
`data/qualification/m2-package-2026-09-06-211122`, and no file under it has been modified since
2026-09-07. Its immutability is what v1 pins and this record does not disturb.

## Results

### AC5 — the package

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Artifacts published | 7 | 7: `master_wav`, `m4a`, `mp3`, `transcript`, `captions`, `chapters`, **`run_report`** | Pass |
| Files in the package directory | 8 | 8: the seven above plus `manifest.json` | Pass |
| Package identity equals `manifest.json` BLAKE3 | equal | `301c4cf8c944468edd3d05fd35801b5a39b0107ad4bde0ecfd412325dc68e5af`, recomputed from the file | Pass |
| Manifest layout | `4.0-skeleton` | `4.0-skeleton` | Pass |
| Master duration | about five minutes | 309.30 s, 7,423,200 frames | Pass |
| Master format | mono, 24 kHz, 32-bit IEEE float | `pcm_f32le`, 24 000 Hz, 1 channel | Pass |
| Segments rendered | 34 | 34, all `selected_take` 0, `take_selection_source` `implicit` | Pass |
| `join_continuity` | empty with no retake | `[]` | Pass |
| Release status | `private_preview` | `private_preview` | Pass |
| File modes | owner-only | `0600` on all eight | Pass |

`plan_hash` `9ac74305ded21d1182c28306a4ff0396b1115d700b54895c0931427d2b44e285`, identical to v1's;
text renderer `1.0-skeleton-text-renderer`.

### AC8 — the run report, which is why this render was taken

`run-report.json`, 12,623 bytes, `schema_version` `3.0-skeleton`, `completion` `complete`,
34 segment rows, 0 join findings. ADR-0002 `:77` retains six measures until its waiver expires.
**All six are present and observed:**

| ADR-0002 measure | Field | Value |
|---|---|---|
| Per-run wall time | `wall_micros` | 1,627,547,877 µs — 27 min 07.55 s |
| RTF | `synthesis.aggregate_real_time_factor_milli` | `observed` 5 709 — 5.709× |
| Peak RAM | `resources.peak_resident_kib` | `observed` 5,520,888 KiB — 5.27 GiB |
| Thread budget | `resources.thread_budget.worker` | 1 worker process, 4 native threads, 1 interop thread |
| Worker identity | `worker_bundle_hash` | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Hardware identity | `hardware_environment_id` | `reference-wsl2-d9d550f06b783405` |

**v1 carried five of the six and no thread budget or identities, and peak RAM was among its listed
limitations.** All four gaps are closed. The thread budget is not a measurement but a declaration,
and the declaration is checkable against its source: `worker/launcher.json` above declares
`threads` 4, and the report publishes `native_threads_per_worker_count` 4.

Other published figures: `model_load_micros` 16.08 s, `assembly_micros` 5.87 s,
`normalize_micros` 5.48 s, `encode_micros` 6.66 s, `open_handles_count` 5,
`worker_restarts_count` 0. Worst segment `seg-0016`, take 0, 31.27 s of wall for 120,000 frames,
RTF 6.254 — against an aggregate of 5.709, so the slowest segment is 9.5% off the mean rather than
an outlier.

### AC5 — loudness

Applied from the manifest verbatim:

```text
loudnorm=I=-27.0:TP=-1.0:LRA=7.0:measured_I=-34.50:measured_TP=-9.92:measured_LRA=5.70:
measured_thresh=-45.25:offset=-0.18:linear=true:print_format=json
```

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Normalization type | `linear` | `linear`; the build published, and `LoudnessNotLinear` refuses anything else | Pass |
| Published integrated loudness | `-27.0` LUFS target | `-27.02` LUFS | Pass |
| Published true peak | at or below `-1.0` dBTP | `-2.42` dBTP, 1.42 dB of margin | Pass |
| Published loudness range | `7.0` LU | `5.70` LU | Pass |

Measured by re-analysing the published master with the same filter, which reports it as this
render's *input*. Every figure equals v1's, which §Determinism explains.

### AC7 — render time and durable cost

| Measurement | This render | v1, 2026-09-06 |
|---|---|---|
| Wall time | 27 min 07.55 s | 33 min 08 s |
| Whole-build ratio to audio | 5.26× real time | 6.43× |
| Synthesis wall | 1,607.75 s of the 1,627.55 s total | — |
| Aggregate RTF | 5.709 | not published |
| Cache | 25.83 MiB — **5.01 MiB per audio-minute** | 5.01 MiB per audio-minute |
| Packages | 36.01 MiB — 6.99 MiB per audio-minute | — |
| Jobs | 0.08 MiB | — |
| Total durable | 61.92 MiB — **12.01 MiB per audio-minute** | 11.99 MiB per audio-minute |

The cache figure is identical to v1's to the same two decimals, which follows from §Determinism:
the same segment bytes were published. Total durable cost rose by 0.02 MiB per audio-minute, which
is the sealed run report — 12,623 bytes across 5.155 audio-minutes is 0.0023 MiB per audio-minute
in the package, and the rest is the job directory's partial report.

The render is **5 min 59 s faster than v1** on the same inputs and the same machine. Nothing here
establishes why, and this record does not guess: wall time is not a controlled measurement, the two
runs were taken five days apart, and `docs/perf/BUDGETS.md`'s baseline is not what either measured.

### Determinism — six of seven artifacts are byte-identical to the 2026-09-06 render

| Artifact | BLAKE3 | Same as v1 |
|---|---|---|
| `lesson.wav` | `06a786e9dc039b8ed00412b64b772a514491149d142e50adbf47ce4a5b91ea79` | **yes** |
| `lesson.m4a` | `221ce362703d592741be088ae0e4d99264c726c65b43ced50e5f7e77a2130e47` | **yes** |
| `lesson.mp3` | `80535a2d27ce77b36028d126746230e2bdd621ef4670e5f115435c14f4b102a2` | **yes** |
| `transcript.txt` | `849aac8c9abcc02eb020eddfec03e04a8054a7094f11ef6e3289ad5ab51e632b` | **yes** |
| `transcript.vtt` | `84fa853a7c3a4bd7b28209433a677e09434114df707d74bc35cb9717de7a4247` | **yes** |
| `chapters.ffmetadata` | `38811e1d8e32dd973fbd404d74453439bbb3b81209a13f8d8f0f4998d2b0c703` | **yes** |
| `run-report.json` | — | new; no counterpart |
| `manifest.json` | `301c4cf8c944468edd3d05fd35801b5a39b0107ad4bde0ecfd412325dc68e5af` | **no** — v1 was `c07575fead79dc123d092cf746ee96680a62fdde1c03c673fdb326e69625486c` |

The master digest was recomputed from the 2026-09-06 package directly rather than read from v1, so
this row is a comparison of bytes and not of two transcriptions.

**This is `E2-INTERFACE-CHANGE-001`'s prediction, met exactly.** Its T-RUNTIME approval row accepts
"the bounded identity claim: two builds differ only by the sealed run report". Five days apart,
across a manifest move from `2.0-skeleton` to `4.0-skeleton`, a run report from nothing to
`3.0-skeleton`, and an executor contract from `3.0` to `4.0`, every byte of audio, text, and
chapter metadata is unchanged. The package identity moved because the manifest moved, and for no
other reason.

It also bounds what a listening session could newly find — see §Listening.

## Raw artifacts

| Artifact | Governed location | Checksum (BLAKE3) | Retention |
|---|---|---|---|
| `lesson.wav` | `m2-package-2026-09-11-085605/workspace/previews/m2-durable-publication/packages/301c4cf8…/` beneath the governed root | `06a786e9dc039b8ed00412b64b772a514491149d142e50adbf47ce4a5b91ea79` | Governed root; not committed |
| `lesson.m4a` | as above | `221ce362703d592741be088ae0e4d99264c726c65b43ced50e5f7e77a2130e47` | as above |
| `lesson.mp3` | as above | `80535a2d27ce77b36028d126746230e2bdd621ef4670e5f115435c14f4b102a2` | as above |
| `transcript.txt` | as above | `849aac8c9abcc02eb020eddfec03e04a8054a7094f11ef6e3289ad5ab51e632b` | as above |
| `transcript.vtt` | as above | `84fa853a7c3a4bd7b28209433a677e09434114df707d74bc35cb9717de7a4247` | as above |
| `chapters.ffmetadata` | as above | `38811e1d8e32dd973fbd404d74453439bbb3b81209a13f8d8f0f4998d2b0c703` | as above |
| `run-report.json` | as above; checksummed by the manifest as the seventh artifact | recorded in `manifest.json` `artifacts.run_report` | as above |
| `manifest.json` | as above; its digest names the package directory | `301c4cf8c944468edd3d05fd35801b5a39b0107ad4bde0ecfd412325dc68e5af` | as above |

No audio, transcript, or voice reference is committed. The governed root is outside Git.

## Listening — AC6, carried forward from 2026-09-07

**No session was held against this package, and this section does not claim one.** What it records
is a decision the project owner made on 2026-09-11: that the 2026-09-07 session stands for package
`301c4cf8…`, because the material it judged is byte for byte the material this package contains.

### The decision, and what makes it available

§Determinism above establishes that all six media and text artifacts hash to the values the
2026-09-06 package carries — the master digest recomputed from that package directly, not read from
v1. Only `manifest.json` differs, and a manifest is not audio.

`m2-acceptance-lesson-render-v1` §Attribution binds its disposition to "the seven in §Raw
artifacts". Six of those seven are the same bytes here; the seventh is the manifest, which changed
because it moved to `4.0-skeleton` and now checksums the sealed run report. The owner's judgment
is that a disposition about how a lesson *sounds* is bound to the audio it was taken against, and
that re-listening to identical bytes would produce a second record of the same act rather than new
evidence.

**Decision:** Ross Todd, project owner, 2026-09-11 — the 2026-09-07 disposition carries forward to
package `301c4cf8…`. Recorded here rather than by amending v1, which is superseded and immutable.

### What carried forward, transcribed from the session that produced it

Taken **2026-09-07, 18:17 CEST (UTC+02:00)**, on built-in laptop speakers, by Ross Todd as code
owner and project owner, against `docs/operations/PREVIEW-REVIEW-CHECKLIST.md` as it then stood.

- **Disposition: approved for private preview. No finding of any kind**, across all 34 segments.
- `Loudness` was answered **`not reachable`** on every row, not `none`: built-in speakers do not
  resolve fine level drift, and `none` would claim a judgment the equipment could not make.
- The production-scope box was **not available** and was not left blank by oversight.
  `take_selection_source` is `implicit` and every segment holds take 0, and the checklist forbids a
  generated take-zero selection backing a production claim. That is still true of this package.
- The three recall prompts — `seg-0006`, `seg-0016`, `seg-0026` — each carry a 2 500 ms response
  interval, and ADR-0001 §13.2's interval was judged sufficient at those rows.
- `lesson.mp3`'s encoder-artifact item was left **unchecked as not reachable** on that equipment.

The full 34-row finding table, with timecodes computed from the published manifest, is in
`m2-acceptance-lesson-render-v1` §Per-segment findings. It is not copied here: the rows describe
bytes this package shares, and a transcription would be a second copy that could drift from the
record that actually holds the session.

### Re-measured against this package, not assumed

Three of v1's six package-review items were answered by measurement rather than by ear, and
measurement does not carry forward on an identity argument — it was taken over different files, one
of which changed. Each was re-run against `301c4cf8…`:

| Item | Result on this package |
|---|---|
| Chapters and captions align with the audio | 34 VTT cues, 34 manifest segments, 34 chapters |
| WAV, M4A, and MP3 all play | All three decode end to end under FFmpeg with **zero errors**, at 309.30 s, 309.30 s, and 309.36 s — the MP3's 60 ms being encoder frame padding |
| No source text, diagnostic data, or voice-reference path leaks into artifact metadata | Only muxer-written tags: `encoder=Lavf60.16.100` on each, plus M4A brand and handler fields. No lesson text, no path, no diagnostic value |

The figures match v1's, which is what six byte-identical artifacts predict — but they were measured,
not inferred.

### What this carry-forward does not cover

- **The run report is new and no listening judged it.** It is not audio and no checklist criterion
  reaches it; §AC8 measures it instead.
- **`PREVIEW-REVIEW-CHECKLIST.md` `1.0`**, made effective 2026-09-11 by
  `E2-S6-INTERFACE-CHANGE-001`, adds a **"Protected terms"** criterion the file did not carry on
  2026-09-07, so the carried session could not have answered it. It is answered **not applicable**
  here, by inspection rather than by ear — see §Protected terms below.
- **Everything v1's session could not arbitrate still cannot be**: fine level drift and encoder
  artifacts beyond built-in speakers, any join between two performances, and the provisional
  loudness references themselves, which stay provisional under `ADR-0001-D012` until ADR-0003 is
  accepted. This record does not promote them.

### Protected terms — not applicable, and why that is not a dodge

The criterion reads: "A term the lesson **declared** protected, spoken as anything other than its
declared form." It is answered `not applicable` for this package on two independent grounds, both
checkable without listening.

**No lesson can declare a protected term.** `schemas/lesson-v3.schema.json` has no such field, and
the word appears nowhere in `schemas/` or `crates/` outside one test's doc comment. The criterion
names a declaration mechanism the lesson format does not have, so no lesson yet rendered could
answer it as worded. That is a defect in the checklist rather than in this package, and it will
misfire the same way on every lesson until the format gains the field or the criterion is reworded.

**The substantive kind is absent from this material.** `study-tts-core/src/lesson.rs:3116` defines a
protected term as one "ADR-0001 §9.3 forbids rewriting", and its examples are identifier-shaped:
`Number.MAX_SAFE_INTEGER`, `std::collections::BTreeMap`, `O(n log n)`. Across all 34 segments of
`m2-durable-publication`, every shape that could carry one is absent:

| Shape searched in `spoken_text` | Occurrences |
|---|---|
| ALL-CAPS run of three or more | 0 |
| Path or scope operator `::` | 0 |
| Call or notation with `()` | 0 |
| Backtick or code fence | 0 |
| `snake_case` identifier | 0 |
| `camelCase` identifier | 0 |
| Dotted identifier | 0 |
| Any digit | 0 |

and `display_text` equals `spoken_text` on all 34 segments, so nothing is spoken in a form that
differs from what is shown. The lesson is deliberately plain prose — it says "synchronize it" and
"rename it over the destination" where a code-bearing lesson would say `fsync` and
`RENAME_NOREPLACE`.

**A session would not make this more answered.** There is no term in the material for the criterion
to bite on, so a listener could only report the same absence this table reports. Recording `none`
would be the wrong answer, because `none` claims a judgment was made about terms that exist; `not
applicable` says correctly that there were none to judge. The distinction is the one v1 already drew
between `none` and `not reachable`, applied to a third case.

## The approval record — M2's "immutable human approval"

`DELIVERY-PLAN.md:740` requires that the lesson "records immutable human approval without a checksum
cycle". Until 2026-09-12 nothing could satisfy it on real material:
`study_tts_runtime::approve_preview` existed and was tested, and `E2-S6-INTERFACE-CHANGE-001`
signed its contract, but no operator path called it — E2-S5 owns the CLI and has not landed. A
contract nothing has run against a real package has recorded nothing.

`crates/study-tts-testkit/examples/approve-preview.rs` closes that, on the same reasoning
`package-render` gives for existing: the approval is written through the path production will use,
so what this gate reads is what a build produces.

Written 2026-09-12 to
`previews/m2-durable-publication/approvals/301c4cf8….json`, mode `0600`:

| Field | Value |
|---|---|
| `schema_version` | `1.0` — the version `E2-S6-INTERFACE-CHANGE-001` made effective 2026-09-11 |
| `manifest_blake3` | `301c4cf8c944468edd3d05fd35801b5a39b0107ad4bde0ecfd412325dc68e5af` |
| `checklist_version` | `1.0` |
| `reviewer` / role | Ross Todd, project owner |
| `playback_environment` | built-in laptop speakers; session taken 2026-09-07, carried forward 2026-09-11 on byte-identical audio |
| `disposition` | `accepted` |

**The playback field names the carried session rather than a session on 2026-09-12**, because that
is what happened. §Listening holds the reasoning; this document holds the decision, and the two must
not disagree about which ears produced it.

**"Without a checksum cycle" is met by construction, not by assertion.** The approval names the
manifest digest and the manifest has no field an approval digest could occupy, so no cycle is
representable. The document is keyed by that digest rather than by lesson, which is why approving
this generation could not overwrite the record of another — a property
`t4_e2_content_change_invalidates_prior_approval` already pins.

**Not released.** `publish_preview_release` is a second decision under E2-S6 task 7 and the
instrument deliberately does not call it: folding both into one command would let a reviewer release
by approving. `release.json` does not exist for this lesson.

## M2 acceptance, conjunct by conjunct

`DELIVERY-PLAN.md:740` states M2 as six conjuncts. This gate review accepts them together, so each
is stated with what discharges it rather than left to a reader to assemble.

| # | Conjunct | Discharged by |
|---|---|---|
| 1 | A reviewed five-minute canonical lesson produces the complete package | This record, §AC5. 309.30 s, eight files, seven artifacts, manifest `4.0-skeleton`, package `301c4cf8…` |
| 2 | Survives interruption | `t4_e2_interrupt_after_cache_publish_reconciles_on_resume` and `t4_e2_interrupt_before_rename_preserves_prior_state`, through the real filesystem at the write boundary |
| 3 | Supports a selected retake | `e2-s2-retake-listening-review-v1`, accepted at this gate. See §The retake conjunct below |
| 4 | Emits a complete run report | This record, §AC8. `3.0-skeleton`, `complete`, all six measures accepted ADR-0002 retains |
| 5 | Records immutable human approval without a checksum cycle | This record, §The approval record. `approvals/301c4cf8….json`, schema `1.0`, written 2026-09-12 |
| 6 | Remains mechanically identified as non-production | `release_status` `private_preview` and `take_selection_source` `implicit`, which the checklist makes a bar to any production claim |

### The retake conjunct, and why three-segment material discharges it

Conjunct 3 is the only one whose evidence is not this lesson, and the reason is a property of the
build rather than a convenience.

`e2-s2-retake-listening-review-v1` demonstrated a selected retake on `e1-s4-three-segment` and was
listened to and signed 2026-09-05. Its §What this material cannot arbitrate records, verified rather
than inferred, that **the retake produced byte-identical audio to the take it replaced**: all six
BLAKE3 digests match across the two generations. The cause is in the source — `take` is an
ADR-0001 §12.5 synthesis-key input but not a model input, the worker seeds generation from `seed`
alone, and this bundle is characterized `reproducible`.

**So no build of this project can currently produce a retake that sounds different**, on three
segments or on thirty-four. Re-running the retake on `m2-durable-publication` would exercise the
same mechanism, produce the same bytes, and put a listener in front of audio they had already
judged. It would add a second generation and a second package identity and answer nothing new.

The alternative — treating the conjunct as unproved until a non-`reproducible` bundle exists — would
block M2 on a capability the architecture deliberately does not have yet, and the E2-S2 record
already said so: the limit "is not a defect in the E2-S2 acceptance criteria, all of which concern
take identity, artifact preservation, and join assessment, and all of which this material
exercises."

**What stays unproved, and is recorded rather than assumed:** no human listening has verified a
retake *join* — a boundary between two different performances. That needs material this build cannot
produce without moving the seed, which invalidates the takes selection with
`TakesError::StaleSynthesisBaseKey`. It belongs to whoever schedules that session when a bundle can
produce it, and M2 does not require it.

## Deviations and limitations

- **The record is unsigned, and its AC6 rests on a carried-forward session rather than a session
  taken against this package.** §Listening states the decision, who made it, and on what evidence.
  The one criterion the carry-forward cannot answer — `PREVIEW-REVIEW-CHECKLIST.md` `1.0`'s
  "Protected terms" — is named there rather than counted as answered.
- **One delivery style.** Every segment is `calm_explanatory`, one of four the `3.1` schema
  declares, because the launcher parameterises exactly one and the worker refuses every other by
  name. Issue #80's AC2 asks for delivery-style variety; it stays open until E5-S1 freezes
  per-voice loudness references. Unchanged from v1.
- **`take_selection_source` is `implicit`.** No take was explicitly accepted, so this package cannot
  back a production claim, and `release_status` is `private_preview` by construction. Unchanged
  from v1.
- **Wall time is not a controlled measurement.** See AC7.
- **Interruption and retake are not exercised here.** M2 acceptance also requires that the lesson
  survive interruption and support a selected retake. `package-render` does neither; those conjuncts
  are proved by `t4_e2_interrupt_after_cache_publish_reconciles_on_resume` and the E2-S2 retake
  suite, and by `e2-s2-retake-listening-review-v1` for the retake material. This record does not
  claim them.

## Review

| Role | Decision sought | Status |
|---|---|---|
| Engineering owner | Accept the render, the run report's six retained measures, and the determinism result | Accepted — Ross Todd, 2026-09-12 |
| Reviewer (AC6) | Accept that the 2026-09-07 session carries forward to this package on byte-identical audio, and that "Protected terms" is not applicable to this material | Decided — Ross Todd, 2026-09-11; see §Listening |
| Project owner | Accept the package for private preview, and M2's six conjuncts as §M2 acceptance states them | Accepted — Ross Todd, 2026-09-12 |

**Every row above is signed.** No row may be filled by anyone who did not perform the act it names.
The AC6 row records a **decision about** a session, not a session: the act it names is the judgment
that identical bytes need not be judged twice, and that is the act Ross Todd performed on
2026-09-11.

- Effective: **M2 accepted 2026-09-12**, against package
  `301c4cf8c944468edd3d05fd35801b5a39b0107ad4bde0ecfd412325dc68e5af`.
  `e2-s2-retake-listening-review-v1` is accepted at this gate in the same act, discharging
  conjunct 3.
