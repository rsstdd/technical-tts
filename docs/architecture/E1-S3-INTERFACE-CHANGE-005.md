# E1-S3 Interface Change 005 — The worker declares reproducible synthesis

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-005`
- Status: **Accepted, 2026-09-02.** §Approval records the decision each role made and the date it
  was signed. §Limits records what acceptance does not settle: the claim is bounded to this
  environment, one seed, and one sentence.
- Contract owner: T-WORKER (`capabilities.deterministic_seed`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-WORKER, T-CORE
- Accepted ADR, if architectural: not applicable. ADR-0001 §12.5 already names `determinism_class`
  a synthesis-key input; this changes the *value* the worker declares, not the input set.

`E1-S3-INTERFACE-CHANGE-004` landed the seeding mechanism and said in as many words that it did
**not** propose flipping the capability, because whether the answer had become `True` was a
measurement nobody had taken. This is that measurement, and the flip it warrants.

## Version and compatibility

| | Before | After |
|---|---|---|
| `capabilities.deterministic_seed` | `False` | `True` |
| `determinism_class` in every synthesis key | `seeded_nondeterministic` | `reproducible` |
| Worker bundle identity | `2206e9c8f1a7b4db56ffec7627d2bddcfacbd254e41f6c5e9005737f722dd972` | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |

**Breaking contract**, twice over and independently: `determinism_class` is an ADR-0001 §12.5 key
input, and `worker.py` is a declared bundle input, so both the value and the identity carrying it
move. Every synthesis key and plan hash moves.

No frame, field, launcher key, or schema changes shape. `worker_protocol.rs`'s
`deterministic_seed` field, `worker_executor.rs`'s mapping onto `DeterminismClass`, and
`identity.rs`'s two spellings were all written to carry either value and are untouched — the four
call sites `E1-S3-INTERFACE-CHANGE-004` §Limits named turn out to be mechanisms rather than
constants, which is why this change is two lines and a comment.

## The measurement

`t5_e1_two_lifetimes_render_identical_audio_under_one_seed`, on the reference machine, inside a
namespace holding only `lo` with no IPv4 route:

| Item | Value |
|---|---|
| Bundle measured | `2206e9c8f1a7b4db56ffec7627d2bddcfacbd254e41f6c5e9005737f722dd972` |
| Launcher seed | `42` |
| Frames compared | 92 160 |
| Frames differing by bit pattern | **0** |
| Canonical WAV, both lifetimes | SHA-256 `6b641ad8f265c1c10d91234e80a7d0a9e751857947e0c6a7995381d751b63d5e` |
| Result artifact | SHA-256 `7ff5160fd442579ea2a80818e9f7c3fd0a05ada1481de0179de634f94aaf0414` |

**Requalified after the flip**, because a measurement taken on the build before a change does not
describe the build after it. All six criteria pass at
`1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`, result artifact SHA-256
`bebee3e0b2c5e0bbe6586ef65d2a5918f57537088d25535477c2097a98b8d4c0`. The canonical WAV digest is
`6b641ad8f265c1c10d91234e80a7d0a9e751857947e0c6a7995381d751b63d5e` in **both** runs, which is the
result to expect and worth stating: declaring a capability moves the key that addresses audio, not
the audio.

**The first run of that criterion failed, and the failure is worth recording rather than
discarding.** It compared the takes as the worker staged them, and those differed by exactly one
byte at offset 61 — inside libsndfile's `PEAK` chunk, which carries the wall-clock time of the
write (`0x6a981564` against `0x6a981587`, thirty-five seconds apart). The audio was already
bit-identical. `cache::write_canonical_samples` decodes, conditions, and rewrites every take
through `hound` from samples and a fixed spec, so no published entry carries that chunk; the
criterion was measuring an artifact the gate does not name. It now re-encodes both takes exactly
as the cache would and compares those bytes, with the staged digests reported and never judged.
`t1_e1_a_container_timestamp_does_not_make_two_takes_irreproducible` is the regression.

## Impact

- **Synthesis identities and plan hashes:** every one moves, for both reasons above.
- **Verification identities:** unchanged.
- **Published schemas:** none. `generate-schemas` leaves `schemas/` byte-identical.
- **Cache entries:** none rewritten or re-keyed; entries under the old class stop being addressed.
- **Fakes:** unaffected. `fake-ndjson-worker` already declared `deterministic_seed: true`, and
  `FakeTtsExecutor` already reported `DeterminismClass::Reproducible` — the fake has always
  claimed what the real worker can now claim, which is why no shared-suite expectation moves.
- **Tests:** `test_capabilities_declares_no_voice_until_one_is_loaded` asserts the declaration, so
  it cannot drift silently. Sample values spelling `SeededNondeterministic` in
  `identity.rs`, `synthesis.rs`, and a `cache.rs` fixture are arbitrary test inputs rather than
  claims about the shipped worker, and are deliberately left alone.

## Limits this change does not close

- **Reproducibility is claimed for this environment, not in general.** ADR-0001 §12.5 says
  identical seeds do not guarantee identical output across dependency, platform, or execution
  changes, and this measurement was taken on one machine, one bundle, one seed, one sentence.
  E0-S3 declined to generalise past "this environment and bounded run set" and this record makes
  the same refusal.
- **One request, not a corpus.** The criterion renders a single sentence in each lifetime. It would
  not see a divergence that only appears in longer text or another voice.
- **Torch's determinism knobs remain unset.** `use_deterministic_algorithms` is not called. The
  result says the current configuration is reproducible, not that it is pinned against a future
  Torch changing a kernel.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that every synthesis key and plan hash moves again, days after the E1-S5 move, and that the E1-S4 package and listening review recede one identity further into history | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-WORKER) | Accept `deterministic_seed: True` on the measurement above, and the bounded scope §Limits states | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-CORE) | Accept `determinism_class` becoming `reproducible` in every key, with no schema or frame change | Accepted — Ross Todd, 2026-09-02 |
| Engineering owner | Accept the corrected criterion, and that its first failure was a defect in the instrument rather than in the worker | Accepted — Ross Todd, 2026-09-02 |

- Effective version and date: `deterministic_seed` `True` and `determinism_class`
  `reproducible`, effective 2026-09-02, at worker bundle
  `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`.
