# E1-S3 Interface Change 002 — Conditioning recorded, and checked on reuse

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-002`
- Status: **Accepted, 2026-08-31.** §Approval records the decision each role made and the date it was signed.
- Contract owner: T-CORE (`CACHE_SCHEMA_VERSION`); T-AUDIO (the cache-entry record)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CORE, T-AUDIO
- Accepted ADR, if architectural: not applicable. This implements ADR-0001 §11.1, §12.6, and
  §13.4 as written. No authority boundary moves.

[`E1-S3-INTERFACE-CHANGE-001.md`](E1-S3-INTERFACE-CHANGE-001.md) is **Accepted** and is not
edited. This record stands beside it, in the numbering E1-S2 established.

An audit found that ADR-0001's edge conditioning was applied at publication and then forgotten:

> `CacheArtifact` has no head/tail padding or ramp counts. On reuse, the cache checks only that
> the first and last samples are zero; it does not establish 10 ms edge silence or valid
> transition ramps.

Both halves were confirmed. ADR-0001 requires the counts twice — §11.1 and §13.4 ("It records the
padding and ramp sample counts") — and `EdgeConditioning`'s own doc comment already claimed
compliance while `condition_staged_audio` discarded the value. §12.6 conditions *using* an entry
on a silence check that was never performed: an entry whose first and last samples happened to be
zero was a cache hit however its edges were shaped.

## Version and compatibility

### Cache-entry record — `CACHE_SCHEMA_VERSION` `1.0` → `2.0`

`artifact.json` gains a required `edge_conditioning` object:

```json
"edge_conditioning": {
  "leading_padding": 240,
  "trailing_padding": 240,
  "leading_ramp": 120,
  "trailing_ramp": 120,
  "calibration_source": "provisional"
}
```

The four counts are what ADR-0001 §11.1 and §13.4 require recorded. `calibration_source` is not in
that list and is added deliberately: the counts describe a *silence threshold*, and this build
conditions against a provisional one under `ADR-0001-D007` because ADR-0003 is Proposed and
records the value as pending. Without the field, an entry conditioned against the provisional
threshold is indistinguishable from one conditioned against the value ADR-0003 will freeze. It is
the enum, never the RMS value: a float in a durable record is a round-trip hazard, and
`crates/study-tts-runtime/src/audio_edges.rs` keeps floats out of anything an identity reads.

A required field is **Breaking contract** under
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes, so the major increments.

`ADR-0001-D005` was considered and **does not apply**, for the reason
`E1-S3-INTERFACE-CHANGE-001` §Version and compatibility gives for its own three seams: condition 2
requires the version being retained to have been introduced by an unreleased breaking move *within
the same story*, and `CACHE_SCHEMA_VERSION` moved to `1.0` in E1-S1.

### The `cache_publication` seam does not move

`CACHE_PUBLICATION_CONTRACT_VERSION` stays `e0.cache-publication.1.0`. That version covers the
Rust seam `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records — `CachePublisher`,
`CacheResolveRequest`, `StagedAudioProducer`, and the opaque `ValidatedCachedArtifact` — and none
of them changes shape. No accessor for the conditioning counts is added, because nothing consumes
them yet; adding one for a future reader would move the seam for no caller.
`CACHE_SCHEMA_VERSION` is precisely the version of the record on disk, and it is what moves.

### Acceptance is narrower than it was

Three refusals are added to `load_validated`, and each refuses an entry this build previously
accepted. That is a semantic change to cache acceptance, carried by the same major increment:

| New refusal | What it establishes |
|---|---|
| `AudioFault::InsufficientEdgeSilence` | ADR-0001 §12.6's *silence* check, re-measured from the audio through the same `measure_edge_silence` the conditioner pads from |
| `CacheEntryFault::ConditioningOutsideRatifiedGeometry` | The declared counts lie inside the geometry §13.4 fixes |
| `CacheEntryFault::ConditionedUnderAnotherCalibration` | The entry was conditioned under the calibration this build applies |

**What is verified and what is attested.** The silence is re-measured and has real force. The ramp
is **not recoverable**: a raised-cosine gain multiplied into arbitrary speech cannot be separated
from it afterwards, so the recorded ramp count is attested by whoever wrote the entry, and the
only available check is that it lies within the ratified bound. This record states that limit
rather than leaving a reader to infer a verification that does not exist.

