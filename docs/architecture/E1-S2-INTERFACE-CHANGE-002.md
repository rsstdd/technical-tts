# E1-S2 Interface Change 002 — Canonical lesson fields, closed vocabularies, planned transcript

## Identification

- Record ID: `E1-S2-INTERFACE-CHANGE-002`
- Status: **Accepted, 2026-08-30.** §Approval records the decision each role made and the date
  it was signed.
- Contract owner: T-CORE (lesson document, render plan)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-RUNTIME, T-CLI, T-AUDIO
- Accepted ADR, if architectural: not applicable. This implements ADR-0001 §8.1 (`learning_objectives`
  and `source` in the canonical format), §8.2 ("speaker, role, style, and voice profile are
  declared"; "recall prompts include a deliberate response interval"), §8.3 (`display_text` is the
  readable transcript), §13.2 (the recall-question pause range), and §13.4 (one frozen loudness
  reference per voice-profile hash and style) as written. No authority boundary moves.

This record closes fourteen gaps, in six rounds. The first four a review of
[`E1-S2-INTERFACE-CHANGE-001.md`](E1-S2-INTERFACE-CHANGE-001.md)'s implementation found against
`DELIVERY-PLAN.md` E1-S2:

1. **Task 1 was incomplete.** ADR-0001 §8.1's canonical lesson carries `learning_objectives` and a
   top-level `source`; `AuthoredLesson` declared neither and denies unknown fields, so the ADR's own
   document was refused at `/learning_objectives`.
2. **Task 2 was incomplete.** `role` and `style` were checked only for blankness, so any string
   passed, and §8.2's recall-prompt invariant was not expressible at all — a recall prompt with a
   zero-millisecond pause validated.
3. **Task 4 was half met.** `PlannedSegment` deliberately dropped `display_text`, and the package
   writer is handed only the plan and the cached audio, so nothing downstream of validation could
   reach the transcript. Sending only `spoken_text` to synthesis was already correct.
4. **`build_preview`'s `# Errors` section was stale**, omitting five lesson and three voice-profile
   refusals that E1-S2 added, one plan refusal, and one audio refusal.

A second review of *this* record's own implementation found two more, closed in the same change
and recorded here rather than in a successor, because this record had not been accepted when
they were found:

5. **The render plan changed incompatibly under a held version.** Task 4's fix added a required
   field and narrowed another while `PLAN_SCHEMA_VERSION` stood at `1.0`, on an argument belonging
   to a deviation whose conditions this change does not meet. §Render plan below is rewritten; the
   version is `2.0`.
6. **`t1_e1_each_lesson_invariant_has_a_distinct_error` did not uphold its name.** Task 2's closed
   vocabularies are refused by `serde`, so an invalid `role`, `style`, and `review_status` all
   carried `LessonError::InvalidJson`; the test admitted one of the three to its distinctness set
   and left the other two to a sibling test that asserted the shared variant. Two task-specific
   invariants could therefore share one refusal while the acceptance test named for distinctness
   passed. §`LessonError` distinctness below records the correction.

A third review found two more, closed on the same terms:

7. **Recall-prompt pauses did not hold to ADR-0001.** Validation enforced §13.2's 1,500 ms floor
   and then the generic 10,000 ms segment ceiling, so a recall prompt could declare 4,001-10,000 ms
   — outside the 1.5-4 second range §13.2 gives a recall question. §8.2 permits a pause outside
   policy "unless an override is annotated", and the lesson format declares no override annotation,
   so there was no reading under which those values were authorized. §Recall response interval
   below records the correction.
8. **The distinctness contract was still provable by sampling.** Correction 6 gave each vocabulary
   its own variant but left an absent field, an unrecognized value, and a wrong type at one of
   those fields sharing `(InvalidJson, pointer)`, and the test sampled one form per field, so the
   collision was unreachable from it. §`LessonError` distinctness records what changed.

A fourth review found three more, closed on the same terms:

9. **This record claimed approvals it had not received.** §Approval carried nine rows signed and
   dated, and `AGENTS.md` §State and `docs/INDEX.md` repeated the claim, while no role had decided
   anything and item 5 above already said this record "has not been accepted". A record cannot
   sign itself, and a reader trusting three documents at once would have read an implemented change
   as an accepted one. §Approval stated each decision as sought and pending from that review
   until this record was signed, and both documents described it as `Proposed` and unsigned for
   the same period.
10. **This record contradicted itself, and cited a superseded record.** §Version and compatibility
    said the recall ceiling was "deliberately not enforced" and §Limits repeated it, while
    §Recall response interval and the build both enforce it; the migration procedure asked for a
    1,500 ms floor and no ceiling, so an author following it could produce a `3.1` document this
    build refuses. §Delivery and recovery still described the provenance reconciliation v2 as owed
    and unwritten, although it exists and is `Accepted`, and
    `evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md` §Result still credited the
    superseded v1. Every one of those statements now says what the tree holds.
