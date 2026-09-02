# E1-S4 Interface Change 001 — The complete minimal package

## Identification

- Record ID: `E1-S4-INTERFACE-CHANGE-001`
- Status: **Accepted, 2026-09-01; the reopened row signed 2026-09-02.** §Approval records the
  decision each role made and the date it was signed. A fifth required manifest field,
  `text_renderer_version`, was added by review *after* those signatures, so its row was signed
  separately on 2026-09-02. The four rows signed on 2026-09-01 were unaffected throughout,
  because the class, the version, and the migration they accepted are unchanged.
- Contract owner: T-AUDIO (`package_writer`, the package manifest, the export profiles)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUDIO, T-CORE
- Accepted ADR, if architectural: ADR-0001 §§13.2, 13.5, and 17.12. The package shape implements
  §§13.2 and 13.5. The WebVTT precision conflict with §17.12 is a conflict between two clauses of
  ADR-0001 itself and is decided in
  [`../adr/deviations/ADR-0001-D010-webvtt-millisecond-caption-projection.md`](../adr/deviations/ADR-0001-D010-webvtt-millisecond-caption-projection.md),
  **Approved 2026-09-01**, which authorizes the projection this record describes. One bounded MP3
  deviation is approved separately in
  [`../adr/deviations/ADR-0001-D009-provisional-mp3-profile.md`](../adr/deviations/ADR-0001-D009-provisional-mp3-profile.md).

E0-S0 published a package holding a master WAV, an M4A, and a manifest. `DELIVERY-PLAN.md` E1-S4
requires the rest of ADR-0001 §13.5's package tree — `lesson.mp3`, `transcript.txt`,
`transcript.vtt`, `chapters.ffmetadata` — every artifact checksummed, and caption and chapter
boundaries derived from the sample counts actually written.

## Version and compatibility

### Package manifest — `0.2-skeleton` → `1.0-skeleton`

`manifest.json` gains required fields in four places:

| Field | What it records |
|---|---|
| `total_frames` | Frames in the finished master |
| `segments[].start_frame`, `segments[].pause_frames` | Where each segment's speech and silence were *written*, beside the `frames` and `pause_after_ms` the cache entry *declared* |
| `artifacts.mp3`, `.transcript`, `.captions`, `.chapters` | The four artifacts E1-S4 adds, each with its published path and BLAKE3 |
| `tools.executions[]` | Every FFmpeg and ffprobe invocation, replacing the single encode-and-probe pair |
| `text_renderer_version` | Identity of the rules that produced the transcript, captions, and chapters, carried by `timeline::TEXT_RENDERER_VERSION` |

**`text_renderer_version` was added by review after the rows below were first signed**, and it is
inside the same break rather than a second one: the class, the version, and the migration are
unchanged, and `1.0-skeleton` has never been written to disk outside this story. The gap it closes
is recorded under §Impact — reuse compared the plan and the tool stack, and FFmpeg never sees the
three text documents, so nothing moved when the rules that produce them changed.

A required field is **Breaking contract** under
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes, so the major increments
and `schemas/manifest-v0.schema.json` is replaced by `schemas/manifest-v1.schema.json`.

`ADR-0001-D005` was considered and **does not apply.** Its condition 3 requires that no durable
artifact and no evidence record outside `Proposed` was written under the shape being corrected;
`0.2-skeleton` packages exist on disk and accepted E1-S1 evidence describes them. Condition 2 fails
for the same reason: `0.2-skeleton` was introduced by E1-S1, not by this story.

**The label keeps `-skeleton` while the major moves.** The two say different things and both are
true: the major says this change was breaking, and the suffix says the layout is still provisional.
`crates/study-tts-runtime/src/schemas.rs::JOB_SCHEMA_VERSION` already settles the reasoning for its
own document — publishing a provisional record as a bare `1.0` "would claim a stability E2-S1 is
going to break" — and §Amendment rules routes package changes to "E1-S4/E2-S3", where E2-S3 adds
loudness normalization and E2-S4 adds the run report to this same manifest.

### Three layouts are now read, one is written

`parse_stored_manifest` accepts `0.1-skeleton`, `0.2-skeleton`, and `1.0-skeleton`, and refuses
every other string. Both older layouts are **preserved and never reusable**: each describes a
two-artifact package, and `manifest::tools_match` compares the whole set of recorded argument
profiles against the set this build publishes, so a package missing three of them cannot satisfy
reuse and is rebuilt instead. `t4_e0_historical_packages_remain_valid_but_cannot_satisfy_current_reuse`
holds both halves.

