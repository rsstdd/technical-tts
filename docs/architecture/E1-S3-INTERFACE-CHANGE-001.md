# E1-S3 Interface Change 001 — Resolved voices, and a reported one

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-001`
- Status: **Accepted, 2026-08-30.** §Approval records the decision each role made and the date
  it was signed.
- Contract owner: T-CORE (plan document); T-WORKER (executor contract, worker frames)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-WORKER, T-AUDIO
- Accepted ADR, if architectural: not applicable. This implements ADR-0001 §10.2 (`synthesize`
  takes a voice profile), §10.3, §12.5 (the voice-conditioning artifact hash is the key input),
  and §12.6 as written. No authority boundary moves.

This record pays the three debts
[`E1-S2-INTERFACE-CHANGE-001.md`](E1-S2-INTERFACE-CHANGE-001.md) §Limits this change does not
close assigned to this story by name:

> **`SynthesisRequest::voice` still carries the speaker name**, not the resolved profile
> identity … E1-S3 consumes the value and owns that move.

> **Generation parameters remain empty**, unchanged from E1-S1 and still owed by E1-S3.

> **E1-S3 owns closing this**, and closing it means the Chatterbox worker reporting the
> conditioning artifact it read from disk, never the value it was handed.

## Version and compatibility

Three provisional seams move together, in one commit, because they describe one exchange: the
planner resolves a voice, the request carries it, and the worker reports back which artifact that
resolved to. Splitting them would leave a build that can name a voice it cannot report on.

`ADR-0001-D005` was considered and **does not apply**. Its condition 2 requires the version being
retained to have been introduced by an unreleased breaking move *within the same story*; these
versions are E0-S4's and E1-S2's. Each therefore takes a full major increment, per
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes.

### Plan document — `2.0` → `3.0`

`PlannedSegment` gains a required `voice_profile`: the profile identity the lesson binds the
segment's speaker to, resolved once by `RenderPlan::for_lesson` instead of at the worker boundary.
A worker has never seen a lesson and cannot resolve a speaker name itself.

It is **in the plan identity and not in the cache key**. Two profile directories holding identical
conditioning artifacts derive one key, so a plan rebound between them would otherwise read as
unchanged while its consent trail moved; ADR-0001 §12.5 keys on the artifact, so renaming a profile
re-plans without re-rendering a segment. `t1_e0_plan_is_stable_for_identical_inputs` pins both
halves — the plan hash moved to `abd889db…`, and the two cache keys beside it did not.

`schemas/plan-v2.schema.json` is deleted and `schemas/plan-v3.schema.json` added.

### `TtsExecutor` — `e1.tts-executor.2.0` → `e1.tts-executor.3.0`

- `SynthesisRequest` gains required `voice_profile`, which is what the `synthesize` frame's
  `voice` field has always meant.
- `SynthesisReport::voice_profile_hash` is replaced by `voice_conditioning_hash` plus
  `voice_profile`. The executor now builds `SynthesisReport::context` from the artifact the
  **worker** reported rather than from the request it was handed, which is the whole of the third
  debt above.

### Worker frames — `e1.worker.1.0` → `e1.worker.2.0` (extension `1.1` → `2.1`)

- `InitializeParameters` gains a required `staging_root`: the one directory the worker may write
  inside, for its lifetime. Until it existed the worker was told one output path and no root, so
  it could inspect only the spelling of the path it was handed —
  `t5_e1_worker_output_cannot_escape_staging_root` therefore asserted a property neither end
  could prove, and both the qualification instrument and the E1-S3 story record recorded that as
  a limitation. The worker now resolves the root once at `initialize` and decides containment
  against the resolved parent of every assigned path, so a path whose components are all inside
  the root by spelling but whose parent is a symlink out of it is refused.

  A required field is a **Breaking contract** change. It is folded into the unreleased
  `e1.worker.2.0` rather than moved to a third major, on the reasoning `ADR-0001-D005` approved
  for E1-S1: `2.0` has never existed outside this working tree, no durable artifact and no
  evidence record outside `Proposed` was written under the shape being corrected, and supervisor,
  worker, fake, tests, fixtures and generated schema move together.
- `WorkerInitializationIdentities::voice_profile_hashes` becomes `voice_conditioning_hashes`,
  mapping profile identity to the conditioning artifact the worker read for it. `initialize`
  carries no voice list, so every entry is something the worker went and looked at.
- `SynthesisSucceeded::voice_profile_hash` becomes `voice_conditioning_hash` plus `voice_profile`.

`schemas/worker-protocol-v1.schema.json` is deleted and `schemas/worker-protocol-v2.schema.json`
added: `WORKER_PROTOCOL_SCHEMA_VERSION` tracks the wire version rather than counting separately,
because a published schema describing frames this build no longer sends would describe nothing.
That path is in `REQUIRED_BUNDLE_INPUTS`, so `worker/bundle-manifest.json` moves with it.

### Why the reported digest is a conditioning hash, not a profile hash

**The worker cannot compute BLAKE3.** `hashlib` offers blake2 only, no locked distribution
provides blake3, and `scripts/qualification/chatterbox_spike.py` — which needed one — shelled out
to a `blake3` **binary**. A production worker spawning an unbundled external tool contradicts the
process-boundary rules in `AGENTS.md`, and adding the distribution would require the full
`docs/operations/WORKER-ENVIRONMENT.md` §Regenerating the lock procedure and a re-qualified
environment for one digest.

So the worker reports the `conditionals_blake3` recorded in the `profile.json` it read from **its
own voice root**. That is a value it went and read rather than one it was handed, which is what
the third debt asks for, and the artifact-versus-record half is already covered on the Rust side:
`voice_gate::load_profile` verifies `conditionals.pt` against that same recorded digest before any
synthesis runs. §Limits below records what the split does not cover.

`VoiceProfileHash` has no production derivation anywhere in this tree — it was reachable only from
a test — so nothing is lost by the frames no longer asking for one.

### Launcher record — `1.0` → `1.1`

`worker/launcher.json` gains `seed`, `model_repository`, `generation_parameters`, and
`voice_root_environment_variable`. The first three are ADR-0001 §12.5 key inputs that were
configured nowhere: `generation_parameters` was the empty map the second debt names. They live in
this file because both ends read it — Rust keys them, the worker uses them — and a parameter one
end keyed while the other used something else would name audio the key does not describe.

**Generation parameters are strings.** ADR-0001 §12.5 admits no floating point into an identity,
and there is no encoding of `0.05` two builds are guaranteed to agree on. The launcher records the
exact spelling, the key hashes that spelling, and the worker parses it once at the call.
`string_map` in `worker/study_tts_worker/protocol.py` refuses one written as a number.

## Impact

- **Synthesis and cache identities.** The worker-bundle identity moved from
  `75d563103eccc766…` to `84baafe98bf861cb…`: `worker/launcher.json`, `worker/bundle-manifest.json`,
  `worker/study_tts_worker/protocol.py`, `worker/study_tts_worker/worker.py`, and the published
  protocol schema are all declared bundle inputs. Every cache key moves with it. **No artifact is
  stranded**: the cache root holds no entries, and this build's worker still refuses `synthesize`,
  so nothing has ever been published under any synthesis key.
- **Plan identities.** Every plan hash moves; no cache key moves for that reason alone.
- **Durable formats.** `ArtifactProvenance` carries `voice_profile` where it carried
  `voice_profile_hash` — the profile identity names the directory holding the consent decision a
  reviewer follows, which is what the field is read for, and it is a value a worker can produce.
  Neither is a synthesis-key input. No stored artifact exists to migrate.
- **Rights and privacy.** Unchanged and slightly narrowed: the launcher names the voice root by
  *environment variable*, never by path, so `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps
  the governed root out of this committed file exactly as it does for the model root.
