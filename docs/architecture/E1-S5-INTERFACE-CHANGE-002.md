# E1-S5 Interface Change 002 — The synthesis key follows the model's bytes

## Identification

- Record ID: `E1-S5-INTERFACE-CHANGE-002`
- Status: **Accepted, 2026-09-02**, together with the amendment it depends on. §Approval records
  the decision each role made and the date it was signed.
- Contract owner: T-CORE (`SynthesisContext`, the synthesis identity)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-WORKER, T-AUDIO
- Accepted ADR, if architectural:
  [`ADR-0001-D011`](../adr/deviations/ADR-0001-D011-model-artifacts-key-input.md), **Approved
  2026-09-02**. ADR-0001 §12.5 enumerates the synthesis-key inputs, so adding one needs an
  amendment rather than this record alone. Both were signed together, which is the only order
  that makes sense: this record is the change the amendment authorizes.

Issue #66, finding 2 of the 2026-08-31 audit of E1-S3: changed model weights kept an unchanged
synthesis identity. The gate added since refuses weights that are not the pinned bytes, so the
defect now needs a Git edit to reach — a commit that moves `DECLARED_MODEL_ARTIFACTS` and leaves
`PINNED_MODEL_REVISION` alone. This closes that mechanically.

## Version and compatibility

### `SynthesisContext` — new required input

| | Before | After |
|---|---|---|
| `model_revision` | names the acquisition | unchanged |
| `tokenizer_revision` | names the tokenizer acquisition | unchanged |
| `model_artifacts_hash` | — | BLAKE3 over the declared name, size, and SHA-256 of every pinned artifact |
| `SYNTHESIS_IDENTITY_VERSION` | `e1-s2-v1` | `e1-s5-v1` |
| `CACHE_SCHEMA_VERSION` | `2.0` | `3.0` |

**Breaking contract.** Two versions move for two independent reasons, and conflating them would
lose one: `SYNTHESIS_IDENTITY_VERSION` moves because the *input list* changed, which is the lever
that constant exists to be; `CACHE_SCHEMA_VERSION` moves because `ArtifactProvenance` gained a
*required field*, without which an entry cannot recompute the key it is published under.

`ModelArtifactsHash` is a value object rather than a `String`, for the reason `WorkerBundleHash` is
one: a digest typed as a string is one any caller can set to anything.

### The derivation

`model_gate::model_artifacts_hash` hashes `DECLARED_MODEL_ARTIFACTS` — the constant the gate
enforces — through `canonical_digest`, so the byte form stays owned by `study-tts-core` rather than
invented at the call site. Three decisions inside it:

- **Derived, not pinned as a second constant.** A pinned value would be a third thing to keep in
  step with the digests and the revision, and the first edit that moved the digests and forgot it
  would reintroduce the defect this closes.
- **Over the declaration, not over the files.** `verify_model_artifacts` has already proven the
  files *are* these bytes; re-reading 3 GB would learn what the constant states.
- **Name beside digest.** Two artifacts swapping names leave the multiset of digests unchanged and
  are a different model root. `t1_e1_two_artifacts_swapping_names_is_a_different_model_identity`
  is that case.

`verify_model_artifacts` now returns `ProvenModel` — the revision *and* the identity — because they
are one fact about one verified root, and a caller taking the revision alone could pair it with a
hash derived somewhere else.

## Impact

- **Synthesis identities:** every one moves. Intended, and the whole point.
- **Plan hashes:** move with them.
- **Verification identities:** unchanged. The disjointness guard in
  `crates/study-tts-core/src/verification.rs` was re-read against this field rather than left to
  compile, and the field is recorded there as synthesis-side.
- **Published schemas:** none. `cargo run --example generate-schemas` leaves `schemas/`
  byte-identical. `SynthesisContext` reaches the plan **hash**, never the plan **document** — issue
  #66 asserted a `plan-v3` → `plan-v4` move, and that was checked against the tree rather than
  inherited. It is not so.
- **Cache entries:** none rewritten, none re-keyed, none deleted. Existing entries stop being
  addressed, which is how every prior identity move in this project was handled.
