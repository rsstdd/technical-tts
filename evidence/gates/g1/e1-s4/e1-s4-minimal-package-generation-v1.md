# E1-S4 — Minimal package generation

- Status: Proposed
- Governing story/gate: `DELIVERY-PLAN.md` E1-S4; gate G1
- Hypothesis or decision: the complete ADR-0001 §13.5 package — master, both lossy exports, transcript, captions, chapters, and a manifest checksumming all six — is produced from one Rust-assembled timeline, published as one atomic directory transaction, and refused before any work when it cannot be produced correctly
- Owner: Engineering owner
- Date/time and timezone: 2026-09-01, local (UTC+00:00 as recorded by the reference environment)
- Environment ID: `docs/operations/REFERENCE-ENVIRONMENT.md`

Opened at the story's implementation and accumulating findings until G1, per
`evidence/README.md` §Accepting a record at its gate. It stays `Proposed`: the story's own
acceptance is a G1 decision, and two of its preconditions are not met here — a real-Chatterbox
three-segment package, and a human listening record for the MP3.

## Provenance

| Input | Identity/revision | URI | Checksum |
|---|---|---|---|
| Lesson fixture | `fixtures/lessons/e0-s0-two-segment.json`, two segments | Repository | SHA-256 `02d3e4e5f777520af7578e182b684eeaebd00f5ea647d7e3a72849b254913dbd` as `docs/testing/TEST-DATA-MANIFEST.md` records |
| Synthesizer | `DeterministicToneWorker`, not Chatterbox | Repository | Deterministic 2,400-frame tone per segment |
| FFmpeg/ffprobe | Ubuntu `6.1.1-3ubuntu5`, built with `libmp3lame` | Reference environment | Identities recorded in `docs/operations/REFERENCE-ENVIRONMENT.md` |
| Interface change | `docs/architecture/E1-S4-INTERFACE-CHANGE-001.md` | Repository | `Accepted`, signed 2026-09-01 |
| Deviations approved | `docs/adr/deviations/ADR-0001-D009-provisional-mp3-profile.md` and `ADR-0001-D010-webvtt-millisecond-caption-projection.md` | Repository | Both `Approved`, signed 2026-09-01 |
| Package reuse inputs | `plan_hash`, the recorded tool and argument-profile set, and `text_renderer_version` | Repository | The third was added by review; `E1-S4-INTERFACE-CHANGE-001` §Impact records the gap and its approval row is `Pending` |
| ADR mirrors closed | `docs/adr/ADR-0001-production-rust-study-guide-tts.md` §13.2 | Repository | Amended to name `crates/study-tts-runtime/src/assembly.rs` and `assembly::verify_recorded_audio` as the enforcement path for its assembly paragraph, and `MIN_RECALL_RESPONSE_MS`/`MAX_RECALL_RESPONSE_MS` in `crates/study-tts-core/src/lesson.rs` as the enforcement path for the one pause-table row code enforces, in the shape §13.1 and §15.3 already use. No decision changed; no in-force record pins this document's digest, and `lesson.rs` has an accounted mismatch against `e1-s1-provisional-contract-baseline-v15` already |

## Acceptance criteria

Stated before the result, per `evidence/README.md`. Accepted when all six hold:

1. The seven `t4_e1_*` names `DELIVERY-PLAN.md` E1-S4 lists are implemented character for character
   and pass.
2. The package holds all six ADR-0001 §13.5 artifacts, each checksummed in `manifest.json`, and the
   manifest validates against its own published schema.
3. Caption boundaries equal the sample boundaries the assembly wrote, read from the caption file
   rather than from the timeline that produced it.
4. Both lossy exports are encoded from the master and never from each other.
5. A refusal that must precede work does: a missing MP3 encoder is reported before any synthesis
   and before any durable state exists.
6. The full verification set passes, and the T4 tier stays inside its five-minute budget.

Criterion 3 cannot be met as ADR-0001 §17.12 states it, and no implementation could meet it:
WebVTT cannot represent every 24 kHz frame boundary, so the flooring this build implements is not
equality for a boundary between milliseconds. It is met **as amended** by
`docs/adr/deviations/ADR-0001-D010-webvtt-millisecond-caption-projection.md`, approved 2026-09-01,
which authorizes the floored projection and rests on the exact frames being retained in
`manifest.json`. The criterion is recorded here in its amended form rather than rewritten, so a
reader can see what was asked, what was delivered, and which record closed the gap.

