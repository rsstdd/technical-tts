# E2-S3 Interface Change 001 — Loudness normalization enters the package path

## Identification

- Record ID: `E2-S3-INTERFACE-CHANGE-001`
- Status: **Proposed.** No row in §Approval is signed.
- Contract owner: T-RUNTIME (the package transaction and reuse comparison)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUDIO, T-RUNTIME, T-CLI
- Accepted ADR, if architectural: ADR-0001 §11.4, §13.3, §13.5. This record implements those
  sections; it changes no architecture.

The provisional references this change applies are authorized by
`docs/adr/deviations/ADR-0001-D012-provisional-loudness-and-discontinuity.md`, which is approved
and expires when ADR-0003 is accepted. Issue #16 is the working record.

## Version and compatibility

**No published schema version moves.** That is the notable fact about this change and is stated
first because the opposite would be assumed: normalization adds two FFmpeg invocations, and both
are recorded in the `executions` array `manifest` `2.0-skeleton` already carries. No field is
added, renamed, or retyped, so `schemas/manifest-v2.schema.json` is byte-identical and `plan`,
`job`, `cache`, and the worker protocol are untouched.

What moves is **identity**, not shape. A document whose schema is unchanged can still describe a
different build, and this one does.

## Identity effect

| # | Identity | Verdict | Where it is enforced |
|---|---|---|---|
| **I-1** | Synthesis and cache keys | **Do not move** | Normalization reads the assembled master; it is downstream of every synthesis key, and no cache key reads an export profile |
| **I-2** | `plan_hash` | **Does not move** | No plan field is added or reinterpreted |
| **I-3** | Verification identity | **Does not move** | E4 does not exist, and nothing here reaches it |
| **I-3a** | Loudness argument-profile digests | **Move again on 2026-09-06** | The D012 target amendment from `-16.0` to `-27.0` changes both `loudnorm` filter strings, so both profile identities move a second time. Same class of effect as I-4 and I-5, not a new one; packages built at `-16.0` are superseded rather than migrated, and no such package was ever published |
| **I-4** | Package transaction identity | **Moves** | `ExportProfiles::identities` grows from four profiles to six, and `preview::transaction_identity` hashes that set |
| **I-5** | Package identity | **Moves** | The published master's bytes change, so every artifact digest and the manifest digest that names the package directory move with them |
| **I-6** | Reuse of an existing package | **Refused; rebuilt** | `manifest::expected_executions` grows from six entries to eight, so `validate_package` reports no match and the build writes a new generation |

**I-6 is a rebuild, not an error.** `validate_package` answers a question rather than raising a
refusal, so a package written before this change is read, found not to match, and superseded by a
new generation. Nothing is migrated and nothing is deleted. Every cache entry survives I-1, so the
rebuild re-encodes and re-normalizes without re-synthesizing a segment.

**`TRANSACTION_IDENTITY_VERSION` is deliberately not touched**, on the reasoning
`E2-S2-INTERFACE-CHANGE-001` §Identity effect already recorded: transaction identity only separates
concurrent work, and reuse is decided by `manifest::validate_package`. The identity moves here
because its inputs moved, which is the mechanism working rather than a version event.

## Measured loudness, and why the target moved

The first render of a real five-minute lesson refused with `ToolError::LoudnessNotLinear`. The
measurements below are why, and they are the first real loudness data this project has for the
Chatterbox backend. ADR-0003's calibration table records **Master loudness target/range** and
**True-peak ceiling** as `Pending` and declares `Depends on: E2-S3`; these are that dependency's
output, recorded here so the eventual calibration can cite them.

| Artifact | Integrated | True peak | LRA |
|---|---:|---:|---:|
| `owner-fallback-v1/reference.wav` (voice conditioning) | **−38.96 LUFS** | −19.95 dBTP | 12.20 |
| `e1-s4-three-segment` master, 3 segments | −35.26 LUFS | −15.72 dBTP | 10.80 |
| `m2-durable-publication` master, 34 segments, 305.34 s | −34.45 LUFS | −9.92 dBTP | 5.80 |

Across all 34 cached segments of that master: integrated spans −35.94 to −32.91, median −34.87 — a
tight 3 dB. True peak spans −17.91 to −9.92, and the six loudest are −9.92, −11.00, −11.12, −11.52,
−11.97, −12.63. That is a smooth distribution rather than one bad segment, and the master's
−9.92 dBTP is exactly the loudest segment's, so assembly contributes no peak of its own.

**The level originates in the voice reference.** Chatterbox clones level from its conditioning
reference, and that reference is about 23 dB below an ordinary spoken-word level. Every synthesis
inherits it.

**Gain cannot correct it.** Reaching −16 LUFS needs +18.45 dB while the −1.0 dBTP ceiling permits
+8.92 dB. Applying the full gain was measured rather than argued: integrated lands at −16.01 and
true peak at **+8.53 dBTP**, above 0 dBFS. Gain preserves crest factor, so applying it per segment
instead of per master changes nothing. With a ~20 dB crest and a −1.0 dBTP ceiling, the reachable
target is about −21 LUFS even if every peak sat at the median.

`ADR-0001-D012`'s amendment of 2026-09-06 therefore moves the target to **−27.0 LUFS**, verified
linear against the real master with 1.57 dB of peak margin; −25.0 still falls back to dynamic. The
ceiling is unchanged, because it is not what is wrong. A louder reference is the actual remedy and
belongs to ADR-0003's per-voice calibration and E5-S1's frozen references.

