# E2-S2 — Retake human listening review

- Status: Proposed
- Governing story/gate: `DELIVERY-PLAN.md` E2-S2; gate G1
- Hypothesis or decision: ADR-0001 §11.4's human listening obligation over a requested
  alternate performance and the joins either side of it
- Owner: Engineering owner, with the listener representative
- Date/time and timezone: material rendered 2026-09-04, local (UTC+00:00 as recorded by the
  reference environment); **no listening has been taken**
- Environment ID: `docs/operations/REFERENCE-ENVIRONMENT.md`

Opened at the story's implementation, per `evidence/README.md` §Accepting a record at its gate.
Every field below that a machine can derive is filled from the rendered material and verified
against it. The reviewer identity, the playback environment, the findings table, and the
disposition are the human half and are deliberately blank: a populated template is not a review,
and this record makes no listening claim.

**Read §What this material cannot arbitrate before scheduling the session.** The retake
generation is byte-identical to the base generation, so listening to both is listening to one.

## Listening material

Rendered 2026-09-04 through `build_preview` on the real Chatterbox worker — the path production
uses. Governed output, so the location is named by root rather than reproduced here, per
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Item | Value |
|---|---|
| Location | `e2-s2-package-2026-09-04-082541/workspace/previews/e1-s4-three-segment/packages/` beneath the governed qualification output root |
| Lesson | `fixtures/lessons/e1-s4-three-segment.json`, `e1-s4-three-segment`, committed and registered |
| Segments | 3, one interior with a join on each side |
| Retaken segment | `seg-0002`, speaker `nadia`, style `calm_explanatory` |
| Voice profile | `owner-fallback-v1`, resolved at `VoiceUse::PrivateSynthesis` |
| Voice conditioning hash | `4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47` |
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Model revision | `ResembleAI/chatterbox` at `1b475dffa71fb191cb6d5901215eb6f55635a9b6` |
| Determinism class | `reproducible`, seed `42` |
| Master | 14.96 s, 24 000 Hz, one channel, IEEE float, both generations |

### Generations

| | Base | Retake |
|---|---|---|
| Package identity | `31a1a0f541d4f3c56d2a851f03de61c6a40c49e9b3dae43241782ec7735450c2` | `e1259b26b719da83c003f921969b59f2c2409def80d18cd26f720a81254b3433` |
| `plan_hash` | `352b8a8bd0275307c505f5e6a968bc32c3223d8a7c2ffefc433eee6b6f0ce9ce` | `92748f59319d48166df7a1b45c6bb1e55afe8fcd46840a17af9c53fe1302eb93` |
| `seg-0002` take | 0 | 1 |
| `seg-0002` cache key | `29f3946240d2d9e3b73e152d428f25b3e1faa9b825f506f94717c9ad9000e4ac` | `e9f6808a65f12efd260425332a34ed1d50d62044de6c84ec20d887f0c684cecc` |
| `seg-0002` `synthesis_base_key` | `29f3946240d2…` | `29f3946240d2…`, unmoved |
| `join_continuity` | `[]`, nothing replaced | both joins recorded, `calibration_source: provisional` |
| `take_selection_source` | `implicit` | `implicit` |

`seg-0001` and `seg-0003` hold take 0 in both generations, at cache keys `445819c66e16…` and
`76f6d4eaabb3…`.

### Artifact digests, identical across both generations

| Artifact | BLAKE3 |
|---|---|
| `lesson.wav` | `9648e1d372acdda707941ce4402021afe2dc6b2bd26b1c7b601adac61f46c406` |
| `lesson.m4a` | `faad94473772806b84dc110f3eedae490c18b4cad81904977a1937ee8b2c5e2c` |
| `lesson.mp3` | `b78a4be65c720b264bafafca943146ea67a36286afb284cef606f47bc47cd217` |
| `transcript.txt` | `2963e0380c803cc9084de97a244e7abd567b40b79a8d2874e2b53a341f96b43c` |
| `transcript.vtt` | `6b1251f99ca612dfc505a48622b2fa5f35882eca68f22d5e9688d0680c8d146c` |
| `chapters.ffmetadata` | `3fd69bae2d9b0b332cc68d522993710437d7d2fc7e81eaee1b9399a581a93aef` |

