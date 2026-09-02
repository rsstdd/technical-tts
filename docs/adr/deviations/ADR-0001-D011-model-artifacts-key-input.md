# ADR-0001-D011 — The synthesis key names the model's bytes as well as its revision

- **Status:** Proposed. Nothing below is approved; §Decision carries no signature.
- **Date raised:** 2026-09-02
- **Controlling ADR and sections:** ADR-0001 §12.5, which enumerates the synthesis-key inputs
- **Requesting story:** E1-S5, for issue #66
- **Owner:** Engineering owner
- **Approver:** Project owner and engineering owner
- **Expiry:** None. This adds an input to a ratified enumeration; it does not resolve itself.

## Proposed amendment

Admit **`model_artifacts_hash`** — BLAKE3 over the declared name, size, and SHA-256 of every model
artifact this build is pinned to — as a synthesis-key input under ADR-0001 §12.5, alongside the
`model_revision` and `tokenizer_revision` that section already enumerates.

An amendment rather than an interface-change record alone, because §12.5 *enumerates* its inputs.
A record can describe how an enumerated input is computed; it cannot add one. That reading is not
new here: the project owner stated it on 2026-08-31, and
`crates/study-tts-runtime/src/model_gate.rs` has recorded it since.

## Why the accepted design cannot be followed

§12.5 keys audio on `model_revision`, which names an **acquisition**. It does not name the
acquisition's **bytes**. In ordinary use the distinction is invisible, because qualifying a new
revision moves the revision string and every key with it.

The two come apart in exactly one case, and it is a case this repository can reach: a commit that
edits the pinned artifact digests in `DECLARED_MODEL_ARTIFACTS` without moving
`PINNED_MODEL_REVISION`. The gate then proves the *new* bytes, the key stands still, and audio
rendered from the old weights is reused for the new ones. A content-addressed cache must never do
that. Issue #66 is that defect, raised as finding 2 of the 2026-08-31 audit of E1-S3.

The refusal already in place does not close it. `verify_model_artifacts` refuses weights that are
not the pinned bytes, which is why the defect needs a Git edit to reach — but the pins and the
revision live in the same tree, so one reviewed commit can move one and not the other. Making the
key follow the digests closes it mechanically rather than by review.

## Impact

- **Architecture and authority boundaries:** none. No boundary moves; §12.5 gains one input.
- **Schemas and interfaces:** no *published* schema moves.
  `cargo run --example generate-schemas` leaves `schemas/` byte-identical, which is the check that
  shows it: `SynthesisContext` reaches the plan **hash**, never the plan **document**. Issue #66
  asserted a `plan-v3` → `plan-v4` move; that was checked against the tree and is not so.
  `CACHE_SCHEMA_VERSION` moves `2.0` → `3.0`, because `ArtifactProvenance` gains a required
  `model_artifacts_hash` without which an entry cannot recompute the key it is published under.
  That record is internal, not one of the seven published schemas.
- **Identities:** `SYNTHESIS_IDENTITY_VERSION` moves `e1-s2-v1` → `e1-s5-v1`. **Every synthesis key
  and every plan hash moves.** Verification identities do not:
  `crates/study-tts-core/src/verification.rs` is separate from synthesis precisely so an ASR-side
  identity cannot be disturbed by a synthesis-side change, and the disjointness guard in that
  module was re-read against this field rather than left to compile.
- **Security, rights, and privacy:** none. The value is a hash of digests of public third-party
  weights, already in Git. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps governed
  *locations* and *bytes* out of the repository, not checksums of public weights — and this does
  not extend to voice digests, which stay in the governed voice root.
- **Tests and evidence:** `t2_e1_every_speech_affecting_field_changes_synthesis_key` gains the
  input and was confirmed to fail when the key is made blind to it.
  `t1_e1_every_declared_artifact_field_changes_the_model_identity` and
  `t1_e1_two_artifacts_swapping_names_is_a_different_model_identity` pin the derivation.
  `t1_e0_plan_is_stable_for_identical_inputs` carries the new golden plan hash and cache keys, with
  the reason for the move recorded beside them as every prior move is.
- **Existing artifacts and migration:** no artifact is rewritten and no entry is re-keyed. Existing
  cache entries stop being addressed and are left alone, which is the same treatment every prior
  identity move in this project received.
- **Schedule and scope:** before G1. `ADR-0001-D005`'s pre-freeze permission expires at the freeze,
  and after it the same change needs the full **Breaking contract** migration and rollback
  procedure `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes requires.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Leave the key on `model_revision` alone | The defect stays: a digest edit with no revision bump keeps every key still. This is what was deferred on 2026-08-31, and the freeze is what changes the calculus |
| Read the artifact digests from the model root's `bundle-manifest.json` and key on those, per issue #66's Level 1 | `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` rejects reading that record, and the reason holds: a digest list beside the weights is trust on first use, since whoever can replace the weights can replace the list |
| Re-hash the artifact files at launch and key on that | Re-reads 3 GB to learn what the gate has just proven. The declaration the gate enforced *is* the identity of the bytes it enforced them to be |
| Pin `model_artifacts_hash` as a second Git constant beside the digests | A third value to keep in step. The first edit that moved the digests and forgot it would reintroduce exactly this defect, so it is derived instead |

## Rollback

Remove the field from `SynthesisContext`, restore `SYNTHESIS_IDENTITY_VERSION` to `e1-s2-v1` and
`CACHE_SCHEMA_VERSION` to `2.0`, and revert the golden in `plan.rs`. Keys return to their previous
values and any entry written under `e1-s5-v1` stops being addressed. No authoritative data is lost:
nothing is rewritten in place at any point, in either direction.

## Decision

- [ ] Approve through an ADR amendment
- [ ] Reject
- [ ] Defer

Approvers and date:
