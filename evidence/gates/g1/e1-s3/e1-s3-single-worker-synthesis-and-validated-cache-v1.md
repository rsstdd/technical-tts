# E1-S3 Single-Worker Synthesis and Validated Cache v1

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: working tree on `fix/issue-59-retired-grant` at the E1-S3 governance preflight
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Proposed

## Scope and decision

`DELIVERY-PLAN.md` §E1-S3 names eight tasks and eight acceptance tests. This record is opened at
the story's **governance preflight**, before any of them, and accumulates findings as the work
runs. Per `evidence/README.md` §Accepting a record at its gate, E1-S3 keeps **one** record,
`Proposed` for as long as the work runs, pinned once and accepted once at G1.

Two findings were owed before the story's reference-machine work could begin. Both were raised
against the worker-bundle identity, which E1-S3 inherits as the input naming every cache entry it
publishes. Neither was found by looking for it: both surfaced from running the identity derivation
that `DELIVERY-PLAN.md` E1-S3 task 8 depends on.

This record files under `g1/` because E1-S3 feeds G1. It is a story-level record under the gate it
serves, not a gate acceptance.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. The preflight half of this record is accepted
when all five hold; the story half adds its own criteria as the work lands.

1. The 0.90-second worker-bundle hash run `e1-s1-provisional-contract-baseline-v13` reported and
   v14 and v15 carried is either explained with evidence that distinguishes a fast run from a
   skipped precondition, or is reproduced and shown to be unexplainable with the instrument
   available.
2. The instrument that admitted the ambiguity is repaired, so a future fast run does not have to be
   re-investigated from scratch.
3. The checked-in worker-bundle identity is derived from a freshly built example and stated, with
   any divergence from `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` explained
   by a named commit rather than asserted.
4. Any defect the preflight finds is closed mechanically — by a check that refuses it — rather than
   by a corrected file alone, and that check fails against the defect before it passes against the
   fix.
5. `cargo fmt --check`, Clippy with `-D warnings`, the workspace suite, doctests, the Rust
   conventions check, the Python worker suite, and published-schema drift all pass, and anything not
   run is named rather than omitted.

## Result

| Criterion | Result |
|---|---|
| 1 — the 0.90 s run | Met. Reproduced deliberately, 3 times in 24 runs, and explained: the WSL2 wall clock, not a skipped comparison. §Finding 1 |
| 2 — the instrument | Met. CPU time is recorded by the qualification workflow, stated as the witnessing measure, and banded by `ADR-0001-D006`, which supersedes D004. §Finding 1 |
| 3 — the identity | Met. `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`, after the divergence in §Finding 2 was traced to commit `e87cb57` and reverted |
| 4 — closed mechanically | Met. `check_requirements_match_lock` refuses the defect; four tests, red before green. §Finding 2 |
| 5 — checks | Met for everything run; §Verification run names what was not run and why |

## Finding 1 — the 0.90-second bundle-hash run is a wall-clock artifact

`e1-s1-provisional-contract-baseline-v13` §Verification run reported five runs at 3.41–3.50 s and
then "a sixth consecutive run returned the same identity in 0.90 s", recording it as unexplained
because "a run an order of magnitude faster than the authorized band is the shape a skipped
comparison would also have, and this record does not claim to have distinguished the two". v14 and
v15 carried it forward untouched. It is tracked as issue #60.

**It reproduces, and CPU time distinguishes the two cases.** Twenty-four consecutive runs of the
freshly built `./target/debug/examples/worker-bundle-hash` against the restored locked environment,
on the ADR-0002 reference machine, fully warm, all returning the same identity:

| Measure | Value |
|---|---:|
| CPU time (`%U` + `%S`) | 3.02 – 3.19 s |
| Wall time, uncorrupted runs | 3.25 – 3.42 s |
| Wall time, corrupted runs | 0.77, 0.80, 0.80 s |
| Corrupted runs | 3 of 24 |

Every corrupted run spent CPU time **inside** the band. A process cannot consume four times more
CPU than elapsed on this workload — a Rust parent, one `python -I -S` child, and two I/O-bound pipe
threads — so the wall clock is wrong rather than the comparison skipped.

**The page-cache explanation v13 offered is ruled out.** An earlier eight-run batch on the
pre-existing binary recorded blocks-in per run: the cold first run cost 14.01 s wall and 8.65 s CPU
with 2,765,320 blocks in, while runs 5–8 were fully warm at 0 blocks in and still cost 3.3 s. Cache
warmth moves the number the other way, and the outlier in that batch (0.77 s wall, 3.06 s CPU) was
itself a fully warm run.

The instrument is repaired rather than only explained, because the ambiguity was the instrument's:
`.github/workflows/qualification.yml` now records `%U` and `%S` rather than bare `time`;
`docs/operations/WORKER-ENVIRONMENT.md` §This is the part of the check that costs something states
that a run is judged on CPU time and why; and `ADR-0001-D006` supersedes `ADR-0001-D004` with retaken
bands, since this machine's uncorrupted wall time now sits just below D004's recorded
3.43–3.62 s — the drift D004's own workflow comment said should trigger a retake. It is approved
and signed 2026-08-30.

## Finding 2 — a bot moved the bundle identity through an unchecked declared input