11. **`AuthoredLesson::validate`'s `# Errors` section was stale.** It omitted
    `LessonError::RecallPromptResponseIntervalTooLong`, which correction 7 had made that function
    return, so a public boundary documented a refusal contract it no longer had. The
    documentation-drift test item 4 added reads one entry point — `build_preview` — so the one
    boundary correction 7 changed was the one it could not see. §Delivery and recovery records the
    widened test.

A fifth review found one more, closed on the same terms:

12. **A speaker declared twice was accepted silently.** `AuthoredLesson::speakers` is a
    `BTreeMap`, and a map keeps the last value for a repeated key, so a document binding one
    speaker to two voice profiles validated and the speaker was rendered with whichever binding the
    author wrote last. ADR-0001 §12.5 makes the resolved conditioning artifact a synthesis-key
    input, so the voice a reviewed lesson is spoken in was being chosen by parser behavior rather
    than by the review. §Repeated speaker bindings below records the correction. The same review
    found three stale statements in governed documents, corrected in place: this record's own
    "eight gaps, in three rounds" above, a `docs/INDEX.md` row crediting the provenance
    reconciliation with ten accounted pins where it accounts for eleven, and a
    `docs/architecture/WALKING-SKELETON.md` paragraph still naming the deleted
    `schemas/lesson-v2.schema.json` as the published lesson schema.

A sixth review found two more, closed on the same terms:

13. **A malformed source digest had no refusal of its own.** `LessonSource::content_hash` parses
    through `SourceContentHash`, which refuses a value that is not a BLAKE3 digest — and `serde`
    delivered that typed refusal as prose inside `LessonError::InvalidJson`, located at
    `/source/content_hash`. A recorded hash that is not a digest and a `content_hash` that is not a
    string therefore arrived as one located refusal, so `MalformedSourceContentHash` — a public
    error type carrying its own remedy, *recompile from the source document* — named an invariant
    no caller could match on and no test asserted. It is correction 6's gap in the one field
    correction 6 did not reach: `t1_e1_each_lesson_invariant_has_a_distinct_error` had no case at
    `/source/content_hash` at all. §Source content hash below records the correction.
14. **The T1 ordering test proved its claim by construction only.**
    `t1_e1_unreviewed_lesson_fails_before_worker_start` calls `plan_requests`, a pure helper, and
    never observes an executor; `build_preview` receives one already constructed. Every assertion
    it makes would still pass against a backend that had started. The end-to-end half,
    `t4_e0_unapproved_content_fails_before_tools_and_synthesis`, asserted `synthesis_count() == 0`
    — which a build that had already asked the backend for its descriptor also satisfies.
    §Worker-start ordering below records what is now observed and what remains argued.

## Version and compatibility

### Lesson document — `2.1` → `3.0` → `3.1`

- Contract ID: `lesson`, published at `schemas/lesson-v3.schema.json`
- Old version: `2.1`
- New version: `3.1`
- Compatibility class: **breaking** (`3.0`), then **compatible extension** (`3.1`)
- Required/defaulted fields: `3.0` adds no field. It narrows two: `segments[].role` becomes the
  closed `SegmentRole` vocabulary and `segments[].style` the closed `DeliveryStyle` vocabulary, and
  a `recall_prompt` segment must declare a pause inside 1,500-4,000 ms. It also refuses a
  `speakers` object that binds one name twice. `3.1` adds optional
  `learning_objectives` (declared default: empty) and optional `source` (declared default: absent);
  when `source` is present its `content_hash` is required.
- Unknown-field behavior: unchanged. `#[serde(deny_unknown_fields)]` on `AuthoredLesson`,
  `LessonSegment`, `SpeakerDeclaration`, and the new `LessonSource`; `additionalProperties: false`
  in the published schema, except `speakers`. Repeated-field behavior is what moved: `serde`'s
  derived struct parsers already refused a field written twice, and `speakers` — the one open
  object — now does too.
- Wire or Rust representation changed: `LessonSegment::role` becomes `SegmentRole` and
  `LessonSegment::style` becomes `DeliveryStyle`; `AuthoredLesson` gains
  `learning_objectives: Vec<String>` and `source: Option<LessonSource>`; `ValidatedLesson` gains
  `learning_objectives()` and `source()`. `AuthoredLesson::new` is unchanged and leaves both `3.1`
  records empty, because both are optional in the format.

