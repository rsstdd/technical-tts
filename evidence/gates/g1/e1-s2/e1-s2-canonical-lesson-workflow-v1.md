# E1-S2 Minimal Canonical Lesson Workflow v1

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Candidate revision: working tree on `main` at the E1-S2 implementation
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Proposed

## Scope and decision

`DELIVERY-PLAN.md` §E1-S2 names five tasks and five acceptance tests. Three of the tasks were
already satisfied by the E1-S1 baseline and are pinned rather than rebuilt here; two were open,
and both were recorded as owed to this story before it began:

- **Voice references were unresolved.** `crates/study-tts-core/src/identity.rs` documented
  `voice_conditioning_hashes` as "absent for a speaker whose voice profile has not been resolved,
  which is every speaker until E1-S2 resolves voice references", and
  `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` §Impact of the two deliberately incomplete
  inputs said the same and required its own record when it landed.
- **No diagnostic carried a location.** `LessonError` named the segment but never the document or
  the field, so `DELIVERY-PLAN.md` E1-S5's
  `t1_e1_validation_error_names_the_offending_field_path` had nothing to read.

The interface consequences are recorded in
[`../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`](../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-001.md),
which this record does not restate.

This record files under `g1/` because E1-S2 feeds G1. It is a story-level record under the gate it
serves, not a gate acceptance.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all six hold:

1. All five tests `DELIVERY-PLAN.md` §E1-S2 names exist under those exact names and pass.
2. Every `LessonError` variant is reachable and distinct, and every refusal — whether the lesson
   module raises it or `serde` raises it first — carries the document, a JSON Pointer to the field
   it is about, and the segment identity when it is inside one, with no `_` arm anywhere that maps
   a variant to a pointer. Each invariant ADR-0001 §8.2 declares has its own variant, including the
   three closed vocabularies `serde` refuses before the module sees a value; what remains under
   `LessonError::InvalidJson` is the document's shape, which is one invariant located by its
   pointer rather than a class of invariants sharing a catch-all.
3. No lesson field marked display-only reaches a `SynthesisRequest`, and the exclusion is
   structural rather than filtered.
4. Every speaker a segment names resolves to a checksummed, consent-gated voice profile before
   planning, and a build with no root to resolve one is refused rather than defaulted.
5. `cargo fmt --check`, `python3 scripts/check-rust-conventions.py`, Clippy with `-D warnings`,
   the full workspace suite including the 35-test walking skeleton against real FFmpeg, the
   doctests, the Python worker suite, schema-drift regeneration, `cargo deny check`, and
   `taplo fmt --check` are all clean.
6. Every accepted evidence record whose pins this change moved is accounted for.

## Result