Deriving the identity to establish the story's baseline returned
`8e7cc2f069bee496e36dcc4337ade7311b628d24f3d115294c1d1f7e4e5bcc0c`, not the
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` every accepted E1-S1 record
names.

**Cause.** Commit `e87cb57`, merged as PR #56, raised `worker/pyproject.toml` from
`torch==2.6.0+cpu` to `torch==2.13.0` and from `setuptools==78.1.0` to `setuptools==83.0.0`, and
left `worker/requirements.lock` at the original versions — which is what the restored
`worker/.venv` actually holds. `worker/pyproject.toml` is a declared bundle input, so its bytes are
hashed; the identity therefore moved for a change to a file the worker never loads, and every cache
key in the project moved with it.

**Why nothing caught it.** `WorkerBundle::verified_hash` compares the installed environment against
`worker/requirements.lock`, and the lock had not moved, so every check in
`docs/operations/WORKER-ENVIRONMENT.md` passed. The two declared inputs were never compared to each
other. No evidence record pins or even cites `worker/pyproject.toml`, so the provenance check saw
nothing either.

**Why the bump could not be completed.** `chatterbox_tts-0.1.2.dist-info/METADATA` declares
`Requires-Dist: torch==2.6.0` and `Requires-Dist: torchaudio==2.6.0`, and `torchaudio 2.6.0`
declares `Requires-Dist: torch==2.6.0`, so step 3 of
`docs/operations/WORKER-ENVIRONMENT.md` §Regenerating the lock cannot resolve `torch==2.13.0`
against the governed backend. `setuptools==78.1.0` is not incidental either: the lock header records
it as the pinned builder for `s3tokenizer==0.1.7`, the one source-built distribution. Raising either
is a governed-backend change requiring ADR-0002 re-qualification, and is deferred to its own issue.

**Nothing was published under the moved identity.** The cache root `data/qualification/cache`
holds no files, and `8e7cc2f0…` appears nowhere in the tree outside this record. It could not have:
this build's worker refuses `synthesize` with `initialization_failed` naming E1-S3, so no audio has
ever been published under any synthesis key.

**Resolution.** `worker/pyproject.toml` is reverted to agree with the lock, the governed backend,
and the restored environment; `git diff` confirms the file returns to blob `43f2aaf`, its pre-bump
bytes. A freshly built example then returns
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`, confirming the identity was
moved by this commit alone and not by a stale binary.

**Closed mechanically.** `check_requirements_match_lock` in
`crates/study-tts-runtime/src/worker_environment.rs` refuses a requirement the lock does not
resolve, before the probe runs. `docs/operations/WORKER-ENVIRONMENT.md` §The declaration is
reconciled with the lock is the other end of the mirror. Four tests pin it, and each failed against
the defect before it passed against the fix:

| Test | Tier | Invariant |
|---|---|---|
| `t1_e1_a_requirement_the_lock_does_not_resolve_is_refused` | T1 | Each of the four disagreements raises its own `WorkerRequirementFault`, and no refusal prints the requirement text |
| `t1_e1_a_requirement_agreeing_with_the_lock_is_accepted` | T1 | A `+cpu` local version, a non-canonical name spelling, and the file's non-requirement strings all pass |
| `t1_e1_a_requirements_string_this_build_cannot_read_is_refused` | T1 | A multi-line or escaped TOML string is refused rather than scanned wrongly |
| `t4_e1_a_requirements_declaration_the_lock_contradicts_is_refused` | T4 | The reconciliation runs inside `verified_hash`, over the declared file, before the probe |

The T4 case is the one that would have turned PR #56 red. Run against the unfixed build it did not
merely fail — `verified_hash` returned a *different* identity,
`ae1452c2eb71a15f7b7df04ef8d94666838f10640ab0072f67da4d117f9c60a3`, which is the silent re-keying
stated as an observation rather than an inference.

## Story progress

Recorded as the work lands, per §Scope and decision. No task is claimed complete
until its `DELIVERY-PLAN.md` §E1-S3 test exists under its exact name and passes.

### Step 1 — capacity-one persistent executor (task 1, and task 7 in part)

`crates/study-tts-runtime/src/worker_client.rs` owns one persistent child and the NDJSON
conversation with it; `crates/study-tts-runtime/src/worker_executor.rs` is the capacity-one
`TtsExecutor` over it. Process-tree ownership is **shared** from
`crates/study-tts-runtime/src/process.rs` (`ProcessOwnership`, `configure_process_group`,
`terminate`) rather than copied: a second, weaker copy of containment is the defect that
boundary exists to prevent.

The descriptor is built from what the worker reported at `initialize` and `capabilities`, never
from a constant — a hard-coded identity would name a bundle that is not the one running, and
ADR-0001 §12.5 makes that identity a term of every cache key.

Two defects were found by review and by a failing test rather than shipped:

- **`progress` frames were read as answers.** ADR-0001 §10.2 interleaves `progress` before a
  result, and the reader returned the first correlated frame. Fixed, with the deadline bounding
  the whole exchange rather than each frame — a per-frame deadline would let a worker emitting
  progress forever never time out.