`3.0` is breaking under `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes as
a **semantic change**: a `2.x` document may write anything in either field, and this build now
refuses everything outside the two vocabularies. `schemas/lesson-v2.schema.json` is deleted rather
than kept beside its successor, for the reason `E1-S2-INTERFACE-CHANGE-001` deleted `lesson-v1`:
`schema_file_name` keys a published schema on its major, and a v2 file no build reads would tell an
author their editor still checks something.

`3.1` follows in the same change for the reason `2.1` followed `2.0`: a major with no older minor
leaves `t3_e1_compatible_minor_extension_is_accepted` — an E1-S1 acceptance test named in
`DELIVERY-PLAN.md` — with no lesson document to exercise. `fixtures/lessons/e1-s2-prior-minor.json`
now declares `3.0` and omits both added records, and that test reads their declared defaults back.

Neither vocabulary is invented. `SegmentRole`'s seventeen variants are ADR-0001 §3.2's two speaker
repertoires and §3.4's default study sequence, which name the same vocabulary twice.
`DeliveryStyle`'s four are §8.1's canonical `calm_explanatory` plus §5.1's "calm, emphatic, and
deliberately slow delivery". `MIN_RECALL_RESPONSE_MS` and `MAX_RECALL_RESPONSE_MS` are the two ends
of §13.2's 1.5–4 second recall range, and both are enforced; §Recall response interval below records
why the ceiling is the recall prompt's own rule rather than the generic `MAX_PAUSE_AFTER_MS`.

`LessonError::MissingRole` and `LessonError::MissingStyle` are **removed as spellings, not as
invariants**. A blank or unrecognized role or style is refused at the parse boundary and located at
its own field, exactly as `review_status` already was; an absent one returns
`LessonError::MissingSegmentRole` or `MissingDeliveryStyle`, which name the vocabulary they belong
to. §`LessonError` distinctness records why absent and unrecognized stayed separate.

### Synthesis identity — unchanged

`SYNTHESIS_IDENTITY_VERSION` stays `e1-s2-v1` and **every cache key in the project is unchanged.**
`DeliveryStyle::as_str` writes the same four spellings a `2.x` lesson wrote, and
`t1_e1_delivery_style_spelling_matches_its_serde_form` holds `as_str` to the serde form so the key's
bytes cannot drift from the document's. The two segment keys pinned in
`t1_e0_plan_is_stable_for_identical_inputs` are byte-identical to their E1-S2 values, which is the
evidence for this paragraph rather than an argument for it. No published cache entry is orphaned.

### Render plan — `1.0` → `2.0`

- Contract ID: `plan`, published at `schemas/plan-v2.schema.json`
- Old version: `1.0`
- New version: `2.0`
- Compatibility class: **Breaking contract**
- `PlannedSegment` gains required `display_text: String`, and `style` becomes `DeliveryStyle`
  instead of `String`, so the published plan schema names the four renderable styles. A required
  field and a narrowed field are each named by
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes' **Breaking contract**
  row, whose required action is a major version.
- `display_text` enters `plan_digest` and therefore the plan hash. It does **not** enter the cache
  key. That asymmetry is the whole point: ADR-0001 §12.5 excludes display-only fields from
  synthesis, so a transcript correction must reuse every cached segment; but a package carries the
  transcript for the audio it selected, and a plan identity blind to it would let corrected text be
  reconciled away as a package already selected.
  `t1_e1_display_text_reaches_the_plan_without_reaching_a_cache_key` asserts both halves, because
  either alone passes for a plan that put display text in both or in neither.
- **The major moves; `ADR-0001-D005` does not reach this change.** An earlier revision of this
  record held the version at `1.0`, arguing that no `plan.json` has ever been written and so a
  `2.0` would tell E2's loader that a `1.x` plan might be encountered when none can be. That
  argument is the deviation's, and the deviation does not apply: condition 2 requires the version
  being retained to have been "introduced by an unreleased breaking move within the same story",
  and `plan 1.0` was introduced by E1-S1 and recorded in
  [`E1-S1-INTERFACE-CHANGE-001.md`](E1-S1-INTERFACE-CHANGE-001.md) §Two of these move a published
  boundary. A permission with five conditions is not a principle to reapply where a condition
  fails. What the empty disk changes is the *cost* of the increment — the migration below is
  "there is nothing to migrate" — not the entitlement to skip it.
- `schemas/plan-v1.schema.json` is deleted rather than kept beside its successor, for the reason
  `lesson-v2` was: `schema_file_name` keys a published schema on its major, and a v1 file no build
  reads would tell an author their editor still checks something.
  `t3_e1_generated_schemas_match_checked_in_files` compares `PUBLISHED_SCHEMAS` against
  `schemas/` in both directions, so the orphaned file could not have been left behind quietly.
- `t3_e1_published_schema_required_fields_match_the_recorded_surface` is what made the added
  required field explicit at the point it was made; its `PUBLISHED_REQUIRED_SURFACE` rows move
  from `plan 1.0` to `plan 2.0` in this change, and a row left at the old key fails the suite
  with the retired version named.
- The plan hash pinned in `t1_e0_plan_is_stable_for_identical_inputs` was **recomputed, not
  relaxed**, as every prior move was.

### `RenderPlan::for_lesson` and `LessonError` — provisional Rust API

- `PlannedSegment.style` moving from `String` to `DeliveryStyle` is breaking for any caller
  constructing one; `crates/study-tts-runtime/src/cache.rs` and the testkit fixtures were moved
  with it. `SynthesisRequest::style` stays a `String` on purpose: the worker protocol is a
  separately versioned wire contract, and narrowing it is E1-S3's move, not this one.
- `LessonError` loses two variants and gains fourteen in the rounds below, moving 29 -> 41, and
  then one variant per round for the last two: `DuplicateSpeaker` (§Repeated speaker bindings) and
  `MalformedSourceContentHash` (§Source content hash), ending at 43. `MissingRole` and
  `MissingStyle` are removed as spellings; their invariants return as `MissingSegmentRole` and
  `MissingDeliveryStyle`. The fourteen are `RecallPromptWithoutResponseInterval`,
  `RecallPromptResponseIntervalTooLong`, `TooManyLearningObjectives`, `EmptyLearningObjective`,
  `LearningObjectiveTooLong`, `TooManyLessonReferences`, `EmptyLessonReference`,
  `LessonReferenceTooLong`, `UnknownSegmentRole`, `UnknownDeliveryStyle`, `UnknownReviewStatus`,
  `MissingSegmentRole`, `MissingDeliveryStyle`, and `MissingReviewStatus`. Adding a variant to a
  public enum is breaking for an exhaustive `match` on it; no consumer outside this workspace has
  one.

### `LessonError` distinctness — six new variants

`crates/AGENTS.md` requires one distinct variant per violated invariant so a test can assert the
exact failure. Task 2 made three fields closed vocabularies, and `serde` refuses all of them the
same way, so the invariants ADR-0001 §8.2 declares arrived as one catch-all. Six variants close
that; the mechanism is one function, not six.

- **The variants.** `UnknownSegmentRole(String)`, `UnknownDeliveryStyle(String)`, and
  `UnknownReviewStatus(String)` for a value outside a vocabulary, each carrying what was written;
  `MissingSegmentRole`, `MissingDeliveryStyle`, and `MissingReviewStatus` for a field the document
  omits. Absent and unrecognized are separate for the reason this enum separates them everywhere
  else: an absent field is one to add, an unrecognized value is one to correct.
  `LessonError::MissingRole` and `MissingStyle`, which an earlier round of this record removed, are
  restored under names that say which vocabulary they belong to.
- **The mechanism.** `vocabulary_refusal` reads the authored value back at the pointer the
  deserializer supplied and classifies it: a string is a vocabulary refusal, an absent key is a
  missing-field refusal, and anything else stays `InvalidJson`. It reads the *document*, never
  `serde`'s message — the one place this boundary reads an upstream crate's prose is
  `omitted_field`, whose own doc says why and which test pins it. So the closed enums stay closed,
  the published schema still names every value, and no second copy of this module's shape exists to
  drift.
- **What stays `InvalidJson`, and why that is not the same gap.** A value of the wrong JSON type is
  the document not having the shape the published schema declares. That is one invariant however
  many fields can violate it, and it is located by its pointer. Absence and unrecognized-value are
  per-field invariants with per-field remedies, which is why they are not.
- **The test.** `t1_e1_each_lesson_invariant_has_a_distinct_error` exercises all three fields in
  all three forms — nine cases, written out rather than generated so a reader can see that all nine
  refusals differ. Sampling one form per field is exactly what let the previous round pass while a
  missing role and an unknown role shared a refusal. Distinctness and coverage are two assertions
  because they are two claims: the pairwise check fails if two invariants agree on variant *and*
  pointer, and a separate count of distinct variants fails until every `LessonError` variant has a
  case. Both remain reachable only through `field_of`, whose match has no `_` arm.
- `t1_e1_a_shape_error_is_located_at_the_field_it_is_about` keeps its omitted-field case but moves
  it from `style` to `spoken_text`: it exists to pin that the *deserializer* locates an omitted key,
  which now needs a field whose absence is still a shape refusal.

### Recall response interval — the ceiling ADR-0001 §13.2 sets

- `MAX_RECALL_RESPONSE_MS` is 4,000, the ceiling of §13.2's 1.5-4 second recall-question range,
  beside the `MIN_RECALL_RESPONSE_MS` floor already enforced. Before this, a recall prompt declaring
  4,001-10,000 ms was accepted by the generic `MAX_PAUSE_AFTER_MS` segment limit.
- `LessonError::RecallPromptResponseIntervalTooLong { segment_id, pause_after_ms, max_ms }` is its
  own variant rather than a second reading of `RecallPromptWithoutResponseInterval`, because the
  remedies are opposite: one is answered by lengthening the pause and the other by shortening it.
- An earlier round of this record argued the ceiling was deliberately unenforced, because §8.2
  admits a pause outside policy "unless an override is annotated". That was backwards. The lesson
  format declares no override annotation, so no author can be outside policy on purpose, and every
  value above the ceiling is outside it by accident. When an annotation lands, this becomes the
  bound it lifts rather than a bound to delete — the constant's doc says so.
- `t1_e1_a_recall_prompt_must_leave_a_response_interval` now asserts both edges from both sides,
  and checks the same over-long pause under a non-prompt role, so the refusal is provably the
  prompt's own rule rather than a limit every segment already had.

### Repeated speaker bindings — a name declared twice

`AuthoredLesson::speakers` is a `BTreeMap`, chosen so the document's serialized form cannot depend
on authoring order. A map also resolves a repeated key by keeping the last value, and `serde` has
no rule of its own for a repeated key inside one: a document declaring `nadia` under two voice
profiles parsed, validated, and synthesized under whichever binding the author wrote last, with
nothing downstream able to see that the other existed. Every other object in this format is a
struct, and `serde`'s derived parsers already refuse a field written twice, so `speakers` was the
only place the gap could be.

- **The refusal.** `LessonError::DuplicateSpeaker(String)`, carrying the repeated name and located
  at `/speakers/<name>`. Its own variant rather than a shape refusal because the bytes *are* the
  shape the published schema declares: what is wrong is the document saying two things about one
  speaker. The remedy is a binding to delete, which no other speaker refusal asks for.
- **The mechanism.** `repeated_speaker` reads the document's bytes a second time on the way through
  `ValidatedLesson::from_json`, collecting the names in document order rather than into a map, and
  reports the first one that repeats. The parsed lesson cannot answer the question — its map has
  already discarded one of the two bindings — so the bytes are the only place the answer survives.
  It runs after the shape is known good, so a document that is not a lesson is still refused as
  one.
- **Why this is not value-dependent.** Two bindings naming the *same* profile are refused on the
  same terms. The document is ambiguous whatever the two say, and a rule that read the values would
  stop being a rule about the document and start being one about how much the ambiguity happened to
  cost.
- **Why `3.0` rather than a fourth major.** RFC 8259 leaves a repeated object name undefined, so no
  document with a defined meaning under `1.x`, `2.x`, or `3.x` is refused by this. It is recorded
  inside `3.0` on the same terms as the recall-range narrowing in §Identification item 7: this
  record was `Proposed` and unsigned when the decision was taken, so `3.0` was not yet a version
  anyone had been told to rely on. Acceptance ratifies that reading rather than reopening it.
- **The test.** `t1_e1_a_speaker_declared_twice_is_refused` (T1) builds the document as text,
  because `serde_json::Value` holds a map and cannot carry a repeated key at all — which is the
  same reason the check reads bytes. It asserts the variant, the name it carries, and the pointer,
  over both the differing-profile and identical-profile cases.
  `t1_e1_each_lesson_invariant_has_a_distinct_error` gains the case as its one entry that cannot be
  written by mutating the fixture, and its variant count moves from 41 to 42.

### Source content hash — a digest that is not one

`SourceContentHash` already refused a value that is not a BLAKE3 digest; what was missing is a
refusal a caller can tell apart from any other thing that can be wrong at that field. `serde`
converts a `try_from` failure into its own error, so the typed refusal survived only as prose, and
`MalformedSourceContentHash` — a public type whose message routes the remedy to recompiling the
lesson from its source document — named an invariant nothing could match on.

- **The refusal.** `LessonError::MalformedSourceContentHash(MalformedSourceContentHash)`, carrying
  the typed error and located at `/source/content_hash`. Its own variant for the reason the three
  vocabularies have theirs: the remedy is specific and different. A hash that is not a digest is
  recompiled; a `content_hash` that is not a string is a document that does not have the declared
  shape.
- **The mechanism.** `source_hash_refusal` reads the authored value back at the pointer the
  deserializer supplied and reparses it, exactly as `vocabulary_refusal` does and for the same
  reason — the classification reads the *document*, never `serde`'s message. A value that parses
  leaves the refusal alone, because it was then about something else.
- **What stays `InvalidJson`.** A wrong type, on the same terms as at the three vocabularies. So
  does an absent `content_hash`: that is `serde`'s missing field like every other required one, and
  `omitted_field` has already pointed it at the key the author must add. No document's acceptance
  changes — every document refused before this correction is refused after it, by a refusal that
  now names which invariant it broke.
- **The test.** `t1_e1_each_lesson_invariant_has_a_distinct_error` gains both forms as separate
  cases — a malformed string and a wrong type at one pointer — which is what makes the classifier
  provable rather than merely present: with the variant added and the classifier removed, the
  malformed-string case is refused as `InvalidJson` and the table's distinctness assertion fails.
  Its variant count moves from 42 to 43.

### Worker-start ordering — observed, not only argued

`t1_e1_unreviewed_lesson_fails_before_worker_start` is a `DELIVERY-PLAN.md` E1-S2 test name and
stays as written: `docs/testing/TEST-STRATEGY.md` makes a `t1_` a pure deterministic function, so
it can only prove the ordering by construction — a draft lesson yields no `ValidatedLesson`,
`plan_requests` accepts nothing else, and no `SynthesisRequest` exists to send. What it cannot do
is observe an executor, and `build_preview` receives one already constructed.

- **What is now observed.** `FakeTtsExecutor` counts every `TtsExecutor` call it receives, not only
  synthesis, and `t4_e0_unapproved_content_fails_before_tools_and_synthesis` asserts that count is
  zero. The build's first touch of the backend is `descriptor()`, and it sits after the lesson gate
  and the voice gate — so a worker that starts on first use has not started. Moving that
  `descriptor()` call above the lesson gate fails that test and no other, which is what makes the
  ordering provable rather than merely present.
- **What remains argued, and whose it is.** An executor that started a process in its own
  *constructor* would satisfy both tests, because construction happens before `build_preview` is
  called and `TtsExecutor` has no lifecycle method to observe. No such executor exists: the trait
  is `descriptor`, `capacity`, `validate`, and `synthesize`, and the product worker refuses
  `initialize` until E1-S3. The claim becomes testable when E1-S3 lands a process-spawning pool,
  and belongs to that story's tests rather than to a seam invented here for a worker that does not
  yet start. This record does not add a factory or lazy-start seam: ADR-0001 §12 already ratifies
  an `#[async_trait] TtsExecutor` pool for E1-S3, and an abstraction added now would be one
  implementation ahead of its purpose.