| Criterion | Result |
|---|---|
| 1. Five named tests | Met. `t1_e1_each_lesson_invariant_has_a_distinct_error` and `t2_e1_unicode_and_protected_terms_survive_round_trip` in `crates/study-tts-core/src/lesson.rs`; `t2_e1_plan_is_stable_for_identical_lesson_input` in `crates/study-tts-core/src/plan.rs`; `t1_e1_unreviewed_lesson_fails_before_worker_start` and `t1_e1_display_text_never_enters_synthesis_request` in `crates/study-tts-runtime/src/pipeline.rs`. |
| 2. Distinct located errors | Met. 46 refusals exercised by `t1_e1_each_lesson_invariant_has_a_distinct_error`, each asserted for its own variant and its own field path, covering all 43 `LessonError` variants. Nine of them are the three closed vocabularies in all three forms each can fail in — absent, outside the vocabulary, and not a string — because sampling one form per field is what previously let a missing role and an unknown role share a refusal while a test asking only for the unknown one passed. `vocabulary_refusal` classifies the first two into per-field variants by reading the authored document at the deserializer's pointer, never `serde`'s message; the third stays `InvalidJson`, which is the document not having the declared shape and is one invariant located by its pointer. Two of the 46 are the two forms `/source/content_hash` can fail in: a string the digest rule refuses now earns `MalformedSourceContentHash` through `source_hash_refusal`, which classifies by reparsing the authored value exactly as `vocabulary_refusal` does, while a value of the wrong type stays `InvalidJson` at that pointer. Two assertions, because two claims: every pair of located refusals is distinct, and the count of distinct variants is 43. `field_of` is an exhaustive match with no `_` arm, so a new variant does not compile until it is given a pointer; the variant count then fails until it is given a case, and the pairwise check fails if that case is answered by a refusal another invariant already produces — confirmed live by pointing two vocabulary cases at one field. Pointer escaping is pinned by `t1_e1_a_field_path_escapes_the_name_it_points_through`; the `serde` boundary by `t1_e1_a_shape_error_is_located_at_the_field_it_is_about`, whose omitted-field case now uses `spoken_text` because an omitted vocabulary field is no longer a shape refusal, and by `t1_e1_bytes_that_are_not_json_name_the_document_and_nothing_else` and `t1_e1_content_after_the_lesson_document_is_refused` for the two refusals that have no field to name. One case is written out rather than tabulated: a repeated `speakers` key cannot be built by mutating a `serde_json::Value`, which is a map, and that is the same reason `repeated_speaker` reads the document's bytes rather than the parsed lesson. |
| 3. Display text excluded | Met, structurally: `SynthesisRequest` declares no display field, and `synthesis_requests` builds it as a literal naming every field with no `..`, so there is nothing for a transcript to be filtered out of. `PlannedSegment` does carry `display_text` — the package writer is handed the plan and needs the transcript for the audio it selected — and `t1_e1_display_text_reaches_the_plan_without_reaching_a_cache_key` asserts that asymmetry. `t1_e1_display_text_never_enters_synthesis_request` marks the display text and asserts no field of any produced request carries the marker. |
| 4. Voices resolved before planning | Met. `voice_gate::resolve_speakers` runs between validation and planning, keys its work by profile identity so one profile is read once however many speakers name it, loads each through the existing fail-closed gate, and `BuildRequest::voice_profile_root` is required rather than optional. `RenderPlan::for_lesson` refuses with `PlanError::UnresolvedSpeaker` rather than deriving a key for a speaker the caller did not resolve, so the guarantee is the planner's own and not a convention callers must keep. |
| 5. Local gate | Met; commands and results in §Verification. |
| 6. Provenance | Met. Eleven pins in `e1-s1-provisional-contract-baseline-v13` moved with E1-S2; each is accounted for by the accepted `e1-s2-evidence-provenance-reconciliation-v3`, which supersedes v2 and through it v1. Each supersession has the same cause: a reconciliation had read fewer rounds of E1-S2 than the bytes it granted — v1 stopped at the lesson document moving `1.1` → `2.1`, and v2 stopped after the third review. The twenty-second E1-S1 audit then moved three further v13 pins — `crates/study-tts-runtime/src/worker_protocol.rs`, `crates/study-tts-runtime/src/lib.rs`, and `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` — none of them E1-S2's; `e1-s1-provisional-contract-baseline-v14` records all three and clears them by superseding v13, and was accepted on 2026-08-30. Three of v14's own pins then moved in turn — `docs/INDEX.md`, `crates/study-tts-core/src/lesson.rs`, and `docs/governance/TRACEABILITY-MATRIX.md`, for the provenance correction and the two E1-S2 audit fixes — and `e1-s1-provisional-contract-baseline-v15` re-pinned all three and cleared them on its acceptance, also 2026-08-30. None of the six was an accounting E1-S2 owes. `python3 scripts/check-evidence-provenance.py` exits zero on this branch. Accepting v14 also retires v3's eleven pairs, because a superseded record is not checked: they suppress nothing now, so v3's own reading stopping at the fifth review — the sixth landed after v3 was accepted and moved four of those eleven files again — grants nothing it did not examine. v3 stays the reading of why each of the eleven moved. |

## How a refusal is located