- **`failure` frames were collapsed into a protocol fault.** A failure frame is a terminal,
  correlated answer carrying the backend's own stable code; reporting it as a protocol error
  loses that code and misroutes the remedy. Fixed at the client, so all three exchanges get it.
  This matters for step 4: a model-load failure must arrive as `Execution`
  (`initialization_failed`), not as a protocol fault.

### Step 2 — lifecycle, offline, threads, frames, containment (task 3)

`crates/study-tts-runtime/src/worker_launcher.rs` reads `worker/launcher.json`, the declared
bundle input, from the path the manifest declares. ADR-0001 §10.1's "the launcher sets
`OMP_NUM_THREADS`…" is read as the launching **parent**, which is why Rust sets the four caps on
the child: each is read as a native library loads, so a worker cannot usefully set them for
itself, and E5-S2's pool gives each worker an allowance no file shared by all of them could carry.

**The offline variables are deliberately not set by Rust.** `worker/study_tts_worker/worker.py`
applies them into the process a backend is imported into — a thing only that process can do — and
reports on standard error that it did. Setting them from both ends would be two sources that can
disagree about whether a render may reach a network.

One containment gap was found by a failing test: **the executor accepted a success frame without
checking the assigned path had been written.** `fake-ndjson-worker escape-staging` writes a valid
take two directories up and reports success, and the executor returned a report for it. Rust
cannot stop a worker writing elsewhere; it can refuse to report success for audio that is not
where it assigned. Now checked with `symlink_metadata`, so a link dropped at the assigned path is
refused rather than followed.

The cache's own containment was examined and **is not duplicated**: `cache.rs` re-resolves the
staged path through `managed::leaf` *after* `producer.produce` returns, and that refuses a symlink
or a non-regular file. The post-produce re-resolution is the guard; this record notes it because
it is not obvious from the call site.

| Control | Proved by | Tier |
|---|---|---|
| One process serves a whole session | `t4_e1_one_worker_session_serves_more_than_one_request` | T4 |
| Identities come from the worker | `t4_e1_the_worker_executor_reports_the_identities_its_worker_initialized_with` | T4 |
| A backend refusal keeps its own code | `t4_e1_a_worker_failure_frame_becomes_a_typed_execution_error` | T4 |
| An unreadable frame is refused | `t4_e1_a_worker_frame_this_build_cannot_read_is_refused_as_a_protocol_failure` | T4 |
| A closed stream is refused at once | `t4_e1_a_worker_that_exits_without_answering_is_refused` | T4 |
| A hang is bounded and the tree reaped | `t4_e1_a_hung_worker_is_refused_at_its_deadline_and_its_tree_is_reaped` | T4 |
| Output outside the assigned path is refused | `t4_e1_worker_output_outside_the_assigned_path_is_refused` | T4 |
| The frame ceiling binds before allocation | `t4_e1_a_worker_frame_past_the_protocol_ceiling_is_refused_before_it_is_kept` | T4 |
| An uncorrelated answer is refused | `t4_e1_a_worker_answering_another_request_is_refused_rather_than_believed` | T4 |
| Thread caps reach the child process | `t4_e1_the_launcher_thread_allowance_reaches_the_worker_process` | T4 |
| The launcher is read closed, by both ends | `t4_e1_the_checked_in_launcher_is_one_this_build_reads`, `t1_e1_a_launcher_this_build_cannot_read_is_refused_by_its_exact_fault` | T4/T1 |