### New provisional resource ceilings

Four, mirrored into `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings: 64
learning objectives, 4 KiB per objective, 256 source references, 4 KiB per reference. Both lists
also count into the aggregate authored-text total the table already bounds. The role and the style
stop counting into it, because a closed vocabulary has a fixed spelling and cannot grow a total.

## Impact

- Synthesis identities affected: **none.** See §Synthesis identity above.
- Verification identities affected: none.
- Plan identities affected: **all of them**, because every planned segment now carries its display
  text. Takes are unaffected: a takes file names synthesis base keys, and those did not move.
- Package identities affected: the plan hash reaches `manifest.json` and `current.json`, so the next
  build of any lesson writes a new immutable preview generation. The audio inside it is served from
  the existing cache entries.
- Consumers and commands affected: `build_preview`, `build_preview_with_services`, and
  `synthesis_requests` in `crates/study-tts-runtime/src/pipeline.rs`. No product CLI command exists
  yet; E1-S5 is where an author meets the closed vocabularies through a scaffold.
- Fakes and shared suites affected: `crates/study-tts-testkit/tests/audio_fixtures.rs` and
  `provisional_contracts.rs` construct `PlannedSegment` and moved with it;
  `crates/study-tts-testkit/tests/schemas.rs` carries the `PUBLISHED_REQUIRED_SURFACE` rows, which
  move from `plan 1.0` to `plan 2.0`.
- Fixtures and schemas affected: `schemas/lesson-v2.schema.json` and `schemas/plan-v1.schema.json`
  deleted, `schemas/lesson-v3.schema.json` and `schemas/plan-v2.schema.json` added.
  `fixtures/lessons/e0-s0-two-segment.json` declares `3.1` and now carries `learning_objectives` and
  `source`, so the `3.1` records are exercised by every property that reads it;
  `e0-s0-cache-identity.json` declares `3.1`; `e1-s2-prior-minor.json` moves to `3.0` and
  `e1-s2-unknown-major.json` to `4.0`; the three `fixtures/contracts/e1-s1-lesson-*.json` move to
  `3.1` and keep the single defect each isolates. Every affected row in
  `docs/testing/TEST-DATA-MANIFEST.md` carries a new SHA-256.
- Existing cached artifacts affected: none. Every key is reachable.
- Published packages or accepted takes affected: no production package exists. Previews under
  `previews/` remain readable and are not rewritten.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes.
- Migration procedure: a `2.x` lesson is migrated by hand — set every `role` to one of the
  seventeen `SegmentRole` spellings and every `style` to one of the four `DeliveryStyle` spellings,
  give each `recall_prompt` segment a pause inside 1,500-4,000 ms, set `schema_version` to `3.1`, and
  point `$schema` at `lesson-v3.schema.json`. `learning_objectives` and `source` may be added or
  left out. `ValidatedLesson::from_json` names the version as the reason it refused, and
  `LessonDiagnostic` names the field for everything after that.
  `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` owes no migration promise before G1, so no
  automated upgrade is supplied; the tree's own lesson documents were migrated in this change.
- Migration procedure, render plan `1.0` → `2.0`: **there is no document to migrate.** ADR-0001
  §12.2 persists plans at E2, no `plan.json` has ever been written by this project, and no reader
  exists — `RenderPlan` is serialized and never deserialized. A `1.0` plan authored by hand outside
  this project is migrated by adding `display_text` to every segment and replacing any `style`
  outside the four `DeliveryStyle` spellings. The migration is empty because the disk is empty, not
  because the change is compatible; §Render plan states why the major moved regardless.
- Rollback procedure: revert this change as a unit. Nothing durable was rewritten in place, and no
  cache key moved, so a revert restores the previous plan hashes and orphans nothing. The
  irreversible steps are the deleted `schemas/lesson-v2.schema.json` and
  `schemas/plan-v1.schema.json`, which a revert restores from history. No consumer needs to be
  moved off `plan 2.0`, for the reason its migration is empty.
- Compatibility evidence: `t3_e1_unknown_major_version_is_rejected`,
  `t3_e1_compatible_minor_extension_is_accepted` (a `3.0` document omitting both added records still
  parses, keeps the version it declared, and reads their declared defaults),
  `t3_e1_published_schema_required_fields_match_the_recorded_surface`,
  `t3_e1_generated_schemas_match_checked_in_files`, and
  `t1_e1_the_canonical_adr_lesson_document_is_accepted`, which parses ADR-0001 §8.1's own document
  at the version this build publishes.
- Mapped tests and qualification rerun: the whole workspace suite, 308 tests at the sixth review —
  306 at the fifth, the two added since belonging to the twenty-second audit in
  [`E1-S1-INTERFACE-CHANGE-001.md`](E1-S1-INTERFACE-CHANGE-001.md) — plus 7 doctests. The sixth
  review adds no test name either: item 13 adds two cases to
  `t1_e1_each_lesson_invariant_has_a_distinct_error`, one per form the field can fail in, and item
  14 adds one assertion to `t4_e0_unapproved_content_fails_before_tools_and_synthesis` and a
  `touch_count` to `FakeTtsExecutor` for it to read. Both guards were confirmed live: removing
  `source_hash_refusal` fails the first, and moving `descriptor()` above the lesson gate fails the
  second and nothing else. New
  in this change: `t1_e1_the_canonical_adr_lesson_document_is_accepted`,
  `t1_e1_delivery_style_spelling_matches_its_serde_form`,
  `t1_e1_a_role_or_style_outside_its_vocabulary_is_refused`,
  `t1_e1_a_recall_prompt_must_leave_a_response_interval`,
  `t1_e1_display_text_reaches_the_plan_without_reaching_a_cache_key`, and
  `t3_e1_every_documented_error_variant_is_named_by_its_errors_section`, and
  `t1_e1_a_speaker_declared_twice_is_refused`, which §Identification item 12 added. The seven
  corrections in §Identification items 5 through 11 add no test name. The plan's move is caught by the two schema
  tests already listed. The distinctness and recall corrections strengthen
  `t1_e1_each_lesson_invariant_has_a_distinct_error` and
  `t1_e1_a_recall_prompt_must_leave_a_response_interval` in place — the first is a name
  `DELIVERY-PLAN.md` §E1-S2 pins character for character and this record's §Result cites, so
  renaming it was never available. Correction 11 widens the drift test from one entry point to a
  table of them, adding `AuthoredLesson::validate` beside `build_preview`, and renames it from
  `…_is_named_by_the_pipeline_contract`, which described only the row it used to have; no evidence
  record or `DELIVERY-PLAN.md` row cites the old name. Widening it exposed a defect in the test
  itself: it read the run of `///` lines above the *byte offset* of a signature, which is a doc
  comment only for an item at column zero, so an indented method read as having no documentation
  at all. Three guards were confirmed live rather than assumed: pointing two vocabulary cases at
  one field fails with "two cases share one located refusal"; the drift test named all four new
  variants as undocumented until `build_preview`'s `# Errors` section carried them, which is the
  test doing the job item 4 added it for; and the widened test named exactly nine —
  `RecallPromptResponseIntervalTooLong` plus the eight the parser raises ahead of `validate` —
  until that function's section accounted for each.