- **Consumers:** `BackendDescriptor` carries the field so what reaches a cache entry is what the
  gate proved for the root that backend launched against. It comes from the configuration and not
  from the worker's answer, deliberately: the worker reads a record and cannot answer for bytes.
- **Fakes:** `PROTOCOL_FAKE_MODEL_ARTIFACTS_HASH`, distinct from `PROTOCOL_FAKE_BUNDLE_HASH` so a
  key derived under the fake names two synthetic identities rather than one repeated.
  `study_tts_testkit::DETERMINISTIC_TONE_MODEL_ARTIFACTS_HASH` is defined as that constant rather
  than a second copy of the literal.
- **Goldens:** `t1_e0_plan_is_stable_for_identical_inputs` carries a new plan hash and two new cache
  keys, with the reason recorded beside them as every prior move is.

## Delivery and recovery

- **Fake and shared-suite update before consumers:** the seam gained a field, and the fake and the
  executable protocol fake were given values before any consumer was touched.
- **Migration:** none required of any stored artifact.
- **Rollback:** as `ADR-0001-D011` §Rollback states. Nothing is rewritten in place in either
  direction, so no authoritative data is lost by reversing.
- **Compatibility evidence:** full CI-equivalent suite, reported step by step; no schema drift.
- **Ordering:** `verify_model_artifacts` runs inside `WorkerConfiguration::for_bundle`, whose
  launch fields are private and whose only constructor is that function — so a configuration that
  could start a real worker cannot exist unless the gate returned `Ok`. That is the type system
  making the ordering unrepresentable, which `rust-testing` ranks above an asserted one, and it is
  why issue #66's row asking for an ordering *test* is answered by a stronger mechanism than the
  one it asked for.

## Limits this change does not close

- **Acceptance is not qualification.** `ADR-0001-D011` was approved on 2026-09-02 and this record
  with it, so the change is authorized. Nothing about it has been *measured*: every key in the
  project has moved and no requalification, re-render, or listening review has run against the
  build that carries them.
- **The key follows the *declaration*, not a re-read of the files at launch.** If the pinned
  digests were wrong about bytes the gate then accepted, the key would be wrong in the same way —
  but that cannot happen, because the gate compares the files against those same digests and
  refuses on mismatch. The two are consistent by construction, which is the argument, not an
  absence of one.
- **A reviewer can still move both together.** A commit that edits the digests *and* bumps the
  revision moves the key twice over and is a governed-backend change under ADR-0002. That is the
  intended path, not a gap.
- **Issue #66's Level 1 is deliberately not implemented.** Reading the artifacts array from the
  model root's own `bundle-manifest.json` is trust on first use, and
  `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §The model root is pinned in Git records the
  decision to pin in Git instead. #66's rows 1 and 4 are answered by that design rather than
  implemented as written; the audit closing #66 should say so rather than leave them looking unmet.
- **No requalification has run.** Every key in the project has moved and nothing has re-rendered.

## Approval

**Every row below is signed.**

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that every synthesis key and plan hash moves before G1, and that the 2026-08-31 direction deferring this term is reversed | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-CORE) | Accept `model_artifacts_hash` as a `SynthesisContext` input, `SYNTHESIS_IDENTITY_VERSION` at `e1-s5-v1`, and `CACHE_SCHEMA_VERSION` at `3.0` | Accepted — Ross Todd, 2026-09-02 |
| Contract owner (T-WORKER) | Accept `ProvenModel`, the descriptor carrying the value from the configuration rather than the worker's answer, and the fake's distinct constant | Accepted — Ross Todd, 2026-09-02 |
| Affected track (T-AUDIO) | Accept that no published schema and no package identity moves | Accepted — Ross Todd, 2026-09-02 |
| Engineering owner | Accept the limits above, in particular that `ADR-0001-D011` is approved while no requalification has run | Accepted — Ross Todd, 2026-09-02 |

- Effective version and date: `SYNTHESIS_IDENTITY_VERSION` `e1-s5-v1` and
  `CACHE_SCHEMA_VERSION` `3.0`, effective 2026-09-02.