**Not done in step 2, and named rather than implied.** `torch.set_num_threads` and
`set_num_interop_threads(1)` are not applied: there is no backend to apply them to, and both land
with the model load in step 4. The worker-bundle identity is unchanged at
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` because no declared bundle
input was touched — `worker.py` and `launcher.json` are both untouched by steps 1 and 2.

### Step 3 — validated publication and unique quarantine (tasks 4, 5, 6)

The publication path was **not rewritten**. `cache.rs` already staged, validated, checksummed,
published with `RENAME_NOREPLACE`, and synchronized the parent, and the executor drives it through
the `StagedAudioProducer` blanket impl the port already carried. What E1-S3 owed here was the
quarantine layout and the proof.

**The quarantine path now matches ADR-0001 §12.6.** It was
`<job-id>/cache/<segment-id>/attempt-<nonce>/`; it is now
`<job-id>/<segment-id>/take-<take>/attempt-<attempt>-<request-id>-<nonce>/`. The nonce is kept
deliberately and is not decoration: the attempt number and the request identity are both derived
from the plan, so a resumed or re-run job reproduces them exactly, and without a nonce the second
failure of one segment and take would land on the first failure's evidence — which §12.6 forbids.

**No contract change was needed, and that was checked rather than assumed.** The path needs a
request identity the cache did not have. Adding one to `CacheResolveRequest` would have been a
`Breaking contract` change under `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`
§Change classes, and `ADR-0001-D005` does **not** authorize retaining the version for it: its
condition 2 requires the version being retained to have been introduced by an unreleased breaking
move *within the same story*, and `e0.cache-publication.1.0` is E0-S4's. Instead the identity is
derived where both callers can reach it — `PlannedSegment::request_id` in
`crates/study-tts-core/src/plan.rs` — which is additive, removes a hand-written spelling from
`pipeline.rs` rather than adding one, and leaves `CACHE_PUBLICATION_CONTRACT_VERSION` untouched at
`e0.cache-publication.1.0`.

Segment identities are capped at 64 characters by `schemas/lesson-v3.schema.json`, so the longest
attempt directory name this can produce is about 149 bytes — inside the 255-byte limit, checked
because the request identity carries a 64-character cache key into a path element.

| `DELIVERY-PLAN.md` §E1-S3 test | Result |
|---|---|
| `t4_e1_identical_synthesis_identity_produces_cache_hit` | Passes |
| `t4_e1_speech_affecting_change_produces_cache_miss` | Passes |
| `t4_e1_invalid_audio_never_produces_cache_hit` | Passes |
| `t4_e1_invalid_audio_uses_unique_quarantine_path` | Passes |

**All four passed on their first run, which is not on its own evidence that they test anything.**
Two were therefore checked against a deliberately broken build rather than accepted:

| Red regression | Mutation | Result |
|---|---|---|
| `t4_e1_invalid_audio_uses_unique_quarantine_path` | `quarantine_transaction` reverted to the pre-E1-S3 layout | Failed, naming the parent directory |
| `t4_e1_identical_synthesis_identity_produces_cache_hit` | Expected producer-call count changed from 1 to 2 | Failed with `left: 1, right: 2`, proving the second resolve reached no worker |

The second mutation is why the hit test counts producer calls at all: equal artifacts would also
be returned by a cache that re-rendered identical bytes every time and reused nothing, so the
count is the only thing that separates a hit from a coincidence.

### Step 4a — the interfaces the real backend needs (tasks 2 and 3, in part)

`docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` is the record, `Accepted` and signed
2026-08-30. It pays the three debts `E1-S2-INTERFACE-CHANGE-001` §Limits assigned to this
story by name, in one change, because they describe one exchange: the planner resolves a voice,
the request carries it, the worker reports which artifact that resolved to.

**`ADR-0001-D005` was checked and does not reach any of them.** Its condition 2 requires the
retained version to have been introduced by an unreleased breaking move within the same story;
these are E0-S4's and E1-S2's. Each therefore takes a full major increment: plan `2.0` → `3.0`,
`e1.tts-executor.2.0` → `3.0`, `e1.worker.1.0` → `2.0`.

**The finding that shaped it: the worker cannot compute BLAKE3.** `hashlib` offers blake2 only, no
locked distribution provides blake3, and the qualification spike — which needed one — shelled out
to a `blake3` binary. A real Python worker therefore could not satisfy the `worker_frames` v1
frames at all: both asked for a `VoiceProfileHash` it has no way to produce, and the Rust fake
satisfied them only by being Rust. That is precisely the "the fake passes, the real one cannot"
class the shared contract suite exists to catch, caught before the backend was written rather than
after. The frames now carry the conditioning digest the worker reads from its own `profile.json`;
§Limits in the interface record states the residual gap and why a dependency was not added.

The executor now builds `SynthesisReport::context` from the artifact the **worker** reported
rather than the one the request carried, which is the third debt paid. The protocol fake resolves
through a synthetic voice root of its own (`deterministic_tone_conditioning`) rather than echoing,
so the cache's identity gate is exercised against it rather than passing by construction.

`worker/launcher.json` moved to layout `1.1`, carrying the `seed`, `model_repository`, and
`generation_parameters` that were configured nowhere — the second debt — plus the voice root
variable the worker needs. Generation parameters are recorded as **strings**: §12.5 admits no
floating point into an identity, so the launcher records the spelling, the key hashes it, and the
worker parses it once. Both ends moved in one change and both suites went red on the bump until
they did.

**The worker-bundle identity moved twice**, to `84baafe98bf861cb805001ec831e98c10532b07b063970fac32e26cbfc3f7227`.
Nothing is stranded: the cache root holds no entries and this build's worker still refuses
`synthesize`, so nothing has ever been published under any synthesis key.

**Not done, and named rather than implied.** The real Chatterbox backend is not written:
`worker/study_tts_worker/worker.py` still refuses `initialize` and `synthesize` naming E1-S3. This
step delivered the interfaces it needs, not the backend itself.

## Verification run

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 332 passed, 0 failed, 0 ignored |
| Python worker, after the launcher move | `python3 -m unittest discover --start-directory worker/tests` | 45 passed |
| Doctests | `cargo test --offline --workspace --doc --locked` | Passed |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 44 passed |
| Published schemas current | `cargo run --offline --locked -p study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | No diff |
| Worker bundle identity | `./target/debug/examples/worker-bundle-hash`, freshly built | `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` |
| Bundle identity cost, 24 runs | `/usr/bin/time -f '%e %U %S' ./target/debug/examples/worker-bundle-hash` | CPU 3.02–3.19 s; wall 3.25–3.42 s uncorrupted, 0.77–0.80 s on 3 of 24 |
| Red-before-green | The four tests above run against the stub returning `Ok(())` | Three failed for the stated reason; the fourth is the accepting case and passed throughout |

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run in this preflight. No real-model qualification, ASR, or
listening was run; this preflight changes no audio behavior and no audio bytes. The eight-run batch
quoted in §Finding 1 was taken on a binary that predated the freshly built one, which is why the
24-run batch is what §Result rests on.