## Procedure

```text
cargo run --offline --locked --package study-tts-runtime --example generate-schemas
cargo check --offline --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline --workspace --all-targets --locked
cargo test --offline --workspace --doc --locked
python3 scripts/check-rust-conventions.py
python3 scripts/check-evidence-provenance.py
git diff --check
```

The T4 tier was timed separately with
`cargo test --offline --workspace --all-targets --locked -- t4_` over a warm build.

## Results

| Measurement | Threshold | Result | Pass/fail |
|---|---:|---:|---|
| Workspace tests | 0 failures | 419 passed, 0 failed | Pass |
| Doc tests | 0 failures | 8 passed, 0 failed | Pass |
| Clippy `-D warnings` | Clean | Clean | Pass |
| `cargo fmt --check` | Clean | Clean | Pass |
| Rust conventions | 0 violations | 0 violations in 77 files | Pass |
| Evidence provenance | 0 unaccounted mismatches | Clean | Pass |
| Generated-schema drift | None | None; `manifest-v1.schema.json` regenerates byte-identical | Pass |
| T4 tier wall time | 5 minutes | 13.0 s | Pass |
| Whitespace (`git diff --check`) | Clean | Clean | Pass |
| Caption boundary contract | Exact written boundaries | Millisecond floors; non-aligned frames differ | **Fail — acceptance open** |

### The named story tests

| Test | What it pins |
|---|---|
| `t4_e1_master_sample_count_equals_segments_plus_silence` | The master holds 10,560 frames and each segment's recorded `start_frame`, `frames`, and `pause_frames` match a table read against the fixture: 2,880 conditioned speech frames per segment, 1,800 and 3,000 frames of declared silence |
| `t4_e1_caption_boundaries_equal_written_sample_boundaries` | Cue timings parsed **out of `transcript.vtt`** equal `00:00:00.000 --> 00:00:00.120` and `00:00:00.195 --> 00:00:00.315`. Those fixture boundaries happen to land on milliseconds; the passing test does not prove ADR-0001 §17.12 for other frame positions |
| `t4_e1_wav_m4a_and_mp3_pass_structural_validation` | An ffprobe run independent of the build reports one mono `pcm_f32le`, `aac`, and `mp3` stream |
| `t4_e1_paths_with_spaces_and_unicode_are_supported` | A workspace named `prévisualisation « ✓ » dir` builds all six artifacts. The workspace carries them because `PORTABLE_ID_PATTERN` forbids either in a lesson ID, and the workspace is what reaches every FFmpeg path argument |
| `t4_e1_ffmpeg_failure_preserves_master_and_prior_state` | An FFmpeg that clears preflight, encodes the M4A, and then fails the MP3 leaves the prior selection and all six prior artifacts unchanged, leaves the new master recoverable in exactly one abandoned stage, and leaves no partial MP3 |
| `t4_e1_manifest_checksums_match_every_output` | Each of the six artifacts hashes to what the manifest records, under the published path ADR-0001 §13.5 names |
| `t4_e1_lossy_output_is_never_source_for_another_export` | Both recorded FFmpeg encodes take `lesson.wav` as `-i`, and there are exactly two |

### Supporting coverage added

