# Evidence Report: m2-acceptance-lesson-render-v1

- Status: Proposed
- Governing story/gate: issue #80, an M2 acceptance gap found by audit and not a
  `DELIVERY-PLAN.md` Version 3 story; gate M2/G3
- Hypothesis or decision: whether the five-minute canonical lesson renders end to end through the
  real Chatterbox worker and publishes a complete package, and what that render costs in wall time
  and cache
- Owner: Engineering owner
- Date/time and timezone: rendered 2026-09-06, 21:11:22 to 21:44:30 CEST (UTC+02:00); measured
  2026-09-07
- Environment ID: `reference-wsl2-d9d550f06b783405`, per `docs/operations/REFERENCE-ENVIRONMENT.md`

Opened at the story's implementation, per `evidence/README.md` §Accepting a record at its gate. It
is the one record issue #80 keeps. It answers **AC5** (render through the real worker and keep
the package), **AC7** (measured render time and cache growth), and now **AC6**: the M2 acceptance
listening session was taken on 2026-09-07 and approved the package for private preview with no
finding. All three are recorded below.

**Status stays `Proposed`.** The listening is signed; the record is not, because `evidence/README.md`
§Accepting a record at its gate accepts a story's record once, at the gate it serves, and this one
serves M2. `DELIVERY-PLAN.md:738` states M2 acceptance over a lesson that also survives
interruption, supports a selected retake, emits a complete run report, and records immutable
human approval without a checksum cycle. The run report is **E2-S4** (issue #17) and does not
exist. So the signature this record still lacks is one nobody is yet in a position to give.

## Acceptance criteria, stated before the results

| # | Criterion, as issue #80 words it | Where it comes from |
|---|---|---|
| AC5 | "Render it through the real worker on the reference environment and keep the package" | `DELIVERY-PLAN.md:738` M2 acceptance requires a reviewed five-minute canonical lesson that "produces the complete package" |
| AC7 | "Record the measured render time and resulting cache growth; `OQ-12` sets no cache budget, so this is the first real number anyone has for private-preview cache size" | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` OQ-12; ADR-0002 §Required evidence obliges per-run wall time and RTF retention while its waiver stands |

A pass on AC5 is seven artifacts under
`previews/m2-durable-publication/packages/<manifest digest>/` and a manifest that validates. AC7
carries no threshold to pass or fail against — OQ-12 sets none — so its rows below record
measurements rather than verdicts, which is what "the first real number" means.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Lesson rendered | `m2-durable-publication`, lesson schema `3.1`, 34 segments | `fixtures/lessons/m2-durable-publication.json` | `b34ace96a262376a892116a92871d71660721e1855362d4bee6dd5bffda1af81` |
| Loudness references applied | `PROVISIONAL_LOUDNESS_TARGET_LUFS` `-27.0`, `PROVISIONAL_TRUE_PEAK_CEILING_DBTP` `-1.0`, amended 2026-09-06 | `docs/adr/deviations/ADR-0001-D012-provisional-loudness-and-discontinuity.md` | `5928a42451d6de2ee241bfbed06161469b6d03eb8365d3a950250c2c913015c2` |
| Launcher configuration | schema `1.1`, CPU device, `threads` 4, seed 42 | `worker/launcher.json` | `147385c95b4b46d4e732b58e191b96ba92d56f063a6c0f77153d31d98ac8b905` |
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` | Computed by `WorkerBundle::verified_hash` at launch | — |
| Model | `ResembleAI/chatterbox` at `1b475dffa71fb191cb6d5901215eb6f55635a9b6` | Governed model root; `PINNED_MODEL_REVISION` | — |
| Voice profile | `owner-fallback-v1`, resolved at `VoiceUse::PrivateSynthesis` | Governed voice root | — |
| FFmpeg / ffprobe | `6.1.1-3ubuntu5`, `/usr/bin/ffmpeg` and `/usr/bin/ffprobe` | Recorded in the manifest's `tools` object | — |

Context references, pinned by nothing here because no result below was measured against their
bytes: `docs/adr/ADR-0001-production-rust-study-guide-tts.md`,
`docs/adr/ADR-0002-model-hardware-voice-format-qualification.md`, `DELIVERY-PLAN.md`,
`docs/operations/REFERENCE-ENVIRONMENT.md`, `docs/architecture/E2-S3-INTERFACE-CHANGE-001.md`,
`docs/testing/TEST-DATA-MANIFEST.md`, `evidence/rights/rights-m2-lesson-sources-v1/record.md`.

The executed FFmpeg arguments are not pinned from source either, and deliberately: the manifest
records every invocation verbatim with its argument-profile digest, so the published package is its
own witness and a later edit to `crates/study-tts-runtime/src/export.rs` cannot silently change
what this render ran.

## Procedure

```bash
cargo run --release --package study-tts-testkit --example package-render -- \
  --bundle-root . \
  --model-root data/models/chatterbox \
  --voice-root data/voices \
  --lesson fixtures/lessons/m2-durable-publication.json \
  --output-root data/qualification/m2-package-2026-09-06-211122
```

`package-render` refuses an existing `--output-root`, so the run began with an empty workspace and
synthesized all 34 segments. No cache entry was reused, which is what makes the cache figure below
a growth-from-zero measurement rather than a delta.

Measurements were taken afterwards over the published bytes:

```bash
ffprobe -v error -show_entries format=duration -show_entries stream=codec_name,sample_rate,channels \
  -of default=noprint_wrappers=1 <package>/lesson.wav
ffmpeg -nostdin -hide_banner -nostats -i <package>/lesson.wav \
  -af loudnorm=I=-27.0:TP=-1.0:LRA=7.0:print_format=json -f null -
du -sb <workspace>/cache <workspace>/previews <workspace>/jobs
```

## Results

### AC5 — the package

| Measurement | Threshold | Result | Pass/fail |
|---|---|---|---|
| Artifacts published | 7 | 7: `lesson.wav`, `lesson.m4a`, `lesson.mp3`, `transcript.txt`, `transcript.vtt`, `chapters.ffmetadata`, `manifest.json` | Pass |
| Package identity equals `manifest.json` BLAKE3 | equal | `c07575fead79dc123d092cf746ee96680a62fdde1c03c673fdb326e69625486c`, both | Pass |
| Master duration | about five minutes | 309.30 s, 7,423,200 frames | Pass |
| Master format | mono, 24 kHz, 32-bit IEEE float | `pcm_f32le`, 24 000 Hz, 1 channel | Pass |
| Segments rendered | 34 | 34, all `selected_take` 0, `take_selection_source` `implicit` | Pass |
| `join_continuity` | empty with no retake | `[]` | Pass |
| Release status | `private_preview` | `private_preview` | Pass |
| File modes | owner-only | `0600` on all seven | Pass |

`plan_hash` `9ac74305ded21d1182c28306a4ff0396b1115d700b54895c0931427d2b44e285`; manifest schema
`2.0-skeleton`; text renderer `1.0-skeleton-text-renderer`.

### AC5 — loudness, which is why there is a second render

The first render of this lesson, on 2026-09-06 at 12:18, **refused**:
`ToolError::LoudnessNotLinear { normalization_type: "dynamic" }`, after 29 minutes. Synthesis and
assembly had completed; packaging did not. That refusal is E2-S3's control working, and it is what
produced the measurements `docs/architecture/E2-S3-INTERFACE-CHANGE-001.md` §Measured loudness
records and `ADR-0001-D012`'s amendment of 2026-09-06 acts on. The target moved from `-16.0` to
`-27.0` LUFS because `-16.0` is unreachable beneath a `-1.0 dBTP` ceiling for audio this voice
reference produces. **The refusal was not weakened**; the reference it enforces was corrected.

This run applied, from the manifest verbatim:

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

Measured integrated loudness of the assembled master before normalization was `-34.50` LUFS, so
the applied gain was `+7.48` dB.

### AC7 — render time

| Measurement | Value |
|---|---|
| Wall time, end to end | 1 988 s (33 min 8 s), including release compilation of two crates in 25.02 s |
| Master duration produced | 309.30 s |
| End-to-end wall time ÷ audio duration | **6.43** |
| Segments synthesized | 34, none reused |
| Pool size | one |
| Thread budget | 4, applied to `OMP_NUM_THREADS`, `MKL_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `NUMEXPR_NUM_THREADS` |
| Peak RAM | **not captured**; see §Deviations and limitations |
| Worker identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Hardware identity | `reference-wsl2-d9d550f06b783405` |

**6.43 is not ADR-0002's RTF and must not be read as one.** ADR-0002 records a worst single-worker
RTF of `14.9804` against a `6.0` target, measured at pool size one with three Torch intra-op
threads over qualification fixtures. The figure above is a different quantity measured under a
different thread budget: it covers the whole build — model load, 34 syntheses, assembly, two
loudness passes, three probes, and two encodes — divided by the audio produced, and it says
nothing about whether the CPU performance gate is met. That gate is waived for development scope
by ADR-0002 and its expiry conditions are unchanged by this run.

### AC7 — cache growth, the first real private-preview number

Grown from an empty workspace by one render of 309.30 s of audio.

| Measurement | Value |
|---|---|
| Cache total | 27 081 710 bytes (25.83 MiB) |
| Cached audio | 27 035 912 bytes across 34 `.wav` files |
| Cached metadata | 45 798 bytes across 34 `.json` files |
| Lock files | 34, zero bytes |
| **Cache per audio-minute** | **5.01 MiB** |
| Published package | 37 752 814 bytes (36.00 MiB) across 7 artifacts |
| Job staging left behind | 60 999 bytes; the master is moved into the package rather than copied |
| **Durable cost of one published five-minute lesson** | **61.83 MiB**, cache plus package |

5.01 MiB per audio-minute is close to the arithmetic floor for the format: mono 24 kHz 32-bit float
is 5.49 MiB per minute of sound, and the cache stores segment audio without the inter-segment
pauses the timeline inserts. So this figure is a property of the audio format ADR-0001 §13.1 fixes,
not of cache overhead, and a caching scheme cannot improve it without changing the stored format.

OQ-12 sets no budget, so nothing here passes or fails. What it now has is a rate: **about 5.0 MiB of
cache and 7.0 MiB of package per audio-minute, 12.0 MiB together**, which is what a budget can be
written against.

## Raw artifacts

Governed output. Located by root rather than reproduced here, per
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Artifact | Governed location | Checksum (BLAKE3) | Retention |
|---|---|---|---|
| `lesson.wav` | `m2-package-2026-09-06-211122/workspace/previews/m2-durable-publication/packages/c07575fe…/` beneath the governed qualification output root | `06a786e9dc039b8ed00412b64b772a514491149d142e50adbf47ce4a5b91ea79` | Until M2 acceptance is recorded |
| `lesson.m4a` | as above | `221ce362703d592741be088ae0e4d99264c726c65b43ced50e5f7e77a2130e47` | as above |
| `lesson.mp3` | as above | `80535a2d27ce77b36028d126746230e2bdd621ef4670e5f115435c14f4b102a2` | as above |
| `transcript.txt` | as above | `849aac8c9abcc02eb020eddfec03e04a8054a7094f11ef6e3289ad5ab51e632b` | as above |
| `transcript.vtt` | as above | `84fa853a7c3a4bd7b28209433a677e09434114df707d74bc35cb9717de7a4247` | as above |
| `chapters.ffmetadata` | as above | `38811e1d8e32dd973fbd404d74453439bbb3b81209a13f8d8f0f4998d2b0c703` | as above |
| `manifest.json` | as above; its digest names the package directory | `c07575fead79dc123d092cf746ee96680a62fdde1c03c673fdb326e69625486c` | as above |

## Listening — AC6, taken and signed

Issue #80's remaining criterion was the human review against
`docs/operations/PREVIEW-REVIEW-CHECKLIST.md`. It is **the M2 acceptance review** and a different
session from issue #79, which reviews the three-segment retake material.

**Taken 2026-09-07, 18:17 CEST (UTC+02:00), on built-in laptop speakers. Approved for private
preview. No finding.**

One thing about this material reached the listener before the session, and is recorded because a
later reader cannot infer it from the verdict: **the master is quiet on purpose.** It sits at
`-27.02` LUFS, roughly 11 dB below an ordinary spoken-word delivery level, because that is the
loudest this voice reference permits beneath a `-1.0 dBTP` ceiling. That is the recorded constraint
`ADR-0001-D012` names, not a defect in the audio, so a finding saying only "too quiet" would record
something already known. The session judged what is audible after the volume is raised.

The criteria are the checklist's, transcribed rather than restated, so this review is comparable
with the four that preceded it — which is the drift `PREVIEW-REVIEW-CHECKLIST.md` exists to stop.
No criterion was added or removed.

### Attribution

| Field | Value |
|---|---|
| Reviewer identity and role | Ross Todd, code owner and project owner |
| Playback environment | Built-in laptop speakers on the reference host `reference-wsl2-d9d550f06b783405`. Room not recorded |
| Date and time, with timezone | 2026-09-07, 18:17 CEST (UTC+02:00) |
| Artifact digests judged | The seven in §Raw artifacts above, unchanged since the 2026-09-06 render |

**Built-in speakers bound what this session could answer.** They do not resolve fine level drift or
encoder artifacts, so `Loudness` is answered `not reachable` on every row and the `lesson.mp3`
artifact item is left unchecked below. That is the form `e2-s2-retake-listening-review-v1` used for
the same limit, and it is not `none`: `none` would claim a judgment the equipment could not make.

### Per-segment findings

Timecodes are computed from `start_frame` and `frames` in the published manifest, so they name the
audio actually published rather than the plan. Roles are the lesson's own.

The session was a full-package listen against the criteria, and the listener reported no finding of
any kind. So `none` on a row records that the package was listened through with nothing found at
that segment — not thirty-four separately annotated judgments. `Loudness` is `not reachable` for
the reason above.

| Segment | Start–end | Role | Content | Pronunciation | Voice | Joins | Loudness | Continuation | Disposition |
|---|---|---|---|---|---|---|---|---|---|
| `seg-0001` | 0:00.00–0:09.92 | `problem` | none | none | none | none | not reachable | none | accept |
| `seg-0002` | 0:10.62–0:19.58 | `prerequisite` | none | none | none | none | not reachable | none | accept |
| `seg-0003` | 0:20.08–0:26.92 | `prerequisite` | none | none | none | none | not reachable | none | accept |
| `seg-0004` | 0:27.62–0:35.82 | `definition` | none | none | none | none | not reachable | none | accept |
| `seg-0005` | 0:36.32–0:45.52 | `definition` | none | none | none | none | not reachable | none | accept |
| `seg-0006` | 0:46.32–0:53.00 | `recall_prompt` | none | none | none | none | not reachable | none | accept |
| `seg-0007` | 0:55.50–1:05.30 | `answer` | none | none | none | none | not reachable | none | accept |
| `seg-0008` | 1:06.00–1:14.00 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0009` | 1:14.50–1:21.78 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0010` | 1:22.28–1:30.48 | `why_it_works` | none | none | none | none | not reachable | none | accept |
| `seg-0011` | 1:31.28–1:42.28 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0012` | 1:42.78–1:54.50 | `pseudocode` | none | none | none | none | not reachable | none | accept |
| `seg-0013` | 1:55.40–2:03.84 | `why_it_works` | none | none | none | none | not reachable | none | accept |
| `seg-0014` | 2:04.54–2:10.66 | `plausible_error` | none | none | none | none | not reachable | none | accept |
| `seg-0015` | 2:11.36–2:20.08 | `correction` | none | none | none | none | not reachable | none | accept |
| `seg-0016` | 2:20.58–2:25.58 | `recall_prompt` | none | none | none | none | not reachable | none | accept |
| `seg-0017` | 2:28.08–2:36.04 | `answer` | none | none | none | none | not reachable | none | accept |
| `seg-0018` | 2:36.84–2:43.08 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0019` | 2:43.58–2:49.70 | `challenge` | none | none | none | none | not reachable | none | accept |
| `seg-0020` | 2:50.20–2:58.28 | `clarification` | none | none | none | none | not reachable | none | accept |
| `seg-0021` | 2:58.78–3:05.78 | `correction` | none | none | none | none | not reachable | none | accept |
| `seg-0022` | 3:06.58–3:18.06 | `why_it_works` | none | none | none | none | not reachable | none | accept |
| `seg-0023` | 3:18.76–3:24.84 | `definition` | none | none | none | none | not reachable | none | accept |
| `seg-0024` | 3:25.34–3:33.54 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0025` | 3:34.04–3:43.44 | `example` | none | none | none | none | not reachable | none | accept |
| `seg-0026` | 3:44.24–3:50.28 | `recall_prompt` | none | none | none | none | not reachable | none | accept |
| `seg-0027` | 3:52.78–4:00.30 | `answer` | none | none | none | none | not reachable | none | accept |
| `seg-0028` | 4:01.10–4:07.78 | `explanation` | none | none | none | none | not reachable | none | accept |
| `seg-0029` | 4:08.28–4:14.76 | `why_it_works` | none | none | none | none | not reachable | none | accept |
| `seg-0030` | 4:15.56–4:26.28 | `example` | none | none | none | none | not reachable | none | accept |
| `seg-0031` | 4:26.78–4:36.70 | `example` | none | none | none | none | not reachable | none | accept |
| `seg-0032` | 4:37.50–4:50.02 | `synthesis` | none | none | none | none | not reachable | none | accept |
| `seg-0033` | 4:50.72–4:58.28 | `compressed_rule` | none | none | none | none | not reachable | none | accept |
| `seg-0034` | 4:59.18–5:08.70 | `recap` | none | none | none | none | not reachable | none | accept |

`seg-0006`, `seg-0016`, and `seg-0026` are the three recall prompts, each followed by a 2 500 ms
response interval before its answer. `Continuation` on those rows is the one place this lesson's
ADR-0001 §13.2 response interval is judged, and the interval was accepted as sufficient.

### Package review

Judged once for the package, not per segment. Three items are answered by the listener and three by
measurement over the published bytes, and which is which is stated rather than blurred: a
laptop-speaker listen cannot confirm caption timing to the frame, and measurement cannot hear.

- [x] Segment order and completeness are correct. *Listener; the package was heard through.*
- [x] Chapters and captions align with the audio. *Measured: 34 VTT cues against 34 manifest
      segments at 0.0000 s worst drift; 34 chapters whose starts match segment starts and whose
      ends match segment end plus trailing pause exactly, contiguous with no gap or overlap, the
      last ending at frame 7 423 200 — the master's own length. Every segment's `spoken_text`
      appears in `transcript.txt`.*
- [x] WAV, M4A, and MP3 all play. *Listener played the package; all three additionally decoded
      end to end under FFmpeg with zero errors, at 309.30 s, 309.30 s, and 309.36 s — the MP3's
      60 ms being encoder frame padding.*
- [ ] `lesson.mp3` carries no encoder artifact absent from the master. **Not reachable** on
      built-in speakers, per §Attribution. Unchecked rather than answered.
- [x] No source text, diagnostic data, or voice-reference path leaks into artifact metadata.
      *Measured with `ffprobe -show_entries format_tags -show_entries stream_tags` on all three:
      the only tags present are muxer-written — `encoder: Lavf60.16.100` on each, plus M4A brand
      and handler fields. No lesson text, no path, no diagnostic value.*
- [x] Every finding above has a disposition.

Both encodes were produced with `-map_metadata -1`, and the metadata item is checked against that
claim rather than assumed from it. The manifest is a different matter and is not artifact metadata:
it embeds absolute host paths, which §Deviations and limitations records against issue #82.

### Disposition

- [x] **Approved for private preview** — Ross Todd, code owner and project owner, 2026-09-07
- [ ] Approved for the stated production scope
- [ ] Rejected; correction required

**The second box is not available to this material and was not left blank by oversight.** Approving
a production scope requires an explicit accepted takes selection; `take_selection_source` on this
package is `implicit` and every segment holds take 0, and the checklist forbids a generated
take-zero selection backing a production claim. `release_status` remains `private_preview`.

### What this session could not arbitrate

- **Fine level drift and encoder artifacts**, which built-in speakers do not resolve. Named in
  §Attribution rather than left implied.
- **Any join between two performances.** `join_continuity` is empty because nothing was retaken, so
  there is no retake join here to judge. That question is issue #79's material, not this one's.
- **The loudness references themselves.** `-27.0` LUFS and `-1.0` dBTP stay provisional under
  `ADR-0001-D012` until ADR-0003 is accepted, and this approval does not promote them. The same
  holds for the provisional join band.

## Deviations and limitations

- **Peak RAM and a per-segment thread measurement were not captured.** ADR-0002 obliges retention
  of "per-run wall time, RTF, peak-RAM, thread-budget, worker identity, and hardware identity in
  qualification and run reports" while its waiver stands. This record carries five of the six;
  `package-render` observes no memory. The structured run report is **E2-S4**'s subject (issue
  #17), and this is a concrete requirement for it rather than an omission to be filled in by hand
  afterwards.
- **One delivery style.** Issue #80's AC2 asks for "segment-role and delivery-style variety". The
  lesson carries 16 of the 17 declared roles and three recall prompts, each with a 2 500 ms
  response interval, but every segment is `calm_explanatory` — one of four styles the `3.1` schema
  allows. This is deliberate and is already disclosed in `docs/testing/TEST-DATA-MANIFEST.md`:
  ADR-0001 §13.4 freezes one loudness reference per voice-profile hash **and style**, and no
  reference is frozen for any style yet. A second style would have added a second uncalibrated
  loudness surface to a render whose first attempt failed on loudness. It stays a gap against AC2
  as worded, to be closed when E5-S1 freezes per-voice references.
- **Synthesis is not reproducible across output roots, and this render proves it.** The refused run
  of 12:18 assembled a 305.34 s master; this one assembled 309.30 s from the same lesson, seed, and
  worker identity. ADR-0001 states the rule — "rerunning a nondeterministic model from the same
  synthesis request is not a byte-reconstruction guarantee" — and this is the first five-minute
  observation of it. Byte-identical reconstruction requires the retained cache artifact, which the
  27 MiB above is.
- **The margin against the true-peak ceiling is 1.42 dB for this lesson only.** A lesson with a
  louder transient will refuse again, correctly and by name. If that recurs, a true-peak limiter
  becomes the answer rather than a lower target, and that is ADR-0003's decision, not this record's.
- **Absolute paths appear in the published manifest**, including the operator's home directory.
  Issue #82 tracks it. The manifest above is governed output and was read, not redistributed.
- **The environment is the constrained WSL2 allocation**, not a production reference machine.
  ADR-0002 §Expiry is unchanged by anything here.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | | |
| Project owner | Ross Todd | | |