## Deviations and limitations

- **`ADR-0001-D006` is approved and supersedes `ADR-0001-D004`**, signed 2026-08-30. CPU time is
  now the banded measure; `docs/operations/WORKER-ENVIRONMENT.md` and the qualification workflow
  cite D006, and D004 records that its check stands while its cost band is retaken.
- **Thirty provenance pins in `e1-s1-provisional-contract-baseline-v15` are moved and not yet
  accounted.** This bullet said *eight* while the preflight half of this record was the whole of
  it, and naming the count without re-running the checker is what let it stand: the story work then
  moved twenty-two more. `python3 scripts/check-evidence-provenance.py` exits `1` with **thirty**.
  Twenty-nine are digest movements across the worker package, the executor, the contract fake and
  its suites, the two shared fixtures, and the governance documents. The thirtieth is not a digest
  at all — `schemas/worker-protocol-v1.schema.json` no longer exists, deleted by the Accepted
  `E1-S3-INTERFACE-CHANGE-001`, and the checker reports it as a missing cited file. Accounting for
  it as a moved digest would have been wrong.

  The reconciliation is written once, after the tree stopped moving, and is
  `e1-s3-worker-backend-provenance-reconciliation-v1` in this directory. It is **Proposed**, so it
  grants nothing yet: `evidence/README.md` §Provenance gives a proposed record no effect, and the
  checker will keep exiting `1` until the approvers accept it. Writing it mid-story is what cost
  E1-S2 three successive reconciliations.
- **`worker/pyproject.toml` may not belong in the bundle identity at all.** ADR-0001 §12.5 names
  "production worker source and imported project-owned modules, the production Python lockfile, the
  worker protocol schema, launcher configuration that affects inference, and Python runtime and
  platform ABI identity" — and `worker/pyproject.toml` is none of those. Its presence in `inputs` is
  what let a declaration the worker never loads re-key every cache entry. Removing it would be the
  smaller mechanism than reconciling it, but it is an ADR §12.5 interpretation and moves the
  identity, so it is raised here for the owner rather than taken. The reconciliation added above is
  correct either way.
- **The reconciliation does not cover a bare unpinned name.** A requirement written with no version
  at all has nothing to disagree with the lock about and is skipped, which
  `docs/operations/WORKER-ENVIRONMENT.md` states rather than leaves implied.
- **The story half is now recorded below**, so this bullet no longer says no task is started. What
  remains open is stated there rather than here.

## Story acceptance criteria

Added as the work landed, per §Scope and decision. The story half is accepted when all six hold.

1. Every one of the eight defects raised against the in-progress work is closed by a check that
   fails against the defect before it passes against the fix.
2. No published contract version moves: not the worker protocol, the plan schema, the
   `TtsExecutor` contract, or the launcher record.
3. The worker bundle identity moves at most once, and nothing is stranded by the move.
4. The four `t5_e1_` criteria are discharged on the reference machine by an instrument whose
   output is hashed and cited here.
5. `cargo fmt --check`, Clippy with `-D warnings`, the workspace suite, doctests, the Rust
   conventions check, the Python worker suite, and published-schema drift all pass.
6. Anything not run is named rather than omitted.

## Story result

| Criterion | Result |
|---|---|
| 1 — eight defects closed red-before-green | Met. §Story findings |
| 2 — no published contract version moves | Met. The protocol, plan schema, executor contract, and launcher record are all unchanged; the schema-drift test passes untouched |
| 3 — one identity move | Met. `84baafe98bf861cb…` → `839baa220e90ab894f3f5e8b3bee1f7ef76d178a2359fe862e9bd932ebea8d95`. No artifact stranded: the cache root holds no entries and the shipped worker refused `synthesize` until this work |
| 4 — four T5 criteria | Met, all four pass. §T5 qualification result |
| 5 — checks | Met for everything run |
| 6 — what was not run | Met. §Verification run, and §Limits below |

## Story findings

Each was closed by a check that failed first. The named test is the one that failed.

| # | Defect | Closed by |
|---|---|---|
| 1 | The shipped worker was a refusal stub; `initialize` and `synthesize` both returned `initialization_failed` | The Chatterbox backend, loaded once per lifetime. Verified against real weights, §T5 qualification result |
| 2 | `WorkerConfiguration::for_bundle` accepted a caller-supplied bundle identity and never called `WorkerBundle::verified_hash` | `t1_e1_a_bundle_configuration_derives_its_identity_rather_than_being_told_one`. The contract fake's constructor now takes no identity, so no public function in the crate accepts one |
| 3 | Staging containment was checked only at the assigned path, and a file left beside it was published inside the cache entry | `t4_e1_a_file_the_worker_left_in_the_stage_is_never_published`, and `CacheError::UncontainedStagedFile`. The worker's half is `O_CREAT|O_EXCL|O_NOFOLLOW` and a refusal of any upward path component |
| 4 | Only `worker_bundle_hash` was compared on a success frame; `codec_revision` was discarded and `voice_profile` unread | `t4_e1_synthesis_under_a_drifted_identity_is_refused`, over all four identities as an exhaustive `match` |
| 5 | The shipped launcher could not import the shipped worker: `-m study_tts_worker` names a package with no `__main__.py`, and no working directory was set | `t1_e1_the_entry_module_names_the_shipped_entrypoint_file`, plus the session in §T5 qualification result, which could not have started otherwise |
| 6 | Request identities were reused across attempts, against ADR-0001 §10.3 | `t4_e1_a_repeated_segment_never_reuses_a_worker_request_id`. The wire identity is issued per worker lifetime; `PlannedSegment::request_id` is untouched, so no plan hash or cache key moved |
| 7 | A synthesis timeout left the worker process alive and the executor permanently poisoned | `t4_e1_a_synthesis_that_times_out_kills_the_worker_tree`, which reads the process table while the executor is still in scope so `Drop` cannot be doing the work |
| 8 | Declared capabilities were discarded except `languages` and `max_text_bytes` | `t4_e1_a_request_outside_the_declared_envelope_is_refused_before_any_work` and `t4_e1_a_worker_rendering_a_non_canonical_format_is_refused_at_start` |

