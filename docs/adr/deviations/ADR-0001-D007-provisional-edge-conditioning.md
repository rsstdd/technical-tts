# ADR-0001-D007 — Edge conditioning proceeds against a provisional silence threshold

- **Status:** Approved
- **Date:** 2026-08-31
- **Controlling ADR and sections:** ADR-0001 §12.6 and §13.4, which delegate the silence
  threshold to ADR-0003
- **Requesting story:** E1-S3
- **Owner:** Engineering owner
- **Approver:** Ross Todd, engineering owner and project owner
- **Expiry:** Acceptance of ADR-0003. At that point the frozen value replaces the provisional one
  and this permission ends.

## Approved deviation

Permit E1-S3 to implement ADR-0001 §13.4 edge analysis, zero padding, and raised-cosine
transition ramps, and to apply them before cache publication, using a **provisional** silence RMS
threshold chosen by this build rather than the value ADR-0003 will freeze.

## The gap

ADR-0001 §13.4 requires each edge to be analyzed "in 5 ms RMS frames using the audio-profile
silence threshold". §12.6 makes duration, silence, and edge checks a condition of publishing or
reusing a cache entry at all. `DELIVERY-PLAN.md` E1-S3 task 4 is "Validate and condition
canonical audio before atomic cache publication".

The threshold those requirements depend on does not exist. `docs/adr/ADR-0003-production-audio-quality-profile.md`
is **Proposed; awaiting calibration**, and its calibration table records both the Silence RMS
threshold and the Join discontinuity threshold as `TBD` / `Pending`. `CLAUDE.md` §Conflict order
states that a Proposed ADR authorizes nothing.

So E1-S3 could not satisfy its own task 4 without either waiting for a calibration it does not
own or choosing a threshold no accepted document states.

Two further facts were put to the project owner before this was approved:

1. `DELIVERY-PLAN.md` assigns the same work to **E2-S3** — tasks 1 through 4 — with seven named
   tests, and story test names are contracts.
2. **E2-S3 depends on E1-S4**, which is not built, so doing the work in E1-S3 runs ahead of a
   declared dependency.

The project owner directed that the conditioning be implemented now regardless, accepting both.

## Conditions

1. The threshold is marked provisional in the type system, not only in prose.
   `SilenceThreshold::production` returns `ProvisionalCalibration` rather than a value, so a
   provisional number cannot reach a caller that requires a calibrated production reference.
   `t3_e2_provisional_measurement_cannot_satisfy_production_calibration` proves it.
2. Only the *threshold* is provisional. The geometry — 5 ms analysis frames, 10 ms of edge
   silence, a ramp no longer than 5 ms — is fixed by ADR-0001 itself and is implemented as
   ratified, pinned by `t1_e2_edge_geometry_matches_the_ratified_constants`.
3. The tests carry the names `DELIVERY-PLAN.md` gives them under E2-S3, so E2-S3 inherits the
   work rather than duplicating it under second names.
4. No preview audio produced under this deviation may be presented as meeting the ADR-0003
   production profile.

## What this does not permit

- **Join discontinuity** is not implemented. `t1_e2_discontinuity_threshold_is_enforced` measures
  the boundary *between* segments, which exists only at assembly, and its threshold is the second
  `Pending` row in the same ADR-0003 table. It stays with E2-S3.
- **Loudness normalization** is not implemented. `t4_e2_loudnorm_requires_linear_result` and
  ADR-0001 §13.3's two-pass normalization need FFmpeg and a calibrated target; both stay with
  E2-S3.
- ADR-0003 is not amended, and nothing here freezes any value in its calibration table.

## What this owes

At ADR-0003 acceptance: replace `PROVISIONAL_SILENCE_RMS` with the frozen value, change the
constructor's `CalibrationSource` to `Frozen`, and re-run the listening review. That migration must
also increment `SYNTHESIS_IDENTITY_VERSION` or `CACHE_SCHEMA_VERSION` before any prior entry is
reused. Both are inputs to `segment_digest`: an identity-version change makes the old key
unreachable, while a cache-schema change also makes `cache::load_validated` independently refuse
the old artifact version. Either route makes entries conditioned under the provisional threshold
misses rather than hits. Existing entries must be invalidated through that versioned identity
before reuse; changing only
`SilenceThreshold::provisional()` would publish different audio under an identity that still
accepts the old conditioning.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately,
which is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that the silence threshold this build applies is chosen rather than derived, that it is fenced by `SilenceThreshold::production` rather than by prose alone, and that the ADR-0001 geometry around it is implemented as ratified | 2026-08-31 |
| Project owner | Ross Todd | Approve — accept conditioning ahead of an accepted ADR-0003 and ahead of E2-S3's declared dependency on E1-S4, in exchange for edge conditioning existing before G1; accept that preview audio produced under it makes no ADR-0003 claim, and that acceptance of ADR-0003 obliges a threshold swap, a re-derived cache identity, and a retaken listening review | 2026-08-31 |