Only `1.0-skeleton` is published as a schema, for the reason the previous layout's omission had:
the schema is generated from the current stored shape, and an older layout carries a different
`artifacts` and `tools` shape it would describe wrongly.
`t3_e1_the_published_manifest_schema_names_every_layout_it_describes` fails until a fourth accepted
layout is either published or deliberately excluded.

### `package_writer` `e0.package-writer.1.0` → `2.0`

`PackagePublication` gains `mp3`, `transcript`, `captions`, and `chapters`; `BuildResult` gains the
same four. The trait shapes are unchanged, but what a successful write produces is not, which the
change-control table classifies as a semantic change. The prefix stays `e0.` because it names the
story that introduced the seam, following `e0.cache-publication.2.0`, which took a major increment
during E1-S3 without changing prefix.

`FakePackageWriter` writes all six artifacts and moves in the same change as the trait, in the order
§Amendment rules before G1 requires. `t4_e1_the_real_package_writer_passes_the_shared_contract` now
runs `FileSystemPackageWriter` through `run_package_writer_contract_scenario`, which
[`PROVISIONAL-CONTRACT-BASELINE.md`](PROVISIONAL-CONTRACT-BASELINE.md) requires of the real
master-first package path before G1 and which only the fake had satisfied.

### Package transaction identity — its own version, and every profile

`TransactionIdentity::identity_version` was `JOURNAL_SCHEMA_VERSION`, the constant that also labels
`publication.json` on disk. The identity now includes the MP3 and encoder-inventory profiles, and
bumping it through the shared constant would have moved the journal's recorded version too, so
`validate_record_version` would have refused every `publication.json` an earlier build wrote. The
identity now carries `TRANSACTION_IDENTITY_VERSION` (`0.3-skeleton-transaction`) and the journal
record keeps `0.1-skeleton-publication` unchanged. One document changed; one did not.

### `ToolOperation` — four new variants

`Mp3Encode`, `Mp3Validation`, `MasterWavValidation`, and `EncoderProbe` are added; `M4aEncode` and
`M4aValidation` keep their names and meanings. This is a public diagnostic vocabulary, not a durable
format: it appears in supervision messages and in no file. Recorded here because it is `pub`.

`ADR-0001-D005` is **not** invoked for it. Adding an enum variant is a compatible extension, not a
breaking correction, so there is nothing for that permission to waive.

### `ToolError::MissingEncoder` — a new refusal

An FFmpeg that cannot encode `libmp3lame` is refused during package preflight, before synthesis and
before any durable state exists. `tools::inspect` reads only the first line of `-version`, which is
identical whether or not the encoder was compiled in, so the inventory is asked for separately.
The refusal is deliberately unrouted: no `docs/governance/ROUTING-TABLES.md` §Failure routing row
fits an absent encoder, because the "Invalid or over-range audio" row answers with quarantine and a
bounded retry, and neither reaches a build that has produced no audio and cannot gain an encoder by
retrying. It is an environment failure like `MissingTool`, and its own message names the encoder to
install.

## What the package now contains, and what derives it

`assembly::assemble` returns a `Timeline` built by the write loop — each segment's `start_frame`,
`audio_frames`, and `pause_frames` — instead of a bare total. Everything downstream reads that
rather than recomputing a duration from `pause_after_ms`, which is what makes ADR-0001 §17.12's
"caption boundaries equal the assembled sample boundaries" a property of the code rather than of
two derivations agreeing.

| Artifact | Derived from |
|---|---|
| `lesson.wav` | Rust PCM assembly, unchanged |
| `lesson.m4a`, `lesson.mp3` | Each encoded from `lesson.wav`, never from each other |
| `transcript.txt` | Plan `speaker` and `display_text`, one line per segment |
| `transcript.vtt` | One speech-only cue per segment at the written frame boundaries |
| `chapters.ffmetadata` | Contiguous speech-plus-pause spans, `TIMEBASE=1/24000` |

**Caption precision needs a decision, and has one open.** ADR-0001 §13.5 calls these captions
"sample-exact" and names WebVTT; §17.12 requires their boundaries to equal the assembled sample
boundaries; WebVTT timestamps are milliseconds and a 24 kHz frame is 1/24 of one. The two clauses
cannot both be met for a boundary that does not divide by 24, whatever this build does — the
conflict is inside ADR-0001, not between the ADR and an implementation choice.

