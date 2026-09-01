# E1-S3 Single-Worker Synthesis and Validated Cache v1

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: opened on the working tree of `fix/issue-59-retired-grant` at the E1-S3
  governance preflight; the candidate it now describes is `story/e1-s3-single-worker-cache` at
  worker bundle identity `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`, after
  five rounds of audit remediation
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
the worker as it stood at this finding refused `synthesize` with `initialization_failed` naming
E1-S3, so no audio had ever been published under any synthesis key.

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
`e0.cache-publication.1.0` for that path-only correction. The later publication and reuse semantic
changes recorded under §Conditioning recorded, and checked on reuse supersede that version with
`e0.cache-publication.2.0`; the Rust seam's shapes remain unchanged.

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
Nothing is stranded: the cache root held no entries and the worker still refused `synthesize`, so
nothing had been published under any synthesis key.

**Not done at this step, and named rather than implied.** The real Chatterbox backend was not
written yet: `worker/study_tts_worker/worker.py` still refused `initialize` and `synthesize` naming
E1-S3. This step delivered the interfaces it needs, not the backend itself, which landed
later in the story: §T5 qualification result and §Listening material are taken against it.

## Verification run

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 332 passed, 0 failed, 0 ignored |
| Python worker, after the launcher move | `python3 -m unittest discover --start-directory worker/tests` | 45 passed |
| Doctests | `cargo test --offline --workspace --doc --locked` | Passed |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
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

Added as the work landed, per §Scope and decision. The story half is accepted when all seven
hold. **Restated 2026-08-31.** As first written this said six, while §Story result had answered
seven since the listening review was taken: criterion 7 below is that seventh, written down rather
than left implied by the row that answers it. The fifth audit's fourth finding is this.

1. Every one of the eight defects raised against the in-progress work is closed by a check that
   fails against the defect before it passes against the fix.
2. No published contract version moves *without an accepted interface-change record*: the
   worker protocol, the plan schema, the `TtsExecutor` contract, and the launcher record each
   move only under one. **Restated 2026-08-31.** As first written this said no version moves at
   all, which was true of the governance preflight it was written for and false of the story:
   `E1-S3-INTERFACE-CHANGE-001` moved three of the four deliberately. The criterion now says what
   it was protecting — that no version moves unrecorded — rather than a stronger thing the story
   never intended.
3. The worker bundle identity moves only for a change to a declared bundle input, and nothing is
   stranded by any move. **Restated 2026-08-31.** As first written this said *at most once*. The
   audit remediation changed `worker/study_tts_worker/`, a declared input, so a second move was
   not optional — a criterion that forbade it would have been asking the identity to lie. What
   the criterion was protecting is that the identity never moves for anything else, and that is
   what it now says.
4. The five reference-machine criteria are discharged by an instrument whose output is hashed and
   cited here. **Restated 2026-08-31.** As first written this said *four*, which is how many
   `t5_e1_` names `DELIVERY-PLAN.md` carries — but the instrument runs and reports five, the fifth
   being `t5_e1_worker_survives_restart_and_starts_offline`, a helper criterion covering ADR-0001
   §17.7's restart and offline requirements that the plan never named. §T5 qualification result and
   `scripts/qualification/README.md` both say so; the criterion did not, so it counted a different
   number from the result answering it. The fifth audit's fourth finding is this.
5. `cargo fmt --check`, Clippy with `-D warnings`, the workspace suite, doctests, the Rust
   conventions check, the Python worker suite, and published-schema drift all pass.
6. Anything not run is named rather than omitted.
7. A human listening review of published audio is taken, bound to the bytes it was made against,
   and its limits stated. **Added 2026-08-31**, for the reason the preamble gives: ADR-0001 §17.5
   makes it a gate condition and §Story result has answered it since it was taken, so the criteria
   list was the half that was missing.

## Story result

| Criterion | Result |
|---|---|
| 1 — eight defects closed red-before-green | Met. §Story findings |
| 2 — no unrecorded contract version move | Met under the restated criterion. The worker protocol moved to `e1.worker.2.0`, the plan schema to v3, and the `TtsExecutor` contract to 3.0, all under `E1-S3-INTERFACE-CHANGE-001`; the audit remediation folded `staging_root` into the same unreleased 2.0 under `ADR-0001-D005`. The launcher record is unchanged. The schema-drift test passes against the moved shapes |
| 3 — identity moves only for a declared input | Met under the restated criterion. `84baafe98bf861cb…` → `839baa220e90ab89…` during the story, then → `7b065eeb5319c6bc…` under the first audit remediation, → `6a158816945dd7d6…` under the second, → `d66e84e4512e2249976523f2ce6a0acaecb7fa6a6494d2aba19b2e4081de37af` under the third, and → `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` under the fourth. Every move follows a change to `worker/study_tts_worker/`, a declared bundle input — the fourth one to a docstring in it, which is the case `e1-s3-protocol-docstring-identity-reconciliation-v1` reconciles. Nothing is stranded by either: the cache root holds no entries, and the shipped worker refused `synthesize` until this work |
| 4 — five reference-machine criteria | Met. All five pass on the reference machine against real weights, in a loopback-only network namespace, and the instrument's output is hashed and cited as `e1-s3-qualification-result-v1.json`. §T5 qualification result |
| 5 — checks | Met for everything run |
| 6 — what was not run | Met. §Verification run, and §Limits below |
| 7 — the listening review | **Pending.** The 2026-08-31 review remains valid for its recorded bytes, but those bytes contain one inserted zero at each quiet edge. The current conditioner instead zeroes the measured quiet edge without adding frames, so a new render and review are required. §Historical review result |

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

Rerun on the reference machine on 2026-08-31, **after the fourth audit remediation**, against the
real model and the governed voice root, **inside a loopback-only network namespace**. Every earlier
run is superseded: the bundle identity moved four times across the remediation rounds, three
criteria changed what they test, and a fifth was added.

**This run is the record's only T5 result.** Earlier revisions of this section described a run and
then said elsewhere that the instrument still had to be re-run, which the second audit raised as
its own finding: a reader could not tell which statement was current. There is now one statement,
here, and nothing later contradicts it.

**The instrument's output is hashed and cited, which it was not before.** It previously printed a
result and wrote nothing, so there were no bytes to hash — the tenth finding of the first audit.
The raw result is filed beside this record:

| Artifact | Value |
|---|---|
| `evidence/gates/g1/e1-s3/e1-s3-qualification-result-v1.json` | SHA-256 `03588f082735ee3e2ab706fd1dd7a8e5410510b1b79549e47964611709214ee1` |
| Worker bundle identity | `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` |
| Criteria | 5 of 5 pass |
| Network namespace | interfaces `["lo"]`, 0 IP routes, `/proc/self/ns/net` inode 4026532310 |

The namespace is not context: the instrument **refuses to start** without it, so a
`qualification-result.json` existing at all is one whose egress was denied. The interfaces, the
route count, and the namespace inode are recorded in the artifact, so this table can be checked
against the bytes rather than believed.

It carries no path and no governed location, which is why it can be committed: every `observed`
string names counts, criteria, and digests only.

| Criterion | Result | Observed |
|---|---|---|
| `t5_e1_worker_bundle_hash_matches_when_all_declared_bundle_inputs_match` | Pass, re-derived | Two derivations on the qualified interpreter agreed at `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` after the fourth remediation, superseding `d66e84e4512e2249…`, `6a158816945dd7d6…`, `7b065eeb5319c6bc…` and, before them, `839baa220e90ab89…`. Derived with `cargo run --package study-tts-runtime --example worker-bundle-hash`, which needs the bundle root and the qualified interpreter but no governed root. Sensitivity to a moved input is pinned at T1 by `t1_e1_worker_bundle_hash_changes_on_owned_runtime_input` |
| `t5_e1_model_load_occurs_once_per_worker_lifetime` | Pass | 3 takes through one worker reported 1 model load |
| `t5_e1_worker_protocol_stdout_remains_clean` | Pass | Every frame of a completed session parsed off standard output **and nothing followed the last one**, while 30,618 bytes of backend diagnostics went to standard error. The second clause is new and is the point: the criterion used to be recorded while the session was still open, and nothing read the response stream after the final request — so a worker could write anything past its last frame, an unterminated tail most of all, and still pass. `shutdown` now drains the stream and refuses anything but the shutdown response, and the criterion is recorded from that result |
| `t5_e1_worker_output_cannot_escape_staging_root` | Pass | A contained take wrote only its assigned path; **five** shapes refused — a symlink planted at the assigned path, a path that walks upward, a path that already exists, **an absolute path outside the staging root**, and **a path whose parent is a symlink out of the root**; zero files outside the staging root. The last two are the shapes the pre-remediation worker could not refuse, because it was told a path and no root |
| `t5_e1_worker_survives_restart_and_starts_offline` | Pass | Two worker lifetimes through one configuration reported identical synthesis identities, both applied their offline settings, **and both ran inside a namespace holding only `lo` with no IP route**. The last clause is new and is what the criterion's name always claimed: it previously passed on the worker's own diagnostics alone, which prove it configured `huggingface_hub` and `transformers` and prove nothing about the backend, a transitive dependency, or a socket. Not a `DELIVERY-PLAN.md` name: a helper criterion covering ADR-0001 §17.7's restart and offline requirements, driven by the same `run_worker_restart_contract_scenario` the T4 suite drives the protocol fake through |

## Audit remediation

### First audit — eleven findings