- Walking skeleton result: green, 35 tests against real FFmpeg and ffprobe.
- Evidence provenance: every file this change touches that
  `e1-s1-provisional-contract-baseline-v13` pins is accounted by
  `evidence/gates/g1/e1-s2/e1-s2-evidence-provenance-reconciliation-v3.md`, which is `Accepted` and
  supersedes v2, which superseded v1. A successor each time rather than an edit because an accepted
  record may not be amended in place under `evidence/README.md` §Provenance, and because each
  reading had stopped short of the tree: v1 read the lesson document moving `1.1` → `2.1`, v2 read
  E1-S2 through the third review and §Identification items 5 through 8, and the fourth and fifth
  reviews landed after it was accepted. A reconciliation that reads fewer changes than the bytes
  carry grants more than it examined. v3 restates the same eleven pairs against the bytes those
  files hold now and extends the reading to every round of this record.
  `python3 scripts/check-evidence-provenance.py` exits zero for this change. The twenty-second
  E1-S1 audit moved three further v13 pins —
  `crates/study-tts-runtime/src/worker_protocol.rs`, `crates/study-tts-runtime/src/lib.rs`, and
  `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` — which
  `evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v14.md` records and clears by
  superseding v13. That record was accepted on 2026-08-30, so v13 is no longer checked and those
  three no longer stand.