Every refusal `crates/study-tts-core/src/lesson.rs` raises itself is located by `field_of`: a JSON
Pointer to the offending field, the segment identity when it is about a segment, and the document
in every case. Speaker bindings point at `/speakers/<speaker>/voice_profile` rather than at the
map, with the name escaped per RFC 6901 because a speaker name is authored text under no
portability rule.

A refusal `serde` raises before this module sees the data is located by the deserializer, through
`serde_path_to_error`: `review_status: "aproved"` reports `/segments/1/review_status` and segment
`seg-0002`, a field of the wrong type reports that field, and a field no version declares reports
itself. The path comes from the deserializer rather than from a hand-written walk of the document
shape, so it cannot drift from the types — this crate *is* that shape. §New dependency in the
interface-change record carries the argument.

An omitted field is the one case the deserializer alone cannot locate: `serde` raises it against
the object that should have carried the key, so the path stops at the parent and criterion 5 would
get `/segments/1` where the author needs `/segments/1/style`. `omitted_field` reads the key from
the message `serde::de::Error::missing_field` formats and then confirms against a lenient parse of
the same bytes that the parent genuinely lacks it, so a message shape this build does not
recognize degrades to the parent pointer rather than inventing a field. The lesson-level and
segment-level cases are both pinned in
`t1_e1_a_shape_error_is_located_at_the_field_it_is_about`.

Two refusals name no field, and correctly. Bytes that are not JSON at all have no document to
point into, so the empty pointer RFC 6901 defines as the whole document is what they carry;
`t1_e1_bytes_that_are_not_json_name_the_document_and_nothing_else` pins that. A document carrying
a second JSON value after the lesson is the same case: a located deserializer stops at the end of
the first value rather than at the end of the input, so `from_json` calls `Deserializer::end` and
refuses the remainder, and the objection is to the document rather than to any field in it.
`t1_e1_content_after_the_lesson_document_is_refused` pins that the trailing bytes cannot be
accepted unread by a validation, checksum, or review.

## What the two T1 ordering tests actually prove

Recorded because the tier is load-bearing. `docs/testing/TEST-STRATEGY.md` requires a `t1_` prefix
to mean a pure deterministic function, and `DELIVERY-PLAN.md` names both of these `t1_`. They are
pure: `pipeline::plan_requests` was extracted so the path from validated lesson to backend request
reaches no filesystem and starts no process.

`t1_e1_unreviewed_lesson_fails_before_worker_start` therefore proves the ordering by construction
rather than by observation — a draft lesson yields no `ValidatedLesson`, `plan_requests` accepts
nothing else, and so no `SynthesisRequest` exists to send. It then corrects the one field the
document is wrong about and shows two requests appear, which is what makes the refusal
attributable to the review state. The end-to-end half of the same claim remains
`t4_e0_unapproved_content_fails_before_tools_and_synthesis`, which points the build at a
nonexistent FFmpeg so a late gate would report the missing tool instead.

**What that end-to-end half now observes, since the sixth review.** It asserted
`synthesis_count() == 0`, which a build that had already asked the backend for its descriptor also
satisfies — so "no worker start" was argued at both ends and observed at neither. `FakeTtsExecutor`
now counts every `TtsExecutor` call it receives, and that test asserts the count is zero: the
backend is not reached at all before the lesson gate, so a worker that starts on first use has not
started. Moving `descriptor()` above the lesson gate in `build_preview_with_services` fails that
test and no other. What remains argued is an executor that starts a process in its own
constructor, which `build_preview` cannot observe because it receives one already built and
`TtsExecutor` declares no lifecycle method; `E1-S2-INTERFACE-CHANGE-002` §Worker-start ordering
records that it becomes testable when E1-S3 lands a process-spawning pool, and why no seam for it
was invented here.

## Deviations and limitations

- **`t1_e1_each_lesson_invariant_has_a_distinct_error` overlaps two E0 tests.**
  `t1_e0_review_context_invariants_have_distinct_errors` and
  `t1_e0_synthesis_selection_invariants_have_distinct_errors` are also `DELIVERY-PLAN.md` names
  and are kept unchanged. The E1 test is the exhaustive one and additionally asserts the field
  path; the E0 pair remain the narrower named contracts they were written as.