| Test | What it pins |
|---|---|
| `t4_e1_missing_mp3_encoder_fails_before_synthesis_and_durable_work` | An FFmpeg listing no `libmp3lame` is refused as `ToolError::MissingEncoder` with `synthesis_count() == 0` and the workspace never created. The stand-in lists a decoder whose *description* contains `libmp3lame`, so a substring search would wrongly pass |
| `t4_e1_the_real_package_writer_passes_the_shared_contract` | `FileSystemPackageWriter` runs through `run_package_writer_contract_scenario`, which `PROVISIONAL-CONTRACT-BASELINE.md` requires of the real package path before G1 and which only the fake had satisfied |
| `t4_e0_historical_packages_remain_valid_but_cannot_satisfy_current_reuse` | `0.1-skeleton` and `0.2-skeleton` packages stay readable and validate, and none can be reused as a complete package |
| `t4_e1_every_package_artifact_is_checksummed` | Editing any one of the six inside a package is refused by `PackageArtifactChecksumMismatch` |
| `t4_e0_encoding_profile_change_names_a_new_package_generation` | A changed **MP3** profile alone stops a selected package being reused |
| `t4_e0_encoding_profile_change_starts_a_new_generation` | The same change also gives the build its own staging directory. The two tests carry different halves and are named alike; this row is the reuse gate's companion, not a duplicate of it |
| `t1_e1_written_segment_positions_follow_speech_and_silence` | The assembly write loop's own positions, against the same table |
| `t1_e1_frame_positions_render_as_floored_webvtt_timestamps` and `t1_e1_the_widest_frame_position_still_renders` | Millisecond flooring, and that the conversion does not overflow at `u64::MAX` frames |
| `t1_e1_cue_text_cannot_escape_its_own_cue` and `t1_e1_chapter_titles_cannot_escape_their_own_record` | Reviewed `display_text` cannot terminate a cue, introduce a second timing line, or open an FFMETADATA key |
| `t4_e1_text_renderer_change_names_a_new_package_generation` | The manifest records `timeline::TEXT_RENDERER_VERSION`, and a package recording a different one is not reused. Both halves in one test: the second is vacuous without the first |
| `t4_e1_text_renderer_change_starts_a_new_generation` | The same identity reaches the staging directory, so the field cannot leave the hashed transaction document unnoticed |
| `t4_e1_an_incomplete_tool_sequence_is_not_reusable` | Reuse compares the exact `(tool, profile)` sequence, so a dropped probe, a repeated encode, a reassigned tool, or a reordered pair each refuse reuse. Every case keeps the profile *set* whole, which is what the previous set comparison could not see |
| `t4_e1_a_self_contradictory_timeline_is_refused` | A recorded timeline whose boundaries, declared pauses, or master length disagree is refused as `IncoherentPackageTimeline` rather than reused |
| `t4_e1_every_package_file_is_owner_only` | All seven published files are mode `600`. The three text documents went through `fs::write` and landed `0666 & ~umask`, so the transcript and captions were the only world-readable files in a `private_preview` package and their mode moved with the operator |
| `t4_e1_a_symlinked_package_file_is_refused` | `manifest.json` or an artifact replaced by a symlink is refused by `managed::leaf`, not followed out of the package |
| `t1_e1_each_packaged_artifact_is_held_to_its_own_codec` | `pcm_f32le`, `aac`, and `mp3` map to the master and the two exports, pinned without FFmpeg or ffprobe so a swapped arm fails offline |

## Raw artifacts

| Artifact | Governed location | Checksum | Retention |
|---|---|---|---|
| Generated preview packages | Test-local temporary roots, removed on `Drop` | Not retained | None |
| Published manifest schema | `schemas/manifest-v1.schema.json` | Regenerated deterministically by the example above | Repository lifetime |

No generated audio is committed. Every package these tests write lives under a `TempDir` and is
removed when the test ends.

## Listening material

Rendered 2026-09-01 by `cargo run --package study-tts-testkit --example package-render`, driving
`fixtures/lessons/e1-s4-three-segment.json` through the real Chatterbox worker and
`build_preview` — the path production uses, rather than a harness that assembles its own package.
Governed output, so the location is named by root rather than reproduced here, per
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Item | Value |
|---|---|
| Location | `e1-s4-package-2026-09-01-212639/workspace/previews/e1-s4-three-segment/packages/1579a41d…/` beneath the governed qualification output root |
| Lesson | `fixtures/lessons/e1-s4-three-segment.json`, `lesson-three-segment-v1`, committed and registered |
| Segments | 3, one interior with a join on each side |
| Voice profile | `owner-fallback-v1`, resolved at `VoiceUse::PrivateSynthesis` through the same rights gate a build passes |
| Worker bundle identity | `3e1f487cf259cd5b17bdeea16845c14426dbbded76f47732dd06b02198003747` |
| Master | 12.400 s, 24 000 Hz, one channel, IEEE float |
| Package identity | `1579a41dc3570658cec9439d71281bf090a7e57f688b8667631b7b0a9f616096` |

