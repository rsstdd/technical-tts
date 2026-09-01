# ADR-0001-D010 — WebVTT captions carry a millisecond projection of the sample boundary

- **Status:** Approved
- **Date:** 2026-09-01
- **Controlling ADR and sections:** ADR-0001 §13.5, which names `transcript.vtt` and calls its
  captions "sample-exact", and §17.12, which requires that "segment-level caption boundaries equal
  the assembled sample boundaries"
- **Requesting story:** E1-S4
- **Owner:** Engineering owner
- **Approver:** Project owner and engineering owner
- **Expiry:** None requested. This is a property of the output format, not of an uncalibrated
  value, so it does not resolve itself the way `ADR-0001-D007` and `ADR-0001-D009` do. §Rollback
  states what would end it.

## Approved deviation

Permit `transcript.vtt` to carry each segment boundary **floored to the millisecond**, rather than
the exact assembled sample boundary §17.12 requires, provided the exact frame boundary is recorded
in `manifest.json` for every segment.

The projection is `timeline::timestamp` in `crates/study-tts-runtime/src/timeline.rs`, which names
this record in return:

```text
seconds      = frames / 24000
milliseconds = (frames % 24000) * 1000 / 24000        // integer division, floored
```

Flooring rather than rounding is part of what is requested: a cue must never begin *after* the
sample it describes, and rounding to nearest would do that for half of all boundaries.

## The gap

The two requirements cannot both be met, because ADR-0001 names the format that makes one of them
impossible.

WebVTT timestamps are `HH:MM:SS.mmm`. The grammar fixes the fractional part at exactly three
digits — thousandths of a second — and provides no sub-millisecond representation. The canonical
sample rate is 24 kHz, so one frame is 1/24 ms and a boundary is representable exactly only when
its frame count is divisible by 24. Segment speech lengths come from the synthesizer and are
arbitrary, so most boundaries are not.

§13.5 requires `transcript.vtt`. §17.12 requires caption boundaries to equal the assembled sample
boundaries. For a boundary not divisible by 24 frames, no `transcript.vtt` this project can write
satisfies §17.12. The conflict is between two clauses of one accepted ADR, not between the ADR and
an implementation choice — which is why it needs a decision rather than a better implementation.

The largest error the projection can introduce is 23/24 ms, under 1 ms, always in the direction of
an earlier cue.

**What is not affected.** `chapters.ffmetadata` declares `TIMEBASE=1/24000` and carries the frame
counts themselves, so chapter boundaries *are* exact and this record does not reach them.
`manifest.json` records `start_frame`, `frames`, and `pause_frames` per segment and `total_frames`
for the master, so the exact boundaries remain available to every later consumer, including the
E2-S4 run report and any E5 verification that needs them.

## Impact

- **Architecture and authority boundaries:** No change.
- **Schemas and interfaces:** None. The exact boundaries were already required fields of
  `manifest.json` `1.0-skeleton`; this record does not add or move a field. It records why that
  document, and not `transcript.vtt`, is the authority on where a segment begins.
- **Synthesis, verification, and cache identities:** None move. No caption byte is a key input.
- **Security, rights, and privacy:** No control is waived.
- **Tests and evidence:** `t4_e1_caption_boundaries_equal_written_sample_boundaries` reads cue
  timings out of the written file and compares them against a literal table. The two-segment
  fixture's boundaries happen to be divisible by 24, so that test alone does not exercise the
  projection; `t1_e1_frame_positions_render_as_floored_webvtt_timestamps` covers a boundary that is
  not, and pins the floor rather than a round. E1-S4 evidence stays `Proposed` until G1.
- **Existing artifacts and migration:** None. No package written before E1-S4 contains captions.
- **Schedule and scope:** None. Rejecting this record does not make §17.12 satisfiable in WebVTT;
  it makes `transcript.vtt` undeliverable until §13.5 names a different format.

## What this does not permit

- It does not permit rounding, or any projection that can place a cue after its sample boundary.
- It does not permit dropping the exact frame boundaries from `manifest.json`. The projection is
  acceptable *because* the exact value survives somewhere; without that this record has no
  compensating control and should be rejected.
- It does not reach chapter boundaries, which are exact, or the automated-check requirement that
  caption timestamps be monotonic — flooring a monotonic sequence stays monotonic, and equal
  adjacent cues would require two boundaries inside the same millisecond, which a nonzero segment
  cannot produce.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Round to nearest millisecond | Places a cue up to half a millisecond *after* the sample it describes for half of all boundaries. Strictly worse than flooring for a caption |
| Pad each segment so every boundary lands on a 24-frame multiple | Changes the audio to suit a caption format. It would also fail: only the pauses are this project's to choose, and a segment's speech end is fixed by the synthesizer |
| Emit a second, exact caption format instead | ADR-0001 §13.5 names `transcript.vtt` specifically, so this needs an ADR amendment, not a deviation. Worth raising if a consumer ever needs frame-exact captions; nothing does today |
| Amend §17.12 to say "equal within the caption format's precision" | The cleaner long-term fix, and out of scope for a story: §17.12 is an automated-check list a release gate reads, and rewriting a gate clause to fit one story is the widest possible instrument |
| Treat the manifest as satisfying §17.12 | It does not. §17.12 constrains caption boundaries, and the manifest is not the caption file. Recording the exact frame is a compensating control, not compliance, and an earlier draft of the E1-S4 interface record overstated this |
| Ship it and note the limitation in the story record | A `Proposed` story record grants no authority, and the code would depart from an accepted ADR on nobody's decision. This record exists so the departure is decided rather than described |

## Compensating control

`manifest.json` carries `start_frame`, `frames`, and `pause_frames` for every segment and
`total_frames` for the master, all as exact frame counts, and every one is checksummed with the
artifact it describes. Any consumer needing the exact boundary reads it there; the WebVTT file is a
player-facing rendering of it, and this record is what says so.

## Rollback

Supersede this record. Either amend ADR-0001 §17.12 to state the precision a caption format is held
to, or amend §13.5 to name a format that can carry frame-exact boundaries, then replace
`timeline::timestamp` accordingly. No authoritative data is lost either way: the exact boundaries
are in every manifest already, so existing packages can be re-rendered without re-synthesis.

Re-rendering is what `timeline::TEXT_RENDERER_VERSION` makes happen. Package reuse compares the
plan and the tool stack, and FFmpeg produces none of the three text documents, so before that
constant existed a replaced `timeline::timestamp` would have left the selected package — and its
old cues — in place. Bumping it is therefore part of this rollback, not an optional tidy-up.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the two rows are separate.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept a floored millisecond projection in `transcript.vtt`, with the exact frame boundary retained in `manifest.json` | 2026-09-01 |
| Project owner | Ross Todd | Approve — accept that ADR-0001 §17.12 is not met exactly for a boundary not divisible by 24 frames, that the error is under one millisecond and always early, and that this is a conflict between two clauses of ADR-0001 rather than an implementation shortfall | 2026-09-01 |