- **No new negative lesson fixture was committed.** The plan drafted one for an undeclared
  speaker. It was dropped: `t3_e1_the_published_lesson_schema_refuses_the_invalid_fixtures` and
  `t3_e1_invalid_lesson_fixtures_are_refused_by_their_own_invariant` are a deliberate pair over
  the *same* fixtures, and an undeclared speaker is a cross-field rule no JSON Schema expresses,
  so such a fixture could only have been added to one half and would have broken the pairing. The
  rule is covered by the table in `t1_e1_each_lesson_invariant_has_a_distinct_error`.
- **`UnusedSpeaker` was not added.** Only speakers a segment names are resolved, so an unused
  declaration triggers no rights check, no checksum, and no identity change. Refusing one would be
  hygiene rather than a control.
- **`t4_e1_two_speakers_may_share_one_voice_profile` does not prove "resolved once".** That one
  profile is read once however many speakers name it is structural — `resolve_speakers` keys its
  work by profile identity — and no seam reachable from `study-tts-testkit` can count reads. The
  test asserts what it can observe, that the shared profile resolves, and its name and doc comment
  say so rather than claiming the stronger property.
- **`MissingSpeaker` keeps its position after the review-status check.** `AuthoredLesson::validate`
  promises that existing semantic checks preserve their relative order, so `UndeclaredSpeaker` was
  inserted immediately after `MissingSpeaker` rather than moved ahead of the review gate.
- **Human listening is unverified and not applicable to this change.** Every cache key moved, so
  every segment will be re-synthesized, but this story produces no audio: the product worker still
  refuses `synthesize` until E1-S3. Listening remains a G1 prerequisite.
- **The voice-profile root is not contained.** It is operator-supplied input joined by path and
  subject to the per-record symlink refusal `voice_gate` already applied, not routed through
  `managed::leaf`. E5-S4 owns directory-relative containment.

## Second review of this story's implementation

A review after the first draft of this record found two defects in what E1-S2 had landed. Both
were corrected before this record was updated, and both are classified and reasoned in
[`../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`](../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-002.md)
§Identification items 5 and 6. They are named here because §Result above now describes the
corrected implementation, and a reader comparing this record against its first draft is entitled
to know which numbers moved and why.

- **The render plan changed shape while `PLAN_SCHEMA_VERSION` stood at `1.0`.** `display_text`
  became required and `style` was narrowed, both **Breaking contract** rows under
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes, whose required action
  is a major version. The held version rested on `ADR-0001-D005`'s reasoning without meeting its
  conditions: condition 2 wants the retained version to have come from an unreleased breaking move
  *within the same story*, and `plan 1.0` came from E1-S1. The plan is now `2.0`, published at
  `schemas/plan-v2.schema.json`, and `schemas/plan-v1.schema.json` is deleted.
  `t3_e1_generated_schemas_match_checked_in_files` reads `schemas/` in both directions, so the
  retired file could not have been left behind, and `PUBLISHED_REQUIRED_SURFACE`'s rows moved with
  it. No identity moved: the plan hash is computed by `plan_digest` over segments, not over the
  document version.
- **Criterion 2 was met more narrowly than it was stated.** Closing the `role` and `style`
  vocabularies made three closed vocabularies share `LessonError::InvalidJson`, and
  `t1_e1_each_lesson_invariant_has_a_distinct_error` admitted only one of the three to its
  distinctness set — so two task-specific invariants could share one refusal while the test named
  for distinctness passed.

A third review then found that the first attempt at that second correction was itself too weak, and
one control was wrong rather than merely overclaimed:

