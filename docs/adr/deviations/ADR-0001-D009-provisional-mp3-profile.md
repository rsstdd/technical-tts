# ADR-0001-D009 — The MP3 export encodes against a provisional codec profile

- **Status:** Approved
- **Date:** 2026-09-01
- **Controlling ADR and sections:** ADR-0001 §§13.3 and 13.5, which require an MP3 export derived
  from the master and delegate its codec settings to ADR-0003
- **Requesting story:** E1-S4
- **Owner:** Engineering owner
- **Approver:** Project owner and engineering owner
- **Expiry:** Acceptance of ADR-0003. At that point the frozen values replace the provisional ones
  and this permission ends.

## Approved deviation

Permit E1-S4 to implement ADR-0001 §13.3's MP3 export using a **provisional** encoder and bitrate
chosen by this build rather than the values ADR-0003 will freeze.

The exact profile is `FFMPEG_MP3_ARGUMENT_PROFILE` in
`crates/study-tts-runtime/src/export.rs`, which names this record in return:

```text
-nostdin -hide_banner -loglevel error -y -i {input_path}
-map_metadata -1 -vn -ac 1 -channel_layout mono
-c:a libmp3lame -b:a 128k {output_path}
```

Every flag except the last four is the pinned M4A profile unchanged, so the two exports differ only
in codec and bitrate and both remain a single mono stream stripped of everything the master did not
carry.

## The gap

`DELIVERY-PLAN.md` E1-S4 task 4 is "Produce master WAV, M4A, MP3, chapters, transcript, captions,
checksums, and manifest", and task 6 requires both lossy formats derived independently from the
master. ADR-0001 §§12.1 and 13.5 name `lesson.mp3` in the package tree.

The settings those requirements depend on do not exist.
`docs/adr/ADR-0003-production-audio-quality-profile.md` is **Proposed; awaiting calibration**, and
its calibration table records **MP3 codec arguments** as `TBD` / `Pending`. `CLAUDE.md`
§Conflict order states that a Proposed ADR authorizes nothing.

So E1-S4 could not satisfy its own task 4 without either waiting for a calibration it does not own
or choosing settings no accepted document states. This is the same gap `ADR-0001-D007` records for
the silence threshold, and this record takes that record's shape and expiry deliberately.

**A pre-existing gap this record does not close.** The M4A profile's `-c:a aac -b:a 96k` is an
equally provisional choice against the same table's `Pending` **M4A codec arguments** row, made in
E0-S0 before this deviation practice existed and carrying no record. This record does not adopt it:
E1-S4 does not change those bytes, and retrofitting a permission onto a decision made under a
different practice would misdate it. It is named here so the gap is visible rather than implied,
and ADR-0003's acceptance closes both at once.

## Impact

- **Architecture and authority boundaries:** No change. Rust owns assembly; FFmpeg encodes. No
  authority moves.
- **Schemas and interfaces:** The MP3 argument profile's BLAKE3 identity becomes a recorded input
  of `manifest.json` (`1.0-skeleton`) and of the package transaction identity, so a later change to
  these arguments starts a new package generation rather than reusing one encoded differently.
  `docs/architecture/E1-S4-INTERFACE-CHANGE-001.md` records that surface.
- **Synthesis, verification, and cache identities:** None move. The encode reads the published
  master; it is downstream of every synthesis key, and no cache key or plan hash reads an export
  profile.
- **Security, rights, and privacy:** No control is waived. The profile carries `-map_metadata -1`,
  so no authored metadata reaches the container.
- **Tests and evidence:** `t4_e1_wav_m4a_and_mp3_pass_structural_validation` proves the produced
  MP3 is one mono `mp3` stream, and `t4_e1_missing_mp3_encoder_fails_before_synthesis_and_durable_work`
  proves an FFmpeg without `libmp3lame` is refused before any work runs.
  `t4_e0_encoding_profile_change_starts_a_new_generation` proves a changed MP3 profile is a new
  generation. E1-S4 evidence stays `Proposed` until G1.
- **Existing artifacts and migration:** None. No package written before E1-S4 contains an MP3, and
  historical packages are preserved and read rather than migrated.
- **Schedule and scope:** No listening claim is made for the MP3. Loudness normalization is E2-S3
  and is not attempted here.

## What this does not permit

- It does not permit treating the resulting MP3 as calibrated, verified, or releasable. Every
  package remains `private_preview`.
- It does not permit a provisional measurement taken from this output to become a production
  reference; ADR-0003 §Fixed constraints already forbids that and this record does not soften it.
- It does not reach the M4A profile, for the reason §The gap gives.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Wait for ADR-0003 | ADR-0003 depends on E2-S3 and E5-S1, both of which depend on E1-S4. Waiting makes the story unbuildable and the dependency circular |
| Ship the package without an MP3 | Contradicts `DELIVERY-PLAN.md` E1-S4 task 4 and ADR-0001 §13.5, and leaves the named test `t4_e1_wav_m4a_and_mp3_pass_structural_validation` with nothing to assert |
| Make the codec and bitrate configurable | Configuration nobody sets, and it moves the choice from a reviewable record to a runtime value no manifest reader could bound. The recorded argument profile is the reviewable form |
| Amend ADR-0003 to freeze `libmp3lame`/`128k` now | Freezes a value with no listener review or measurement behind it, which is exactly what ADR-0003's acceptance criteria exist to prevent |
| Record it inline in the interface-change record | A story record granting itself a permission to depart from an ADR is how a rule quietly stops being the rule; `ADR-0001-D005` §Why this is a record and not a paragraph settles this |

## Compensating control and expiry

The profile is pinned, hashed, and recorded: `manifest.json` carries the executed arguments and the
argument-profile digest for every FFmpeg invocation, so any package can be traced to the exact
settings that produced it. When ADR-0003 is accepted, the frozen values replace these, the
argument-profile digest changes, and every package built under the provisional profile is visibly a
different generation rather than silently equivalent to a calibrated one.

## Rollback

Supersede this record and replace `FFMPEG_MP3_ARGUMENT_PROFILE` with ADR-0003's frozen arguments.
No authoritative data is lost: packages built under the provisional profile keep their manifests and
remain readable, and the next build produces a new generation because the recorded profile digest
moved. No cache entry is re-keyed, because no cache identity reads an export profile.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the two rows are separate.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept `libmp3lame` at `128k` as this build's provisional MP3 profile, and that it is recorded and hashed rather than frozen | 2026-09-01 |
| Project owner | Ross Todd | Approve — accept a bounded permission, expiring at ADR-0003's acceptance, to ship an uncalibrated MP3 inside a `private_preview` package | 2026-09-01 |
