# Release Profiles

- **Status:** Ratified
- **Owner and approver:** Ross Todd
- **Ratified:** 2026-08-23
- **Authority:** ADR-0001 §1, §17.18, §18; ADR-0001-D001; ADR-0001-D002;
  `DELIVERY-PLAN.md` §1

Every artifact this project produces declares exactly one release profile. A profile is a claim
about what an artifact *is*, not a stage it has reached. The distinction is mechanical: code
enforces it, and no build may assert a profile it has not earned.

## 1. Profiles

### `private_preview`

The artifact was rendered and passed structural validation. It is not verified, not approved,
and not releasable. Every build produces this profile unless it passes every gate in §3.

Outputs are written beneath `previews/<lesson-id>/`. The manifest declares
`release_status: private_preview`. A private preview must never be represented as verified,
production approved, or releasable, regardless of how many checks happen to have passed.

### `production_release`

Every gate in §3 has a passing evidence record. Only the `publish` operation produces this
profile, and only after gate evidence is complete.

## 2. Permitted transitions

| From | To | Permitted | Reason |
|---|---|:---:|---|
| (none) | `private_preview` | Yes | Any successful build produces a preview. ADR-0001 §6.4 |
| `private_preview` | `production_release` | Only through `publish` with complete §3 evidence | ADR-0001 §18; `DELIVERY-PLAN.md` §1 |
| `production_release` | `private_preview` | No | A release is never downgraded. A new build produces a new preview with its own identity |
| `production_release` | `production_release` | No | Republication requires a new build and a new gate evidence set |
| any | unrecognized value | No | An unknown release status is a schema error, not a state |

A private preview carrying evidence for every gate is still a private preview. The profile
records whether the production workflow ran, not how many checks incidentally succeeded.

## 3. Gates required for `production_release`

Twelve gates, transcribed from ADR-0001 §18 and grouped for mechanical checking. This list is
mirrored by `REQUIRED_PRODUCTION_GATES` in `crates/study-tts-core/src/release.rs`; the two must
agree. Changing either requires an ADR amendment, not an edit.

| Gate identifier | ADR-0001 §18 criteria covered |
|---|---|
| `long_form_soak` | A reviewed 60-minute lesson builds on the reference WSL2 system; interruption loses no completed valid segment; the same build reuses every valid segment; editing one segment regenerates only that segment and the final assembled outputs; requesting a retake creates a distinct cache identity and preserves the prior take |
| `content_integrity_review` | No raw Markdown syntax is spoken accidentally; no unapproved claim enters the lesson through compilation; no omitted, duplicated, inserted, or materially mispronounced technical content survives review |
| `asr_triage_recorded` | Post-render ASR triage runs for every selected cached segment and records its verification identity, lattice evidence, findings, and adjudication |
| `worker_unloaded_before_verification` | The Chatterbox pool is unloaded before verification and is not invoked by ASR-only invalidation or recalibration |
| `explicit_take_selection` | Production selection is explicit in a current takes file, and the plan and manifest record each selected take, cache key, and audio checksum |
| `frozen_loudness_references` | Each production voice/style pair uses a calibrated frozen loudness reference |
| `voice_identity_and_format` | The selected owner voice remains recognizable and consistent throughout the lesson; the build truthfully uses the approved single-instructor format and never relabels the profile as Nadia, Tom, or multiple speakers |
| `automated_audio_checks` | Automated audio checks pass for every segment and every export |
| `package_provenance` | Output packages contain valid manifests and checksums |
| `offline_render_verified` | Offline rendering is verified with network egress denied |
| `rights_and_licensing` | The selected model, dependencies, voices, source content, and FFmpeg use have complete license and consent records |
| `clean_machine_operations` | Installation, rendering, inspection, recovery, pruning, and uninstall documentation passes on a clean machine |

### ASR calibration is not among these gates

ADR-0001 §17.18 and §18 stated different ASR release conditions. Version 1.0 adopts the §18
condition: triage must run and record evidence. Failure of ADR-0005's numerical calibration
gates blocks the claim of automated text-integrity coverage and keeps human review authoritative
under ADR-0001 §10.5, but does not block release. Recorded in
`docs/adr/deviations/ADR-0001-D001-asr-release-condition.md`, and §17.18 is amended to match.

### Version 1 uses the single-instructor format

ADR-0001-D002 selects `owner-fallback-v1` before E0-S3. The Nadia/Tom recognizability and
two-speaker dialogue criteria are not applicable to version 1. Voice consistency, rights,
offline rendering, loudness, audio quality, human listening, and long-form qualification remain
required. A future two-speaker release needs a superseding accepted decision and two separately
approved profiles.

## 4. Distribution scope, and the condition that reopens it

**Current scope: internal, owner use only.** `publish` writes to a local directory on the
owner's machine. No artifact is distributed, published, or shared with any other person.

That scope removes four obligations from `production_release`:

- binary signing is not required;
- FFmpeg distribution licensing does not arise while an external installation is used
  (ADR-0001 §13.6);
- captions are a convenience artifact, not a compliance requirement;
- the source-content rights determination is limited to lawful private use.

**Reopening condition.** If any output is ever distributed, published, or shared with another
person, this scope no longer holds. Before that happens, the following must be reopened and
answered afresh: source-content rights (OQ-05), caption compliance, watermark preservation
verification (OQ-09), binary signing (OQ-08), and the FFmpeg licensing review. A release produced under the
internal scope must not be redistributed on the strength of gates that were evaluated under it.

## 5. Enforcement

| Rule | Enforced by |
|---|---|
| A private preview cannot claim production release | `t3_e0_private_profile_cannot_report_production_release` |
| A production release with missing gate evidence is refused | `t3_e0_production_profile_rejects_missing_gate_evidence` |
| An unrecognized release status is rejected at parse time | `t3_e0_unknown_release_status_is_rejected` |
| `REQUIRED_PRODUCTION_GATES` still lists exactly the twelve gates of §3 | `t3_e0_required_gates_match_the_release_profile_document` |
| The E0-S0 manifest cannot enter production publication | `t4_e0_private_preview_cannot_enter_production_publication` |