### Recorded join measurements, retake generation only

| Join | `loudness_ratio` | `rate_ratio` | `calibration_source` |
|---|---|---|---|
| `seg-0001` → `seg-0002` | 1.0495234 | 1.2639548 | `provisional` |
| `seg-0002` → `seg-0003` | 1.1610107 | 0.9611233 | `provisional` |

Neither is a production reference. `JoinContinuity::production` refuses to serve them while
ADR-0003 is `Proposed`, and the speaking-rate measure is open question **G-B** of
`docs/architecture/E2-S2-INTERFACE-CHANGE-001.md`.

## What this material cannot arbitrate

**The retake produced byte-identical audio to the take it replaced.** Verified rather than
inferred: all six recorded BLAKE3 digests match across the two generations, and `sha256sum` over
`lesson.wav`, `lesson.m4a`, `lesson.mp3`, and `transcript.txt` in both package directories agrees.

The cause is in the source and is a property of the design rather than of this run:

- `take` is an ADR-0001 §12.5 synthesis-key input — `study-tts-core/src/identity.rs:436` — and is
  carried to the worker as a request field, `study-tts-runtime/src/worker_protocol.rs:118`.
- The worker seeds generation from `seed` alone, `worker/study_tts_worker/worker.py:784`. Nothing
  derives `seed` from `take`, so the two takes issue the same generation call.
- This bundle is characterized `reproducible` — "the same request and seed reproduce the same
  bytes on the same bundle" — so the identity is guaranteed here, not merely observed.
- `seed` is itself a synthesis-key input, `study-tts-core/src/identity.rs:512`. Changing it moves
  every segment's `synthesis_base_key`, and a takes file recorded against the earlier plan is then
  refused with `TakesError::StaleSynthesisBaseKey`.

So the joins recorded above are measurements of real segment boundaries, but not of a boundary
between *different* performances: both sides of each join are the same audio in both generations.

**What this record can therefore discharge, once taken:** the package review for these bytes.
Nobody has listened to this package; `E1-S5` recorded that the E1-S4 listening does not transfer,
and these bytes differ from those again.

**What it cannot discharge:** any claim that human listening has verified a *retake* join. That
needs material in which the replacement differs from what it replaced, which this build cannot
produce under a `reproducible` bundle without moving the seed and invalidating the selection.

Recorded as a finding rather than resolved here. It is not a defect in the E2-S2 acceptance
criteria, all of which concern take identity, artifact preservation, and join assessment, and all
of which this material exercises. It is a limit on what the qualification output can be asked to
prove, and it belongs to whoever schedules the session.

## Segment findings

To be completed by the reviewer. Criteria are fixed before listening, for the reason
`e0-s3-g0-qualification-report-v1.md` gives about its own: criteria chosen after hearing the audio
are criteria chosen to fit it.

| Segment | Audio checksum | Content | Pronunciation | Voice | Joins | Loudness | Continuation | Disposition |
|---|---|---|---|---|---|---|---|---|
| `seg-0001` | `1c27cc71f4fdec13f3cf41eed7521f2b2a3b26b70a36b5634deb58febc76f097` | | | | | | | |
| `seg-0002` | `7c5722ce53c39e0af4689393b12737fda888467336960cb4d3eeb1cbdd240de9` | | | | | | | |
| `seg-0003` | `8e3b822e98a3e72e249b991ea02c299f2012f6b2b1844b039ef1693c8e53d10a` | | | | | | | |

## Package review

- [ ] Segment order and completeness are correct.
- [ ] Chapters and captions align with the audio.
- [ ] WAV, M4A, and MP3 play correctly.
- [ ] No source or diagnostic data leaks into metadata.
- [ ] Every finding has a disposition.

## Approval

- [ ] Approved for private preview
- [ ] Approved for the stated production scope
- [ ] Rejected; correction required

Reviewer identity and role:

Playback environment and equipment:

Date/time:

Signature/identity, date, and rationale:

**Unsigned. No disposition has been entered, and none is implied by the material above.**