This build floors each written frame boundary and retains the exact frame in `manifest.json`, so a
cue precedes its boundary by under one millisecond and never follows it. The manifest is a
compensating control; it does not make the WebVTT cue equal the frame boundary, and an earlier
draft of this record overstated that. `ADR-0001-D010`, **Approved 2026-09-01**, is what authorizes
the projection, and it carries no expiry because the constraint is the output format rather than an
uncalibrated value. Chapter boundaries are unaffected: `chapters.ffmetadata` declares
`TIMEBASE=1/24000` and carries the frame counts themselves.

**Chapters ship as a sidecar and are not embedded.** ADR-0001 §17.10 says "embed ordered chapters",
qualified at §13.3 by "where supported". [`WALKING-SKELETON.md`](WALKING-SKELETON.md) §Extension
scopes E1-S4 to extending FFmpeg invocation "without changing pinned arguments", and embedding would
change the pinned `-map_metadata -1` that keeps the container holding exactly the stream ffprobe
verifies. Embedding is E2-S3 work.

**The master is probed by ffprobe, not re-read with `hound`.** `hound` wrote it, so reading it back
would check the writer against itself; an independent decoder is what makes
`t4_e1_wav_m4a_and_mp3_pass_structural_validation` a claim about the file.

**Authored text is escaped where it lands.** `display_text` is reviewed but authored: cue payloads
escape `&`, `<`, and `>` and collapse line breaks, so no cue can be terminated early or a `-->`
introduced; FFMETADATA values escape `=`, `;`, `#`, and `\` on the same terms.

## Impact

- **Synthesis, verification, and cache identities:** None move. No export profile, timeline field,
  or artifact name is a synthesis-key input, so no cache key and no plan hash changes and no cached
  audio is re-synthesized.
- **Package identity gains a third input.** `manifest::validate_package` compared the plan hash and
  the tool stack, and neither reaches `timeline`: FFmpeg never sees `transcript.txt`,
  `transcript.vtt`, or `chapters.ffmetadata`, so a build that changed how those three are rendered
  would have reused a selected package written by the old rules. The concrete case is the rollback
  `ADR-0001-D010` §Rollback describes — replacing `timeline::timestamp` rewrites every cue while
  the plan and the tools stand still. `text_renderer_version` is now recorded and compared, a
  package recording a different one is rebuilt rather than reused, and a layout that predates the
  field can never match. `preview::TRANSACTION_IDENTITY_VERSION` moves to `0.3-skeleton-transaction`
  for the same input, so two renderers cannot share a staging directory; that constant is hashed
  and never stored, so no existing record is refused by the move.
- **Durable formats:** `manifest.json` (`1.0-skeleton`) and the published
  `schemas/manifest-v1.schema.json`, which replaces `manifest-v0.schema.json`. `publication.json`,
  `current.json`, and `artifact.json` are unchanged.
- **Existing artifacts:** No package is migrated, rewritten, or deleted. Existing `0.1-skeleton` and
  `0.2-skeleton` packages under `previews/` stay readable and validate; the next build of the same
  lesson writes a new generation beside them, because the recorded profile set no longer matches.
- **Durability:** `preview::publish_transaction` synchronizes seven files — the six artifacts plus
  the manifest — before the package directory is renamed into place. The list comes from
  `manifest::PACKAGE_ARTIFACT_NAMES`, so a format added there cannot be published unflushed.
- **Reuse compares an execution sequence, not a profile set.** A set could not see a missing
  probe, a repeated encode, or an execution recorded against the wrong binary, so a package
  missing half its verification compared equal to a complete one. `manifest::expected_executions`
  now states the six invocations in order and `package_port` performs exactly them.
- **A recorded timeline is validated for self-agreement.** `start_frame`, `pause_frames`, and
  `total_frames` were parsed and discarded; a manifest whose segments overlap, whose pause is not
  its declared duration in frames, or whose master is longer than its own segments is now refused
  as `DurableStateError::IncoherentPackageTimeline`.
- **Public Rust surface:** `pipeline::validate_encoded_output` is renamed
  `pipeline::validate_m4a_output`. It always validated an AAC stream and its `# Errors` always said
  so, but the package now holds a second encoded artifact and the old name invited a call that
  would refuse a correct MP3. No behavior changes and no contract in
  `PROVISIONAL-CONTRACT-BASELINE.md` names it.
- **Rights and privacy:** No control is waived, and one is tightened. Every file in a published
  package is now created mode `600`. The master, both exports, and the manifest already were,
  because `tempfile` creates them so; the three text documents used `fs::write` and inherited
  `0666 & ~umask`. No document states a mode — this adopts the mode the other four files already
  carried rather than inventing a policy, and `t4_e1_every_package_file_is_owner_only` holds all
  seven to it. `-map_metadata -1` keeps authored metadata out of
  both containers, and no voice reference, model artifact, or corpus enters the package.
