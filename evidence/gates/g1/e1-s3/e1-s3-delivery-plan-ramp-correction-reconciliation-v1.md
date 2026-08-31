# E1-S3 Delivery Plan Ramp Correction Provenance Reconciliation v1

- Date/time and timezone: 2026-08-31, Europe/Berlin
- Candidate revision: working tree on `fix/issue-59-retired-grant`, after the edge-ramp correction
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Proposed
- Supersedes: nothing

## Scope and decision

An audit found that `condition_edges` in `crates/study-tts-runtime/src/audio_edges.rs` never
smoothed anything. It inserted zero padding and then applied the raised-cosine gain to samples
*inside that padding*; multiplying zeros by a gain is arithmetically inert, so a segment whose
signal began at `0.5` still stepped `0.0` to `0.5` at the onset. Correcting it required editing
`DELIVERY-PLAN.md`, which `e0-s3-g0-qualification-decision-v3` pins.

This record accounts for that one path and for nothing else. It supersedes no record and withdraws
no conclusion.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e0-s3-g0-qualification-decision-v3` | `DELIVERY-PLAN.md` |

## Why it moved

Two ratified sentences were in direct conflict, and the code had silently resolved them the wrong
way.

| Document | Rule |
|---|---|
| `docs/adr/ADR-0001-production-rust-study-guide-tts.md` §13.4 | "smooth each silence-to-signal transition with a raised-cosine ramp no longer than 5 ms" |
| `DELIVERY-PLAN.md` E1-S3 task 3, as written | "Apply a raised-cosine transition ramp no longer than 5 ms **without entering speech**" |

The two cannot both hold. After padding, the silence side of the transition is exactly zero, so
smoothing is possible only by attenuating signal samples — which "without entering speech"
forbids. The plan's own test name, `t1_e2_ramp_never_extends_into_speech`, asserted that every
speech sample survived unchanged, which is true precisely *because* nothing had been smoothed. The
test confirmed the defect rather than detecting it.

`CLAUDE.md` §Conflict order places ADR-0001 above `DELIVERY-PLAN.md` and requires a genuine
conflict to be flagged rather than resolved silently. It was flagged, and the project owner
directed that ADR-0001 prevail and the plan be corrected. `DELIVERY-PLAN.md` E1-S3 task 3 now
carries ADR-0001's wording, and the test contract is
`t1_e2_ramp_smooths_the_silence_to_signal_transition`.

A third defect is corrected in the same change and moved no governed document: the partial-frame
branch of `leading_silent_samples` had no length bound, though its own comment described it as
measuring "a remainder shorter than one frame". Unbounded, a quiet burst followed by a second of
silence averaged below the threshold, the segment was classified wholly silent, and conditioning
returned before ramping any real signal.

`ADR-0001-D007` is **not** edited and needs no amendment. Its condition 2 states that the ramp
geometry "is implemented as ratified" — a claim that was false when signed and that this change
makes true.

## What this does not change

- ADR-0001 §13.4 is not amended. The correction moves `DELIVERY-PLAN.md` to agree with it.
- The silence threshold remains provisional under `ADR-0001-D007`. Only the ramp's placement moved.
- Join discontinuity and loudness normalization remain E2-S3's, per `ADR-0001-D007` §What this does
  not permit.
- No conclusion `e0-s3-g0-qualification-decision-v3` rests on is withdrawn. That record's G0
  progression decision concerns E0-S3 qualification measurements, not E2-S3 audio conditioning.

## Identity effect

Conditioning runs before the staged audio is hashed, so every segment's `audio_blake3` moves. No
committed fixture or evidence file pins a post-conditioning digest — `cache.rs` computes them in
test — so no recorded value in this repository moves with it.

The consequence that does bind: the six blinded takes under
`data/qualification/listening-2026-08-31/listening` were rendered by the pre-correction
conditioner and are **stale before review**. They must be re-rendered and the review retaken, per
`docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §3.

## Verification

| Command | Result |
|---|---|
| `python3 scripts/check-evidence-provenance.py`, while this record is Proposed | Exit `1`, one unaccounted — the state this record is written to account for |
| `python3 scripts/check-evidence-provenance.py`, from acceptance | Not yet run; this record is unsigned |

The remaining gate results are recorded in the E1-S3 story record's verification section, taken
against this change.

## Approvals

Unsigned. A Proposed reconciliation grants nothing
(`scripts/check-evidence-provenance.py` `UNACCEPTED_DECISIONS`), so
`check-evidence-provenance.py` exits `1` until both rows are decided and `- Status:` reads
`Accepted`.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Pending — would accept that the ramp now attenuates the first and last 5 ms of signal, that this is the only placement that can smooth a transition out of exact zero, and that `DELIVERY-PLAN.md` moved to match ADR-0001 rather than the reverse | |
| Project owner | Ross Todd | Pending — would accept a corrected E1-S3 test contract name, that published audio changes again, and that the pending 2026-08-31 listening review is superseded before it was taken | |
