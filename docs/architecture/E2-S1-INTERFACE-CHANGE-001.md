# E2-S1 Interface Change 001 — The job document and its state machine

## Identification

- Record ID: `E2-S1-INTERFACE-CHANGE-001`
- Status: **Accepted 2026-09-03.** Every row in §Approval is signed.
- Contract owner: T-CORE (the `job` document) and T-RUNTIME (the `job_state` port)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-RUNTIME, T-AUDIO
- Accepted ADR, if architectural: ADR-0001 §6.4, §12.1, §12.3, §12.4, §12.7. This record
  implements those sections; it changes no architecture. Two readings the ADR leaves open are
  fixed here and mirrored into §6.4 and §12.4 (see §Version and compatibility).

`docs/architecture/G1-FREEZE-CHARTER.md` froze `job` at `0.1` with the row saying, in bold,
that E2-S1 replaces the state machine and a `1.0` there would claim a stability that story was
going to break. This is that story and its planned breaking change. Issue #14 and the
[reviewed plan](https://github.com/rsstdd/technical-tts/issues/14#issuecomment-5514220213) on
it are the working record.

## Version and compatibility

### `job` — `0.1` → `1.0`, Breaking contract

| | Before | After |
|---|---|---|
| Rust type | `ProvisionalJobSnapshot` | `JobDocument` |
| Version field | `schema_version: "e0.job-state.0.1"`, a `String` label | `schema_version: "1.0"`, a `SchemaVersion` under `accepted_by` |
| State | `ProvisionalJobStage` — `planned`, `caching`, `packaging`, `package_selected` | `JobState` — the thirteen ADR-0001 §6.4 states; `may_transition_to` transcribes its 22 edges; `last_successful_state` keeps post-render failure and cancellation context |
| Build identity | none | `build_attempt: NonZeroU32`; `abandoned_attempt: Option<{build_attempt, state}>`; `JobDocument::open_attempt` returns `Result` and refuses attempt-counter overflow |
| Lesson identity | none | `lesson_blake3: LessonDigest`, the checksum of the retained `lesson.json` |
| Segments | none | `segments: BTreeMap<segment_id, {cache_key, audio_blake3}>`, written durably after each segment |
| Completion | `selected_package`, a *stage* | `preview_package`, a separate field beside `release_status`; never a state |
| Other required fields | — | `release_status`, `application_version` |
| Schema | `schemas/job-v0.schema.json` | `schemas/job-v1.schema.json`; `job-v0` retired |
| Unknown fields | refused | refused |

**Two readings of the ADR are fixed here** and written into the ADR beside the sections they
interpret, so the interpretation is visible where a reader would look for it:

1. **The §6.4 table is scoped to one build attempt.** §6.4 has no edge from `Rendered`,
   `Verified`, `Assembling`, `QualityChecked`, or `Published` back to `Planned`, so a rebuild
   of a finished job cannot *transition*. It opens attempt *N+1* at `Created` through
   `JobDocument::open_attempt`, and the finished attempt is retained as `abandoned_attempt`
   (§12.7 step 5). §12.4 lists "job **and build** identity" as two things; this is why.
2. **Private-preview completion is not a state.** A preview build has no verifier (E4), no
   quality profile (E2-S3), and no production publication path, so `Rendered` is as far as the
   §6.4 machine can honestly take it. The selected package is recorded in `preview_package`
   beside `ReleaseStatus::PrivatePreview` — the "separate private-preview completion status"
   DELIVERY-PLAN E2-S1 task 4 names. `QualityChecked → Published` stays in the table because the
   diagram has it; `JobDocument::transition` refuses it under a preview through
   `ReleaseError::PrivateProfileCannotClaimProduction`, so the edge exists and the release status
   is what forbids taking it. `t1_e2_private_preview_cannot_transition_to_published` pins both
   halves.

**Deferred §12.4 rows, with their owner**, admitted later as compatible extensions under
`SchemaVersion::accepted_by`: attempts, synthesis base keys, and selected takes (E2-S2);
verification keys, token diffs, and adjudications (E4); failure classification and safe recovery
action (E5); worker and model identities, which every recorded cache key already fixes, when
E5-S2 pools workers. Timestamps are on every `events.ndjson` line rather than on the document.
Publishing a field nothing writes would freeze a shape nothing has exercised.

### `job_state` — `e0.job-state.0.1` → `1.0`, Breaking contract

`JobRepository` gains four methods and changes the type of two:

| Method | Change |
|---|---|
| `load` | returns `Option<JobDocument>`; refuses a `0.1` record as `UnsupportedDurableRecord` before the strict parse, so an old record is reported as unsupported rather than as malformed; also strictly parses every `events.ndjson` line and checks its job identity so resume parses authoritative state before reconciliation |
| `replace` | takes `&JobDocument`; refuses a replacement that skips an ADR state edge or does not name the current attempt as its exact predecessor, validates the event log and the pending event's size **before** the replacement so a torn or full log refuses while `job.json` still holds its prior bytes, then appends a `StateDurable` event **after** the atomic replacement returns |
| `retain_inputs` | new: writes `lesson.json` (exact validated bytes) and `plan.json` beside the document (ADR §12.1) |
| `retained_lesson` | new: reads `lesson.json` back through the same bounded reader the build used |
| `retained_plan` | new: strictly parses and version-gates `plan.json`, checks its job identity, and recomputes its plan hash before resume compares it with `job.json` |
| `validate_preview_selection` | new: after package-journal reconciliation, compares a completion in `job.json` with the selected package in `current.json` and requires that package's validated manifest to name the job's plan before any retained input or job-state replacement; an absent completion accepts a selected package because publication may have won the crash race |

The repository repeats the state edge and private-preview publication checks at the durable
boundary. The public `JobDocument` fields exist for strict schema serialization, so trusting only
callers to use `transition` would let an individually coherent document skip an edge or persist
`Published` under `private_preview`.

The port no longer carries a version string of its own. `JOB_STATE_CONTRACT_VERSION` in the
runtime was a second copy of the core label, which the charter's §Deliberately not frozen already
called a copy that could drift; it is deleted and the port follows the `job` document's version.

### `study-tts-runtime` public API — compatible extension

| Item | What it is |
|---|---|
| `ResumeRequest` | Job identity plus the environment a resume cannot read from the job directory: workspace, both tool paths, the voice root |
| `resume_preview`, `resume_preview_with_services` | ADR-0001 §12.7 as one entry point: claim, load, verify the retained lesson's checksum, gate, then run the same attempt a fresh build runs |
| `DurableStateError::{MalformedJobEventLog, IllegalJobTransition, JobAttemptOverflow, RetainedLessonMismatch, RetainedLessonIdentityMismatch, MalformedRetainedPlan, RetainedPlanIdentityMismatch, RetainedPlanHashMismatch, JobPlanHashMismatch, JobPreviewSelectionMismatch, NoJobToResume}` | One variant per new invariant; integrity refusals route to "State or checksum corruption", while `NoJobToResume` is unrouted like `LiveJobLock` |
| `DurableStateError::LiveJobLock` | `pid` and `process_start` become `Option`, absent only when the holder has cleared its record and not yet closed the descriptor |
| `DurableStateError::{JobSnapshotAttemptMismatch, JobReplacementPredecessorMismatch, JobSnapshotSelectionMismatch, JobSnapshotPackageIdentityMismatch, JobSnapshotLastSuccessfulStateMismatch}` | Refuse internally non-consecutive attempt identities, a replacement whose predecessor is not the document on disk, a package recorded before rendering, two disagreeing package identities, or state that its recorded last successful state cannot explain |

`study-tts resume` as a command is **not** here. DELIVERY-PLAN E2-S5 task 1 lists `resume` among
the commands it implements and depends on this story; the library entry point is what E2-S5
wraps.

### `study-tts-testkit` — compatible extension

`InMemoryJobRepository` stores `JobDocument`s and retained lessons and plans; `snapshots()` is renamed
`documents()`; `FakeJobCall` gains `RetainInputs`, `RetainedLesson`, and
`ValidatePreviewSelection`, and `Replace` carries a `JobState`. `InterruptingJobRepository` is
new: it delegates to the real adapter and fails the first `replace` into a chosen state, which
leaves a workspace exactly as a process killed at that moment would.

### Lock and event log — no version move

The lock record's on-disk shape is unchanged. Its *use* changes: a record found on a free lock
is verified against `/proc` before it is replaced, and `JobLock` clears the record on release so
a record's presence means the owner died. `JOB_EVENT_SCHEMA_VERSION` (`e2.job-event.0.1`) is
new and listed in the charter's §Deliberately not frozen with `JOB_LOCK_SCHEMA_VERSION`, which
the charter's own derivation rule required and which had been missing.

## Impact

- Synthesis identities affected: **none.** Nothing here reaches a cache key.
- Verification identities affected: **none.**
- Plan, takes, or package identities affected: **none.** `plan.json` is retained, not re-keyed;
  package reuse is unchanged, which `t4_e2_no_op_rebuild_produces_identical_manifest` proves by
  byte comparison. A legal transition back to `Planned` clears the prior preview selection because
  the ADR labels it as an input, rendering, or selected-take change; the package bytes remain
  immutable. Before opening another attempt, a recorded preview completion must agree with the
  package layer's reconciled selection and that package's manifest must name the recorded plan;
  the records are preserved when they disagree.
- Consumers and commands affected: `build_preview` (walks the new machine; writes per segment);
  nothing in `study-tts-cli`.
- Fakes and shared suites affected: `InMemoryJobRepository`, `RecordingJobRepository`,
  `run_job_repository_contract_scenario`, `provisional_contracts.rs`; updated before the
  adapters, per §Change procedure step 4.
- Fixtures and schemas affected: `schemas/job-v1.schema.json` generated, `job-v0` retired;
  `fixtures/contracts/e1-s1-job-{valid,unknown-version,malformed-digests}.json` rewritten at
  `1.0` with their `docs/testing/TEST-DATA-MANIFEST.md` digests; `PUBLISHED_REQUIRED_SURFACE`
  records the four required-field surfaces of `job 1.0`.
- Existing cached artifacts affected: **none.** Not one is re-keyed, moved, or deleted.
- Published packages or accepted takes affected: **none.**

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes — `contracts.rs` and
  `provisional_contracts.rs` moved with `job.rs`, then `job_repository.rs` and `pipeline.rs`.
- Migration procedure: **none, by decision.** A `job.json` declaring `e0.job-state.0.1` is refused
  as `UnsupportedDurableRecord` and left in place
  (`t4_e2_unsupported_job_record_version_is_refused_without_migration`). It is provisional state
  for an interrupted preview. The refusal deliberately blocks both build and resume until the
  runtime owner reconciles that record; only a later fresh build can revalidate its independently
  addressed cache and package artifacts. Reading an old record under new semantics is the failure
  `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` exists to prevent.
- Rollback procedure: revert the change set. A `1.0` document is then refused by the `0.1` reader
  as `UnsupportedDurableRecord` in the same way; the runtime owner must reconcile the incompatible
  record before an older build can revalidate independent artifacts.
- Compatibility evidence: the seven issue-named E2-S1 tests pass, plus the thirty-nine supporting names
  listed beside them under `DELIVERY-PLAN.md` §Story E2-S1; every pre-existing `t4_e0_*`/`t4_e1_*`
  test passes. The two shared-contract tests are updated to exercise the new repository surface, as
  §Impact records. Two names are retired, neither of them a `DELIVERY-PLAN.md` contract.
  `t4_e0_corrupt_job_snapshot_is_refused_without_overwrite`: its assertions move to
  `t4_e2_corrupt_job_state_is_not_overwritten` on the new document and gain a resume path.
  `t1_e0_advancing_drops_a_recorded_package_selection`: it held that a stage other than
  `package_selected` may not carry a selection, and that invariant does not survive as one rule,
  because `preview_package` is a field beside the state rather than a stage. Its parts are pinned
  separately — a package recorded before rendering is refused by
  `t1_e2_a_preview_package_before_rendering_is_refused`, a plan-changing return to `Planned` clears
  one by `t1_e2_returning_to_planned_clears_preview_completion` from both `NeedsReview` and
  `Failed`, where the retired test covered a single provisional advance, and a post-render failure
  deliberately keeps one by `t1_e2_a_failed_post_render_attempt_retains_preview_completion`.
  Keeping either retired name would duplicate behavior the new document already proves.
- Mapped tests and qualification rerun: `cargo test --workspace --all-targets --locked` — 499
  passed; `error_documentation`, `schemas`, and `provisional_contracts` suites included. The
  count moved from the 497 recorded at acceptance by
  `t4_e2_a_full_event_log_refuses_state_replacement`, added with the event-log preflight
  in PR #75, and by `t4_e2_a_fresh_build_restores_a_retained_plan_that_disagrees_with_job_state`,
  which pins the recovery the §Limits window below relies on; no other result changed.
- Walking skeleton result: `cargo test --offline -p study-tts-testkit --test walking_skeleton
  --locked` — 56 passed.

## Limits this change does not close

- **`plan.json` and `job.json` are published separately.** `retain_inputs` replaces the
  retained plan before the attempt's first `Planned` document records its hash, so an attempt
  interrupted between them leaves the two naming different plans whenever the re-derived plan
  differs from the recorded one — which needs a changed executor descriptor or voice
  conditioning, since the lesson bytes are checksum-verified identical. The next resume
  refuses with `JobPlanHashMismatch` and a fresh build restores agreement; no artifact is
  lost, which `t4_e2_a_fresh_build_restores_a_retained_plan_that_disagrees_with_job_state`
  pins. Ordering cannot close the window, only a single staged publication of both files,
  which no story owns yet.
- **Verification, takes, and approval reconciliation** (§12.7 steps 8–9) have no producer until
  E4, E2-S2, and E2-S6. `JobState` carries their states; nothing enters them.
- **`Failed` and `Cancelled` have no writer.** A refused build propagates its error and leaves
  `job.json` at the last durable state; a resume abandons that attempt. E5 owns retry budgets
  and cancellation, which is when those states get a producer and the document gets its failure
  classification.
- **A contended reader can still observe a partial record** while the owner is between
  `set_len(0)` and its serialized write — a window that predates this change. The contended
  branch now tolerates an *empty* record; a partial one is still `MalformedJobLock`.
- **A resume of an unknown job leaves `jobs/<job-id>/` behind**, because `claim` creates the job
  directory before `load` can report `NoJobToResume`. ADR §12.7 orders the lock first, so this
  is the honest order; E2-S5's command surface is where a pre-flight existence check belongs.
- **Durable records have provisional resource ceilings.** `job.json` is 16 MiB, retained
  `plan.json` is 32 MiB, a lock record and one event line are 4 KiB each, and one event log is
  8 MiB. Job and plan segment collections share the lesson's 4,096-segment ceiling. The values,
  owning constants, and tests are recorded together in `WALKING-SKELETON.md` §Provisional
  resource ceilings.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately. On
2026-09-03, the project owner explicitly authorized the remediation agent to record these owner
decisions; the rows remain separate because the decisions and risks remain separate.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept the attempt-scoped reading of ADR-0001 §6.4 and the separate private-preview completion, both written into the ADR | Accepted — Ross Todd, 2026-09-03 |
| Contract owner (T-CORE) | Accept `job` `1.0`, the deferred §12.4 rows and their owners, and that no identity moves | Accepted — Ross Todd, 2026-09-03 |
| Contract owner (T-RUNTIME) | Accept the four new `JobRepository` methods, the recovery-specific `DurableStateError` variants above, and `resume_preview` as the library entry point E2-S5 wraps | Accepted — Ross Todd, 2026-09-03 |
| Affected-track reviewer (T-AUDIO) | Accept invalid-cache quarantine and regeneration during resume, and that no synthesis, plan, package, or audio identity moves | Accepted — Ross Todd, 2026-09-03 |
| Engineering owner | Accept refusal-not-migration of `e0.job-state.0.1`, the retired E0 test, the provisional resource ceilings, and the verification results recorded above | Accepted — Ross Todd, 2026-09-03 |

- Effective version and date: `job` and `job_state` `1.0`, effective 2026-09-03.
