# E1-S2 Interface Change 001 — Declared voices, located diagnostics

## Identification

- Record ID: `E1-S2-INTERFACE-CHANGE-001`
- Contract owner: T-CORE (lesson document, synthesis identity); T-WORKER (executor contract)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-WORKER, T-CLI
- Accepted ADR, if architectural: not applicable. This implements ADR-0001 §8.1 (`speakers`),
  §8.2 ("speaker, role, style, and voice profile are declared"; "references source material or is
  explicitly marked editorial"), §8.3, §12.5 (voice-conditioning artifact hash), and §14
  (a diagnostic names where a failure happened) as written. No authority boundary moves.

This record closes the first of the two gaps
[`E1-S1-INTERFACE-CHANGE-001.md`](E1-S1-INTERFACE-CHANGE-001.md) §Impact of the two deliberately
incomplete inputs left owing to a later story: "**Voice-conditioning artifact hash.** Planning
currently supplies an empty map … E1-S2 resolves voice references and will populate it, changing
every cache key again. … Each still needs its own record when it lands." Generation parameters
remain E1-S3's.

## Version and compatibility

Three contracts move. Each is listed separately because their classes differ.

### Lesson document — `1.1` → `2.0` → `2.1`

- Contract ID: `lesson`, published at `schemas/lesson-v2.schema.json`
- Old version: `1.1`
- New version: `2.1`
- Compatibility class: **breaking** (`2.0`), then **compatible extension** (`2.1`)
- Required/defaulted fields: `2.0` adds required `speakers`, an object mapping each speaker name
  to `{ "voice_profile": <portable id> }`. `2.1` adds optional `segments[].editorial`, whose
  declared default is `false`.
- Unknown-field behavior: unchanged. `#[serde(deny_unknown_fields)]` on `AuthoredLesson`,
  `LessonSegment`, and the new `SpeakerDeclaration`; `additionalProperties: false` in the
  published schema, except `speakers` itself, whose additional properties are the declarations.
- Wire or Rust representation changed: `AuthoredLesson` gains
  `speakers: BTreeMap<String, SpeakerDeclaration>`; `LessonSegment` gains `editorial: bool`;
  `ValidatedLesson` gains `speakers()`. `AuthoredLesson::new` takes the map.

`2.0` is breaking because a `1.x` document declares no voice at all: there is no default a build
could supply without inventing which voice speaks. `SchemaVersion::accepted_by` therefore refuses
every `1.x` lesson, and `schemas/lesson-v1.schema.json` is deleted rather than kept beside its
successor — `schema_file_name` keys a published schema on its major, and a v1 file no build reads
would be a document telling an author their editor still checks something.

`2.1` follows in the same change because a major with no older minor leaves
`t3_e1_compatible_minor_extension_is_accepted` — an E1-S1 acceptance test named in
`DELIVERY-PLAN.md` — with no lesson document to exercise. `editorial` is not invented for it:
ADR-0001 §8.2 requires every segment to reference source material **or** be explicitly marked
editorial, and until now the "or" had no spelling, so `source_refs` was simply required. It is
display-only and reaches no identity. E1-S1 landed `1.0` and `1.1` in one story for the same
reason.

### Synthesis identity — `e1-s1-v1` → `e1-s2-v1`

- Contract ID: `SYNTHESIS_IDENTITY_VERSION` in `crates/study-tts-core/src/identity.rs`
- Compatibility class: **breaking**; every cache key changes.
- `voice_conditioning_hash` stops serializing as absent for every speaker. ADR-0001 §12.5 names it
  a speech-affecting input, and `CanonicalValue::optional` writes absent and present differently,
  so a resolved profile could not have matched an unresolved one even without the version move.
  The constant moves anyway, because the *definition* of the key changed rather than one value in
  it.
- `speaker` remains a key input. E1-S1 justified it by conditioning being unresolved; the reason
  is now that two speakers may lawfully share one voice profile, and keeping the name only ever
  splits a key — the safe direction.

### `TtsExecutor` — `e1.tts-executor.1.0` → `e1.tts-executor.2.0`

- Contract ID: `TTS_EXECUTOR_CONTRACT_VERSION`
- Compatibility class: **breaking**; required field.
- `SynthesisRequest` gains required `voice_conditioning_hash: VoiceConditioningHash`.
- Not defaulted, and not optional. `crates/study-tts-runtime/src/cache.rs` recomputes the
  synthesis key from `SynthesisReport::context` and refuses publication when it is not the key the
  plan derived (`AudioError::SynthesizerIdentityMismatch`). Once the planner resolves a voice, an
  executor can only make that comparison meaningful if it was told which conditioning artifact the
  key names. Without this field the deterministic fake could not publish at all, and a real worker
  could not be checked against what the plan asked for.

### New dependency — `serde_path_to_error` 0.1.20

- Added to `study-tts-core`; one crate, no transitive dependency beyond `serde`, MIT OR
  Apache-2.0, by serde's own author. `cargo deny check` passes.
- `DELIVERY-PLAN.md` E1-S2 requires a lesson refusal to name its source, segment, and field path.
  Every invariant the lesson module checks itself is located by `field_of`, but a refusal `serde`
  raises first — an unknown field, a wrong type, a value outside a closed vocabulary such as
  `review_status` — carries only a line and column, because `serde_json` exposes no path.
- The alternative was a hand-written locator walking a lenient `Value` against the document shape.
  That shape *is* `study-tts-core`, so the locator would be a second copy of it with no compiler
  behind it, drifting the moment a field is added. The risk this dependency removes is a wrong
  location, not a missing convenience. The argument is written into
  `crates/study-tts-core/Cargo.toml` beside the declaration, per `crates/AGENTS.md`.
- Pinned by `t1_e1_a_shape_error_is_located_at_the_field_it_is_about` over five malformed
  documents, and by `t1_e1_bytes_that_are_not_json_name_the_document_and_nothing_else` for the one
  refusal with no field to name.

### `RenderPlan::for_lesson` — provisional Rust API

- Returns `Result<Self, PlanError>` instead of `Self`; `study_tts_runtime::BuildError` gains a
  `Plan` category.
- Compatibility class: **breaking**, on a pre-G1 Rust API with no wire form.
- Once the conditioning artifact is a real key input, an unresolved speaker had two possible
  outcomes and both were wrong: `CanonicalValue::optional` would serialize the absence as `null`
  and yield a well-formed key naming audio no voice produced, or the request mapping would panic.
  Planning now refuses with `PlanError::UnresolvedSpeaker`, so no such key exists to publish
  under. `SynthesisContext::key_for` stays total on purpose — it recomputes an *executor's*
  reported identity, where a dropped artifact must produce a key that differs rather than a panic.

### `BuildRequest` — provisional Rust API

- `voice_profile_dir: Option<PathBuf>` becomes `voice_profile_root: PathBuf`, required.
- A build resolves `speakers[*].voice_profile` to `<root>/<profile_id>/`. Absent would leave the
  conditioning map empty, which is precisely the false cache hit `identity.rs` warns about, so the
  field fails closed rather than defaulting.

## Impact

- Synthesis identities affected: **all of them.** Every cache key in the project changes, twice
  over: the identity version moved and the conditioning input became present. No published cache
  entry can be reused, and none can be mistaken for current — the key is different, so an old
  entry is simply not found.
- Verification identities affected: none directly. `VERIFICATION_IDENTITY_VERSION` is unchanged
  and `t2_e1_a_verification_input_never_changes_the_synthesis_key` still holds. Verification
  evidence recorded against an old audio digest is stale for the ordinary reason: the audio it
  described will be regenerated under a new key.
- Plan, takes, or package identities affected: plan hashes change because their segments' cache
  keys do. The plan *document* stays at `1.0` — no field moved. Takes stay at `1.0`; a takes file
  naming an old synthesis base key is refused by the check `takes.rs` already performs.
- Consumers and commands affected: `build_preview`, `build_preview_with_services`, and
  `validate_production_manifest` in `crates/study-tts-runtime/src/pipeline.rs`. `BuildError` gains
  a `Plan` category, named rather than folded into `Lesson` because a lesson that fails to plan is
  valid — what failed is the caller's resolution of the profiles it declares, and sending its
  author back to the document would be the wrong remedy. No product CLI
  command exists yet; E1-S5 is where an author meets `speakers` through a scaffold.
- Fakes and shared suites affected: `study_tts_testkit::FakeTtsExecutor` echoes the requested
  conditioning artifact into its report; `write_voice_profile_root` and `FIXTURE_VOICE_PROFILES`
  are new; every shared seam scenario supplies a conditioning map.
- Fixtures and schemas affected: `schemas/lesson-v1.schema.json` deleted,
  `schemas/lesson-v2.schema.json` added. `fixtures/lessons/e0-s0-two-segment.json` and
  `e0-s0-cache-identity.json` gain `speakers`; `e1-s1-prior-minor.json` and
  `e1-s1-unknown-major.json` are replaced by `e1-s2-prior-minor.json` (`2.0`) and
  `e1-s2-unknown-major.json` (`3.0`); the three `fixtures/contracts/e1-s1-lesson-*.json` gain
  `speakers` and keep the single defect each isolates. Every affected row in
  `docs/testing/TEST-DATA-MANIFEST.md` carries a new SHA-256.
- Existing cached artifacts affected: every one. They remain on disk and are neither deleted nor
  quarantined; they are unreachable under the new keys, and `docs/governance/ROUTING-TABLES.md`
  routes their removal to a prune workflow E4 owns.
- Published packages or accepted takes affected: no production package exists. Private-preview
  packages under `previews/` remain readable and are not rewritten; the next build of the same
  lesson produces a new immutable generation because its plan hash moved.

`fixtures/lessons/e0-s0-cache-identity.json` maps both `nadia` and `tom` to one profile
deliberately. Its design is one variable per segment, and `seg-f` differs from `seg-a` only by
speaker; giving the two speakers different profiles would have made that segment differ by two
things and quietly weakened `t4_e0_cache_identity_proves_hits_and_speech_affecting_misses`.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes. `FakeTtsExecutor`, the seam
  scenarios in `crates/study-tts-testkit/src/contracts.rs`, and the committed fixtures were
  brought to the new contract before `pipeline.rs` was reordered.
- Migration procedure: a `1.x` lesson is migrated by hand — add `speakers`, bind each speaker
  named by a segment to an installed voice profile, set `schema_version` to `2.1`, and point
  `$schema` at `lesson-v2.schema.json`. `ValidatedLesson::from_json` names the version as the
  reason it refused, and `LessonDiagnostic` names the field for everything after that.
  `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` owes no migration promise before G1, so no
  automated upgrade is supplied; the tree's own lesson documents were migrated in this change.
- Rollback procedure: revert this change as a unit. Nothing durable was rewritten in place — cache
  entries, previews, and `current.json` records from before it remain valid under the old keys, so
  a revert restores reuse rather than orphaning it. The one irreversible step is the deleted
  `schemas/lesson-v1.schema.json`, which a revert restores from history.
- Compatibility evidence: `t3_e1_unknown_major_version_is_rejected` (a `3.0` document is refused,
  and the same document with only its version corrected is accepted),
  `t3_e1_compatible_minor_extension_is_accepted` (a `2.0` document omitting `editorial` still
  parses, keeps the version it declared, and reads the declared default),
  `t3_e1_published_schema_required_fields_match_the_recorded_surface` (the required-field surface
  moved deliberately), and `t3_e1_generated_schemas_match_checked_in_files`.
- Mapped tests and qualification rerun: the whole workspace suite. The five `DELIVERY-PLAN.md`
  E1-S2 tests are new; `t1_e0_plan_is_stable_for_identical_inputs` had its three pinned digests
  **recomputed, not relaxed**, as E1-S1 did before it.
- Walking skeleton result: green. `cargo test --offline -p study-tts-testkit --test
  walking_skeleton --locked` passes all 35 tests against real FFmpeg and ffprobe.

## Limits this change does not close

Recorded here rather than left for a reader to discover.

- **`SynthesisRequest::voice` still carries the speaker name**, not the resolved profile identity,
  even though `crates/study-tts-runtime/src/worker_protocol.rs` documents the wire field as "voice
  profile identity, never a raw reference path". Correcting it is a semantic change to the same
  field and would add a required field to `plan-v1`; E1-S3 consumes the value and owns that move.
  The conditioning artifact — the part that reaches the key — is carried correctly today.
- **Generation parameters remain empty**, unchanged from E1-S1 and still owed by E1-S3.
- **The voice-profile root is not contained.** It is operator-supplied input resolved with
  `Path::join` and the same symlink refusal `voice_gate` already applied per record; it is not
  routed through `managed::leaf`, for the reason that module's own comment gives. E5-S4 owns
  directory-relative containment.
- **An unused speaker declaration is not refused.** Only speakers a segment actually uses are
  resolved, so an unused binding costs no rights check and no checksum, and it reaches no
  identity. A rule refusing one would be hygiene rather than a control.
- **The identity gate this field exists to serve is inert until E1-S3.** The gate at
  `crates/study-tts-runtime/src/cache.rs` recomputes the synthesis key from
  `SynthesisReport::context` and refuses publication when it is not the key the plan derived,
  which is what stops a worker publishing audio rendered with a voice nobody asked for. It can
  only do that if the reported conditioning artifact is the one the worker *loaded*. The only
  executor in the tree loads nothing, so `study_tts_testkit::FakeTtsExecutor` echoes the requested
  hash straight back into its report — the most honest thing a fake that resolves no profile can
  report, and also a tautology: the comparison currently passes by construction rather than by
  evidence. **E1-S3 owns closing this**, and closing it means the Chatterbox worker reporting the
  conditioning artifact it read from disk, never the value it was handed. A worker that echoes the
  request would satisfy every test in this suite while leaving the control that motivated this
  contract change doing nothing. The suite cannot catch that substitution today, because it has no
  executor that loads a profile to disagree with.

## Approval

Ross Todd holds each role below under
`docs/governance/PROJECT-EXECUTION-CHARTER.md`; each row records that role's separate decision and
accepted risk.

The last two rows were added after a review of the implementation found that planning still
derived a key for an unresolved speaker, and that a refusal `serde` raised carried no field path
even though `DELIVERY-PLAN.md` E1-S2 requires one. Both are now closed in code rather than
recorded as limits, and both are approved below on the same terms as the rows above.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept the lesson document moving `1.1` → `2.0` → `2.1`, that every `1.x` lesson is now refused as a different major with no automated migration, and that `2.1` was landed in the same change so an older minor of the current major exists for `t3_e1_compatible_minor_extension_is_accepted` to read | 2026-08-29 |
| Contract owner | Ross Todd for T-CORE | Accept `SYNTHESIS_IDENTITY_VERSION` moving to `e1-s2-v1` and the consequence that every cache key in the project changes, leaving every published entry unreachable rather than deleted, with pruning owed to E4 | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the executor contract moving to `e1.tts-executor.2.0` with `SynthesisRequest::voice_conditioning_hash` required, and accept the limit recorded above: the identity gate it serves is a tautology until E1-S3's worker reports the artifact it loaded rather than the one it was handed | 2026-08-29 |
| Engineering owner | Ross Todd | Accept the change on the 292-test workspace suite, the 35-test walking skeleton against real FFmpeg and ffprobe, clean fmt, conventions, Clippy `-D warnings`, doctests, the 44-test Python worker suite, and no schema drift; and accept that the three pinned digests in `t1_e0_plan_is_stable_for_identical_inputs` were recomputed rather than relaxed | 2026-08-29 |
| Project owner | Ross Todd | Accept that a lesson must now declare a voice profile per speaker and that a build with no root to resolve one is refused rather than defaulted, making a declared, consented, checksummed voice a precondition of planning rather than of synthesis | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept `BuildRequest::voice_profile_dir` becoming the required `voice_profile_root`, the voice gate moving ahead of planning, and that the root is operator-supplied input not routed through `managed::leaf` until E5-S4 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-CLI | Accept that `ValidatedLesson::from_json` now names its document and returns a located `LessonDiagnostic`, which E1-S5 surfaces through `study-tts lesson validate` | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required by this change; listening remains a G1 prerequisite once E1-S3 renders under the new identity | 2026-08-29 |
| Contract owner | Ross Todd for T-CORE | Accept `RenderPlan::for_lesson` becoming fallible and `BuildError` gaining a `Plan` category, so an unresolved speaker is refused rather than keyed as absent or panicked on | 2026-08-29 |
| Contract owner | Ross Todd for T-CORE | Accept `serde_path_to_error` 0.1.20 as a `study-tts-core` dependency — one crate, no transitive dependency beyond `serde`, MIT OR Apache-2.0, `cargo deny check` clean — so a refusal `serde` raises is located at its field rather than at the document | 2026-08-29 |

- Effective version and date: provisional `lesson 2.1`, `e1-s2-v1`, `e1.tts-executor.2.0`,
  2026-08-29
