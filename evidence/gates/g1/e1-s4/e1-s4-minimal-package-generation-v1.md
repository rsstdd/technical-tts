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

## Deviations and limitations

- **The MP3 profile is uncalibrated.** `ADR-0001-D009`, approved 2026-09-01, bounds the
  permission and expires at ADR-0003, which still records MP3 codec arguments as `Pending`. This
  record does not claim a calibrated export.
- **Listening is unverified.** No human listening record was produced for either export, and none
  is claimed. `DELIVERY-PLAN.md` and `AGENTS.md` §Completion require the check or an explicit
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
- **The synthesizer is a deterministic tone, not Chatterbox**, and the fixture has two segments,
  not three. A real-Chatterbox three-segment package remains a G1 integration requirement.
- **One pre-existing flake observed.** `t4_e0_timeout_terminates_escaped_descendant_that_closes_capture_pipes`
  failed once under a full parallel run and passed on every isolated rerun and on every subsequent
  full run. It is in `process.rs` containment, which this story does not touch. Recorded rather
  than dismissed; `docs/testing/TEST-STRATEGY.md` calls a flaky test a defect, so it is owed an
  owner and an issue before G1.

## Review

**No row below is signed.** Each records a decision a role is asked for and has not yet made.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Pending | |
| Project owner | Ross Todd | Pending | |
