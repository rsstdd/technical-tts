# G1 Interface Freeze Charter

## Status

**Accepted 2026-09-02.** Every row in §Approval is signed. The G1 interfaces are frozen as of that
date, and `ADR-0001-D005`'s permission to correct a pre-freeze mistake while retaining a version is
spent.

`E2-S1-INTERFACE-CHANGE-001`, accepted 2026-09-03, amends the `job` and `job_state` rows under
the change procedure and makes their recorded `1.0` versions effective on that date.

Successor to [`PROVISIONAL-CONTRACT-BASELINE.md`](PROVISIONAL-CONTRACT-BASELINE.md), which set the
E0-S4 baseline and said in its own words that it claimed no production contract and made no
migration promise before G1. This is the record that ends that. `DELIVERY-PLAN.md` §Ownership
assigns "Provisional contract baseline and G1 freeze charter" to the engineering owner, "baseline
at G0; freeze at G1".

`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §G1 freeze mandates nine fields for every
frozen contract — contract, owner, consumers, canonical representation, compatibility rule,
contract tests, identity effect, migration, approval. **The columns below are the first eight
fields and are not a second copy of them.** That document is the authority for what each column
means; if the two ever disagree, it wins and this charter is wrong. Approval is the one field kept
out of the table, in §Approval, because it is one decision per role over the whole charter rather
than a cell per row.

## What "frozen" means here, and what it does not

A frozen contract may still change. Freezing sets the *price*: after G1 every change takes its
class from §Change classes and a **Breaking contract** move needs a major version, a migration, an
impact report, and owner approval. `ADR-0001-D005`'s permission to correct a pre-freeze mistake
while retaining a version **expires at this charter's acceptance**, and that expiry is the main
practical consequence of signing it.

Freezing is not a claim that a contract is finished. Several rows below name a stabilization story
in a later epic; those stories may still move their contract, at the post-freeze price.

## The inventory is derived, not curated

Every row below comes from one of two mechanical sources, and the completeness claim rests on
that rather than on review:

- `crates/study-tts-runtime/src/schemas.rs::PUBLISHED_SCHEMAS`, a seven-element array, one row per
  published JSON Schema; and
- every `*_SCHEMA_VERSION`, `*_CONTRACT_VERSION`, `*_IDENTITY_VERSION`, `*_PROTOCOL_VERSION`, and
  `*_RENDERER_VERSION` constant declared in `crates/*/src/*.rs`.

**A charter that freezes a subset while reading as complete leaves the remainder unfrozen and
nothing reports it.** So every constant from those two sources appears here — as a frozen row, or
in §Deliberately not frozen with the reason. Nothing is omitted for being uninteresting.

## Frozen contracts

| Contract | Owner | Consumers | Canonical representation | Compatibility rule | Contract tests | Identity effect | Migration |
|---|---|---|---|---|---|---|---|
| `lesson` / `3.1` | T-CORE | Authors, `study-tts lesson new`/`validate`, planning | `schemas/lesson-v3.schema.json`; `study_tts_core::AuthoredLesson`, `ValidatedLesson` | `SchemaVersion::accepted_by`: a higher major is refused, an older minor is read with its declared defaults | `t3_e1_published_schema_required_fields_match_the_recorded_surface`; the `t1_e1_*` lesson-invariant suite; `t4_e1_scaffolded_lesson_validates_without_manual_repair` | `spoken_text`, `style`, and `language` reach every synthesis key; `display_text`, roles, objectives, and review metadata deliberately do not | Older minors readable in place; a major needs a document rewrite |
| `plan` / `4.0` | T-CORE | Cache, package writer, job state | `schemas/plan-v4.schema.json`; `study_tts_core::RenderPlan` | as above | `t1_e0_plan_is_stable_for_identical_inputs` pins the plan hash and two cache keys as a checked-in golden | The plan hash is derived from the segment cache keys; it moves whenever a synthesis-key input moves. E2-S2 added `take_selection_source`, `synthesis_base_key`, and `audio_blake3` **outside** it, under the invariants in `E2-S2-INTERFACE-CHANGE-001` §Identity effect, so the golden did not move | Plans are derived, never migrated: re-plan |
| `job` / `1.0` | T-CORE / T-RUNTIME | Preview orchestration, resume | `schemas/job-v1.schema.json`; `study_tts_core::JobDocument`, `JobState` | as above | `run_job_repository_contract_scenario`, against both `InMemoryJobRepository` and `FileSystemJobRepository`; `t1_e2_illegal_state_transition_is_refused` pins the ADR-0001 §6.4 table; `t3_e1_published_schema_required_fields_match_the_recorded_surface` | Durable state only; defines no synthesis or verification key | Moved from the provisional `0.1` snapshot by `E2-S1-INTERFACE-CHANGE-001`. A `0.1` record is refused and preserved, never migrated; the runtime owner must reconcile it before a fresh build can revalidate independently addressed artifacts |
| `takes` / `1.0` | T-CORE | Retake selection | `schemas/takes-v1.schema.json`; `ValidatedTakes` | as above | `takes` invariant suite; segment identity shared with `lesson` | Selects which rendered take is authoritative; does not key synthesis | Older minors readable in place |
| `verification` / `1.0` | T-CORE | E4 ASR verification | `schemas/verification-v1.schema.json`; `VerificationContext`, `VerificationKey` | as above | `t2_e1_every_verification_input_changes_the_verification_key`; `t2_e1_a_verification_input_never_changes_the_synthesis_key` | Verification identities are **disjoint** from synthesis by construction, so re-running ASR never re-runs synthesis | Unused in production until E4 |
| `manifest` / `2.0-skeleton` | T-AUDIO | Package consumers, publication gate | `schemas/manifest-v2.schema.json` | as above | `t3_e1_published_schema_required_fields_match_the_recorded_surface`; E1-S4 package suite | Records what produced a package; `text_renderer_version` and, since E2-S2, `take_selection_source` make a change that alters package bytes without altering audio rebuild rather than reuse | `-skeleton` retained: E2-S3 moves it again. E2-S2 moved it to `2.0` under `E2-S2-INTERFACE-CHANGE-001`; a `1.0-skeleton` package is refused and rebuilt |
| `worker-protocol` / `2.0` | T-WORKER | Rust supervisor, Python worker | `schemas/worker-protocol-v2.schema.json`; `worker_protocol.rs` ↔ `worker/study_tts_worker/protocol.py` | as above, enforced at both ends | `t3_e1_both_protocol_ends_decide_the_committed_cases_alike` over `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | Protocol interpretation is a worker-bundle input, therefore synthesis-affecting | Both ends move together or not at all |
| `tts_executor` / `e1.tts-executor.3.0` | T-WORKER | Preview orchestration | `study_tts_runtime::TtsExecutor` and `BackendDescriptor` | `ContractDescriptor::assess_successor` | `run_tts_executor_contract_scenario`, against the fake in T4 and the real worker in the T5 instrument | Every `BackendDescriptor` field but `contract_version` and `max_text_bytes` reaches every synthesis key | E5-S2 pools above capacity one |
| `cache_publication` / `e0.cache-publication.2.0` | T-AUDIO | Orchestration, assembly, manifest | `CachePublisher`, `CacheResolveRequest`, `ValidatedCachedArtifact` | as above | `run_cache_contract_scenario`, against `FakeCachePublisher` and `FileSystemCachePublisher` | Acceptance changes affect reuse; speech-affecting ones need synthesis-identity review | E2-S2, E4 prune/recovery |
| `package_writer` / `e0.package-writer.2.0` | T-AUDIO | Orchestration, job state | `PackageWriter` and `PackagePublication` | as above | `run_package_writer_contract_scenario`, against the fake and — since E1-S4 — `FileSystemPackageWriter` | Tool profile and `text_renderer_version` gate reuse | E2-S3 |
| `job_state` / `1.0` | T-CORE / T-RUNTIME | Orchestration, resume | `JobRepository` (claim, load, replace, retain_inputs, retained_lesson, retained_plan, validate_preview_selection), `JobOwnership` | as above; the port carries the `job` document's version rather than a second one | as the `job` row, plus `t4_e2_interrupt_after_cache_publish_reconciles_on_resume`, `t4_e2_resume_regenerates_only_missing_or_invalid_segments`, `t4_e2_resume_refuses_a_job_package_that_disagrees_with_selected_output`, and `t4_e2_resume_refuses_a_selected_package_for_a_different_job_plan` through the real adapter | Durable state only | E4-S4 and E5 recovery |
| `synthesis_identity` / `e1-s5-v1` | T-CORE | Every cache key | `SYNTHESIS_IDENTITY_VERSION`; `SynthesisContext` | Moves whenever the **input list** changes — that is the lever it exists to be | `t2_e1_every_speech_affecting_field_changes_synthesis_key`, exhaustive by destructuring | Invalidates every cache entry when it moves | Entries under an old version stop being addressed; none is re-keyed or deleted |
| `cache_schema` / `3.0` | T-AUDIO | Cache entry records | `CACHE_SCHEMA_VERSION`; `ArtifactProvenance` | A required field is a Breaking move | Cache acceptance suite | Itself a synthesis-key input, so a move invalidates reuse | As above |
| `verification_identity` / `e1-s1-v1` | T-CORE | E4 verification keys | `VERIFICATION_IDENTITY_VERSION` | as `synthesis_identity` | `t2_e1_every_verification_input_changes_the_verification_key` | Invalidates verification results only | Unused until E4 |
| `worker_bundle_identity` / `e1-s1-v4` | T-WORKER | Every synthesis key | `WORKER_BUNDLE_IDENTITY_VERSION`; `worker/bundle-manifest.json` `inputs` | Derived from declared inputs; ADR-0001 §12.5 sets the input set | `t1_e1_worker_bundle_hash_changes_on_owned_runtime_input`; `t1_e1_worker_bundle_hash_ignores_unrelated_repository_files` | Any declared input's bytes move every synthesis key | Requalification, per ADR-0002 |
| `bundle_manifest` / `1.2` | T-WORKER | Bundle identity derivation | `BUNDLE_MANIFEST_SCHEMA_VERSION` | `1.0` and `1.1` still readable; each decoder refuses a later layout's fields | `worker_bundle` suite | The manifest declares itself among its own inputs, so its bytes are hashed | Older layouts readable |
| `launcher` / `1.1` | T-WORKER | Worker launch, both ends | `LAUNCHER_SCHEMA_VERSION`; `worker/launcher.json` | Read closed: an undeclared field is refused | `t1_e1_the_checked_in_launcher_is_the_shape_this_build_reads` and the Python `LAUNCHER_SHAPE` suite | `seed` and `generation_parameters` reach every synthesis key | Both ends move together |
| `text_renderer` / `1.0-skeleton-text-renderer` | T-AUDIO | Transcript, captions, chapters | `timeline::TEXT_RENDERER_VERSION`, recorded in the manifest | A rule change moves it | E1-S4 package suite | Gates package reuse for the three documents FFmpeg never touches | Packages rebuild rather than reuse |

## Deliberately not frozen

| Name | Why not |
|---|---|
| `JOB_EVENT_SCHEMA_VERSION` (`e2.job-event.0.1`) | An internal diagnostic line in `jobs/<job-id>/events.ndjson`, as `publication.json` is an internal journal. E2-S1 owns only the append-after-durable ordering it carries; DELIVERY-PLAN E2-S4 decides whether the line becomes a published schema, and until then its shape is not promised |
| `JOB_LOCK_SCHEMA_VERSION` (`0.1-skeleton-job-lock`) | The strict record inside `build.lock`, read by one module and never by a consumer. Its version exists so a record from another build is refused as `IncompatibleJobLock` rather than misread; nothing else depends on its shape, so it is not frozen |
| `SCHEMA_VERSION_PATTERN` | A grammar, not a version. It constrains how a version is spelled and carries no compatibility promise of its own |
| `PROTOCOL_FAKE_BUNDLE_HASH`, `PROTOCOL_FAKE_MODEL_ARTIFACTS_HASH` | Test identities. Fixed so a fake's keys are stable and reachable by no real bundle or model root; nothing in production reads them |
| `PINNED_MODEL_REVISION`, `DECLARED_MODEL_ARTIFACTS` | Governed-backend content, not an interface. ADR-0002 owns the qualified revision, and changing it is a requalification rather than a version move |
| `MAX_*` ceilings (`MAX_LESSON_JSON_BYTES`, `MAX_WORKER_FRAME_BYTES`, and siblings) | Provisional resource bounds, pinned by `t3_e1_provisional_lesson_resource_ceilings_match_walking_skeleton_document` against the document that sets them. They are refusal thresholds rather than contracts, and tightening one is not a compatibility event |

## The open §12.5 question this charter must answer

Issue #65 raised it and deliberately did not decide it: **does `worker/pyproject.toml` belong in the
worker-bundle identity at all?**

ADR-0001 §12.5 names "production worker source and imported project-owned modules, the production
Python lockfile, the worker protocol schema, launcher configuration that affects inference, and
Python runtime and platform ABI identity". `worker/pyproject.toml` is none of those. It is declared
in `worker/bundle-manifest.json` `inputs` anyway, and that is what let a dependency-bot commit
re-key every cache entry in the project by editing a file the worker never loads — PR #56, which
`check_requirements_match_lock` now refuses.

Two readings, and the charter is where one is chosen:

- **Keep it declared.** The reconciliation guard makes the failure mode impossible, and removing a
  declared input moves the bundle identity once more.
- **Remove it.** §12.5's list is the authority on what the identity is derived from, and a file
  outside that list widens the identity beyond what the ADR specifies. This is the smaller
  mechanism: it removes the class of defect rather than guarding one instance of it.

Removing it moves the worker-bundle identity and therefore every synthesis key.

**Decided 2026-09-02: it stays declared, and the removal is owed at the next identity move.**

Not because the second reading is wrong — it is the better one, and §12.5 is the authority. Because
of when the question arrived. The bundle identity moved twice on the day of the freeze, and the
reference-machine requalification and the three-segment package render that E1-S5's evidence rests
on were both taken at `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`. Removing a
declared input now would move it a third time, invalidate both, and buy a third qualification cycle
and a second listening review — for a defect `check_requirements_match_lock` already makes
unreachable.

So it is deferred rather than dismissed, and deferred to a named occasion rather than to nobody:
**the next change that moves the worker-bundle identity for its own reasons carries this with it, at
no additional qualification cost.** That is the batching the E1-S5 work used for #66, #70, and the
capability flip, applied once more.

Freezing it in does not close the door — it prices the door, and this paragraph records the price
as one already-paid identity move rather than a new one. `worker/bundle-manifest.json` `inputs` is
frozen **as it stands**, `pyproject.toml` included, with this as the standing exception.

## Obligations owed at the next worker-bundle identity move

Two corrections are owed and neither is discretionary. Both are deferred for the same reason — each
would otherwise force a qualification cycle of its own — and both are **mandatory** at the next
change that moves the worker-bundle identity for reasons of its own. They are recorded here rather
than in `worker/study_tts_worker/worker.py`, because that file is a declared bundle input and a
comment added to it would itself move the identity this charter freezes.

### 1. Remove `worker/pyproject.toml` from the declared bundle inputs

Per §The open §12.5 question above. It does not semantically belong in the identity under
ADR-0001 §12.5, and its continued inclusion is a compatibility decision rather than an endorsement
of the current identity definition.

### 2. Narrow or requalify `capabilities.deterministic_seed`

**`deterministic_seed = True` is retained for the qualified identity
`1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`, for evidence consistency.** The
committed implementation, the requalification result, the package render, both listening reviews,
and the G1 gate record all describe that one identity, and changing the declaration now would move
the identity out from under every one of them.

**Retention is not endorsement, and the distinction is the point.** What was measured is one
environment, one seed, one sentence, one voice, and one pair of worker lifetimes.
`t5_e1_two_lifetimes_render_identical_audio_under_one_seed` observed zero of 92 160 frames differing
and byte-identical canonical WAVs — a real result, and a **narrow** one. It does **not** establish
that a blanket deterministic-worker capability is warranted. The qualification envelope is too
narrow to support that general claim, and a capability declared to every consumer is a general
claim.

At the next otherwise-required identity move, in the **same** cycle as obligation 1:

- change `deterministic_seed` to the value and semantics the evidence actually supports;
- establish a broader qualification envelope **before** asserting any general deterministic
  capability — enough variation in input text, voices and configurations, and seeds or deterministic
  conditions to justify what is being asserted;
- perform requalification, package rendering, listening review, and gate evidence for the new
  identity **after both changes are present**, so one cycle covers both.

**Nothing in the current G1 evidence may be read as claiming more than it demonstrates.**
`E1-S3-INTERFACE-CHANGE-005` §Limits states the observed envelope; where any record generalizes
beyond it, that record is wrong and this paragraph governs.

## What this charter does not freeze, and cannot

- **Nothing measured.** A charter records contracts and versions. It attests no qualification
  result, no listening review, and no rendered package.
- **`determinism_class`'s value, as distinct from its field.** The charter freezes the field. The
  bounded result used at acceptance is recorded outside this charter in
  `evidence/gates/g1/e1-s3/e1-s5-requalification-result.json`: two fresh lifetimes produced
  byte-identical canonical WAVs over 92 160 frames. This charter references that result; it does
  not attest it, qualify a wider corpus, or widen it beyond one environment, one seed, and one
  sentence.
- **The E1-S5 story and the G1 gate.** Those are evidence records against measured bytes, and this
  is not one.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Engineering owner | Accept the inventory as complete against its two mechanical sources, and accept that `ADR-0001-D005`'s pre-freeze correction permission expires on acceptance | Accepted — Ross Todd, 2026-09-02 |
| Project owner | Decide the §12.5 question | **Stays declared.** Removal is the better reading and is owed at the next change that moves the worker-bundle identity for its own reasons, riding it at no additional qualification cost — Ross Todd, 2026-09-02 |
| Contract owner (T-CORE) | Accept the lesson, plan, job, takes, verification, synthesis-identity, and verification-identity rows | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-WORKER) | Accept the worker-protocol, executor, bundle-identity, bundle-manifest, and launcher rows | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-AUDIO) | Accept the manifest, cache-publication, cache-schema, package-writer, and text-renderer rows | Accepted — Ross Todd, 2026-09-02 |
| Affected-track review | Accept §Deliberately not frozen as reasoned exclusions rather than omissions | Accepted — Ross Todd, 2026-09-02 |

- Effective version and date: the eighteen contracts above, frozen 2026-09-02.
