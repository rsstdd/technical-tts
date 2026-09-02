# E1-S3 Interface Change 004 — Seeding before the model is constructed

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-004`
- Status: **Accepted, 2026-09-02.** §Approval records the decision each role made and the date it
  was signed. §Limits records what acceptance does *not* settle: the reference-machine
  reproducibility measurement has not run, and `deterministic_seed` stays `False` until it does.
- Contract owner: T-WORKER (`tts_executor`, the worker bundle identity)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-WORKER, T-CORE
- Accepted ADR, if architectural: not applicable. ADR-0001 §12.5 already names the seed among the
  synthesis-key inputs, and this makes the worker honor the seed it was already given. No
  authority boundary moves.

The shipped worker seeded `random`, NumPy, and Torch inside the `synthesize` handler and nowhere
else. `ChatterboxTTS.from_local` runs at `initialize`, before any of that — so everything model
construction drew from the global generators, the vocoder's noise among it, was drawn once per
process from a generator nobody had seeded.

The consequence is precise and worth stating in full, because it is not the obvious one: **each
lifetime was internally consistent and lifetimes disagreed with each other.** Two workers handed
the same request, the same seed, and the same bundle would render different audio, and the
ADR-0001 §12.5 cache key naming that seed could not see the difference. A second machine, or the
same machine after a restart, could therefore publish audio under a key that already described
somebody else's.

## Version and compatibility

### `worker/study_tts_worker/worker.py` — behavior, not shape

| | Before | After |
|---|---|---|
| Seeded at model construction | Not at all | `random`, `numpy.random`, `torch` from `launcher.seed` |
| Seeded per take | The same three, inline in the handler | Unchanged in effect, through the shared `_seed_generators` |
| `initialize` and `synthesize` frames | — | Unchanged |
| `capabilities.deterministic_seed` | `False` | `False`. See §Limits |

**Compatible patch by shape and Breaking by identity**, which is why this record exists rather
than a test alone. No frame, field, launcher key, or schema changes: a supervisor written against
the old worker speaks to this one unaltered. But `worker/study_tts_worker/worker.py` is a declared
bundle input under `worker/bundle-manifest.json`, so its bytes move
`WORKER_BUNDLE_IDENTITY_VERSION`'s hash, and that hash is an ADR-0001 §12.5 synthesis-key input.
Every synthesis key and every plan hash moves with it.

That is the intended and correct outcome, on the rule E1-S3's own record states: an identity moves
when a declared input changes, and this is a declared input changing. Nothing is stranded — no
entry is re-keyed, no cache is deleted, and any existing entry simply stops being addressed.

`_seed_generators` is one function called from both places rather than three lines written twice.
The two sites have to agree, and a generator added to one and forgotten in the other would leave a
lifetime whose first take was reproducible and whose second was not.

The per-take seeding is retained exactly as it was. Generation advances the global state, so a
second take under one seed would otherwise sample from wherever the first left off, and the seed
would describe only the first.

## Impact

- **Synthesis identities:** every one moves, through the worker-bundle hash. Intended.
- **Plan hashes:** move with the synthesis context.
- **Verification identities:** unchanged. `crates/study-tts-core/src/verification.rs` is separate
  from synthesis for exactly this reason, so an ASR-side identity cannot be disturbed by a
  synthesis-side change.
- **Cache and package identities:** no format moves; the keys addressing them do.
- **Schemas:** none. `cargo run --example generate-schemas` leaves `schemas/` unchanged.
- **`worker/bundle-manifest.json`:** unchanged. It declares input *paths* and installed
  distribution records, not the digests of repository files, so the hash moves at runtime with no
  edit here. `worker/tests/test_worker.py` is not a declared input, so the tests below do not move
  it.
- **Consumers:** `WorkerTtsExecutor` reads the new identity and reports it, unmodified.
- **Rust code:** none changed by this record.
- **Accepted evidence:** `evidence/gates/g1/e1-s4/e1-s4-minimal-package-generation-v1.md` records
  a package rendered at worker bundle
  `3e1f487cf259cd5b17bdeea16845c14426dbbded76f47732dd06b02198003747`, and the listening review
  taken against it. That identity is now historical. The record is accepted and is **not** edited;
  what it attests remains true of the bytes it names. A re-render and a fresh listening review are
  required before any G1 evidence describes the current worker, because the seeded decoder noise
  and every cache key have changed and a listening disposition does not transfer across them.

## Delivery and recovery

- **Fake and shared-suite update before consumers:** no seam changed shape. The protocol fake
  reports a fixed bundle hash of its own and is unaffected.
- **Migration:** none. Nothing stored has to be rewritten.
- **Rollback:** revert the commit; the previous bundle identity returns with the previous bytes.
- **Compatibility evidence:** `python3 -m unittest discover --start-directory worker/tests` — 66
  tests, 2 skipped for the absent restored environment. `python3 -m compileall` clean. The full
  Rust suite is unaffected and passes.
- **Ordering proof:** `test_every_generator_is_seeded_before_the_model_is_constructed` records the
  calls this module makes into recording `torch`, `numpy`, and `chatterbox` modules and asserts
  each seeding precedes `from_local`. It was confirmed to **fail** with the seeding moved after
  model construction — `random.seed must precede model construction, got ['torch.set_num_threads',
  'torch.set_num_interop_threads', 'from_local', 'random.seed', …]` — rather than assumed to
  discriminate. `test_the_seed_a_lifetime_uses_is_the_one_its_launcher_records` holds the second
  half: the value seeded is the launcher's, not a constant.
- **Reference-machine qualification:** **not run.** The criterion that would run it now exists —
  `t5_e1_two_lifetimes_render_identical_audio_under_one_seed` in
  `crates/study-tts-testkit/examples/worker-qualification.rs` — and its decision logic is covered
  at T1 by six tests in that example's own `tests` module, because the criterion itself can only
  run on the reference machine and nothing else would exercise the code deciding its verdict. Two
  of those tests were confirmed to fail under a mutated comparison rather than assumed to
  discriminate: `==` in place of bit-pattern equality, and a zip with no length term.
  `scripts/qualification/README.md` §Requalifying after the seeding change is the procedure. See
  §Limits for what running it does and does not settle.

## Limits this change does not close

- **`deterministic_seed` stays `False`, and this record does not propose flipping it.** What
  landed is a mechanism: the one defect that made the answer *necessarily* `False` is gone.
  Whether the answer is now `True` is a measurement — two fresh worker lifetimes producing
  identical decoded PCM and byte-identical canonical WAVs — and that measurement is now
  *instrumented* rather than performed. Only the reference machine has the weights, the lawful
  voice profile, and the qualified interpreter it needs. Claiming reproducibility before it runs
  would put an unproven claim into every cache key, which is the precise thing the capability's
  own comment refuses.
- **The criterion is not a `DELIVERY-PLAN.md` name**, and none was added: that document is
  digest-pinned by accepted evidence, so adding one is a project-owner edit with a provenance
  recomputation behind it. It has the standing
  `t5_e1_worker_survives_restart_and_starts_offline` already has — a helper criterion the README
  records as such.
- **The criterion's verdict is byte equality, and sample equality is only reported.** That is
  deliberate, because a cache entry is validated and addressed by the bytes of its canonical WAV:
  two renders whose audio agrees and whose containers do not still publish different bytes under
  one key. What the sample comparison buys is the diagnosis — audio that differs is a sampler that
  is not reproducible, while identical audio in differing containers is a reproducible sampler and
  an artifact that is not — and the observation says which.
- **The criterion costs two more model loads per qualification run**, on top of the restart
  criterion's two.
- **Four call sites move with that flip, when it is made.**
  `crates/study-tts-runtime/src/worker_protocol.rs` declares `deterministic_seed` and is a
  two-sided coupling with `worker/study_tts_worker/protocol.py`;
  `crates/study-tts-runtime/src/worker_executor.rs` maps the capability onto `determinism_class`;
  `crates/study-tts-core/src/identity.rs` spells `reproducible` and `seeded_nondeterministic`; and
  a test in `crates/study-tts-runtime/src/cache.rs` pins `"seeded_nondeterministic"`. They are
  named here so the flip is a design step rather than a CI discovery.
- **Reproducibility is not defined in a ratified document.** The flip needs one — *the same
  request, seed, and worker bundle produce identical canonical audio across restarts* — and
  ADR-0002 is the document that should carry it. ADR-0002 is digest-pinned by accepted evidence,
  so it cannot be edited in place; whether that definition arrives as an
  `docs/adr/deviations/ADR-0002-D…` amendment or a superseding ADR version is a project-owner
  decision this record does not make.
- **NumPy's legacy global generator takes 32 bits**, so the seed is reduced for it alone. That is
  deliberate: the value is a coordinate, not an identity, and the identity reaching a synthesis
  key is the unreduced `seed` the Rust end reads from the same launcher.
- **Torch's own reproducibility knobs are untouched.** `torch.use_deterministic_algorithms`,
  cuDNN determinism, and thread-count effects on reduction order are not set or measured here.
  On CPU with `torch.set_num_threads(threads)` already pinned by ADR-0001 §10.1 they may not
  matter; "may not" is the honest statement, and the reference-machine run is what would replace
  it.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that the worker bundle identity moves, that every synthesis key and plan hash moves with it, and that the accepted E1-S4 package and its listening review become historical rather than current | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-WORKER) | Accept seeding at model construction as well as per take, `_seed_generators` as the single site, and `deterministic_seed` remaining `False` until measured | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-CORE) | Accept that no frame, launcher field, or schema moves, and that verification identities are untouched | Accepted — Ross Todd, 2026-09-02 |
| Engineering owner | Accept the limits recorded above, in particular that the reference-machine reproducibility criterion has not been run | Accepted — Ross Todd, 2026-09-02 |

- Effective version and date: worker seeding before model construction, effective 2026-09-02.