| Artifact | SHA-256 of BLAKE3-addressed bytes | BLAKE3 |
|---|---|---|
| `lesson.wav` | — | `165d8fa6a314ccc0e7c69628598f401d02f698dd83ae2f9c2ccf4a70370fb9c0` |
| `lesson.m4a` | — | `e83ad31ab30e65d1519dd5877af18b5942b7234fb6b118c40cbb80911130bcb4` |
| `lesson.mp3` | — | `bde064f729a82a63d3cc79e8367741f9d78f63562f7027e15c6ea80f9e6e8e77` |

## Review result

**Not yet taken.** The criteria below are fixed before listening, for the reason
`e0-s3-g0-qualification-report-v1.md` states about its own: criteria chosen after hearing the audio
are criteria chosen to fit it. No disposition is recorded here, and none may be entered by anyone
who did not listen.

One artifact, not a blinded set. `listening-render`'s shuffling answers a question this review does
not ask — which line produced which take — because there is one recording and its content is known.
What replaces blinding as the binding control is the digest: a judgment recorded below is bound to
`lesson.mp3` at `bde064f7…`, and a re-render produces different bytes and voids it.

**Reviewer to complete.** Listen to `lesson.mp3` end to end, then to `lesson.m4a` for comparison.
The comparison is not optional: it is the only way to tell an artifact introduced by
`libmp3lame` at `128k` from one already present in the master, and separating those two is the
whole purpose of reviewing the MP3 rather than the package.

| # | Criterion | What a finding looks like | Finding |
|---|---|---|---|
| 1 | Joins | A click, a truncation, or an audible discontinuity at either segment boundary | |
| 2 | Pauses | A silence that does not land where the transcript implies, or that reads as a fault rather than a beat — the 2 000 ms interval after the recall prompt especially | |
| 3 | Encoding | Anything audible in `lesson.mp3` that is *not* in `lesson.m4a`: swirl on sibilants, pre-echo, high-frequency loss | |
| 4 | Continuity | Level, tone, or pace shifting between segments so the three do not read as one recording | |
| 5 | Text integrity | Any word spoken that is not in `transcript.txt`, or any word omitted | |

| Field | Value |
|---|---|
| Reviewer | |
| Date | |
| Playback environment | |
| Overall finding | |
| Disposition | `accept` / `retake` |

**What this review will not cover**, whatever it finds.

- **One voice profile, one style.** `owner-fallback-v1` at `calm_explanatory`, which is the only
  style the worker declares. Nothing here reaches another profile or another style.
- **12.4 seconds.** Long-form continuity, the 45–60 minute soak ADR-0001 §17 requires, and
  listener fatigue are all out of reach of a three-segment lesson.
- **No loudness normalization.** ADR-0001 §13.4's two-pass normalization is E2-S3, so a level
  judgment here is a judgment about unnormalized audio and does not transfer.
- **The MP3 profile is uncalibrated.** `ADR-0001-D009` bounds it and expires at ADR-0003. An
  `accept` records that nothing was audible at `128k` on one lesson in one environment; it does
  not calibrate the profile, and ADR-0003 still owes that.
- **One reviewer, and the environment masks what it masks.** No second listener and no inter-rater
  agreement, on the terms E1-S3's review recorded.

## Deviations and limitations

- **The MP3 profile is uncalibrated.** `ADR-0001-D009`, approved 2026-09-01, bounds the
  permission and expires at ADR-0003, which still records MP3 codec arguments as `Pending`. This
  record does not claim a calibrated export.
- **Listening is unverified.** No human listening record has been taken for either export, and
  none is claimed. §Review result now carries the instrument — the artifact, its digest, and
  criteria fixed before listening — but its disposition is empty and only a person who listened
  may fill it. `DELIVERY-PLAN.md` and `AGENTS.md` §Completion require the check or an explicit
  statement that it remains outstanding; this is that statement.
- **Caption precision is amended, not achieved.** The exact frame boundaries remain in
  `manifest.json`, but the WebVTT projection floors to milliseconds and cannot satisfy ADR-0001
  §17.12 for a boundary that does not divide by 24 frames. The error is under one millisecond and
  always early. `ADR-0001-D010`, approved 2026-09-01, authorizes the projection and carries no
  expiry, so this limit is permanent until an ADR amendment changes §13.5 or §17.12. Chapter
  boundaries are exact and unaffected.