## Limits this change does not close

- **The role vocabulary is not bound to a speaker.** ADR-0001 §3.2 assigns each repertoire to Nadia
  or Tom, and nothing here refuses a learner speaking a `definition`. That rule needs speaker
  archetypes the lesson document does not yet declare.
- **A recall prompt cannot leave §13.2's range on purpose.** Both ends are enforced, and the
  lesson format still declares no override annotation for §8.2 to admit, so an author who has a
  reason to exceed 4,000 ms has no way to say so. §Recall response interval records that this
  becomes a bound the annotation lifts rather than a bound to delete.
- **No style is calibrated.** ADR-0001 §13.4 forbids a style entering production without a frozen
  loudness reference, and ADR-0003 is Proposed. Closing the vocabulary is the precondition for that
  gate, not the gate.
- **`display_text` reaches the plan and stops there.** `manifest.json` records no transcript, so the
  package still holds none; what changed is that the package writer can now reach one. The
  transcript and caption timing ADR-0001 §13.2 lists belong to the story that writes them.
- **`SynthesisRequest::voice` still carries the speaker name**, unchanged and still owed by E1-S3.
  [`E1-S2-INTERFACE-CHANGE-001.md`](E1-S2-INTERFACE-CHANGE-001.md) §Limits says correcting it
  "would add a required field to `plan-v1`"; that record is filed and stands as written, and
  the field E1-S3 would add is now a required field of `plan 2.0`.
