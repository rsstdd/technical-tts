# Rights Record: rights-voice-nadia-v1

- Artifact type: voice (planned "Nadia" instructor voice)
- Owner or rightsholder: TBD — no source recording has been acquired
- Source URI: None; no reference audio exists
- Exact revision/checksum: None
- License or consent document URI/checksum: None
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-23

## Permitted scope

- Private use: Not granted — no consent or license record exists
- Commercial use: Not granted
- Modification/voice cloning: Not granted; cloning requires one of the three acquisition routes in ADR-0001 §5.2 (owner recording, commissioned recording with signed release, or a specifically identified permissively licensed recording with reviewed dataset and speaker terms)
- Internal distribution: Not granted
- External publication: Not granted
- Geographic/audience limits: Not applicable
- Watermark or attribution: Not applicable

## Data handling

- Storage location: Not applicable; no artifact exists. Any future reference is stored under the restricted managed voice root, never Git or CI
- Access: Not applicable
- Retention: Not applicable
- Backup: Not applicable
- Revocation/deletion procedure: Defined at acquisition in the superseding record

## Decision

- [ ] Approved for recorded scope
- [ ] Restricted
- [x] Review required
- [ ] Prohibited

Rationale and approver: No lawful source for the "Nadia" voice was available by the G0 deadline. Per E0-S2 task 3 and the descope ladder, the pre-authorized owner-recorded single-instructor fallback (`rights-voice-owner-fallback-v1`) is the authorized substitute; profile load for any "nadia" profile fails closed until a superseding record is Approved (`t4_e0_missing_voice_consent_blocks_profile_load`, `t4_e0_unapproved_voice_profile_cannot_enter_preview_or_production`). A generic claim that candidate audio is public, synthetic, or included in a corpus is not sufficient evidence of permission (ADR-0001 §5.2); public-figure cloning is prohibited. — Ross Todd, 2026-08-23.