- **The renderer identity is unsigned.** `text_renderer_version` closes a real reuse gap and its
  test passes, but the approval row `E1-S4-INTERFACE-CHANGE-001` §Approval carries for it is
  `Pending`: it was added after the other four rows were signed. The field is a required manifest
  field on that record's authority for the `1.0-skeleton` break, not on a signature of its own.
- **No loudness normalization.** ADR-0001 §13.4's two-pass normalization is E2-S3 and is not
  attempted here, so neither export carries a loudness claim.
- **Chapters are a sidecar, not embedded.** The reasoning is in
  `E1-S4-INTERFACE-CHANGE-001` §What the package now contains.
- **The suite's synthesizer is a deterministic tone, not Chatterbox**, and its fixture has two
  segments. That is unchanged and correct for T1–T4, which must stay offline and fast. What has
  changed is that the G1 material now exists beside it: a three-segment lesson rendered through
  real Chatterbox into a complete package, recorded in §Listening material. G1 still owes the
  listening judgment against it, and a second real render is not required to obtain that.
- **Only one delivery style survives a real render.** `DeliveryStyle` offers `calm`,
  `calm_explanatory`, `emphatic`, and `deliberate`; `worker/study_tts_worker/worker.py` declares
  only `calm_explanatory`, because Chatterbox has no style axis and the worker refuses the rest by
  name rather than mapping them onto identical parameters. A lesson using the other three passes
  the published schema, the parser, and every fake-executor test, and is refused only when a real
  worker sees it. The first render of the three-segment fixture was refused exactly so. The
  refusal is correct and fail-closed; the gap between what the schema accepts and what the backend
  declares is a fake-versus-real divergence G1 owns.
- **One flake observed once, and not reproduced.**
  `t4_e0_timeout_terminates_escaped_descendant_that_closes_capture_pipes` failed once during this
  story's work and has not failed since. `docs/testing/TEST-STRATEGY.md` calls a flaky test a
  defect, so it was investigated rather than dismissed, and it remains owed an owner and an issue
  before G1.

  An earlier draft of this record called it **pre-existing**. That was an inference from
  `process.rs` carrying no diff, and it was not evidence: a story can induce a timing failure in a
  file it never edits. It was measured instead, on 2026-09-01, against `b562852` (the tree before
  this story) and the current tree:

  | Condition | Tree | Runs | Failures |
  |---|---|---:|---:|
  | Test alone | pre-change | 50 | 0 |
  | Test alone | current | 50 | 0 |
  | `cargo test --workspace --all-targets` | pre-change | 10 | 0 |
  | `cargo test --workspace --all-targets` | current | 10 | 0 |
  | Same, with all eight cores saturated | current | 6 | 0 |

  126 executions, no reproduction. **This neither convicts nor exonerates the story.** Under the
  rule of three, zero failures in the sixteen post-change full-suite runs bounds the per-run rate
  at roughly 19% with 95% confidence — so a rate of one or two percent, which is what a single
  observation across a day's work suggests, is entirely consistent with this result. The
  disposition is *unreproduced*, not *fixed* and not *absent*.

  One hypothesis was eliminated. This story took a package render from two subprocess spawns to
  six across twenty-nine T4 tests, and an earlier note here proposed that as the load that
  destabilized a 250 ms deadline. It cannot be: Cargo runs test binaries one at a time, so
  `walking_skeleton` in `study-tts-testkit` never runs concurrently with `process.rs` in
  `study-tts-runtime`'s lib target.

  What to instrument if it recurs, rather than what to change now: the helper is the test binary
  re-invoking itself, and it must start, initialize the harness, spawn a descendant, and write a
  PID file inside the 250 ms deadline before
  `fs::read_to_string(&descendant_pid_path).expect("helper must record the escaped descendant")`
  runs. That `expect` is the most load-sensitive line in the test and the first place to capture
  diagnostics. Raising the deadline without a reproduction would be changing a value to silence a
  failure nobody has seen twice, which is the shape of a fix that hides a defect.

## Review

**No row below is signed.** Each records a decision a role is asked for and has not yet made.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Pending | |
| Project owner | Ross Todd | Pending | |