The re-render at −27.0 published on 2026-09-06. Its master is a different performance — synthesis
is not reproducible across output roots — and measures −34.50 LUFS over 309.30 s, so the applied
gain was +7.48 dB and the published master sits at **−27.02 LUFS / −2.42 dBTP**, 1.42 dB inside the
ceiling. `evidence/gates/g3/m2-acceptance-lesson/m2-acceptance-lesson-render-v1.md` records that
render, and the table above stays as measured on the render that refused, because that is the
material the target was chosen against.

## What the package path now does

Ordered, because reuse compares position by position:

| # | Tool | Operation | Note |
|---|---|---|---|
| 1 | FFmpeg | encoder preflight | unchanged |
| 2 | FFmpeg | **loudness measurement** | new; writes no audio |
| 3 | FFmpeg | **loudness normalization** | new; rewrites the master |
| 4 | ffprobe | master validation | now validates the normalized master |
| 5–6 | FFmpeg, ffprobe | M4A encode and validation | now derives from the normalized master |
| 7–8 | FFmpeg, ffprobe | MP3 encode and validation | likewise |

**The master is what is normalized, not the exports.** ADR-0001 §13.5 has M4A and MP3 derive
independently from the master, so normalizing the master is one loudness decision the encodes
inherit; normalizing each export would be three decisions that could disagree.

**Ordering is load-bearing.** Normalization runs before the master probe, so the bytes ffprobe
validates are the published bytes rather than an intermediate nothing verified.

## Published API added

| Item | Kind | Why it is public |
|---|---|---|
| `normalize_master_output` | function | A T4 seam, on the precedent `validate_m4a_output` set: proving FFmpeg refuses a non-linear result requires handing a real FFmpeg audio it cannot normalize linearly |
| `JoinTolerance` | enum | The one reading `ADR-0001-D012` permits of a recorded join |
| `JoinContinuity::provisional_tolerance` | method | Produces it |
| `RemedyOwner::HumanReview` | variant | The first refusal routed to the `Human review finding` row of `docs/governance/ROUTING-TABLES.md` |
| `ToolError::LoudnessNotLinear` | variant | That refusal |
| `ToolError::UnreadableLoudnessReport`, `ToolError::LoudnessChangedLength` | variants | Tool-disagreement refusals, routed to the audio owner |
| `ToolOperation::LoudnessMeasure`, `ToolOperation::LoudnessNormalize` | variants | Name the two passes in a refusal |

All are additive. No published item is removed, renamed, or retyped.

## Impact

- **Synthesis identities affected:** none. See I-1.
- **Existing artifacts:** every published package is superseded and rebuilt; every cache entry
  survives. No package is migrated, deleted, or rewritten in place.
- **Security, rights, and privacy:** no control is waived. Normalization rewrites sample values and
  touches no metadata; `-map_metadata -1` on both encodes is unchanged.
- **Determinism:** the normalized master is a deterministic function of the assembled master and the
  pinned filter arguments. The measured values are per-lesson data substituted into the recorded
  arguments; the *profile* digest covers the targets and the argument shape, which is what keeps a
  profile identity from moving per lesson and destroying reuse.
- **Recovery:** normalization writes through a staged file beside the master and renames over it, so
  a failure partway leaves the assembled master rather than a half-rewritten one. It happens inside
  the package transaction, which remains the atomicity unit.

## Open questions

**G-A — where an advisory join finding is recorded.** `JoinTolerance::Outside` is computed and has
no consumer. `docs/governance/ROUTING-TABLES.md` §Failure routing answers a human review finding
with "Record finding; retake or accept with authority" and blocks production rather than the build,
so it is not a refusal: a join outside the provisional band still produces audio worth listening to,
and `release_status: private_preview` already blocks production.

Recording the verdict in the manifest was considered and rejected. It is a pure function of
`loudness_ratio`, `rate_ratio`, and `PROVISIONAL_MAX_JOIN_RATIO`, all of which the manifest already
carries, so storing it duplicates derivable data that can then disagree with itself. Carrying it on
`PackagePublication` was also rejected: the reuse branch returns early from the published package,
so a finding computed only on the fresh-write path would make a reused publication unequal to the
one it reuses, which `t4_e1_the_real_package_writer_passes_the_shared_contract` asserts against.

The destination is the structured run report, which **E2-S4 owns** and which does not yet exist.
This question stays open and is assigned there rather than answered here.

**G-B — carried forward.** The frames-per-character speaking-rate proxy remains the provisional
measure `E2-S2-INTERFACE-CHANGE-001` §Open questions opened. `PROVISIONAL_MAX_JOIN_RATIO` now bounds
that proxy, which gives the open question a consumer but does not resolve it: bounding an
unratified measure does not ratify it.

## Approval

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the rows stay separate.

The T-AUDIO row is an acceptance of the loudness references **as provisional**, not an endorsement
of them as calibrated values. They stand only until ADR-0003's calibration replaces them, on the
terms `ADR-0001-D012` sets.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that every existing published package is superseded and rebuilt, while every cache entry survives | |
| Contract owner (T-RUNTIME) | Accept that package and transaction identity move with no schema version change, and the eight-entry reuse comparison | |
| Affected track (T-AUDIO) | Accept `-27 LUFS`, `-1.0 dBTP`, and the `2.0` join band as provisional under `ADR-0001-D012`, on the measurements in §Measured loudness | |
| Affected track (T-CLI) | Accept that open question G-A assigns advisory join findings to E2-S4's run report | |
| Engineering owner | Accept `normalize_master_output` as a published T4 seam and the first `RemedyOwner::HumanReview` routing | |
| Effective version and date | No schema version moves; package identity moves | |

## Amendments

| Date | Amendment | Approval |
|---|---|---|
