# Rights Record: rights-voice-tom-v1

- Artifact type: voice (planned "Tom" instructor voice)
- Owner or rightsholder: TBD — no source recording has been acquired
- Source URI: None; no reference audio exists
- Exact revision/checksum: None
- License or consent document URI/checksum: None
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-23

## Permitted scope

- Private use: Not granted — no consent or license record exists
- Commercial use: Not granted
- Modification/voice cloning: Not granted; cloning requires one of the three acquisition routes in ADR-0001 §5.2
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

Rationale and approver: No lawful source for the "Tom" voice was available by the G0 deadline. The pre-authorized owner-recorded single-instructor fallback (`rights-voice-owner-fallback-v1`) is the authorized substitute; profile load for any "tom" profile fails closed until a superseding record is Approved. The packaged Chatterbox default voice cannot be relabeled as a second distinct speaker (ADR-0001 §15.3), so a two-voice lesson requires a second lawful acquisition, not a relabeling. — Ross Todd, 2026-08-23.