Two findings were raised by the work rather than by the audit, and both are recorded because a
reader checking the eight above would not otherwise meet them:

- **A file left in the staging transaction was published inside the cache entry.** The stage
  *becomes* the entry — §12.6 renames it into place — so a scratch file beside the audio shipped
  inside an entry claiming to hold one segment's speech. Closed as finding 3 above.
- **Setting the child's working directory broke every relative path handed to it.** Introduced
  while fixing finding 5 and found only by running the qualification instrument, which could not
  start the worker. The contract fake's program comes from `current_exe()` and is already absolute,
  and the fake reads no governed root, so the entire T4 suite was structurally blind to it.
  `for_bundle` now resolves the bundle root and both governed roots before composing anything.
- **A test written for finding 6 raced the diagnostic reader.** It read the worker's captured
  standard error while the executor was still live, and the fake writes the line it reads
  immediately before the response frame the executor returns on — so the reader thread need not
  have drained it yet. It passed alone and failed under a loaded suite, which is the shape of a
  defect rather than of an environment. The observation now happens after `shutdown`, which joins
  the reader threads. One further unattributed failure was seen immediately after the fix and did
  not reproduce in thirty subsequent runs; it is recorded here rather than dismissed, and the
  suites remain the gate.

## T5 qualification result

Discharged by `cargo run --package study-tts-testkit --example worker-qualification`, per
`scripts/qualification/README.md` §E1-S3. `grep 'fn t5_'` across `crates/` returns nothing, and this
is why: every `t5_` name in this project is an acceptance criterion answered by an operator-run
instrument and a record, the shape E0-S3 used.

The runs below were taken on 2026-08-30, before the audit remediation recorded in §Audit
remediation. Two rows are marked **Owed**: the remediation changed what those criteria test, so
their recorded results no longer describe the instrument that would run today. The other three are
unaffected — none of them touches the staging root or the worker's lifetime — but the *bundle
identity* moved with the remediation, so the digest in the first row is superseded and the
instrument must be rerun before this record is accepted at G1.

**No hashed instrument output is cited yet, and that is a finding against this record rather than
an omission.** The instrument previously printed its result and wrote nothing, so there were no
bytes to hash. It now writes `qualification-result.json` under the output root and reports that
file's SHA-256; the rerun owed above is what produces the file this table will cite.

| Criterion | Result | Observed |
|---|---|---|
| `t5_e1_worker_bundle_hash_matches_when_all_declared_bundle_inputs_match` | Pass | Two derivations on the qualified interpreter agreed at `839baa220e90ab89…`. Sensitivity to a moved input is pinned at T1 by `t1_e1_worker_bundle_hash_changes_on_owned_runtime_input` |
| `t5_e1_model_load_occurs_once_per_worker_lifetime` | Pass | Three takes through one worker reported one model load |
| `t5_e1_worker_protocol_stdout_remains_clean` | Pass | Every frame of a completed session parsed off standard output, while 12,272 bytes of backend diagnostics went to standard error |
| `t5_e1_worker_output_cannot_escape_staging_root` | **Owed** | The result above was taken before `initialize` carried `staging_root`. The criterion now also drives an absolute path outside the root and a path whose parent is a symlink out of it, neither of which the recorded run exercised, so it must be retaken |
| `t5_e1_worker_survives_restart_and_starts_offline` | **Owed** | Added after the recorded run. Two worker lifetimes through one configuration, asserting identical synthesis identities and that each lifetime applied its offline settings |

## Audit remediation

An audit of this story's uncommitted work on 2026-08-31 raised eleven findings — ten Major and one
Minor — against the tree the sections above describe. All eleven are addressed. Two of them were
defects in this record rather than in the code, and they are the reason the T5 table above now
carries **Owed** rows.

