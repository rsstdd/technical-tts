# Human preview review checklist

The checklist a reviewer fills when judging a private preview package. `DELIVERY-PLAN.md` §5
requires it by M2, owned and approved by the project owner.

It exists because the criteria drifted. Four listening reviews have been taken against four
different criteria sets — `e1-s4-minimal-package-generation-v1` used Joins, Pauses, Encoding,
Continuity, and Text integrity; `e1-s5-canonical-json-authoring-v1` used the same five reworded;
`e2-s2-retake-listening-review-v1` used Content, Pronunciation, Voice, Joins, Loudness, and
Continuation; and the E1-S3 blinded sheet uses five different fields again. Four sets means no two
reviews are comparable, and a criterion can go missing without anyone noticing it left. A story
record should cite this file and record findings against it rather than restate its own columns.

## Scope

This covers the **private preview package** — the six artifacts a preview generation writes, judged
as a whole by a person who listened to it.

It is not the E1-S3 blinded qualification listening. That set is rendered by the `listening-render`
example, reviewed against `review-sheet.json`, and verified by
`scripts/qualification/check_listening_review.py`, which binds each judgment to a take's SHA-256 and
refuses an unanswered sample. Those two flows answer different questions and neither replaces the
other: qualification asks whether the synthesizer is good enough, this asks whether *this package*
may be used.

## Before you listen

**Fix the criteria before playing anything.** The criteria below are already fixed, which is the
point of a standing file — criteria chosen after hearing the audio are criteria chosen to fit it,
for the reason `e0-s3-g0-qualification-report-v1.md` gives about its own. Adding a criterion for
this package is allowed; removing one requires saying why in the record.

**Bind the review to bytes.** Record the BLAKE3 of every artifact judged. A judgment against audio
that no longer hashes to what the record names is a judgment about audio nobody can produce again.
Re-take the review whenever the audio changes, not only when the text does — edge conditioning, a
model revision, a voice-profile change, or a threshold swap all produce different audio from the
same script.

**Write `none`, never blank.** A blank means "not yet answered". `none` means you listened and there
was nothing there. `reject` is as complete an answer as `accept`.

## Attribution

Record all four. A review without them is unattributed and does not count.

| Field | Meaning |
|---|---|
| Reviewer identity and role | Who listened, and in what role they signed |
| Playback environment | Hardware and room; laptop speakers and monitors do not hear the same faults |
| Date and time | When the session was taken, with timezone |
| Artifact digests | BLAKE3 of every artifact judged |

## Per-segment findings

One row per segment. Every cell answered.

| Criterion | What a finding looks like |
|---|---|
| Content | A word spoken that is not in the segment's `spoken_text`, or one omitted |
| Pronunciation | Anything said wrong: a term, a name, an acronym, a number |
| Voice | Drift in timbre or identity, within the segment or against its neighbours |
| Joins | A click, truncation, overlap, or audible discontinuity at either boundary |
| Loudness | Level shifting so the segments do not read as one recording |
| Continuation | Pace, pauses, and breath placement — including whether a declared interval is long enough to do what it is for, such as answering a recall prompt |
| Disposition | `accept` or `reject` for this segment |

## Package review

Judged once for the package, not per segment.

- [ ] Segment order and completeness are correct.
- [ ] Chapters and captions align with the audio.
- [ ] WAV, M4A, and MP3 all play.
- [ ] `lesson.mp3` carries no encoder artifact absent from the master: no swirl on sibilants, no
      pre-echo, no obvious high-frequency loss.
- [ ] No source text, diagnostic data, or voice-reference path leaks into artifact metadata.
- [ ] Every finding above has a disposition.

## Disposition

Exactly one, signed:

- [ ] Approved for private preview
- [ ] Approved for the stated production scope
- [ ] Rejected; correction required

Approving for a production scope requires an explicit accepted takes selection; a generated
take-zero selection cannot back a production claim.

## What this checklist cannot arbitrate

Say so in the record rather than leaving it implied.

- **Anything the playback environment cannot reach.** Built-in laptop speakers do not resolve
  low-frequency content, encoder artifacts, or fine level drift. Naming the environment is what
  makes the limit legible.
- **A join between two performances that are byte-identical.** Under a `reproducible` bundle a
  retake reproduces the prior audio exactly, so listening to both is listening to one. Material
  like that can discharge the package review and cannot verify a retake join.
- **Any measurement without a ratified reference.** Loudness and speaking-rate figures stay
  `provisional` until ADR-0003 is accepted, and a listening judgment does not promote them.

## Related

- `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` — where this sits in the code, qualification,
  listening, and evidence order
- `evidence/README.md` §Accepting a record at its gate — one record per story, `Proposed` until its
  gate, accepted against the bytes that gate approved
- `DELIVERY-PLAN.md` §5 — the row this file answers
