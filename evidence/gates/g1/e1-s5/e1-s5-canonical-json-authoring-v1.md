# E1-S5 Canonical JSON Authoring Ergonomics v1

- Date/time and timezone: 2026-09-02, Europe/Berlin
- Candidate revision: working tree at the E1-S5 implementation, worker bundle identity
  `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted

## Scope and decision

`DELIVERY-PLAN.md` §E1-S5 names four tasks and three acceptance tests. All four are implemented and
all three tests pass. This record is `Proposed` because §Review result is not answered: the human
listening review is a person's to take, and `evidence/README.md` accepts a story record at its
gate rather than when its code lands.

The story's own scope is small — two authoring commands. What this record has to carry is larger,
because closing E1-S5 required moving every synthesis key in the project twice, and a record that
described only the CLI would not let a reader check the audio they are being asked to accept.

## Tasks

| Task | Result |
|---|---|
| `study-tts lesson new` scaffolds a valid lesson with `$schema`, stable IDs, roles, styles, and review fields | Done. `study_tts_runtime::scaffold_lesson`; the scaffold is validated as bytes before publication, so an `Ok` return is a document `load_lesson` accepts |
| `study-tts lesson validate` with field-path diagnostics and nonzero failure status | Done. Renders document, RFC 6901 pointer, segment, and reason; exits `1` on any refusal |
| Document the scaffold, edit, validate, and preview loop | Done. `docs/operations/AUTHORING.md` |
| Add one reviewed worked example | `fixtures/lessons/e1-s4-three-segment.json`, registered in `docs/testing/TEST-DATA-MANIFEST.md` and linked from the guide, which neither duplicates nor modifies it |

## Named acceptance tests

| Test | Result |
|---|---|
| `t4_e1_scaffolded_lesson_validates_without_manual_repair` | Pass |
| `t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton` | Pass |
| `t1_e1_validation_error_names_the_offending_field_path` | Pass |

Eight further CLI cases cover the published command list, an invalid lesson ID refused before any
file is created, an existing destination preserved byte for byte, `0600` publication, a nonzero
exit naming the document, and the absence of lesson text from a refusal.

## What moved beneath the story, and why it had to

Two changes moved every synthesis key. Neither is E1-S5's subject; both had to precede the G1
freeze, because after it each needs the full **Breaking contract** migration
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes requires.

| Change | Record | Effect |
|---|---|---|
| Seeding before model construction | `E1-S3-INTERFACE-CHANGE-004`, Accepted 2026-09-02 | Worker bundle identity; issue #70 |
| `model_artifacts_hash` as a key input | `E1-S5-INTERFACE-CHANGE-002` with `ADR-0001-D011`, both Accepted 2026-09-02 | `SYNTHESIS_IDENTITY_VERSION` → `e1-s5-v1`, `CACHE_SCHEMA_VERSION` → `3.0`; issue #66 |
| `deterministic_seed` → `True` | `E1-S5-INTERFACE-CHANGE-005`, **Proposed** | `determinism_class` → `reproducible`; worker bundle identity again |

## Reference-machine requalification

Run on the reference machine, offline, inside a network namespace holding only `lo` with no IPv4
route. Six of six criteria pass.

| Item | Value |
|---|---|
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Launcher seed | `42` |
| Result artifact | [`e1-s5-requalification-result.json`](../e1-s3/e1-s5-requalification-result.json), SHA-256 `bebee3e0b2c5e0bbe6586ef65d2a5918f57537088d25535477c2097a98b8d4c0` |

`t5_e1_two_lifetimes_render_identical_audio_under_one_seed` is new at E1-S5 and is what admitted
the `deterministic_seed` flip: two fresh lifetimes produced byte-identical canonical WAVs over
92 160 frames, zero frames differing by bit pattern, at
`6b641ad8f265c1c10d91234e80a7d0a9e751857947e0c6a7995381d751b63d5e`.

**Its first run failed, and the failure was the instrument's.** It compared the takes as the worker
staged them; those differ by one byte at offset 61, inside libsndfile's `PEAK` chunk, which records
the wall-clock time of the write. The audio was already identical. The criterion now re-encodes
both takes as `cache::write_canonical_samples` would and compares those bytes, which is the
artifact a cache entry is addressed by. Recorded here rather than discarded because a corrected
instrument that hides its correction is one nobody can audit.

## Listening material

Rendered 2026-09-02 by `cargo run --package study-tts-testkit --example package-render`, driving
`fixtures/lessons/e1-s4-three-segment.json` through the real Chatterbox worker and `build_preview`
— the path production uses. Governed output, so the location is named by root rather than
reproduced here, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Item | Value |
|---|---|
| Lesson | `fixtures/lessons/e1-s4-three-segment.json`, `e1-s4-three-segment`, committed and registered |
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Package identity | `3dbc3415d84a08177d7fe2e0b0b791a854b9d0309ffb8986424ce09b07b78fe6` |
| Master | 14.960 s, 359 040 frames, 24 000 Hz, one channel, IEEE float |
| Segments | 3 — `seg-0001` at frame 0, `seg-0002` at 86 400, `seg-0003` at 212 160 |
| Release status | `private_preview` |

| Artifact | BLAKE3 |
|---|---|
| `lesson.wav` | `9648e1d372acdda707941ce4402021afe2dc6b2bd26b1c7b601adac61f46c406` |
| `lesson.m4a` | `faad94473772806b84dc110f3eedae490c18b4cad81904977a1937ee8b2c5e2c` |
| `lesson.mp3` | `b78a4be65c720b264bafafca943146ea67a36286afb284cef606f47bc47cd217` |
| `transcript.txt` | `2963e0380c803cc9084de97a244e7abd567b40b79a8d2874e2b53a341f96b43c` |
| `transcript.vtt` | `6b1251f99ca612dfc505a48622b2fa5f35882eca68f22d5e9688d0680c8d146c` |
| `chapters.ffmetadata` | `3fd69bae2d9b0b332cc68d522993710437d7d2fc7e81eaee1b9399a581a93aef` |
| `manifest.json` | `3dbc3415d84a08177d7fe2e0b0b791a854b9d0309ffb8986424ce09b07b78fe6` |

**The E1-S4 listening review does not transfer, and this is not a formality.** That review was
taken 2026-09-02 against `lesson.mp3` at `bde064f7…` under bundle `3e1f487c…`. Every one of those
identities has moved, and the audio itself is measurably different: the same three segments now run
**14.960 s against E1-S4's 12.400 s**. The seeded decoder draws different noise and generates
different-length speech from the same text. A disposition is a judgment about bytes, and these are
not those bytes.

`evidence/gates/g1/e1-s4/e1-s4-minimal-package-generation-v1.md` is accepted and is **not** edited.
What it attests remains true of the artifacts it names, which are now historical.

## Review result

**Taken 2026-09-02.** The criteria below were fixed *before* listening, for the reason
`e0-s3-g0-qualification-report-v1.md` states about its own: criteria chosen after hearing the audio
are criteria chosen to fit it. They were not changed after the listening, and the disposition below
was entered by the person who listened.

| # | Criterion | Finding |
|---|---|---|
| 1 | Segment joins are inaudible: no click, truncation, or overlap at either boundary | None |
| 2 | Pauses fall where the lesson declares them — 400 ms, 2 000 ms, 600 ms — and the recall prompt's interval is long enough to answer in | None |
| 3 | `lesson.mp3` carries no encoder artifact absent from `lesson.wav`: no swirl on sibilants, no pre-echo, no obvious high-frequency loss | None audible in this environment |
| 4 | Level and tone are consistent across all three segments, with no audible drift between them | None audible in this environment |
| 5 | Spoken text matches `spoken_text` for every segment, with no omission, addition, or substitution | None |

| Field | Value |
|---|---|
| Reviewer | Ross Todd, project owner and engineering owner |
| Date | 2026-09-02 |
| Playback environment | Built-in laptop speakers |
| Overall finding | No finding on any of the five criteria |
| Disposition | `accept` |

### What this review will not cover

- **Duration alone is not a finding.** The master grew 2.56 s against E1-S4's for the same text.
  That is the seeded decoder generating different-length speech, not evidence of a defect, and
  criterion 5 is what would catch it if the words themselves had changed.
- **Reproducibility is measured, not heard.** Whether two lifetimes agree is
  `t5_e1_two_lifetimes_render_identical_audio_under_one_seed`'s answer. A listener cannot hear it
  and is not asked to.
- **Built-in laptop speakers.** This is the limit that bears hardest on criterion 3, which is the
  criterion the MP3 review exists for. `libmp3lame` artifacts at `128k` — swirl on sibilants,
  pre-echo, high-frequency loss — sit in exactly the band small drivers reproduce least, so the
  clear result above records that nothing was audible on those speakers and does not establish that
  nothing is audible on headphones or monitors. Criterion 4's level and tone judgment is bounded the
  same way. Criteria 1, 2, and 5 — joins, pause placement, and spoken-versus-written text — are not:
  a click, a misplaced silence, or a wrong word carries on any speaker.

## Limitations

- **One lesson, one voice, one language.** Three segments of English through `owner-fallback-v1`.
- **Listening is verified once, on one environment.** §What this review will not cover states what
  built-in laptop speakers reach and what they do not.
- **Reproducibility is bounded to what was measured.** One environment, one seed, one sentence, per
  `E1-S3-INTERFACE-CHANGE-005` §Limits. The capability declares `reproducible`; ADR-0001 §12.5's
  warning that identical seeds do not guarantee identical output across dependency, platform, or
  execution changes still stands, and no result here contradicts it.
- **The frozen bundle identity depends on a deferred decision.**
  `docs/architecture/G1-FREEZE-CHARTER.md` records that `worker/pyproject.toml` stays a declared
  bundle input though ADR-0001 §12.5 does not list it, with removal owed at the next identity move.
  When that happens, the render and listening above become historical and a fresh review is owed.

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately.

**Every row below is signed.**

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Contract owner (T-CLI) | Ross Todd for T-CLI | Accept `lesson new` and `lesson validate` as the published E1-S5 surface | 2026-09-02 |
| Contract owner (T-CORE) | Ross Todd for T-CORE | Accept the authoring API additions and that no published schema moved | 2026-09-02 |
| Engineering owner | Ross Todd | Accept the requalification result and the corrected reproducibility criterion | 2026-09-02 |
| Project owner | Ross Todd | Accept the candidate as a whole. All four tasks, all three named tests, six of six reference-machine criteria, and a listening review taken 2026-09-02 with no finding | 2026-09-02 |