The silence is measured against the threshold, not against exact zero. Conditioning pads only
until an edge *has* its 10 ms, so audio that already began quiet-but-nonzero is lawfully unpadded;
the measured silent region is normalized to zero so the exposed-endpoint check still holds.

## Impact

- **Synthesis and cache identities.** `CACHE_SCHEMA_VERSION` is an ADR-0001 §12.5 key input
  (`crates/study-tts-core/src/identity.rs`), so **every cache key moves**, and with it every plan
  hash. `t1_e0_plan_is_stable_for_identical_inputs` re-pins all three goldens: the plan hash to
  `46bf2c57d31eb5cf…`, superseding the `abd889db…` `E1-S3-INTERFACE-CHANGE-001` §Plan document
  cites, and the two cache keys to `01ffb5593c2e0daa…` and `d4248913a9a39a2e…`. Unlike the two
  moves that record describes, this one *is* a cache-wide invalidation — which is what the
  constant being a key input is for.
- **No artifact is stranded.** The cache root under `data/qualification/cache` holds no files, and
  no fixture or evidence record carries a derived cache key: the fixture keys in
  `fixtures/contracts/` are synthetic placeholders. Nothing has to be migrated or deleted.
- **Durable formats.** `artifact.json` only. No published JSON schema covers the cache entry, so
  `schemas/` is unchanged.
- **Rights and privacy.** No change. The recorded counts are sample counts and an enum; nothing
  governed enters the record.
- **Audio.** No published byte changes. This records and checks; it conditions nothing
  differently, so the listening set rendered on 2026-08-31 is unaffected.

## Delivery and recovery

Every end moves in one change, in the order §Amendment rules before G1 requires: the shared
measurement (`measure_edge_silence`, `samples_for`) before its two callers, the record shape before
the checks that read it, then the goldens. There is no fake or fixture to move first — the cache
entry is not a wire format and no fixture declares one.

Recovery is deletion rather than migration, because nothing durable was written under the old
shape: revert the constant and the record together, and the goldens recompute.

## Limits this change does not close

- **The ramp is attested, not verified**, as §Version and compatibility states. Closing it would
  need the conditioner to record something recoverable from the audio, and no such quantity
  exists for a gain multiplied into speech.
- **Join discontinuity is still not checked.** ADR-0001 §13.4 requires joins verified against the
  ADR-0003 discontinuity threshold; that threshold is `Pending` and the check belongs to assembly,
  not to one entry. `ADR-0001-D007` §What this does not permit already records it.
- **The calibration check has no live counterpart yet.** Every entry this build writes records
  `provisional`, so the refusal fires only once an accepted ADR-0003 gives this build a frozen
  threshold. That is the point of recording it, and `ADR-0001-D007` expires at the same moment.

## Approval

**Every row below is signed, on 2026-08-31.** Each records a decision a role was asked for and has
now made.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate for one signatory. A row is signed by recording the deciding role's
name and the date beside it, which every row now carries.

This acceptance covers the contract this record describes. It does **not** accept
`evidence/gates/g1/e1-s3/e1-s3-single-worker-synthesis-and-validated-cache-v1.md`, which stays
`Proposed` until G1: an interface record accepts a contract change, never the story that carried it.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept `CACHE_SCHEMA_VERSION` taking a major increment to `2.0`, invalidating every cache key and plan hash, on the reasoning that a required field is a Breaking contract and `ADR-0001-D005` does not reach a version another story introduced | Accepted — Ross Todd, 2026-08-31 |
| Contract owner (T-CORE) | Accept the three golden identities moving to `46bf2c57d31eb5cf…`, `01ffb5593c2e0daa…`, and `d4248913a9a39a2e…`, superseding the plan hash `E1-S3-INTERFACE-CHANGE-001` cites, and that no artifact is stranded because the cache root holds none | Accepted — Ross Todd, 2026-08-31 |
| Contract owner (T-AUDIO) | Accept the `edge_conditioning` record including `calibration_source`, the three narrowed acceptance rules, and the stated limit that the ramp count is attested rather than verified | Accepted — Ross Todd, 2026-08-31 |

- Effective version and date: **2026-08-31.** `CACHE_SCHEMA_VERSION` `2.0`; `SYNTHESIS_IDENTITY_VERSION`
  `e1-s2-v1` unchanged; `e1.tts-executor.3.0` and `e1.worker.2.0` unchanged.