- **Recall-prompt pauses did not hold to ADR-0001.** Validation enforced §13.2's 1,500 ms floor and
  then the generic 10,000 ms segment ceiling, so a recall prompt declaring 4,001-10,000 ms was
  accepted although §13.2 gives a recall question 1.5-4 seconds. §8.2 admits a pause outside policy
  only "unless an override is annotated", and the lesson format declares no override annotation, so
  nothing authorized those values. This is the one item of the four that was a **control defect**
  rather than a documentation or test defect: a lesson the ADR does not permit was being accepted.
  `MAX_RECALL_RESPONSE_MS` and `LessonError::RecallPromptResponseIntervalTooLong` close it, and
  `t1_e1_a_recall_prompt_must_leave_a_response_interval` now asserts both edges from both sides and
  checks the same over-long pause under a non-prompt role.
- **Keying distinctness on `(variant, pointer)` did not carry the claim either.** An absent role, an
  unrecognized role, and a role that is not a string all produced `InvalidJson` at
  `/segments/0/role`, and the test sampled one form per field, so the collision was unreachable from
  it. `crates/AGENTS.md` requires one distinct variant per violated invariant, so the correction is
  in the enum after all: six variants — `UnknownSegmentRole`, `UnknownDeliveryStyle`,
  `UnknownReviewStatus`, `MissingSegmentRole`, `MissingDeliveryStyle`, `MissingReviewStatus` — plus
  `vocabulary_refusal`, which classifies a located `serde` refusal by reading the authored document
  at the deserializer's pointer rather than parsing `serde`'s message. The closed enums stay closed
  and the published schema still names every value, so the fail-closed parse boundary is unchanged.
  A wrong JSON type deliberately stays `InvalidJson`; that reasoning is in the interface record's
  §`LessonError` distinctness.

None of the four weakened a validation, containment, rights, checksum, consent, offline, or
recovery control. Three replaced a claim with a true one or a test with a stricter one; the fourth
added a refusal where the ADR required one and none existed.

## Fourth and fifth reviews

A fourth review found three defects, all of them in what the repository *said* rather than in what
it did, and all classified in
[`../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`](../../../../docs/architecture/E1-S2-INTERFACE-CHANGE-002.md)
§Identification items 9 through 11: that record described itself as accepted and signed while no
role had decided anything, with `AGENTS.md` and `docs/INDEX.md` repeating the claim; it
contradicted itself on the recall ceiling and cited a superseded reconciliation; and
`AuthoredLesson::validate`'s `# Errors` section omitted a refusal correction 7 had made it return.
The last of those widened `t3_e1_every_documented_error_variant_is_named_by_its_errors_section`
from one entry point to a table of them.

A fifth review found one **control defect**, the second this story has had:

- **A speaker declared twice was accepted, and chose a voice.** `AuthoredLesson::speakers` is a
  `BTreeMap`, so a document binding `nadia` under two voice profiles parsed, validated, and
  synthesized under whichever binding the author wrote last, with nothing downstream able to see
  the other. That is not a cosmetic ambiguity: ADR-0001 §12.5 makes the resolved conditioning
  artifact a synthesis-key input, so a reviewed lesson could be spoken — and cached — in a voice
  the review never selected. It also makes criterion 2 above false as stated, because the invariant
  "one speaker, one voice" had no refusal at all. `LessonError::DuplicateSpeaker`, located at
  `/speakers/<name>`, closes it, and `repeated_speaker` finds the repeat by reading the document's
  bytes a second time, because the parsed lesson has already discarded one of the two bindings.
  `t1_e1_a_speaker_declared_twice_is_refused` pins it over both the differing-profile and
  identical-profile cases; refusing the identical case too keeps the rule about the document rather
  than about how much the ambiguity happened to cost.

The same review found three stale statements in governed documents — a
`docs/architecture/WALKING-SKELETON.md` paragraph still naming the deleted
`schemas/lesson-v2.schema.json` as the published schema, a `docs/INDEX.md` row crediting the
reconciliation with ten pins where it accounts for eleven, and the interface record's own count of
"eight gaps, in three rounds" where it lists twelve across five. Each is corrected in place; none
is a control.

None of the four documentation defects weakened a validation, containment, rights, checksum,
consent, offline, or recovery control. The one control defect is a refusal that did not exist and
now does.