- **Tests and evidence.** The workspace suite is 332 and the Python worker suite 45, both green.
  `t3_e1_published_schema_required_fields_match_the_recorded_surface` required both new surfaces
  to be recorded before it would pass, which is the guard working.

## Delivery and recovery

Every end moved in the same change, in the order
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Amendment rules before G1 requires:
shared fixtures (`fixtures/contracts/`), the Python parser and its shape, the Rust frames, the
executable fake, the generated schemas, then the consumers. `docs/testing/TEST-DATA-MANIFEST.md`
re-pins the five fixtures whose bytes moved.

Recovery is deletion rather than migration, because nothing durable was written under the old
shapes: revert the seams together and regenerate the schemas.

## Limits this change does not close

- **The worker does not re-hash the conditioning artifact it loads.** It reports the digest
  `profile.json` records and Rust verifies that record against the file. A voice root whose
  `profile.json` and `conditionals.pt` were edited *together* would satisfy both ends. Closing it
  needs a digest the worker can compute — see §Why the reported digest is a conditioning hash —
  and is a dependency decision, not a code one.
- **The real Chatterbox backend is not in this record.** These are the interfaces it needs;
  `worker/study_tts_worker/worker.py` still refuses `initialize` and `synthesize` naming E1-S3.
- **`SynthesisReport::voice_profile` is not checked against the request.** The cache's identity
  gate compares the conditioning artifact, which is the key input; a worker reporting a profile
  identity that disagrees with the one it was asked for would be caught only if the artifact
  differed too.

## Approval

**Every row below is signed, on 2026-08-30.** Each records a decision a role was asked for and has
now made.

Ross Todd holds each role below under
`docs/governance/PROJECT-EXECUTION-CHARTER.md`; each row records that role's separate decision and
accepted risk, which is why the rows stay separate for one signatory.

This acceptance covers the three interface moves recorded here. It does **not** accept
`evidence/gates/g1/e1-s3/e1-s3-single-worker-synthesis-and-validated-cache-v1.md`, which stays
`Proposed` until G1 for the reason `evidence/README.md` §Accepting a record at its gate gives.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept three provisional seams taking major increments in one change, on the reasoning that they describe one exchange and that `ADR-0001-D005` does not reach them | Accepted — Ross Todd, 2026-08-30 |
| Engineering owner | Accept the worker-bundle identity moving to `84baafe98bf861cb…`, that no artifact is stranded because none was ever published, and the plan, executor, and frame shapes recorded above | Accepted — Ross Todd, 2026-08-30 |
| Worker owner (T-WORKER) | Accept that the worker reports the conditioning digest its own `profile.json` records rather than one it computes, that BLAKE3 is unavailable to the worker environment, and the residual gap stated in §Limits | Accepted — Ross Todd, 2026-08-30 |