- **Generation parameters remain empty**, unchanged and still owed by E1-S3.

## Approval

**Every row below is signed, on 2026-08-30.** Each records a decision a role was asked for and has
now made. §Identification items 5 through 14 record six rounds of correction in this record rather
than in a successor because nothing here was accepted while they were made; a further correction
amends this record from outside, in a successor, now that it is in force.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate for one signatory. A row is signed by recording the deciding role's
name and the date beside it, which every row now carries.

This acceptance covers this record as corrected through §Identification item 14. It does **not**
accept `evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md`, which stays `Proposed`
until G1 for the reason that record's own §Open findings gives.

| Role | Decision sought | Status |
|---|---|---|
| Contract owner (T-CORE) | Accept the lesson document moving `2.1` → `3.0` → `3.1`, that every `2.x` lesson naming a role or style outside the two vocabularies is now refused with no automated migration, and that `3.1` was landed in the same change so an older minor of the current major exists | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept `SegmentRole` and `DeliveryStyle` as the closed vocabularies transcribed from ADR-0001 §3.2, §3.4, §5.1, and §8.1, and both `MIN_RECALL_RESPONSE_MS` and `MAX_RECALL_RESPONSE_MS` as the two ends of §13.2's recall range, enforced because the format declares no override annotation for §8.2 to admit | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept `PlannedSegment` carrying `display_text` inside the plan hash and outside every cache key, and the render plan moving `1.0` → `2.0` with `schemas/plan-v1.schema.json` deleted, on the reading that `ADR-0001-D005` condition 2 fails because `plan 1.0` came from E1-S1 rather than this story | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept six new variants giving each closed vocabulary its own absent and unrecognized refusal, `vocabulary_refusal` classifying them by reading the document rather than `serde`'s message, a wrong JSON type remaining `InvalidJson` as one shape invariant located by its pointer, and `t1_e1_each_lesson_invariant_has_a_distinct_error` exercising all three fields in all three forms | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept `LessonError::MissingRole` and `MissingStyle` being removed as spellings while their invariants return as `MissingSegmentRole` and `MissingDeliveryStyle`, and accept that `LessonError` growing to 43 variants is breaking for an exhaustive match no consumer outside this workspace has | Accepted — Ross Todd, 2026-08-30 |
| Contract owner (T-CORE) | Accept `LessonError::DuplicateSpeaker` refusing a `speakers` object that binds one name twice, whatever the two bindings say, and accept that this is recorded inside `3.0` rather than as a fourth major because RFC 8259 leaves a repeated object name undefined and no document with a defined meaning is refused by it | Accepted — Ross Todd, 2026-08-30 |
| Engineering owner | Accept the change on the 308-test workspace suite, the 35-test walking skeleton against real FFmpeg and ffprobe, 7 doctests, clean fmt, conventions, evidence provenance, Clippy `-D warnings`, rustdoc `-D warnings`, and no schema drift; and accept that the plan hash pinned in `t1_e0_plan_is_stable_for_identical_inputs` was recomputed rather than relaxed while both cache keys stood | Accepted — Ross Todd, 2026-08-30 |
| Affected-track reviewer (T-RUNTIME) | Accept `t3_e1_every_documented_error_variant_is_named_by_its_errors_section` as a source-reading test, and that `build_preview`'s and `AuthoredLesson::validate`'s `# Errors` sections are now held to their error enums in both directions | Accepted — Ross Todd, 2026-08-30 |
| Affected-track reviewer (T-CLI) | Accept that an author now meets two closed vocabularies, so E1-S5's scaffold must offer them rather than a free-text field | Accepted — Ross Todd, 2026-08-30 |
| Affected-track reviewer (T-AUDIO) | Accept that no audio bytes changed and no cache key moved, so no listening evidence is required by this change | Accepted — Ross Todd, 2026-08-30 |

- Effective version and date: **2026-08-30.** Provisional `lesson 3.1`, `plan 2.0`, `e1-s2-v1`
  unchanged, `e1.tts-executor.2.0` unchanged