## Defect found and fixed during review

`docs/testing/TEST-STRATEGY.md` §Failure policy calls a flaky test a defect, so this is recorded
rather than retried away.

Installing the synthetic voice profiles inside `build_request` made
`t4_e0_concurrent_jobs_share_one_internally_consistent_cache_winner` and
`t4_e0_live_lesson_job_lock_refuses_a_second_build` intermittently fail, roughly one run in eight:
both build two previews in one workspace, so two threads installed into one profile root and one
rewrote `reference.wav` while the other hashed it. The refusal was
`VoiceProfileError::VoiceChecksumMismatch` — a control correctly reporting the artifact it read
did not match its record, against a profile nobody had tampered with.

The cause was the helper, not either test, so the fix is there: each request installs its own
`voices-<n>` root. The profiles are byte-identical, so two builds still derive the same
conditioning hash and share a cache, which is what those tests assert. Twenty consecutive
`walking_skeleton` runs and ten consecutive full-workspace runs are clean; before the fix the same
loop reproduced it.

Worth stating plainly: the race was introduced by this story and found only because the suite was
run repeatedly rather than once.

## Open findings and the acceptance this record is waiting for

None affecting this story's own criteria; all six are met.

**This record itself stays `Proposed`, deliberately.** `evidence/README.md` §Accepting a record at
its gate says a story keeps one record, "`Proposed` for as long as the work runs … and accepted
once, at the gate it serves, against the bytes that gate actually approved. A version after that
means a conclusion was wrong." E1-S2 serves G1, which has not run. Accepting it now would accept
it against bytes G1 has not seen, and any later correction would have to arrive as a `-v2`
carrying a signal — that a conclusion was wrong — which would not be true. It is accepted at G1,
not here.

That is not the same question as the provenance reconciliation, which had to be accepted now:
it grants a suppression that only takes effect while accepted, and it makes a claim about bytes
that exist today rather than about a gate's outcome.

## Verification run

Run from the repository root on Ubuntu 24.04 under WSL2, with `ffmpeg` and `ffprobe` on `PATH`.

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `python3 scripts/check-rust-conventions.py` | Pass, 0 violations in 67 files |
| `cargo clippy --workspace --all-targets --all-features --offline --locked -- -D warnings` | Pass |
| `cargo test --workspace --offline --locked --all-targets` | Pass, 306 tests; 10 consecutive runs clean. Re-run at the sixth review: 308, the two added since belonging to the twenty-second E1-S1 audit |
| `cargo test --workspace --offline --locked --doc` | Pass, 7 doctests |
| `cargo test --offline -p study-tts-testkit --test walking_skeleton --locked` | Pass, 35 tests, real FFmpeg and ffprobe; 20 consecutive runs clean |
| `python3 -m unittest discover --start-directory worker/tests` | Pass, 44 tests |
| `cargo deny check` | Pass, including the added `serde_path_to_error` |
| `taplo fmt --check` | Pass |
| `cargo run --offline --locked --package study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | Pass, no drift |
| `python3 scripts/check-evidence-provenance.py` | Pass, 0 unaccounted mismatches. It has been red twice since first run, both times on pins belonging to E1-S1 baseline records rather than to any accounting E1-S2 owes: three v13 pins the twenty-second E1-S1 audit moved, cleared when `e1-s1-provisional-contract-baseline-v14` was accepted and superseded v13; then three of v14's own pins — `docs/INDEX.md`, `crates/study-tts-core/src/lesson.rs`, and `docs/governance/TRACEABILITY-MATRIX.md` — moved by the provenance correction and the two E1-S2 audit fixes, cleared when `e1-s1-provisional-contract-baseline-v15` was accepted on 2026-08-30 and superseded v14. The eleven pins E1-S2 moved were accounted for by the accepted `e1-s2-evidence-provenance-reconciliation-v3` and are retired with v13 |

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner (T-CORE) | | Pending | |
| Engineering owner | | Pending | |
| Project owner | | Pending | |