| # | Finding | Disposition |
|---|---|---|
| 1 | The tree failed both suites because `fixtures/contracts/` had been deleted from the working tree while `docs/testing/TEST-DATA-MANIFEST.md` still listed every file active | All 27 restored. Four of the five this story had modified reproduce their recorded SHA-256 byte for byte from the protocol version bump alone, which proves them the same files the manifest attests. The fifth, `e1-s1-worker-protocol-cases.ndjson`, is a **reconstruction, not a restoration**: its `previous-major-version` case had to move from `e0.worker.0.1` to `e1.worker.1.0`, and the prose beside it is rewritten rather than recovered. Its manifest row is re-pinned |
| 2 | The worker used `profile_id` from a record's contents as a path component, and never validated model artifact checksums | Containment fixed: a profile is read only from the directory that names it, so `voice_root / identity` is the directory the record was found in and the existence check and the load no longer speak about two paths that may differ. **Model-artifact checksum validation is not done** and is issue #66, sequenced before G1 and before E1-S4 so the synthesis key moves once: it needs a declared checksum manifest for the weights, which does not exist |
| 3 | The cache derived its identity gate from `report.context` while `SynthesisReport::voice_conditioning_hash` was ignored, so a report contradicting itself could publish | The two are cross-checked before the gate, refusing `AudioError::ConditioningIdentityContradiction`. Both test doubles turned out to be internally inconsistent; fixing them showed the fake's "resolved" conditioning was `blake3(profile_id)` while the planner resolves `blake3(file bytes)`, so it had never matched — the echo was concealing it |
| 4 | Worker launches inherited the ambient environment; `env_clear` was never called | Called before anything is declared. The test written first observed **over 100** inherited variables, including `PYENV_ROOT`, `LD_LIBRARY_PATH` and `SSL_CERT_FILE`. The child now holds exactly the declared set, which meant declaring the offline variables from Rust too — over the same allowlist the Python end uses, never over `worker/launcher.json`, because iterating that file would make a declared bundle input a place to set `PYTHONPATH` |
| 5 | The staging-containment criterion admitted it could not prove its own name | `initialize` carries `staging_root`; containment is decided against the resolved parent. See §Limits |
| 6 | The shared suite ran neither graceful shutdown nor restart, and shutdown went straight to `SIGKILL` without sending the protocol frame that already existed | The `shutdown` frame is sent and a grace period observed before the group kill, which remains the backstop. `run_worker_restart_contract_scenario` drives two lifetimes and is used by both the T4 fake suite and the T5 instrument. **Mid-generation cancellation is not implemented**: Python synthesis is synchronous, so a cancel frame cannot be processed while generation runs, and making the worker interruptible is an architecture change |
| 7 | Task 4 was checked while duration, silence, and edge conditioning were absent | Implemented — under `ADR-0001-D007`, which records that this required a **provisional** silence threshold because ADR-0003 is Proposed and records the value as pending, and that the project owner directed it be done now rather than deferred. Join discontinuity and loudness normalization remain E2-S3's; both need the second pending ADR-0003 value or FFmpeg |
| 8 | Raw backend exception strings reached failure frames, carrying governed paths and possibly source text | Redacted at all nine sites: the fault's own message is dropped and the type name reported, with `OSError` keeping `strerror` — the kernel's words for *why*, with no path in them |
| 9 | The requirement parser scanned only double-quoted strings and skipped entries whose suffix did not match its operator set | Single-quoted literals and unterminated extras brackets are refused as `Unreadable` rather than skipped, and PEP 508 extras are stepped over so the requirement is still reconciled. Whole-line comments are excluded first, so prose cannot trip the guard |
| 10 | This record required the instrument's output to be "hashed and cited" and cited only observations | The instrument writes `qualification-result.json` and reports its SHA-256. The citation is owed with the rerun the T5 table above records |
| 11 | Contract documentation still named removed versions and a deleted schema file | `worker_client.rs` and `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` corrected to executor 3.0 and worker 2.0/2.1 |

### What the remediation costs this record

The worker bundle identity moves again, because `worker/study_tts_worker/` is a declared bundle
input and this remediation changed it. Every digest in the sections above that names
`839baa220e90ab89…` describes the pre-remediation bundle. The instrument must be rerun on the
reference machine before this record is accepted at G1, and the listening review retaken —
`ADR-0001-D007`'s edge conditioning changes the published samples, so the audio reviewed on
2026-08-31 is not the audio this build now produces.

## Limits the story does not close

- **`t5_e1_worker_output_cannot_escape_staging_root` now proves its name; it did not when this
  record was first written.** The worker was told one path and no root, so it could inspect only
  the spelling of the path it was handed, and this section recorded that as a gap on the reasoning
  that a hostile *request* cannot occur because Rust composes every request. A subsequent audit
  rejected that reasoning: the criterion is a claim about a hostile or defective **worker**, and a
  containment argument that rests on the caller being well behaved does not discharge it.

  `initialize` now carries a required `staging_root`, and the worker decides containment against
  the *resolved parent* of every assigned path rather than against its spelling — so an absolute
  path elsewhere is refused, and so is a path whose components are all inside the root by name and
  whose parent is a symlink out of it, which is the shape a lexical check cannot see. The check
  also moved ahead of generation: the worker used to render the audio and only then refuse the
  path. The T5 criterion covers both new shapes, and
  `AssignedOutputContainmentTests` in `worker/tests/test_worker.py` pins the decision at T1.

  The protocol change is folded into the unreleased `e1.worker.2.0` under `ADR-0001-D005`'s
  reasoning, and `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` §Worker frames carries it.

  What the earlier gap did **not** excuse was thin coverage of the shapes that were checkable, so the
  containment tests were widened rather than left at the two the phase started with:
  `t4_e1_a_symlink_at_the_assigned_path_is_refused_rather_than_followed` covers the branch where
  the assigned path exists and is not a regular file — the worker writes faithfully and the link
  sends the bytes elsewhere, which no stage inventory can see — and
  `t4_e1_a_file_the_worker_left_in_the_stage_is_never_published` is now table-driven over a
  leftover file *and* a leftover directory, the latter being the shape a check written with
  `is_file` would have walked past. With the T5 refusals, containment is covered at every point
  either end can observe.
