# ADR-0001-D012 — Preview loudness and the join verdict use provisional references

- **Status:** Approved
- **Date:** 2026-09-05
- **Controlling ADR and sections:** ADR-0001 §11.4, which requires automated loudness and
  speaking-rate comparison at a retake join, and §13.3, which requires final-package loudness
  normalization and delegates its references to ADR-0003
- **Requesting story:** E2-S3
- **Owner:** Engineering owner
- **Approver:** Project owner and engineering owner
- **Expiry:** Acceptance of ADR-0003. At that point the frozen values replace the provisional ones
  and this permission ends.

## Approved deviation

Permit E2-S3 to implement ADR-0001 §11.4's join verdict and §13.3's final-package loudness
normalization using **provisional** references chosen by this build rather than the values ADR-0003
will freeze.

Three constants, each of which names this record in return:

| Constant | Module | Provisional value | ADR-0003 row it stands in for |
|---|---|---|---|
| `PROVISIONAL_MAX_JOIN_RATIO` | `crates/study-tts-runtime/src/audio_edges.rs` | `2.0` | Join discontinuity threshold |
| `PROVISIONAL_LOUDNESS_TARGET_LUFS` | `crates/study-tts-runtime/src/export.rs` | `-16.0` | Master loudness target/range |
| `PROVISIONAL_TRUE_PEAK_CEILING_DBTP` | `crates/study-tts-runtime/src/export.rs` | `-1.0` | True-peak ceiling |

`PROVISIONAL_MAX_JOIN_RATIO` bounds both ratios `assess_join` already measures, symmetrically: a
ratio is within tolerance when it lies between its reciprocal and itself, so `0.5 <= r <= 2.0`. One
constant rather than four bounds, because a fold-change is the same quantity in either direction and
two separate bounds invite them to drift apart.

The values are deliberately loose. A provisional threshold exists to refuse audio that is obviously
broken — a segment rendered at half the level of its neighbour, or at twice its speaking rate — not
to express a quality standard nobody has calibrated. A tight provisional bound would fail joins that
a listener would accept and would make the threshold look ratified.

`-16 LUFS` and `-1.0 dBTP` are the ordinary spoken-word delivery references. The true-peak ceiling
carries headroom for the lossy encodes specifically: `lesson.m4a` and `lesson.mp3` derive from the
master, and inter-sample peaks that are inaudible in float PCM clip once encoded.

## The gap

ADR-0001 §11.4 requires that "after a mid-lesson retake, automated loudness and speaking-rate
comparisons plus a listening check evaluate both joins", and §13.3 requires two-pass final-package
loudness normalization. `DELIVERY-PLAN.md` E2-S3 task 4 is "Validate exact zero endpoints, join
discontinuity, finite samples, and `max(abs(sample)) <= 1.0`", task 5 is "Apply two-pass
final-package loudness normalization", and the story names
`t1_e2_discontinuity_threshold_is_enforced` and `t4_e2_loudnorm_requires_linear_result`. Story test
names are contracts.

The references those requirements depend on do not exist.
`docs/adr/ADR-0003-production-audio-quality-profile.md` is **Proposed; awaiting calibration**, and
its calibration table records **Join discontinuity threshold**, **Master loudness target/range**, and
**True-peak ceiling** as `TBD` / `Pending`. `CLAUDE.md` §Conflict order states that a Proposed ADR
authorizes nothing.

So E2-S3 cannot satisfy tasks 4 and 5 without either waiting for a calibration it does not own or
choosing references no accepted document states. This is the same gap `ADR-0001-D007` records for
the silence threshold and `ADR-0001-D009` for the MP3 profile, and this record takes their shape and
expiry deliberately.

**ADR-0003 depends on this story.** It declares `Depends on: ADR-0001, E2-S3, E5-S1`. E2-S3 is what
produces the provisional measurements ADR-0003's calibration is performed against, so waiting for
ADR-0003 is not a slower path to the same place — it is a deadlock. D009's Alternatives table
already recorded this reasoning for the MP3 row.

**A gap this record does close.** `ADR-0001-D007` §The gap names the Join discontinuity threshold
alongside the Silence RMS threshold, but D007's approved deviation covers only edge conditioning and
E1-S3 built no verdict. The join row was named as visible and left open. This record takes it.

## Impact

- **Architecture and authority boundaries:** No change. Rust owns edge analysis, the join verdict,
  and the decision to normalize; FFmpeg measures and applies gain. No authority moves.
- **Schemas and interfaces:** The two loudness argument profiles' BLAKE3 identities become recorded
  inputs of `manifest.json` and of the package transaction identity, so a later change to these
  arguments starts a new package generation rather than reusing one normalized differently. The
  master's bytes change, so every artifact digest moves.
  `docs/architecture/E2-S3-INTERFACE-CHANGE-001.md` records that surface.