An audit of this story's uncommitted work on 2026-08-31 raised eleven findings — ten Major and one
Minor — against the tree the sections above describe. All eleven are addressed. Two of them were
defects in this record rather than in the code.

| # | Finding | Disposition |
|---|---|---|
| 1 | The tree failed both suites because `fixtures/contracts/` had been deleted from the working tree while `docs/testing/TEST-DATA-MANIFEST.md` still listed every file active | All 27 restored. Four of the five this story had modified reproduce their recorded SHA-256 byte for byte from the protocol version bump alone, which proves them the same files the manifest attests. The fifth, `e1-s1-worker-protocol-cases.ndjson`, is a **reconstruction, not a restoration**: its `previous-major-version` case had to move from `e0.worker.0.1` to `e1.worker.1.0`, and the prose beside it is rewritten rather than recovered. Its manifest row is re-pinned |
| 2 | The worker used `profile_id` from a record's contents as a path component, and never validated model artifact checksums | Containment fixed: a profile is read only from the directory that names it, so `voice_root / identity` is the directory the record was found in and the existence check and the load no longer speak about two paths that may differ. **Model-artifact checksum validation is not done** and is issue #66, sequenced before G1 and before E1-S4 so the synthesis key moves once: it needs a declared checksum manifest for the weights, which does not exist. **Closed by the third remediation below**: `model_gate` declares the four artifacts and their SHA-256 digests in Git and verifies them in `for_bundle`, and the premise that no manifest exists turned out to be stale |
| 3 | The cache derived its identity gate from `report.context` while `SynthesisReport::voice_conditioning_hash` was ignored, so a report contradicting itself could publish | The two are cross-checked before the gate, refusing `AudioError::ConditioningIdentityContradiction`. Both test doubles turned out to be internally inconsistent; fixing them showed the fake's "resolved" conditioning was `blake3(profile_id)` while the planner resolves `blake3(file bytes)`, so it had never matched — the echo was concealing it |
| 4 | Worker launches inherited the ambient environment; `env_clear` was never called | Called before anything is declared. The test written first observed **over 100** inherited variables, including `PYENV_ROOT`, `LD_LIBRARY_PATH` and `SSL_CERT_FILE`. The child now holds exactly the declared set, which meant declaring the offline variables from Rust too — over the same allowlist the Python end uses, never over `worker/launcher.json`, because iterating that file would make a declared bundle input a place to set `PYTHONPATH` |
| 5 | The staging-containment criterion admitted it could not prove its own name | `initialize` carries `staging_root`; containment is decided against the resolved parent. See §Limits |
| 6 | The shared suite ran neither graceful shutdown nor restart, and shutdown went straight to `SIGKILL` without sending the protocol frame that already existed | The `shutdown` frame is sent and a grace period observed before the group kill, which remains the backstop. `run_worker_restart_contract_scenario` drives two lifetimes and is used by both the T4 fake suite and the T5 instrument. **Mid-generation cancellation is not implemented**: Python synthesis is synchronous, so a cancel frame cannot be processed while generation runs, and making the worker interruptible is an architecture change |
| 7 | Task 4 was checked while duration, silence, and edge conditioning were absent | Implemented — under `ADR-0001-D007`, which records that this required a **provisional** silence threshold because ADR-0003 is Proposed and records the value as pending, and that the project owner directed it be done now rather than deferred. Join discontinuity and loudness normalization remain E2-S3's; both need the second pending ADR-0003 value or FFmpeg. **A later audit found the ramp half of this inert** — see §The ramp correction |
| 8 | Raw backend exception strings reached failure frames, carrying governed paths and possibly source text | Redacted at all nine sites: the fault's own message is dropped and the type name reported, with `OSError` keeping `strerror` — the kernel's words for *why*, with no path in them |
| 9 | The requirement parser scanned only double-quoted strings and skipped entries whose suffix did not match its operator set | Single-quoted literals and unterminated extras brackets are refused as `Unreadable` rather than skipped, and PEP 508 extras are stepped over so the requirement is still reconciled. Whole-line comments are excluded first, so prose cannot trip the guard |
| 10 | This record required the instrument's output to be "hashed and cited" and cited only observations | The instrument writes `qualification-result.json` and reports its SHA-256. The citation is owed with the rerun the T5 table above records |
| 11 | Contract documentation still named removed versions and a deleted schema file | `worker_client.rs` and `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` corrected to executor 3.0 and worker 2.0/2.1 |

### The ramp correction

A later audit found that the edge conditioning row 7 reports as implemented had never smoothed
anything. `condition_edges` inserted the zero padding and then applied the raised-cosine gain to
samples *inside that padding*. Scaling zeros is arithmetically inert, so a segment whose signal
began at `0.5` still stepped `0.0` to `0.5` at its onset — the discontinuity the ramp exists to
remove survived every build that reported the ramp applied.

Its test agreed with it. `t1_e2_ramp_never_extends_into_speech` asserted that every speech sample
came through unchanged, which held precisely *because* nothing had been smoothed. The test
confirmed the defect instead of detecting it, and the module's doc comment attributed the rule it
was enforcing — "without entering speech" — to ADR-0001 §13.4, which does not contain that phrase
and requires the opposite.

**The two governing sentences were in genuine conflict**, and it was resolved in the wrong
direction without being flagged:

| Document | Rule |
|---|---|
| ADR-0001 §13.4 | "smooth each silence-to-signal transition with a raised-cosine ramp no longer than 5 ms" |
| `DELIVERY-PLAN.md` E1-S3 task 3, as written | "without entering speech" |

Both cannot hold: after padding, the silence side is exactly zero, so smoothing requires
attenuating signal. Under `CLAUDE.md` §Conflict order ADR-0001 prevails, and the project owner so
directed. The ramp now covers the first and last 5 ms of *signal*, capped at half the signal so two
ramps on a short segment abut rather than overlap.
`t1_e2_ramp_smooths_the_silence_to_signal_transition` replaces the test that confirmed the bug and
was checked to fail against the old placement before being accepted as passing.
`DELIVERY-PLAN.md` carries ADR-0001's wording, and
`e1-s3-delivery-plan-ramp-correction-reconciliation-v1` accounts for the digest that moved.

One further defect was corrected on the same path: the partial-frame branch of
`leading_silent_samples` had no length bound, though its own comment described it as measuring "a
remainder shorter than one frame". Unbounded, a quiet burst followed by a second of silence
averaged below the threshold, the segment was classified wholly silent, and conditioning returned
before ramping any real signal.

`ADR-0001-D007` is not edited. Its condition 2 states the ramp geometry "is implemented as
ratified"; that claim was false when signed and is true now.

### The publication ceiling correction

A further audit found the ten-minute segment ceiling applied to the wrong bytes. `validate_wav`
holds the worker's WAV to `MAX_SEGMENT_AUDIO_MS`; `condition_staged_audio` then adds up to 10 ms
of zero padding at each exposed edge and converted the new length with `u32::try_from` alone.
Nothing re-applied the ceiling, so audio arriving at exactly the limit was published up to 20 ms
over it.

**The published entry was stranded, not merely wrong.** Every quarantine path in
`synthesize_transaction` sits above the rename, so once `publish_directory_noreplace` had moved the
stage into place there was nothing left to collect it. `load_entry` on the next line re-validated
the entry this build had just written and refused it `AudioFault::TooLong`; `resolve` would find
the same entry first on every later run and refuse it again, and this module's own doctrine is that
"a corrupt published entry is refused rather than repaired, because repair would hide tampering".
That cache key was unbuildable until a person deleted the directory by hand.

Reserving headroom in `validate_wav` was rejected: that function is shared with the reuse path, so
an entry legitimately holding up to the full ceiling would have started being refused, and every
segment's ceiling would have narrowed including those whose edges are already silent. The ceiling
means what `audio_edges.rs` says — the longest audio this build will condition **or publish** — so
audio that cannot be conditioned within it is refused before publication, where quarantine still
reaches it.

| Decision | What was chosen, and why |
|---|---|
| Which fault | A distinct `AudioFault::ConditionedTooLong` carrying **both** counts. Reusing `TooLong` would have reported 14,400,480 against a file carrying 14,400,000 and read as "the worker wrote too much", sending the operator to the wrong component |
| When it runs | Before the conditioned samples are written back, so the quarantined stage holds exactly what the worker produced and the count in the message is one the operator can measure |
| How it is proven | Both a T1 table over `check_segment_ceiling` and a T4 test at the real ceiling. Only the T4 test proves the wiring, which is the half this defect got through |

The ceiling arithmetic `validate_wav` computed inline is now `max_segment_frames`, shared by both
places the ceiling applies so one rule cannot drift into two.

Both tests were checked red before the fix, against the reverted check alone:
`t1_e1_conditioning_may_not_carry_a_segment_past_the_audio_ceiling` failed "one frame past it:
published a count past the ceiling", and
`t4_e1_at_limit_audio_is_refused_rather_than_conditioned_over_the_ceiling` failed "conditioning
past the ceiling must be refused: 14400480" — the defect itself, reproduced. The T4 test writes a
57.6 MB WAV, which is what the ceiling is; **measured at 2.5-3.5 s** against `TEST-STRATEGY.md`'s
five-minute T4 budget.

`docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings said the constant bounds
"what one segment may hand the edge conditioner". That was the narrower reading the code had been
enforcing. Both halves of the mirror now state that it bounds conditioning's output as well, and
the document names `cache.rs` beside `audio_edges.rs` as the enforcing path.

No published byte moves: this refuses a case that was previously published, so nothing already in a
cache or a listening set changes. No provenance moves either — `cache.rs` and
`WALKING-SKELETON.md` are both already accounted by the accepted
`e1-s3-worker-backend-provenance-reconciliation-v1`, whose suppression is by citing-record and path
rather than by digest, and `crates/study-tts-runtime/src/error/audio.rs` is pinned by no record.

### Conditioning recorded, and checked on reuse

A further audit found ADR-0001's edge conditioning applied at publication and then forgotten, in
two halves that had to be closed together.

**Nothing was recorded.** ADR-0001 requires the padding and ramp sample counts twice — §11.1 and
§13.4, "It records the padding and ramp sample counts". `condition_staged_audio` received the
`EdgeConditioning` the conditioner returned and used it only to decide whether to rewrite the
file; `CacheArtifact` had no field for it. `EdgeConditioning`'s own doc comment already stated the
requirement — "recorded rather than merely applied, so a reviewer can tell audio that needed no
work from audio that was rebuilt at both ends" — which made it a one-sided claim with nothing
behind it, the same shape as the misattributed ramp comment above.

**Nothing was checked on reuse.** §12.6 conditions *using* an entry on "duration, silence, edge,
`max(abs(sample)) <= 1.0`, and finite-sample checks". `load_validated` ran `validate_wav`
(duration, range, finite) and `check_exposed_endpoints` (two samples). The **silence** check did
not exist, so an entry whose first and last samples happened to be zero was a cache hit however
its edges were shaped. Reproduced before the fix: audio of a constant 0.25 with only its two
endpoints zeroed, re-pinned so the record stayed self-consistent, came back from `load_validated`
as a `ValidatedCachedArtifact`.

`artifact.json` now carries a required `edge_conditioning` object: the four counts ADR-0001 names,
plus the `calibration_source` they were produced under. The extra field is deliberate — the counts
describe a silence threshold, and this build's is provisional under `ADR-0001-D007`, so without it
an entry conditioned against the provisional value cannot be told from one conditioned against the
value ADR-0003 will freeze. Recording the enum and not the RMS keeps a float out of a durable
record.

**What is verified and what cannot be reconstructed**, stated because those are different claims:

| Property | On reuse |
|---|---|
| ≥10 ms edge silence | Re-measured through the same `measure_edge_silence` the conditioner pads from. Real force |
| Exposed endpoints exactly zero | Checked before this work and still checked |
| Raised-cosine ramp metadata | Checked against the ratified ≤5 ms bound, symmetry, and minimum and maximum counts derived from the decoded audio's frame count and nonzero span. The arbitrary pre-ramp waveform itself cannot be reconstructed after multiplication by the gain |

The silence is measured against the threshold rather than tested for exact zero: conditioning pads
only until an edge *has* its 10 ms, so audio that already began quiet-but-nonzero is lawfully
unpadded and an exact-zero test would refuse it.

Four refusals were added and each was proven by reverting its own check alone, with the entry
then returning as a cache hit: `AudioFault::InsufficientEdgeSilence`,
`CacheEntryFault::ConditioningOutsideRatifiedGeometry`, and
`CacheEntryFault::ConditionedUnderAnotherCalibration`, plus
`CacheEntryFault::ConditioningInconsistentWithAudio`. The first three are rows in the existing
`t1_e0_every_rejection_names_the_entry_directory_and_the_remedy` table. The fourth is pinned by
`t4_e1_conditioning_metadata_detached_from_audio_is_refused`, which edits only the recorded ramp
and proves that metadata no longer survives as a detached declaration.
`t1_e1_a_published_entry_records_what_conditioning_did` drives real publication through `resolve`
rather than the fixture helper — an earlier draft used the helper, passed immediately, and was
testing the fixture rather than the code.

`validate_wav` now returns the decoded samples it already validated. `load_validated` derives the
frame count from that buffer and reuses it for endpoint, silence, and conditioning checks instead
of reopening and decoding the WAV. `condition_staged_audio` consumes the same validated samples on
the publication path, so neither flow has a second sample-decoding implementation.

**This moves the synthesis identity, against criterion 3 above.** A required field is a Breaking
contract, so `CACHE_SCHEMA_VERSION` took a major increment to `2.0`; it is an ADR-0001 §12.5 key
input, so every cache key and plan hash moved with it —
`t1_e0_plan_is_stable_for_identical_inputs` re-pins the plan hash to `46bf2c57d31eb5cf…` and the
two cache keys to `01ffb5593c2e0daa…` and `d4248913a9a39a2e…`. **The move is lawful under that
criterion**: `cache_schema_version` is a declared key input, not an undeclared one. Nothing is
stranded — the cache root holds no entries, and no fixture or evidence record carries a derived
cache key. `docs/architecture/E1-S3-INTERFACE-CHANGE-002.md` records the move and is `Accepted`;
`E1-S3-INTERFACE-CHANGE-001` is Accepted and was not edited, and 002 states that it supersedes the
`abd889db…` plan hash 001 cites.

Two smaller corrections travelled with it. `docs/INDEX.md` described
`E1-S3-INTERFACE-CHANGE-001` as `Proposed` where that record declares itself Accepted and signed
on 2026-08-30; the index is corrected. And `t2_e1_every_speech_affecting_field_changes_synthesis_key`
does **not** cover `cache_schema_version` and cannot: it destructures `SynthesisContext`, and the
constant is not a context field. The golden in `plan.rs` is what catches a move in it, and it did.

The metadata checks themselves do not alter published audio. The later quiet-edge normalization
correction does alter conditioned bytes and supersedes the 2026-08-31 listening set, as recorded
below.

### The listening instrument reviewed audio no build publishes

A further audit found that `crates/study-tts-testkit/examples/listening-render.rs` — the
instrument this record names as the remedy for the superseded 2026-08-31 review — did not review
what the cache publishes, and did not pass the gate a build passes. Both halves confirmed.

**It reviewed unconditioned audio.** The instrument called the executor contract directly, wrote
the worker's raw WAV, and blinded a copy of it. `condition_edges` runs only inside `cache::resolve`,
reached through `CachePublisher`, which that path never invoked. So the retake this record offers
as the answer to "the published samples differ from the ones reviewed" produced samples that were
never conditioned either. **The 2026-08-31 set is stale twice over**, and the sentence above
naming the instrument as the remedy was true only of the words, not of the audio.

**It bypassed the rights gate.** `governed_voice` read `profile.json` with `serde_json::Value` and
lifted `profile_id` and `conditionals_blake3` straight out: no consent status, no rights decision,
no permitted-use scope, and neither `conditionals.pt` nor `reference.wav` hashed. Its own doc
comment called the record "the same record `voice_gate::load_profile` verifies `conditionals.pt`
against before any synthesis runs" — true of the production path and false of that one. It is the
fourth comment in this audit series claiming a control the tree did not implement.

**The same bypass was in `worker-qualification.rs`**, hand-rolled the same way with the same
comment. The audit named only the listening instrument; the project owner directed both be fixed,
so a known-identical bypass is not left standing in an instrument whose output this record cites.

`resolve_voice_conditioning` is now public on `study-tts-runtime` — the existing private
`load_conditioning`, unchanged — and both instruments call it. `resolve_speakers` stays
crate-private: it takes a `ValidatedLesson`, and neither instrument has one. The listening render
now composes what `pipeline.rs` composes for a real build — the backend's descriptor, the script's
language, the gate's conditioning hash — derives each segment's cache key from it, and publishes
through `FileSystemCachePublisher`, blinding the published entry. The cache's identity gate stays a
real comparison: the planned key comes from a record read off disk, the reported one from what the
worker says it loaded.

**`VoiceUse::VoiceQualification` had never been requested anywhere in the tree.** The variant was
written for exactly these instruments — "Model, hardware, or voice qualification runs that never
reach a lesson" — and `grep` found no call site, because both instruments bypassed the gate that
takes it. Both now request it.

Three checks, each proven by reverting it alone:

| Test | Failure against the unfixed instrument |
|---|---|
| `t1_e1_the_reviewed_audio_is_the_conditioned_audio_the_cache_publishes` | "carries 1 leading and 0 trailing zero samples", against the 240 each exposed edge requires |
| `t1_e1_a_revoked_consent_refuses_the_render_before_any_synthesis` | rendered instead of refusing; `synthesis_count()` is asserted zero, so the gate's *ordering* is what is pinned |
| `t1_e1_a_revoked_consent_refuses_the_governed_voice` and `t1_e1_a_voice_outside_the_qualification_scope_is_refused` | the qualification instrument resolved a revoked voice, and one outside its scope |

They run offline against `FakeTtsExecutor`, a real `FileSystemCachePublisher`, and a synthetic
rights-clean profile. `VoiceProfileFixtureSpec` gained a `permitted_use` field so a fixture declares
the scope it admits; the default stays `private_synthesis`, so no existing test changes meaning.

**One thing was owed here and has since been done**: the qualification instrument was re-run on
the reference machine and its `qualification-result.json` digest cited. §T5 qualification result
carries the result, and it is this record's only one.

**One consequence stands.** If the governed `consent.json` does not list `voice_qualification` in
`permitted_use`, both instruments now refuse. That refusal is the control working: it would mean
this project has been rendering qualification and listening material under a consent scope that
does not cover it, which is precisely what the bypass concealed. The consent record is what must
change, and that is the voice owner's decision, not a code change. The 2026-08-31 runs recorded
here passed the gate, so the governed record does carry the scope.

**Measured, not inferred.** The 2026-08-31 set on disk was checked against the property the
conditioner exists to produce. All six samples carry **zero** silent samples at each exposed edge,
against the 240 ADR-0001 §13.4 requires, and their first and last samples are therefore not zero
either:

| Sample | Frames | Leading zero samples | Trailing zero samples |
|---|---:|---:|---:|
| `sample-01.wav` | 144 000 | 0 | 0 |
| `sample-02.wav` | 133 440 | 0 | 0 |
| `sample-03.wav` | 96 960 | 0 | 0 |
| `sample-04.wav` | 104 640 | 0 | 0 |
| `sample-05.wav` | 114 240 | 0 | 0 |
| `sample-06.wav` | 145 920 | 0 | 0 |

So the set is not merely unconditioned: as cache entries these files would be refused outright by
`check_exposed_endpoints`. The sheet and `sample-01.wav` still hash to the digests §Listening
material cites, so that table accurately describes what is on disk — it is the audio that is wrong,
not the record of it. `check_listening_review.py` refuses the set for a second and independent
reason: "the review sheet states no reviewer". **That review was never taken**, so nothing is lost
by discarding the set.

The worker bundle identity was unchanged at `7b065eeb5319c6bc…` after this remediation, re-derived
by `cargo run --package study-tts-runtime --example worker-bundle-hash`: none of the four rounds
described here touched a declared bundle input. It has since moved to
`6a158816945dd7d66a5a32a33e5fce720a5b7f2c7ae87b06e9b79972fe2951d6` under the second audit, which
did.

The listening review was therefore owed against audio no instrument had yet produced, and
§Listening material's sheet and sample digests were superseded again. Both were superseded once
more by §Finding 11, and the review that was finally taken is recorded in §Review result — this
paragraph describes the state at the time, not the state now.

### What the first remediation cost this record

The worker bundle identity moved, because `worker/study_tts_worker/` is a declared bundle input
and that remediation changed it: to `7b065eeb5319c6bc…`, superseding `839baa220e90ab89…`. It has
since moved once more under the second remediation below, and §T5 qualification result carries the
current value. Every digest in the sections above naming either of the two earlier identities
describes a bundle this build no longer produces.

### Second audit — findings 5 to 10

A second audit on 2026-08-31 raised six further findings against the remediated tree. Five are
closed here. One — the model half of finding 6 — stays open as issue #66, which already carries a
design and moves every cache key; folding it in would mix a remediation round with an interface
change.

Four of the five share one shape, and it is the shape §Finding 1 and §The ramp correction already
describe in this record: **a check whose name is stronger than its predicate.** Each fix replaces
an attestation with a measurement.

| # | Finding | Disposition |
|---|---|---|
| 5 | Offline qualification was self-attestation. The criterion passed when the worker's stderr contained `offline environment applied:`, which proves it configured `huggingface_hub` and `transformers` and proves nothing about the backend, a dependency, or a socket | The instrument now **refuses to start** outside a loopback-only network namespace, reading `/proc/net/dev` and `/proc/net/route` before it creates the output root — the same check `validate_network_isolation` has made for the E0-S3 harness since G0, now two-sided between the two files. Interfaces, route count, and namespace inode are recorded in the result. The diagnostics check stays: the namespace proves egress was denied, the diagnostics prove the worker configured itself, and neither implies the other |
| 6 | Worker and model identities were not bound to the loaded artifacts. Rust sent the bundle identity it had verified and then believed whatever the worker answered with, and that answer reached every cache key | **Rust half closed**: the echo is compared against what was sent, before `capabilities`, refusing `BackendValidationError::BundleIdentityNotEchoed`. `t4_e1_a_worker_echoing_another_bundle_identity_is_refused_at_start` drives a new `drift-bundle-at-initialize` fake behavior — the existing `drift-bundle` spoils a *synthesis* frame and could not reach this. **Model half not done**: issue #66, sequenced before G1 and before E1-S4 so the synthesis key moves once. Note for whoever works it that `bundle-manifest.json` in the governed model root already carries a `model/artifacts` array with `sha256` and byte counts, so the issue's premise that no such array exists is partly stale |
| 7 | Staging containment had a parent-directory race. The worker resolved and checked the destination's parent, released that pathname, generated audio, and later opened the resulting string. `O_NOFOLLOW` protects the final component only | `_contained_output` now returns an **open directory descriptor** and the file name, and the write goes through `dir_fd`, so no part of the path is walked twice. Containment is re-read off the descriptor via `/proc/self/fd`, which reports where the directory this process *holds* actually is, so an ancestor swapped during the open is caught rather than raced with. The pre-fix contract was reproduced first and demonstrably wrote outside the root; `test_an_ancestor_swapped_after_the_check_cannot_redirect_the_write` pins it. **Residual, recorded rather than closed**: a directory proven inside the root and then *moved* out keeps the descriptor pointing at it, because a descriptor follows the inode. Closing that needs the root held open and every component reopened relative to it, and whoever can move a directory out of the staging root can already write inside it |
| 8 | Graceful shutdown did not prove the process tree was gone. If the child exited during the grace period, shutdown discarded its process ownership and returned without signalling or checking anything | Ownership is refreshed **before the worker is asked to leave**, which is the only moment its children are still nameable — `/proc/<pid>/task/*/children` disappears with the process. On the graceful path `contain_descendants` signals the recorded descendants by pidfd and proves them gone. Not by process group: the child has been reaped by then, its PID is free, and a group kill by number could reach a stranger. `t4_e1_a_gracefully_shut_down_worker_leaves_no_descendant_behind` starts a real descendant and asks the kernel. **Corrected by the fourth remediation below**: the reasoning in this row is wrong about the kernel — a reaped leader's PID is *not* free while its process group still has a member, and the fix that followed from believing otherwise left the window finding 2 of the fourth audit found |
| 9 | The protocol-cleanliness test could miss trailing stdout contamination. On end of input the reader silently dropped a nonempty unterminated frame, and nothing read the channel after the last request | `ProtocolEvent::Unterminated` reports the tail rather than dropping it, and `shutdown` drains the stream and refuses anything but the shutdown response — which also closes the finding's second half, that the shutdown response was never validated. The T5 criterion is now recorded **from that result**, after shutdown, because the end of the stream is the part an open session cannot see. `t4_e1_trailing_bytes_past_the_last_frame_are_refused` pins it. The drain runs before the reader threads are joined: the response channel holds one frame, so a worker that wrote past its last frame leaves the reader blocked on a full channel and joining first is a deadlock rather than a wait |
| 10 | This record contradicted itself about qualification completion — §T5 described a completed run while two later sections said the instrument must still be re-run, and §Audit remediation claimed the T5 table carried `Owed` rows it did not carry | One statement, in one place. §T5 qualification result carries the only T5 result this record claims, re-run after the remediation above and cited by digest; the sections that said otherwise are rewritten to describe what they actually cover. The two audits are now named apart, since "the audit remediation" meant different things in different paragraphs |

### What the second remediation costs this record

The worker bundle identity moved again — `worker/study_tts_worker/worker.py` is a declared bundle
input and finding 7 changed it — to `6a158816945dd7d6…`, superseding `7b065eeb5319c6bc…`. It moved
once more under the third remediation below, and §T5 qualification result carries the current
value.

### Verification run for the second remediation

Taken on the reference machine, 2026-08-31. Every check below actually ran; what did not run is
named after the table rather than omitted.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 376 passed, 0 failed |
| Doctests | `cargo test --offline --workspace --doc --locked` | 8 passed, 0 failed |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 61 passed |
| Qualification tooling | `(cd scripts/qualification && python3 -m unittest discover --start-directory tests)` | 33 passed |
| Provenance checker's own tests | `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | 23 passed |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py` | One mismatch, pre-existing and not this work's — see below |
| Whitespace | `git diff --check` | Clean |
| Worker bundle identity | `cargo run -p study-tts-runtime --example worker-bundle-hash` | `6a158816945dd7d66a5a32a33e5fce720a5b7f2c7ae87b06e9b79972fe2951d6` |
| T5 qualification | `unshare --user --map-root-user --net ./target/debug/examples/worker-qualification …` | 5 of 5 pass. §T5 qualification result |
| Namespace refusal | The same binary without `unshare` | Refused before creating the output root, naming the 13 interfaces it found |
| Listening render | `./target/debug/examples/listening-render …` | **Failed.** §Finding 11 |

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run. The listening review was not taken, because
§Finding 11 leaves nothing lawful to review.

**The provenance mismatch is not this work's.** `e0-s3-g0-qualification-decision-v3.md:51` pins
`DELIVERY-PLAN.md` at `add598619c5e…` while it hashes `2561bfe840a7…`. `DELIVERY-PLAN.md` is
unmodified by this remediation, so the mismatch predates it and is recorded here rather than
repaired, since repairing another gate's record is not this story's to do.

**Two documents this remediation edited are cited by name and not by digest.**
`docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` and `scripts/qualification/README.md` both gained the
`unshare` wrapper; `grep -rn` across `evidence/` shows neither is pinned by SHA-256 anywhere, so no
citation moved.

### Finding 11 — conditioned edges were not zero, and publication refused them

Found by running the listening instrument as the last step of the second remediation, not by
looking for it. The render refused every take:

```text
Audio(UnusableAudio { fault: ExposedEndpointNotZero { edge: "first", value: 4.312751e-6 } })
```

**Two committed gates disagreed, and the ADR sides with both.** ADR-0001 §13.4 states two edge
rules in one sentence — "add zero samples until each edge has at least 10 ms of silence" *and*
"require exposed endpoints to be zero" — and satisfying the first says nothing about the second.
`condition_edges` padded by *quantity*: `required.saturating_sub(leading_silence)`. A take whose
first 240 samples already sit below the silence threshold got **no padding at all**, and its first
sample stayed at whatever quiet-but-nonzero value the model produced.
`check_exposed_endpoints` then correctly refused it.

Measured on the refused take: 104 640 frames, peak `0.119`, first 240 samples all at or below
`1.1e-5`, first sample `4.3e-6`. Real model output is quiet at the edges rather than digitally
silent, so **no real take could be published at all**.

`MAX_TRANSITION_RAMP_MS`'s own comment stated the assumption that failed — "the silence side is
exactly zero once padded" — which is not true when nothing was padded.

**The same shape as §The ramp correction**: a conditioning step that satisfied the arithmetic of
its rule without producing the property the rule exists for. It is also the shape that made this
gate's own test agree with the defect — `t1_e2_exposed_endpoints_are_exactly_zero` used a fixture
that is signal from the first sample, so the full padding always ran and the endpoint was zero for
a reason a real take does not have.

**Closed at that round.** The project owner directed the padding fix on 2026-08-31: `condition_edges` still pads
by silence duration, and additionally pads **one zero** at any edge whose sample is not exactly
zero. That is the only mechanism ADR-0001 §13.4 names — "add zero samples" — it keeps the silence
measurement's documented meaning, and it leaves a join reading `0.0` → `0.0`.
That mechanism was later superseded by §Quiet-edge normalization correction: the current test is
`t1_e2_a_quiet_but_nonzero_edge_is_normalized_without_padding`, which preserves the exact endpoint
without changing duration.

Measured after that fix, on the 2026-08-31 set §Historical listening material now cites: every sample carries
exactly one leading and one trailing zero, first and last samples exactly `0.0`, and each is two
frames longer than the take it replaces. Those are the historical bytes the later normalization
correction invalidates.

## Third remediation — the four issues the second audit left open

The project owner directed on 2026-08-31 that everything still outstanding be closed in one pass.

| # | Item | Disposition |
|---|---|---|
| 11 | Conditioned edges were not zero | Closed above |
| 7 residual | A held descriptor could be carried out of the staging root | **Narrowed, and the rest recorded as unreachable from Python.** Containment no longer resolves a pathname at all: the staging root is opened once at `initialize` and held, and every component of an assigned path is opened relative to the one before it with `O_NOFOLLOW`. Containment stops being a check performed on a name and becomes a property of the walk. It is also stricter — a symlinked directory *inside* the root is now refused for being a symlink rather than admitted for pointing somewhere lawful, which the pre-fix contract did admit. The `/proc/self/fd` verification and the `resolve()` call are both deleted, so this is shorter than what it replaces. **What no descriptor can close**: a directory proven inside the root and then *moved* out carries the descriptor with it, because a descriptor follows the inode. That needs a filesystem sandbox, which the audit finding itself offered as the alternative, and whoever can move a directory out of the staging root can already write inside it |
| 6 model half | Model weights were identified by strings, never by their bytes | **Closed by verification, deliberately not by keying.** `crates/study-tts-runtime/src/model_gate.rs` pins the qualified revision and the SHA-256 and byte count of all four artifacts, and `WorkerConfiguration::for_bundle` hashes them before it can return a launchable configuration. `WorkerTtsExecutor::start` then refuses a worker that reports a *different* revision, which is what stops it loading a directory the gate never read. Hashing 3.19 GB costs 2.1 s. See §Why verification and not a key term |
| Coverage | A `NameError` shipped through `_render` because no test could reach it | `RenderPlumbingTests` drives `_render` with a stub model and the real numerical libraries. It is gated on those libraries being importable, so it skips under the system interpreter and runs under `worker/.venv/bin/python`; `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §1 now runs the worker suite under both, and says that `OK (skipped=2)` is the signal the second one did not happen. Reintroducing the defect makes it fail with the original `NameError` |

### Why verification and not a key term

Issue #66 proposes both an artifact check and a `model_artifacts_hash` input to `SynthesisContext`.
They are separable, and the project owner directed on 2026-08-31 that only the first be done.

Verification closes the security property on its own. Changed weights are **refused** rather than
admitted, so no audio is ever produced under weights the supervisor did not prove — and a
*legitimate* weights change moves the pinned revision, which is already an ADR-0001 §12.5 key
input, so the key moves anyway. The key term would buy the ability to hold audio from several
weight-sets in one cache, which nothing in this project does.

What it would have cost is the reason to defer it: every cache key moves, `schemas/plan-v3` moves,
an interface-change record is owed, the bundle identity is re-derived and the listening review
retaken. The owner also settled the question #66 flagged rather than decided: **adding a
`SynthesisContext` input needs an ADR-0001 §12.5 amendment**, not an interface-change record alone,
because §12.5 is ratified and enumerates its inputs. That decision is recorded here so it does not
have to be retaken when the term is eventually added.

**Two implementation choices differ from #66's stated design**, and both make the change smaller.

The digests live in Git and the governed `bundle-manifest.json` is not parsed. #66 proposed two
levels — an `artifacts` array in the model root, and a derived hash pinned in Git — but that array
already exists and is complete, and #66's own argument for the second level is that the first is
trust on first use. Once the authoritative list is in Git, parsing the record beside the weights
adds nothing a reader could rely on; it only adds a second parser of a format
`worker/study_tts_worker/worker.py` already reads. All four declared digests were checked against
the bytes and match.

No `RemedyAdvice` is attached. `docs/governance/ROUTING-TABLES.md` §Failure routing establishes no
owner for a model-artifact mismatch, and `crates/study-tts-runtime/src/error/mod.rs` states the
rule that governed advice is added only where that table does. Adding a row would have moved a
document two **accepted** E0-S2 records pin, so the owner is named in each message instead, from
§Decision routing's "Chatterbox/model revision" row. The same reasoning kept the operator half of
the model-pin mirror out of `docs/operations/WORKER-ENVIRONMENT.md`, which many accepted records
pin, and put it in `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md`, which none do.

### Verification run for the third remediation

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 382 passed, 0 failed |
| Doctests | `cargo test --offline --workspace --doc --locked` | 8 passed, 0 failed |
| Python worker, system interpreter | `python3 -m unittest discover --start-directory worker/tests` | 64 passed, 2 skipped |
| Python worker, restored environment | `worker/.venv/bin/python -m unittest discover --start-directory worker/tests` | 64 passed, 0 skipped |
| Qualification tooling | `(cd scripts/qualification && python3 -m unittest discover --start-directory tests)` | 33 passed |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py` | Exit `0`, no unaccounted mismatches |
| Whitespace | `git diff --check` | Clean |
| Worker bundle identity | `cargo run -p study-tts-runtime --example worker-bundle-hash` | `d66e84e4512e2249976523f2ce6a0acaecb7fa6a6494d2aba19b2e4081de37af` |
| Model artifact digests | Hashed against the governed acquisition record | 4 of 4 match, 3.19 GB in 2.1 s |
| T5 qualification | `unshare --user --map-root-user --net ./target/debug/examples/worker-qualification …` | 5 of 5 pass. §T5 qualification result |
| Listening render | `./target/debug/examples/listening-render …` | 6 takes published. §Listening material |

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run. The listening review itself was not taken; it is the
one thing here no instrument may supply.

**The one provenance mismatch that stood through this work is now accounted.**
`e0-s3-g0-qualification-decision-v3.md:51` pins `DELIVERY-PLAN.md` at a digest it no longer hashes
to, and `e1-s3-delivery-plan-ramp-correction-reconciliation-v1.md` was written to account for
exactly that pin — but `scripts/check-evidence-provenance.py:105` counts a reconciliation record
only when its status reads `Accepted`, and it was `Proposed`. Both approval rows were decided on
2026-08-31 and the record now reads `Accepted`, so the checker exits `0`. Nothing in the tree
changed to achieve that; the mismatch was never a defect, only an unsigned accommodation.

That record's own §Approvals now also carries the correction §Finding 11 records: the ramp
correction it accounts for was necessary but did not by itself make a take publishable, so the
listening set it superseded was replaced twice rather than once.

## Fourth remediation — the audit of 2026-08-31 (findings 1 to 4)

### Fourth audit — four findings

A fourth audit raised three Major findings and one Minor against the tree the third remediation
left. All four are closed here.

| # | Finding | Disposition |
|---|---|---|
| 1 | Voice profiles were loaded before the consent and integrity gates ran. The worker deserializes **every** `conditionals.pt` beneath the governed root during `initialize`, and both instruments started the executor before calling the Rust gate — so a revoked, unpermitted, or checksum-invalid profile went through `torch.load` before anything could refuse it, including one unrelated to the selected voice | `voice_gate::admit_voice_root` runs the consent, rights, scope, and checksum gate over every profile the worker would load, and it is called from `WorkerConfiguration::for_bundle` beside `verify_model_artifacts` — not from the instruments. `for_bundle` is the only constructor of a launchable configuration, so no caller reaches a worker around it, which is the same structural argument `model_gate` records for the weights. **Corrected 2026-08-31 by the fifth audit:** that sentence was false. `WorkerConfiguration::for_protocol_fake` is also public and takes a caller-chosen program and environment, so it could be pointed at the bundle interpreter over a governed root; the claim is made true in §Fifth remediation, finding 2, and the gate's skip list is corrected there too — it *skipped* a directory name that is not UTF-8, which the worker still loads. The skip list is two-sided with `_voice_conditioning` in `worker/study_tts_worker/worker.py` and must skip *at most* what that skips, since anything skipped here and loaded there reaches `torch.load` ungated; `t1_e1_the_gate_skips_exactly_what_the_worker_skips` pins the three cases, and two further T1 tests refuse a revoked and an altered profile **the request never names** |
| 2 | Graceful shutdown had a process-tree escape race. Descendants are enumerated once, before the worker is asked to leave, and the voluntary path signalled only those recorded pidfds — so a worker that started a child *after* enumeration and then exited left it running. The existing test spawns at startup and could not see it | The two shutdown paths are collapsed into one. `wait_for_voluntary_exit` now observes the exit with `waitid(…, WNOWAIT)` instead of `try_wait`, so the child is **not reaped**: its PID stays allocated, and with it the process group ID that equals it, because POSIX keeps a process group ID unusable while the group still has a member. `terminate` then runs on both paths — group kill, reap, and the existing proof that the group is empty *and* every recorded descendant is gone. `contain_descendants` and the branch that called it are deleted; the fix is a net deletion. `t4_e1_a_descendant_started_during_shutdown_is_contained` drives a new `spawn-descendant-at-shutdown` fake behavior that starts its child in answer to the `shutdown` frame, and failed before the change for exactly the stated reason |
| 3 | A breaking cache contract was implemented without acceptance. The code publishes `CACHE_SCHEMA_VERSION` `2.0` while `docs/architecture/E1-S3-INTERFACE-CHANGE-002.md` was `Proposed` with all three approvals `Pending`, and a Proposed record authorizes nothing | That record is now `Accepted`, signed 2026-08-31, with all three rows decided. Its later amendment also moves `CACHE_PUBLICATION_CONTRACT_VERSION` to `e0.cache-publication.2.0` and binds ramp metadata to audio-derived feasible bounds. The historical authorization finding was closed by the recorded decisions; the later semantic correction is covered by the amended T-AUDIO decision |
| 4 | Source and record still described superseded behavior — `AGENTS.md` §State said the product worker refuses `initialize` and `synthesize`; a TODO in `worker_executor.rs` said the model artifacts were unverified, immediately above the check that verifies them; `protocol.py` named the deleted `schemas/worker-protocol-v1.schema.json` twice; and this record's §Review asked approval for issue #60 and a superseded bundle hash rather than for the E1-S3 candidate | All four corrected, and two more of the same class found while doing it: §Limits still said the listening review "must be retaken before G1" after it had been retaken and accepted, and carried a bullet saying the model's bytes are never hashed beside one saying `model_gate` hashes them. §Review now asks for the decisions this candidate needs and says why the preflight rows were replaced. The `protocol.py` correction moved the bundle identity, which is what §What the fourth remediation costs this record is about |

### What the fourth remediation costs this record

`worker/study_tts_worker/protocol.py` is a declared bundle input, so correcting two references to a
schema file that no longer exists moved the worker bundle identity to
`58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`, superseding
`d66e84e4512e2249976523f2ce6a0acaecb7fa6a6494d2aba19b2e4081de37af`. `worker.py` moved too, for the
two-sided comment finding 1 owes.

The T5 qualification result was therefore retaken, and §T5 qualification result carries the new
one. **The listening review was not retaken.** The worker reports `deterministic_seed: false`, so a
re-render would produce different bytes and could not be compared with the reviewed set — the
review would have had to be taken again by a person. It is carried forward instead, under
`e1-s3-protocol-docstring-identity-reconciliation-v1`, on the argument that a docstring in a
declared input cannot reach audio. That argument is written down and signed rather than assumed,
because it is exactly the kind of accommodation this record has refused elsewhere.

### Verification run for the fourth remediation

Taken on the reference machine, 2026-08-31. Every check below actually ran; what did not run is
named after the table rather than omitted.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 387 passed, 0 failed |
| Doctests | `cargo test --offline --workspace --doc --locked` | 8 passed, 0 failed |
| Python worker, system interpreter | `python3 -m unittest discover --start-directory worker/tests` | 64 passed, 2 skipped |
| Python worker, restored environment | `worker/.venv/bin/python -m unittest discover --start-directory worker/tests` | 64 passed, 0 skipped |
| Qualification tooling | `(cd scripts/qualification && python3 -m unittest discover --start-directory tests)` | 33 passed |
| Provenance checker's own tests | `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | 23 passed |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py` | Exit `0`, no unaccounted mismatches |
| Whitespace | `git diff --check` | Clean |
| Worker bundle identity | `cargo run -p study-tts-runtime --example worker-bundle-hash` | `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` |
| T5 qualification | `unshare --user --map-root-user --net ./target/debug/examples/worker-qualification …` | 5 of 5 pass, in a namespace holding only `lo` with no IP route. §T5 qualification result |
| Listening review still binds to its audio | `python3 scripts/qualification/check_listening_review.py …/listening` | Exit `0`; all six sample digests match the sheet, and the key was revealed |

Two tests were written red and failed first, for the reason each finding gives:
`t4_e1_a_descendant_started_during_shutdown_is_contained` reported the surviving PID before the
shutdown paths were collapsed, and the four `admit_voice_root` tests could not compile against a
gate that did not exist.

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run. No audio was re-rendered and no listening review was
retaken — that is the decision `e1-s3-protocol-docstring-identity-reconciliation-v1` records, not
an omission. The `setsid` residual in shutdown containment is stated in §Limits and is not
covered by any test here, because nothing can name a process that has left every group and holds
no recorded pidfd.

## Fifth remediation — the audit of 2026-08-31 (findings 1 to 4)

### Fifth audit — four findings

A fifth audit raised three Major findings and one Minor. Three of them attack the *arguments* the
fourth remediation wrote rather than the mechanisms it built, which is the more useful kind of
finding and the harder one to see from inside. All four are closed here. None was a false positive,
and the first was reproduced against the real Python before it was believed.

| # | Finding | Disposition |
|---|---|---|
| 1 | The pre-load voice gate was bypassable through a non-UTF-8 profile name. `admit_voice_root` **skipped** a loadable profile directory whose name is not UTF-8, on the recorded ground that such a name "cannot equal any `profile_id` a JSON record states". It can. Python reads the directory name through `surrogateescape`, so the byte `0xff` arrives as a lone surrogate; `profile.json` is read as UTF-8 and `json.loads` decodes the six ASCII characters `\udcff` to that same surrogate; the two compare equal, the path re-encodes to the original bytes, and `_load_backend` deserializes the artifact. An entry this build skipped was one the worker loaded — with no consent, rights, scope, or checksum check | Refused rather than skipped. `VoiceProfileError::VoiceProfileNameNotUtf8` names the entry and the root, and the refusal sits after `holds_a_loadable_profile`, so a stray file or a directory without `profile.json` is still skipped by both sides. This is the one place the two filters may disagree, and it disagrees in the only safe direction: Rust now skips strictly *less* than the worker, which is what the two-sided rule asks for. `t1_e1_a_profile_name_that_is_not_utf8_refuses_the_root` builds the name from bytes and failed first — the root was admitted. The false comment is replaced with the mechanism, per `rust-comment` |
| 2 | `for_bundle` was not structurally the only route to a real worker. `WorkerConfiguration::for_protocol_fake` is public and takes an arbitrary program, argument list, and environment: pointed at the bundle interpreter with the governed-root variables set, it starts the **real** worker with neither `verify_model_artifacts` nor `admit_voice_root` having run. `start` sends `initialize` first and compares identities afterwards, so the model and every `conditionals.pt` are loaded before `ModelRevisionNotEchoed` refuses the session. The gates were skippable, not merely deferred | The fake is never told where a governed root is. `for_protocol_fake` returns `Result` and refuses an environment naming either variable in `GOVERNED_ROOT_ENVIRONMENT`, with `WorkerBundleError::ProtocolFakeNamedAGovernedRoot`. A real worker started that way then hits `_governed_root`'s refusal, which runs *before* the `torch` import, so nothing is deserialized. Refused by **name and not by value**, because a stand-in root and a real one are the same string to that constructor. `t4_e1_the_governed_root_variables_are_the_ones_the_launcher_declares` reads `worker/launcher.json` and refuses the drift, which is the half that keeps the mirror from coming apart; `t1_e1_the_protocol_fake_cannot_be_handed_a_governed_root` is table-driven over the constant and failed first. The two doc comments that claimed `for_bundle` was the only route now say what is actually true |
| 3 | Full process-tree containment remains knowingly incomplete. A descendant that calls `setsid()` between the last enumeration and its parent's exit is in no group this build owns and holds no recorded pidfd. ADR-0001 §10.3 requires the parent to terminate the full child process tree; the residual was stated in §Limits, but a §Limits bullet in a `Proposed` record accepts nothing, so an accepted-ADR conflict was being carried silently | Put to the owner instead. `ADR-0001-D008` is Approved and signed 2026-08-31, permitting E1-S3 to ship group-plus-pidfd containment, naming the four mechanisms that would close it and why each is rejected *now* — a PID namespace needs `Command::pre_exec`, which `unsafe_code = "forbid"` bans workspace-wide; an `unshare` wrapper replaces the bundle interpreter as the spawned program and so breaks the identity every cache key rests on; cgroup v2 `cgroup.kill` needs delegation the reference environment does not require; `PR_SET_CHILD_SUBREAPER` is process-global state a library may not set for its caller. `DELIVERY-PLAN.md` §Story E5-S4 carries the closure task and `t4_e5_a_descendant_that_leaves_its_process_group_is_still_contained`, which is the deviation's expiry. **No code changed to close this finding, and no test was written to assert the escape is contained** — it is not, and a test shaped to pass against it would be the weakened control `CLAUDE.md` forbids. `WorkerClient::shutdown`'s own doc comment now states what it reaches |
| 4 | This record contradicted itself. §Story acceptance criteria said acceptance needs six criteria while §Story result answered seven, and criterion 4 said four `t5_e1_` criteria while the result row and the filed `qualification-result.json` both report five | Both corrected in place, under the "**Restated 2026-08-31**" convention criteria 2 and 3 already use, so the correction is visible rather than retroactive. Criterion 7 is the listening review, which ADR-0001 §17.5 makes a gate condition and which §Story result has answered since it was taken. Criterion 4 now says five and names the fifth: `t5_e1_worker_survives_restart_and_starts_offline` is a helper criterion `DELIVERY-PLAN.md` never named, which §T5 qualification result and `scripts/qualification/README.md` both already explained — the criteria list was the only place that had not caught up |

### What the fifth remediation costs this record

**Nothing that a person had to redo at this round.** No declared bundle input changed: the fix for finding 1 is
Rust-only, and `worker/study_tts_worker/` is untouched. `worker-bundle-hash` was re-derived to
prove it, and still answers
`58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`. The T5 qualification result
and the 2026-08-31 listening review therefore both still describe the code that ships, and neither
was retaken.

The Python half of finding 1 was considered and declined. Adding the same refusal to
`_voice_conditioning` would be defence in depth, but `worker.py` is a declared input, so it would
have moved the identity, superseded the qualification result, and forced a third carry-forward of
the listening review — for a path the Rust gate already closes by refusing the whole root before
any worker starts. The docstring on the Python side already states the rule correctly and needed no
edit, because the correction makes Rust skip *less*.

What it does cost is a governance obligation and a smaller safety claim. `ADR-0001-D008` is an
accepted deviation from ADR-0001 §10.3 that E5-S4 now owes closure on, and finding 2's argument
now rests on a two-element denylist pinned to `worker/launcher.json` rather than on `for_bundle`
being the sole constructor. Both are recorded in §Limits rather than rounded away.

Three rows were added to `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement — one of
them owed since the fourth remediation landed `admit_voice_root` without one. That moved the
policy's digest, which two accepted G0 records pin;
`e1-s3-rights-policy-enforcement-rows-reconciliation-v1` accounts for both and is Accepted.

### Verification run for the fifth remediation

Taken on the reference machine, 2026-08-31. Every check below actually ran; what did not run is
named after the table rather than omitted.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Lints | `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Tests | `cargo test --offline --workspace --all-targets --locked` | 390 passed, 0 failed |
| Doctests | `cargo test --offline --workspace --doc --locked` | 8 passed, 0 failed |
| Python worker, system interpreter | `python3 -m unittest discover --start-directory worker/tests` | 64 passed, 2 skipped |
| Python worker, restored environment | `worker/.venv/bin/python -m unittest discover --start-directory worker/tests` | 64 passed, 0 skipped |
| Qualification tooling | `(cd scripts/qualification && python3 -m unittest discover --start-directory tests)` | 33 passed |
| Provenance checker's own tests | `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` | 23 passed |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py` | Exit `0`, no unaccounted mismatches |
| Whitespace | `git diff --check` | Clean |
| Worker bundle identity | `cargo run -p study-tts-runtime --example worker-bundle-hash` | `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`, **unmoved**, which is the premise this round rests on |

Three tests were written red and failed first, for the reason each finding gives:
`t1_e1_a_profile_name_that_is_not_utf8_refuses_the_root` was admitted by the gate before the
refusal replaced the skip; `t1_e1_the_protocol_fake_cannot_be_handed_a_governed_root` was run
against a constructor that returned `Ok` with no check, so the red is the missing refusal rather
than a missing signature; and `t4_e1_the_governed_root_variables_are_the_ones_the_launcher_declares`
is a pin rather than a defect and passed on its first run, which is stated rather than dressed up.

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run. **`worker-qualification` and `listening-render` were
deliberately not re-run**, because no declared bundle input moved and `worker-bundle-hash` proves
it; at that point the filed T5 result and accepted listening review still described the code. No
test asserts the `setsid` escape is contained, because it is not — `ADR-0001-D008` is where that
is decided rather than a gap left implicit.

### Quiet-edge normalization correction

A subsequent review found that `condition_edges` satisfied an already-long-enough quiet edge by
inserting one zero sample. The output passed the exact-endpoint check, but its duration changed even
though the measured silence already met the requirement. The conditioner now zeroes the measured
quiet leading and trailing samples and adds no padding in that case.

This is Rust-only and does not move the worker bundle identity, so the recorded five-of-five T5 run
still supports the worker criteria and need not be repeated. It does change the conditioned WAV
bytes and frame count. The 2026-08-31 listening review is therefore historical evidence for its
recorded set, not acceptance evidence for the current candidate; a new listening render and human
review are the one remaining qualification item.

Verification after this correction:

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Clean |
| `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| `cargo test --offline --workspace --all-targets --locked` | 396 passed, 0 failed |
| `cargo test --offline --workspace --doc --locked` | 8 passed, 0 failed |
| `python3 -m unittest discover --start-directory worker/tests` | 64 passed, 2 skipped on the system interpreter |
| `(cd scripts/qualification && python3 -m unittest discover --start-directory tests)` | 36 passed |
| `python3 scripts/check-rust-conventions.py` | Clean |
| `python3 scripts/check-evidence-provenance.py` | Exit `0` |
| `git diff --check` | Clean |

**Not run, and not claimed:** the protected reference-machine workflow, a new real-worker T5 run,
and a post-correction listening render and review. The existing T5 artifact remains applicable for
the reason above; the listening review does not.

## Limits the story does not close

- **The model's bytes are verified, not keyed.** `model_gate` refuses weights that are not the
  pinned ones, so no audio is produced under unproven bytes — but ADR-0001 §12.5's key still names
  the revision rather than the artifacts. §Why verification and not a key term records the decision
  and the ADR amendment it would need. Issue #66 stays open for the key term alone; its
  verification half is closed here, and its premise that the governed manifest carries no
  `artifacts` array is stale.
- **`code.commit` is still a string.** The model gate hashes the *weights*; the Chatterbox code
  revision the worker reports as `tokenizer_revision` is still read from the acquisition record and
  never hashed. It reaches every cache key. Closing it means declaring and hashing the code tree
  the same way, and is not attempted here.
- **Shutdown containment reaches the process group and what it enumerated, and nothing else.** A
  descendant that calls `setsid()` between the last enumeration and its parent's exit is in no
  group this build owns and holds no pidfd it recorded, so nothing can name it, and
  `wait_for_containment` reports success without having seen it. Both halves of the containment are
  real — the group kill reaches a child started after enumeration, and the recorded pidfds reach a
  child that left the group *before* it — but their union is not the whole tree, and ADR-0001 §10.3
  requires the whole tree. **This is now an owner-approved deviation rather than a limitation this
  record states on its own authority:** `ADR-0001-D008`, Approved 2026-08-31, names the four
  mechanisms that would close it and why each is rejected now, and expires at E5-S4's
  `t4_e5_a_descendant_that_leaves_its_process_group_is_still_contained`. It also says the owner must
  revisit it before any worker pool above size one, since a pool multiplies the escapees. The fifth
  audit's third finding was that a §Limits bullet in a `Proposed` record accepts nothing, which was
  correct.
- **No configuration this crate builds can reach a governed root ungated, but the argument is a
  denylist now.** `for_bundle` gates the model artifacts and every voice profile, and
  `for_protocol_fake` refuses the two variable names in `GOVERNED_ROOT_ENVIRONMENT` so it cannot be
  pointed at the real worker over a governed root. The second half is a mirror of
  `worker/launcher.json`, pinned by a test, rather than a structural impossibility — a worker that
  learned where its roots are by some other means would be outside it. That is a smaller claim than
  "`for_bundle` is the only constructor", which is what the fifth audit's second finding showed was
  false, and it is the claim this record makes.
- **The worker still loads every voice in the governed root.** `admit_voice_root` gates all of them
  first, so nothing ungated is deserialized, but the blast radius is still the root rather than the
  request: one revoked profile refuses every build until it is moved out. Narrowing it means
  `initialize` carrying the admitted profile list so the worker loads only those, which is a change
  to `e1.worker.2.0` and needs its own interface-change record. Not folded in here, for the reason
  the model half of the second audit's finding 6 was not: a remediation round is the wrong place
  for an interface change.
- **Staging containment is a walk, not a sandbox.** §Third remediation records the residual: a
  directory proven inside the staging root and then *moved* out of it carries the held descriptor
  with it, because a descriptor follows the inode. No in-process change closes that; it needs a
  filesystem sandbox, which the audit finding itself named as the alternative.
- **The model gate cannot close its own window.** It hashes the artifacts, then the worker opens
  them. An attacker who can rewrite the governed model root between those two moments could not be
  caught by any amount of care on this side, and could equally rewrite it before the build.
- **Egress denial is the operator's to supply.** The instrument refuses to run outside a
  loopback-only namespace, which makes a filed result trustworthy, but nothing forces the operator
  to run the instrument at all. `listening-render` is deliberately not gated the same way: it
  asserts no offline property and produces audio for a human, not evidence about a network.
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
- **The listening review was superseded twice; the second retake is pending.** An earlier review was complete and
  accepted against takes that `ADR-0001-D007`'s edge conditioning and the ramp correction then
  changed, so the published samples no longer matched the ones reviewed. That retake was taken on
  2026-08-31 and remains recorded below as history. Quiet-edge normalization then changed those
  published bytes again by replacing one-sample padding with zeroing of the already-measured silent
  region. No post-correction listening set or completed review is recorded yet.

  **The retake has an instrument; the original did not.** The 2026-08-31 takes were produced by
  piping a hand-built NDJSON session into the worker, which left the material unreproducible — that
  session predates the required `staging_root` and cannot even be replayed against the worker that
  exists now. `crates/study-tts-testkit/examples/listening-render.rs` replaces it: it renders
  through `WorkerTtsExecutor`, reads its words from the committed
  `fixtures/listening/e1-s3-listening-script.json`, blinds the takes, and writes a pending sheet
  plus a separate key. `scripts/qualification/check_listening_review.py` refuses an incomplete sheet
  or one whose digests no longer match the audio, and is the sanctioned way to reveal the mapping.
  `scripts/qualification/README.md` §E1-S3: the listening review carries the procedure.

  E1-S3 produces audio for the first time, and the other T5 criteria measure session behavior
  without listening to any of it. Six takes were rendered and reviewed on 2026-08-31; that accepted
  review is historical context for the superseded bytes. The current post-correction review remains
  pending.
- **Restart after a timeout is not implemented.** A timed-out executor now kills its worker tree
  and refuses every later request, which is what ADR-0001 §10.3 asks of it, but recovery is E5-S3's
  and is noted on that issue.
- **`worker/pyproject.toml` stays in the bundle inputs**, and the §12.5 question this record raised
  stays open — with a constraint now attached: `worker_bundle.rs` reads the file only when the
  manifest declares it, so removing it from `inputs` would silently make
  `check_requirements_match_lock` a no-op. The reconciliation must be decoupled from manifest
  membership before the file can leave.

## Historical listening material

Re-rendered 2026-08-31 after the third remediation, once §Finding 11 was closed and
`listening-render` could publish a take at all. This set was reviewed and accepted, but quiet-edge
normalization has now superseded it: these files contain one inserted zero at each edge, while the
current conditioner zeroes the measured quiet region without adding frames. No current
post-correction set has been rendered or reviewed. Rendered by
`cargo run --package study-tts-testkit --example listening-render`. The 2026-08-30 set it replaced
was produced by piping a hand-built NDJSON session into the worker and could not be re-rendered.
Governed output, so the location is named by root rather than reproduced here, per
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

| Item | Value |
|---|---|
| Location | `listening-2026-08-31-180808/listening/` beneath the governed qualification output root |
| Samples | 6, `sample-01.wav` through `sample-06.wav`, blinded |
| Script | `fixtures/listening/e1-s3-listening-script.json`, committed and registered |
| Total duration | 30.8 s |
| Format | 24 000 Hz, one channel, IEEE float |
| Voice profile | `owner-fallback-v1` |
| Style | `calm_explanatory` |
| Seed | 42, from `worker/launcher.json` |
| Worker bundle identity that rendered it | `d66e84e4512e2249976523f2ce6a0acaecb7fa6a6494d2aba19b2e4081de37af`, superseded by `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`. The review is carried across that move by `e1-s3-protocol-docstring-identity-reconciliation-v1`, not re-rendered |
| Exposed endpoints | Every sample: first and last sample exactly `0.0`, one zero padded at each edge |
| Completed review sheet | SHA-256 `d744eb70c45760c8587a42a6c2c23f42896f2931d0c5ef26a91336a82bd7f167` |
| Pending sheet, as rendered | SHA-256 `7f8fe2df628cb36cf8becea7bcdbcab4012f210cadbeb5daf31f999281759a39` |

Each sample's digest, which every judgment is recorded against:

| Sample | SHA-256 |
|---|---|
| `sample-01.wav` | `ffddccaef29e37d538158efe5ebfcd2df5f40efa04dbd02ef55e11f098070ded` |
| `sample-02.wav` | `64c5709a0e87de425ab82f0557f334e4dfce45caa92c302ba149bf6d0733bede` |
| `sample-03.wav` | `dfa2a9853a10597dab6ff2e88462e1e6c9ca5da64225a7136d495dff22847323` |
| `sample-04.wav` | `774ac64441d96e1a6552dbaa57d02540b7ac4ef869c273e89c06628158ccf31e` |
| `sample-05.wav` | `5454cc67f71f23c7c6c1ee805caaa38988afe5d1db2f8484ee9d997a010654c1` |
| `sample-06.wav` | `6c0d4c69b346886dbf974c5cf562cdae8a5f453f8381dec6e7bacc1e07e445c5` |

The order is randomized and the mapping withheld in `randomization-key.json`;
`scripts/qualification/check_listening_review.py` reveals it only once the sheet is complete and
still matches these bytes.

### Historical review result

**Taken and accepted for the superseded bytes.** Reviewed 2026-08-31 by Ross Todd on laptop built-in
speakers, against the six samples and digests §Historical listening material cites. It does not
accept the current conditioner; the post-correction review is pending.

`python3 scripts/qualification/check_listening_review.py` accepted the sheet and revealed the key,
which is what binds a judgment to bytes rather than to a filename: it re-hashes every sample
against the digest the sheet records before it will open `randomization-key.json`, so a sheet
completed against a different render cannot be filed against this one.

| Criterion | Result across all six samples |
|---|---|
| `omissions_or_additions` | none |
| `pronunciation` | none |
| `voice_consistency` | none |
| `pacing` | none |
| `noise_or_artifacts` | none |

**Historical finding: accept 6 of 6, no findings on any criterion.**

The revealed mapping, recorded so a retake can be compared line for line rather than sample for
sample: `sample-01`→`line-03`, `sample-02`→`line-05`, `sample-03`→`line-06`, `sample-04`→`line-01`,
`sample-05`→`line-02`, `sample-06`→`line-04`.

The five criteria are E0-S3's. Its sixth, `audible_difference_from_other_runs`, is not applicable:
it compared ten runs of one line for determinism, while these are six different lines.

Three earlier sets are superseded, and only one of them was ever reviewed. The **2026-08-30** set
was reviewed by Ross Todd on laptop built-in speakers — sheet SHA-256
`08fbf7fcb1e98f0fe3252b74cccac490bf253bcce46a98543eb8e826fd4888ea`, accept 6 of 6, no findings — and
that judgment is recorded here as history rather than as a result, because `ADR-0001-D007`'s edge
conditioning pads and ramps every segment and the build no longer produces the audio it was taken
against. The two **earlier 2026-08-31** renders were superseded before anyone listened to them: the
first by the ramp correction, the second by §Finding 11, which is why `listening-2026-08-31-180808`
is the first set a person has both heard and been able to accept against published bytes.

The bundle-identity move recorded above is the one supersession this record does **not** treat as
invalidating: it changed ten lines, all inside docstrings, and
`e1-s3-protocol-docstring-identity-reconciliation-v1` §Why the review carries and the qualification
did not sets out the argument and the three signatures behind it.

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

The rows below were written at the governance preflight and asked approval for the two findings
that opened this record — issue #60, the reverted declaration, and the identity's return to
`75d56310…9d2bab3`. Those decisions were made and are recorded in
`e1-s3-worker-backend-provenance-reconciliation-v1`, which is `Accepted`. Asking for them again
here would have a reader approve the story on the strength of a settled preflight rather than on
the candidate this record now describes, which is what the fourth audit's Minor finding was about.
The rows are therefore the decisions **this candidate** needs, restated again after the fifth
audit: two of them asked approval for reasoning that audit showed was wrong.

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Project owner | Ross Todd | Pending the post-correction listening review; criterion 7 is not yet met | |
| Engineering owner | Ross Todd | Accept the three gates that now precede a worker — the derived bundle identity, `model_gate`'s four SHA-256-pinned artifacts, and `admit_voice_root` over every profile in the governed root — all inside `WorkerConfiguration::for_bundle`; and accept that the property relied on is **not** that `for_bundle` is the sole constructor — the fifth audit showed it is not, since `for_protocol_fake` takes a caller-chosen program and environment — but that `for_bundle` is the only constructor *given* a governed root and `for_protocol_fake` is *refused* one, which together leave no configuration this crate builds that reaches a governed root ungated. The second half is a denylist mirrored to `worker/launcher.json` and pinned by a test, which §Limits states as the smaller claim it is | |
| Worker owner | Ross Todd for T-WORKER | Accept that `shutdown` now observes a voluntary exit without reaping it so the process group is still signallable, and that the tree is proven gone by group *and* recorded pidfd on both paths; and accept `ADR-0001-D008` as the owner-approved permission for the residual that a descendant calling `setsid()` between the last enumeration and its parent's exit is reachable by neither, expiring at E5-S4 and to be revisited before any worker pool above size one | |
| Contract owner (T-AUDIO) | Ross Todd for T-AUDIO | Accept the five-of-five T5 qualification result; defer candidate audio acceptance until the quiet-edge-normalized listening set is rendered and reviewed | |