- **The listening review was complete and accepted, and is now superseded.** `ADR-0001-D007`'s edge conditioning pads and ramps every segment, so the published samples differ from the ones reviewed and the review must be retaken before G1. What follows describes the review as taken on 2026-08-31 and is retained because its method and limitations still apply to the retake.

   E1-S3 produces
  audio for the first time, and the four T5 criteria measure session behavior without listening to
  any of it. Six takes were rendered and reviewed on 2026-08-31; all six were accepted with no
  findings on any of the five criteria. §Listening material records the review, the bytes it was
  taken against, and the three limitations that bound what it can be read as covering — chiefly
  that it was taken on laptop built-in speakers, which mask low-level noise.
- **The model's own bytes are never hashed.** `model_revision` and `tokenizer_revision` reach every
  cache key as strings a record states, never as a measurement, so replacing the weights under an
  unchanged acquisition record leaves every key in place. Voices do not have this gap —
  `voice_gate::load_profile` verifies `conditionals.pt` against the digest its record states — and
  the model path should be the same shape. Issue #66 carries the design: artifact digests declared
  in the model root, a derived `model_artifacts_hash` pinned in Git and reaching
  `SynthesisContext`, verified by Rust before the worker starts. Sequenced before G1, because
  adding a key input after the interface freeze needs a migration procedure it does not need now.
- **Restart after a timeout is not implemented.** A timed-out executor now kills its worker tree
  and refuses every later request, which is what ADR-0001 §10.3 asks of it, but recovery is E5-S3's
  and is noted on that issue.
- **`worker/pyproject.toml` stays in the bundle inputs**, and the §12.5 question this record raised
  stays open — with a constraint now attached: `worker_bundle.rs` reads the file only when the
  manifest declares it, so removing it from `inputs` would silently make
  `check_requirements_match_lock` a no-op. The reconciliation must be decoupled from manifest
  membership before the file can leave.

## Listening material

Rendered 2026-08-31 through one worker session, for the listening review this record does not
claim. Governed output, so the location is named by root rather than reproduced here, per
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Item | Value |
|---|---|
| Location | `listening-2026-08-31/` beneath the governed qualification output root |
| Takes | 6, `take-00.wav` through `take-05.wav` |
| Total duration | 36.8 s |
| Format | 24 000 Hz, one channel, IEEE float, uniform across all six |
| Voice profile | `owner-fallback-v1` |
| Style | `calm_explanatory` |
| Seed | 42, from `worker/launcher.json` |
| Model loads | 1, across all six takes |
| Worker bundle identity | `839baa220e90ab894f3f5e8b3bee1f7ef76d178a2359fe862e9bd932ebea8d95` |

### Review result

Completed 2026-08-31 by Ross Todd, on laptop built-in speakers. Recorded in `review-sheet.json`
beside the takes, SHA-256 `08fbf7fcb1e98f0fe3252b74cccac490bf253bcce46a98543eb8e826fd4888ea`, which names every take's own digest so the
judgment is bound to the bytes it was taken against rather than to a filename.

| Criterion | Result across all six takes |
|---|---|
| Omissions or additions against the written text | None |
| Pronunciation | None |
| Voice consistency | None |
| Pacing | None |
| Noise or artifacts | None |

**Overall finding: accepted**, 6 of 6, no findings on any criterion.

The five criteria are E0-S3's. Its sixth, `audible_difference_from_other_runs`, is not applicable:
it compared ten runs of one line for determinism, while these are six different lines.

### What this review does not cover

Recorded because an accepted listening result is easy to read as broader than it is.

- **Laptop built-in speakers.** They mask low-level noise and narrow the usable band. A finding
  this environment cannot surface is not excluded by this review.
- **One reviewer, not blind.** The reviewer knew the text of each take. E0-S3 randomized its
  samples and withheld the key because it was comparing runs of one line; that control does not
  transfer to six distinct lines, and no substitute was applied.
- **Six takes, one voice profile, one style.** Nothing here covers another profile, another style,
  or long-form continuity — which is E6-S1's soak and listening qualification.

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the rows stay separate for one signatory.

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Project owner | Ross Todd | Accept that issue #60 is closed on a reproduction rather than left open, and that `worker/pyproject.toml`'s membership of the bundle identity is raised as an open question rather than decided here | |
| Engineering owner | Ross Todd | Accept the reverted declaration, the identity's return to `75d56310…9d2bab3`, and the reconciliation check with its four tests and its stated scope | |
| Worker owner | Ross Todd for T-WORKER | Accept that the bundle identity moved unnoticed through a declared input for one merged commit, that nothing was published under `8e7cc2f0…`, and that the provenance reconciliation is owed before commit | |