- **Synthesis, verification, and cache identities:** None move. Normalization reads the assembled
  master; it is downstream of every synthesis key, and no cache key or plan hash reads an export
  profile. The join verdict reads measurements already taken and adds no key input.
- **Security, rights, and privacy:** No control is waived. Normalization rewrites sample values and
  touches no metadata; the existing `-map_metadata -1` on both encodes is unchanged.
- **Tests and evidence:** `t1_e2_discontinuity_threshold_is_enforced` proves a join outside the
  provisional bound is refused rather than recorded silently, and
  `t4_e2_loudnorm_requires_linear_result` proves a dynamic normalization result is refused. The
  existing `t3_e2_provisional_measurement_cannot_satisfy_production_calibration` already proves a
  provisional value cannot serve as a production reference, and this record adds no exception to it.
  E2-S3 evidence stays `Proposed` until M2.
- **Existing artifacts and migration:** None. Packages written before E2-S3 carry an unnormalized
  master and are preserved and read rather than migrated; the moved profile identities make them
  visibly a different generation.
- **Schedule and scope:** No listening claim is made for the normalized master. Frozen per-voice
  LUFS references remain E5-S1 and are not attempted here.

## What this does not permit

- It does not permit treating the normalized master as calibrated, verified, or releasable. Every
  package remains `private_preview`.
- It does not permit a measurement taken from this output to become a production reference.
  ADR-0003 §Fixed constraints forbids that, `SilenceThreshold::production` and
  `JoinContinuity::production` make it mechanical, and this record does not soften either.
- It does not permit the provisional join bound to be reported as a quality verdict. It refuses
  broken audio; it does not approve the audio it passes.
- It does not reach the M4A codec arguments, which `ADR-0001-D009` §The gap records as an open
  pre-existing gap carrying no permission. ADR-0003's acceptance closes that one.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Wait for ADR-0003 | ADR-0003 declares `Depends on: E2-S3`. Waiting makes the story unbuildable and the dependency circular, which is the same reason D009 gave |
| Ship E2-S3 without a join verdict | Contradicts `DELIVERY-PLAN.md` task 4 and leaves the named test `t1_e2_discontinuity_threshold_is_enforced` with nothing to assert. A named story test is a contract |
| Ship E2-S3 without normalization | Contradicts ADR-0001 §13.3 and task 5, and leaves `t4_e2_loudnorm_requires_linear_result` with nothing to assert |
| Normalize the M4A and MP3 separately instead of the master | Breaks ADR-0001 §13.5's requirement that both lossy outputs derive independently from the master, and makes three loudness decisions where the package needs one |
| Make the thresholds configurable | Configuration nobody sets, and it moves the choice from a reviewable record to a runtime value no manifest reader could bound. The recorded constant is the reviewable form |
| Amend ADR-0003 to freeze these values now | Freezes references with no listener review or long-form measurement behind them, which is exactly what ADR-0003's acceptance criteria exist to prevent |
| Record it inline in the interface-change record | A story record granting itself permission to depart from an ADR is how a rule quietly stops being the rule; `ADR-0001-D005` §Why this is a record and not a paragraph settles this |

## Compensating control and expiry

Each constant carries `CalibrationSource::Provisional` through the same mechanism E1-S3 built, so a
caller that needs a production reference receives `ProvisionalCalibration` rather than a number. The
argument profiles are pinned, hashed, and recorded: `manifest.json` carries the executed arguments
and the argument-profile digest for every FFmpeg invocation, and the measured loudness values are
published beside them marked `provisional`.

When ADR-0003 is accepted, the frozen values replace these, the argument-profile digests change, and
every package built under the provisional references is visibly a different generation rather than
silently equivalent to a calibrated one.

## Rollback

Supersede this record and replace the three constants with ADR-0003's frozen values. No
authoritative data is lost: packages built under the provisional references keep their manifests and
remain readable, and the next build produces a new generation because the recorded profile digests
moved. No cache entry is re-keyed, because no cache identity reads an export profile or a join
measurement.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the two rows are separate.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept a join band of `2.0` and its reciprocal, a `-16 LUFS` master target, and a `-1.0 dBTP` ceiling as this build's provisional references, and that they are recorded and hashed rather than frozen | 2026-09-05 |
| Project owner | Ross Todd | Approve — accept a bounded permission, expiring at ADR-0003's acceptance, to normalize and to refuse a join against uncalibrated references inside a `private_preview` package, on the understanding that no measurement taken under it may become a production reference | 2026-09-05 |
