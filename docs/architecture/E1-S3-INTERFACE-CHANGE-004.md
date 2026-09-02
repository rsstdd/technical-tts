# E1-S3 Interface Change 004 — Seeding before the model is constructed

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-004`
- Status: **Proposed.** §Approval carries no signature. Nothing here may be read as accepted.
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
- **Reference-machine qualification:** **not run.** See §Limits.

## Limits this change does not close

- **`deterministic_seed` stays `False`, and this record does not propose flipping it.** What
  landed is a mechanism: the one defect that made the answer *necessarily* `False` is gone.
  Whether the answer is now `True` is a measurement, and the measurement is two fresh worker
  lifetimes on the reference machine producing identical decoded PCM and byte-identical canonical
  cached WAVs. Nothing in this repository can run that. Claiming reproducibility before it runs
  would put an unproven claim into every cache key, which is the precise thing the capability's
  own comment refuses.
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

**No row below is signed.** Each records a decision a role is being asked for.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that the worker bundle identity moves, that every synthesis key and plan hash moves with it, and that the accepted E1-S4 package and its listening review become historical rather than current | Pending |
| Contract owner (T-WORKER) | Accept seeding at model construction as well as per take, `_seed_generators` as the single site, and `deterministic_seed` remaining `False` until measured | Pending |
| Contract owner (T-CORE) | Accept that no frame, launcher field, or schema moves, and that verification identities are untouched | Pending |
| Engineering owner | Accept the limits recorded above, in particular that the reference-machine reproducibility criterion has not been run | Pending |

- Effective version and date: not effective; `Proposed`.
