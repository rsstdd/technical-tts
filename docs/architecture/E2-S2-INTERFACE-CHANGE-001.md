# E2-S2 Interface Change 001 — Explicit take selection in the plan and manifest

## Identification

- Record ID: `E2-S2-INTERFACE-CHANGE-001`
- Status: **Accepted 2026-09-04.** Every row in §Approval is signed.
- Contract owner: T-CORE (the `plan` document) and T-RUNTIME (the `manifest` document)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-RUNTIME, T-AUDIO
- Accepted ADR, if architectural: ADR-0001 §11.4, §12.1, §12.2, §12.5, §13.2, §15.4. This
  record implements those sections; it changes no architecture.

`docs/architecture/G1-FREEZE-CHARTER.md` froze `plan` at `3.0` and `manifest` at `1.0-skeleton`,
and the manifest row already recorded that E2-S3 and E2-S4 would break it again. Issue #15 and
the [reviewed plan](https://github.com/rsstdd/technical-tts/issues/15#issuecomment-5530455864)
on it are the working record. That plan's §6 Step 0 is reproduced as §Identity effect below,
because the freeze record's identity row is where the decision belongs.

## Version and compatibility

### `plan` — `3.0` → `4.0`, Breaking contract

| | Before | After |
|---|---|---|
| Document | `schema_version`, `lesson_id`, `plan_hash`, `segments` | adds required `take_selection_source` |
| Segment | `id`, `speaker`, `voice_profile`, `display_text`, `spoken_text`, `style`, `pause_after_ms`, `take`, `cache_key` | adds required `synthesis_base_key` and nullable `audio_blake3` |

### `manifest` — `1.0` → `2.0`, layout `1.0-skeleton` → `2.0-skeleton`, Breaking contract

| | Before | After |
|---|---|---|
| Document | as E1-S4 wrote it | adds required `take_selection_source` and required `join_continuity` |
| Segment | `segment_id`, `cache_key`, `audio_blake3`, `frames`, `pause_after_ms`, `start_frame`, `pause_frames` | adds required `selected_take` and `synthesis_base_key` |

- Compatibility class: **Breaking contract** for both. Required fields enter two published
  schemas, which `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
  answers with a major version, migration, impact report, and owner approval.
- Required/defaulted fields: every field above is required except `plan.segments[].audio_blake3`,
  which is nullable. It is always *written* — `null` where no selection approved audio — so
  ADR-0001 §12.2's "repeat the audio checksum for every segment" holds; it is not schema-required
  because a segment planned at take zero with no recorded selection has no approval to state.
- Unknown-field behavior: unchanged. Both documents keep `deny_unknown_fields`.
- Layouts read after this change: `0.1-skeleton`, `0.2-skeleton`, and `2.0-skeleton`.
  `1.0-skeleton` leaves the read set, because a `1.0-skeleton` segment records neither
  `selected_take` nor `synthesis_base_key` and its own struct would have to be kept to say so.
  The two older layouts keep their decoders for the reason
  [`E1-S4-INTERFACE-CHANGE-001.md`](E1-S4-INTERFACE-CHANGE-001.md) §Impact gives — packages in
  those layouts exist on disk and accepted E1-S1 evidence describes them. That same condition
  holds for `1.0-skeleton` and is answered differently here; §Implementation consequences records
  what that costs, rather than leaving it to be discovered. **This supersedes that record's
  §Three layouts are now read, one is written**, which states the set as it stood on 2026-09-01;
  that record is otherwise unchanged and stays in force.
- Wire or Rust representation changed: `RenderPlan`, `PlannedSegment`,
  `manifest::{Manifest, ManifestSegment, StoredManifest, StoredManifestSegment}`, and
  `manifest::ReuseExpectations`. `TakeSelection` and `TakeSelectionSource` are new in
  `study-tts-core`; `JoinContinuity` and `JoinSide` are new in `study-tts-runtime`.

### G-A — which fields `plan.json` carries, resolved

ADR-0001 §12.2 names three per-segment values for `plan.json` and the published `manifest.json`:
the selected take, the selected cache key, and the audio checksum. §13.2's edit-decision list
additionally names synthesis base keys. The two readings are reconciled rather than chosen
between:

- §12.2 is normative for both documents, so the plan gains the audio checksum it lacked and the
  manifest gains the selected take it lacked. The other two values were already present in each.
- §13.2's base key is carried on **both** documents, for two different readers. On the plan it is
  the edit decision itself: the plan is the edit-decision list §13.2 describes, and I-2 keeps the
  key derived rather than authored. On the published manifest it is what retention reporting
  reads — `prune_candidates` treats a published manifest as a live root, and
  `manifest::referenced_cache_keys` contributes each segment's selected key *and* its base key, so
  a superseded take's artifact stays live for as long as a package referencing it stands. Without
  the field the manifest cannot name that artifact, and prune would offer it as a candidate.

  An earlier draft of this bullet said the base key was carried on the plan only, and that a
  manifest copy would record "the same derived value twice with no second reader". That was
  already inconsistent with §Version and compatibility above, which has recorded the manifest's
  `synthesis_base_key` since this record was signed; the field was never absent, and retention
  reporting is the second reader the sentence said did not exist. Corrected here rather than
  argued away — no field moves, and the G-A resolution about which fields `plan.json` carries is
  unchanged.

One major move for each document rather than two.

## Identity effect

Whether a change invalidates synthesis, verification, plan, takes, or package identity — and,
for this change, the rule that decides it.

**The organizing rule.** A field belongs in `plan_digest` when changing it should make the build
consider this a *different plan*. A field that records something derivable — from inputs already
hashed, or from an artifact the build resolves anyway — is a **reproducibility field**: it
belongs in the document for audit, and it needs a stated verification instead of a hash
contribution. A recorded-but-unhashed field is legitimate; a recorded-but-unhashed **and
unverified** field is not.

| # | Invariant | Verdict | Where it is enforced |
|---|---|---|---|
| **I-1** | Two plans differing only in a segment's `take` (and the `cache_key` derived from it) **must** have different `plan_hash` | In the digest | `plan_digest`'s `..`-less destructure; `t1_e0_plan_is_stable_for_identical_inputs` |
| **I-2** | Two plans differing only in `synthesis_base_key` **must not be constructible** | Out of the digest | Derived at construction in `for_lesson_with_takes`; `RenderPlan::verify_recorded_selection` on the retained-plan load path |
| **I-3** | Two plans differing only in `audio_blake3` for the same `cache_key` **must not be constructible** | Out of the digest | Compared against the resolved cache entry in `pipeline::render_attempt`, refused as `TakesError::ApprovedAudioMismatch` |
| **I-4** | Two plans differing only in `take_selection_source` **must** have the same `plan_hash` **and must** produce a different package generation | Out of the digest; into `ReuseExpectations` | `manifest::validate_package`; `t4_e2_an_accepted_takes_file_makes_the_selection_explicit` |

**Why I-4 is not a hash input.** Ratifying the take zero a build already rendered is a
governance act, not a rendering change: the audio is byte-identical. Hashing it would move
`plan_hash`, refuse the next resume, and rebuild a package for unchanged audio. Leaving it out of
*everything* would be worse — the existing package, whose manifest records `implicit`, would be
reused, and a production claim gated on that manifest would be evaluating a document this build
never wrote. `ReuseExpectations` is where `text_renderer_version` already sits for exactly this
shape: a value that changes package bytes without changing audio.

`TRANSACTION_IDENTITY_VERSION` is deliberately **not** touched. `preview.rs`'s own doc says
transaction identity "only separates concurrent work" and that "Reuse is decided by
`manifest::validate_package`, not here", so adding the field there would add migration surface
without strengthening I-4.

**A-1 — Resume authority invariant.** Once a job has retained a valid `plan.json`, resume
recovers its selection semantics from that plan and performs no discovery that could produce a
semantically different plan from external mutable inputs. This is architecture rather than a
workaround for `ResumeRequest` carrying no `lesson_path`: the retained plan is the already
authoritative statement of what the job renders, and a sibling `<lesson-stem>.takes.json` is an
external file that may have moved since the attempt that established the plan. It is load-bearing
because `JobDocument::open_attempt` compares no plan hashes — a rediscovering resume would record
take zero as authoritative with nothing reporting it. Carried by `TakeSelection::Recovered` and
proved by `t4_e2_a_resumed_retake_keeps_its_selected_take`. A changed selection is a new build
attempt, which ADR-0001 §6.4 already has the edge for.

## Impact

- Synthesis identities affected: **none.** No cache key moves, and
  `t1_e0_plan_is_stable_for_identical_inputs` pins that: its two golden cache keys and its golden
  plan hash are unchanged by this record.
- Verification identities affected: none.
- Plan, takes, or package identities affected: `plan_hash` values are unchanged (I-1 through
  I-4). Every **package generation** is invalidated, because the manifest layout moved.
  `takes` stays at `1.0`; no takes-document field changed.
- Consumers and commands affected: `build_preview`, `resume_preview`, `validate_production_manifest`,
  and the package writer. No CLI command changes — `study-tts retake`, `takes accept`, and
  `cache prune` remain E2-S5's.
- Fakes and shared suites affected: `walking_skeleton`, `schemas`, `provisional_contracts`,
  `voice_rights`, `error_documentation`.
- Fixtures and schemas affected: `schemas/plan-v3.schema.json` and
  `schemas/manifest-v1.schema.json` are **replaced** by `plan-v4` and `manifest-v2`;
  `t3_e1_generated_schemas_match_checked_in_files` compares the published set exactly, so a
  superseded file cannot be left behind. The v3 and v1 documents therefore cease to exist for
  external readers. No lesson or takes fixture changed.
- Existing cached artifacts affected: **none.** Cache reuse is per segment on `cache_key` and
  never consults `plan_hash`.
- Published packages or accepted takes affected: an existing `1.0-skeleton` package is refused by
  `manifest::parse_stored_manifest` as `UnsupportedPackageManifest` and rebuilt. No accepted takes
  document is affected.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes — the published surface table and
  the schema comparison fail until edited, and were edited with the types.
- Migration procedure: none on disk, matching the E2-S1 precedent of refusing rather than
  migrating. A `plan.json` at `3.0` is refused by `SchemaVersion::accepted_by` as an unsupported
  durable record; a `1.0-skeleton` package is refused as an unsupported package manifest. Both are
  rebuilt from the lesson. Nothing is deleted: the refusals preserve what they refuse.
- Rollback procedure: revert this story's commits. Because no cache entry and no synthesis
  identity moved, a reverted build reuses every cache entry and rebuilds only its packages.
- Compatibility evidence: `cargo test --workspace --all-targets --locked` (513 passed, 0 failed),
  `cargo test --workspace --doc`, schema regeneration a no-op, `cargo clippy` clean under
  `-D warnings`.
- Mapped tests and qualification rerun: the seven `DELIVERY-PLAN.md` E2-S2 tests, plus
  `t4_e2_no_op_rebuild_produces_identical_manifest` and
  `t4_e0_cache_hit_avoids_synthesis_and_is_byte_identical` as reuse regressions.
- Walking skeleton result: green, with real `ffmpeg`/`ffprobe` 6.1.1 on WSL2.

## Implementation consequences

Two consequences of this change are operational rather than semantic, and are recorded here so
they are read rather than discovered.

### `package-render --retake` joins the qualification surface

ADR-0001 §11.4's alternate performance cannot be observed in a fresh workspace — "retains the
prior artifact" and "both joins" both need the workspace that already holds the take being
replaced. `crates/study-tts-testkit/examples/package-render.rs` therefore accepts
`--retake <segment-id>=<take>` and, for that invocation only, **requires** an existing
`--output-root` where a plain render still refuses one. It is a qualification instrument that
writes governed output, so it carries publication's durability properties:

- **Existing root required, and only for a retake.** A plain rerender still refuses an existing
  root, so it cannot overwrite a package a review was taken against.
- **Content-addressed output.** A package directory is named by its manifest digest, so a retake
  lands beside the generation it supersedes rather than on top of it.
- **No overwrite of a package or a cache entry.** Publication is
  `publish_directory_noreplace`; the prior generation's directory and the superseded take's cache
  entry are both left byte-identical. What *does* advance is `current.json`, which is the
  selection record and is meant to.
- **Deterministic derivation, from the lesson rather than from the prior plan.** A retake derives
  its plan from the validated lesson, the resolved synthesis context, and the explicit retake
  map, and then *replaces* the retained `plan.json`; it does not read it. That is deliberate and
  is the counterpart to invariant A-1: recovering selection from a retained plan is the **resume**
  path, and a retake is a new build attempt, which ADR-0001 §6.4 already has the edge for.

### A `1.0-skeleton` package disables retention reporting for its whole workspace

`prune_candidates` treats every published manifest as a retention root, and
`manifest::referenced_cache_keys` refuses a layout it cannot decode. One `1.0-skeleton` package
therefore refuses retention reporting for the **entire** workspace — not only for the lesson that
owns it — until that workspace is rebuilt.

The boundary is that layout and not "before `2.0`": `0.1-skeleton` and `0.2-skeleton` keep their
decoders, so a workspace holding them reports normally. Those layouts predate the retake, so
every segment they record is at take zero and its base key is its own cache key, which is what
`legacy_record` supplies and why they contribute a complete retention root rather than a partial
one.

**One `1.0-skeleton` package is governed, and this build cannot decode it.** The E1-S4
qualification output at
`e1-s4-package-2026-09-01-212639/workspace/previews/e1-s4-three-segment/packages/1579a41d…/` is
named by path in the accepted `e1-s4-minimal-package-generation-v1` gate record. Retention
reporting and package reuse against *that* workspace now refuse with
`DurableStateError::UnsupportedPackageManifest`, and the remedy is the one this record's own
decision already names: rebuild it.

The evidence itself is unaffected, and that is checkable rather than asserted: the record binds
its artifacts by SHA-256 of the bytes, so re-verifying it hashes files and never parses the
manifest through `parse_stored_manifest`. Nothing in the test suite or in CI reads that
workspace, and `data/` is untracked, so the package is local state rather than repository
content.

Whether a `1.0-skeleton` decoder is owed anyway is **already decided, not open**. §Approval's
manifest row is signed against "the `1.0-skeleton` refusal", and `G1-FREEZE-CHARTER.md`'s
`manifest` / `2.0-skeleton` row states the disposition in the same accepted commit: "a
`1.0-skeleton` package is refused and rebuilt". The package above is an instance of the
*rebuilt* half rather than a cost the signers did not price — "rebuilt" presupposes packages
existing to rebuild.

The asymmetry with E1-S4 §Impact — which kept the `0.1`/`0.2` decoders on the argument that
packages in those layouts exist and accepted evidence describes them — is real, and stating it
plainly is better than reasoning it away. A `1.0-skeleton` decoder is feasible on the same terms
as those two: it would record `take_selection_source: None`, exactly as `legacy_record` does, and
`validate_package` compares that field with `== Some(expected)`, so such a package could
contribute a retention root while never being reused. Nothing technical forces the refusal.

It is a decision, priced and signed: a fourth stored-segment decoder, carried for the life of the
layout, against one rebuild of an untracked qualification package. The signers took the rebuild.
Recorded here so a later reader finds the trade rather than re-deriving it and mistaking a choice
for an oversight.

This is a compatibility boundary rather than a defect, and it is fail-closed in the right
direction: reading an unreadable root as "references nothing" would report live artifacts as
prunable, which is a misleading report now and data loss once E2-S5 makes prune destructive. It
is recorded for operators in `docs/operations/UPGRADE-RUNBOOK.md` §Known compatibility
limitations, which names the module, and in the module's own header.

## Open questions

- **G-B — the speaking-rate measure is undefined by any ratified document.** ADR-0001 §11.4
  requires a speaking-rate comparison and no document in this repository says what one is.
  `audio_edges::assess_join` uses a **provisional proxy**: speech frames per character of
  `spoken_text`, chosen because both inputs are already exact and already in hand, so it adds no
  new input and no new dependency. It stands in for ADR-0003's "Join discontinuity threshold"
  row. **Owner: the audio owner**, with the listener representative, under ADR-0003. Until that
  row is frozen, `JoinContinuity::production` refuses to serve the measurement as a production
  reference.
- No ADR-0003 threshold is introduced by this record. Both of its relevant calibration rows read
  `Pending`, and a `Proposed` ADR authorizes nothing.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate although one person signed them all. On 2026-09-04 the project owner
approved this record directly rather than by delegation, so no delegated-authority sentence applies
here as it does to `E2-S1-INTERFACE-CHANGE-001`.

The T-AUDIO row is an acceptance of the proxy **as provisional**, not an endorsement of it as a
measure. It stands only until ADR-0003's calibration replaces it, on the same terms
`SilenceThreshold` already carries: the value declares
`study_tts_runtime::CalibrationSource::Provisional`, and
`study_tts_runtime::JoinContinuity::production` refuses to serve it as a production reference. The
open question in §Open questions stays open, and this row does not close it.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that every existing published package is invalidated and rebuilt, while every cache entry survives | Accepted — Ross Todd, 2026-09-04 |
| Contract owner (T-CORE) | Accept `plan` `4.0`, the G-A field-set resolution, and invariants I-1 through I-4 | Accepted — Ross Todd, 2026-09-04 |
| Contract owner (T-RUNTIME) | Accept `manifest` `2.0`, the `1.0-skeleton` refusal, and `take_selection_source` in `ReuseExpectations` | Accepted — Ross Todd, 2026-09-04 |
| Engineering owner | Accept invariant A-1 and that resume performs no takes discovery | Accepted — Ross Todd, 2026-09-04 |
| Affected track (T-AUDIO) | Accept the provisional frames-per-character speaking-rate proxy, or replace it | Accepted as provisional — Ross Todd, 2026-09-04 |
| Effective version and date | `plan` `4.0`, `manifest` `2.0` | Effective 2026-09-04 |

## Amendments

| Date | Amendment | Approval |
|---|---|---|
| 2026-09-04 | §Version and compatibility gained the layouts-read bullet, which states the read set this change leaves and supersedes `E1-S4-INTERFACE-CHANGE-001` §Three layouts are now read, one is written. §Implementation consequences corrected the retention boundary from "before `manifest` `2.0`" to `1.0-skeleton`, which is what `parse_stored_manifest` refuses; the wider claim was never true, because both older layouts kept their decoders. It also records the one governed package with the `1.0-skeleton` layout that this build cannot decode, and why refusing it is a priced decision rather than an oversight. §G-A's base-key bullet is corrected: it claimed the key was carried on the plan only and that a manifest copy would have "no second reader", which contradicted §Version and compatibility in the same document and is falsified by `referenced_cache_keys`, the reader retention reporting uses. The G-A field-set resolution itself is unchanged. An earlier draft of this row opened that as an open question **G-C**; it was withdrawn the same day, because the question was already answered when it was asked — §Approval's manifest row is signed against "the `1.0-skeleton` refusal" and `G1-FREEZE-CHARTER.md` states "refused and rebuilt", both in the accepting commit. Recorded in place rather than as a successor record because this record is not yet in force — it has not merged — so this is authoring rather than amendment of a landed control. | No re-approval sought: the signed disposition is unchanged, and the corrections move no contract, version, identity, or byte |
| 2026-09-04 | §Implementation consequences added, recording the `--retake` qualification surface and the retention-reporting boundary — stated then as pre-`2.0`, corrected in the row above. Both were properties of the change as signed; neither is a new decision, and no contract, version, identity, or byte moves. Recorded in place rather than as a successor record, because there is no claim to correct — `E1-S2-INTERFACE-CHANGE-003` exists for that case. | No re-approval sought: §Approval is unchanged |