- **Operations:** `libmp3lame` becomes a prerequisite of the FFmpeg build, recorded in
  `README.md` §Prerequisites and `docs/operations/REFERENCE-ENVIRONMENT.md`.
- **Audio:** The MP3 is encoded under the provisional profile `ADR-0001-D009` approves, which
  expires at ADR-0003. No loudness
  normalization is applied — that is E2-S3 — and no listening claim is made.

## Delivery and recovery

Every end moves in one change, in the order §Amendment rules before G1 requires: the timeline before
its readers, the manifest shape and generated schema before the fixtures that validate against it,
the fake and the shared contract suite before their consumers, then the documents.

Recovery is reversion rather than migration, because nothing durable was rewritten: revert the
manifest layout, the schema file, and the contract version together. Packages written under
`1.0-skeleton` remain on disk and are refused as an unread layout by the reverted build, which is
the fail-closed outcome rather than a silent misread.

## Limits this change does not close

- **The MP3 profile is uncalibrated**, pending ADR-0003. `ADR-0001-D009` bounds it and expires with
  that acceptance.
- **The M4A profile's `96k` remains an unrecorded provisional choice** from E0-S0, named in
  `ADR-0001-D009` §The gap and closed by the same ADR-0003 acceptance.
- **No loudness normalization**, and therefore no listening claim for either export. E2-S3.
- **Chapters are not embedded in the M4A**, for the reason §What the package now contains gives.
- **Caption boundaries are not frame-exact in `transcript.vtt`**, and cannot be while §13.5 names
  WebVTT. `ADR-0001-D010` authorizes the projection rather than closing the underlying conflict;
  its §Rollback names the two ADR amendments that would end it instead.
- **No three-segment G1 fixture and no real-Chatterbox package.** Both were G1 integration
  requirements when this record was signed on 2026-09-01. Both were met the same day:
  `fixtures/lessons/e1-s4-three-segment.json` is committed and registered, and a complete package
  was rendered from it through the real Chatterbox worker. The E1-S4 evidence record was accepted
  at G1 on 2026-09-02, once the listening record against that package was taken.

## Approval

**The first four rows were signed on 2026-09-01.** Each records a decision a role was asked for and
has made. The fifth was added afterwards, when review found that package reuse did not cover the
text documents, and it is not signed: nothing may claim a signature for a field that did not exist
when the signing happened.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate for one signatory.

This acceptance covers the contract this record describes. It did **not** accept
`evidence/gates/g1/e1-s4/e1-s4-minimal-package-generation-v1.md`, which stayed `Proposed` until its
own G1 acceptance on 2026-09-02: an interface record accepts a contract change, never the story
that carried it.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept the package manifest taking a major increment to `1.0-skeleton` and retaining its provisional suffix, and that `ADR-0001-D005` does not reach it | Accepted — Ross Todd, 2026-09-01 |
| Contract owner (T-AUDIO) | Accept `e0.package-writer.2.0`, the four added publication paths, the six-artifact package, and the provisional MP3 profile `ADR-0001-D009` requests | Accepted — Ross Todd, 2026-09-01 |
| Contract owner (T-CORE) | Accept `manifest-v1.schema.json` replacing `manifest-v0.schema.json`, the recorded required-field surface, and that no synthesis, verification, or cache identity moves | Accepted — Ross Todd, 2026-09-01 |
| Contract owner (T-AUDIO) | Accept `text_renderer_version` as a fifth required manifest field and a third package-reuse input, added by review inside the same `1.0-skeleton` break | Accepted — Ross Todd, 2026-09-02. Raised after the rows above were signed and signed separately |

- Effective version and date: **2026-09-01.** `manifest.json` `1.0-skeleton`;
  `PACKAGE_WRITER_CONTRACT_VERSION` `e0.package-writer.2.0`; `TRANSACTION_IDENTITY_VERSION`
  `0.3-skeleton-transaction`, which is the value `preview.rs` publishes and covers both the full
  argument-profile set and `text_renderer_version`; `CACHE_SCHEMA_VERSION` `2.0`, `SYNTHESIS_IDENTITY_VERSION`
  `e1-s2-v1`, `e1.tts-executor.3.0`, `e1.worker.2.0`, and `e0.cache-publication.2.0` unchanged.
