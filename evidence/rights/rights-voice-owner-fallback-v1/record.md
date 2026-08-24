# Rights Record: rights-voice-owner-fallback-v1

- Artifact type: voice (owner-recorded single-instructor fallback)
- Owner or rightsholder: Ross Todd (owner of the recording and the recorded voice)
- Source URI: Restricted managed voice root (outside Git); planned profile directory `data/voices/owner-fallback-v1/` per ADR-0001 §12.1
- Exact revision/checksum: BLAKE3 checksums of `reference.wav` and `conditionals.pt` recorded in `profile.json` and `consent.json` at recording time; profile load fails closed on mismatch (`t4_e0_voice_checksum_mismatch_blocks_use`)
- License or consent document URI/checksum: Owner permitted-use declaration recorded as `consent.json` in the profile directory (ADR-0001 §5.2 route 1: maintainer's own recording with a permitted-use declaration)
- Reviewer: Ross Todd (project owner and rightsholder; roles held separately per `docs/governance/PROJECT-EXECUTION-CHARTER.md`)
- Review date: 2026-08-23

## Permitted scope

- Private use: Yes — private lesson synthesis for the owner's own study use
- Commercial use: No
- Modification/voice cloning: Conditional extraction from the owner's own reference recording only; the resulting profile is single-speaker and is not relabeled as multiple speakers (ADR-0001 §15.3)
- Internal distribution: Owner's own devices only
- External publication: No — publication rights are not granted by this record; `docs/governance/RELEASE-PROFILES.md` §4 records the internal, owner-only distribution scope separately
- Geographic/audience limits: Owner only
- Watermark or attribution: Watermark policy deferred to OQ-09 (before G3)

## Data handling

- Storage location: Restricted managed voice root on an access-controlled Linux filesystem; never Git, CI, fixtures, or logs
- Access: Project owner only
- Retention: Minimum required by the recorded scope; reference audio never enters exported packages (ADR-0001 §15.4)
- Backup: Only via an approved encrypted, verified backup process; otherwise none
- Revocation/deletion procedure: The owner may revoke by setting `consent_status` to `revoked`, which disables all new use immediately (profile load fails closed); deletion follows `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling, best-effort secure deletion per ADR-0001 §15.4

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: Pre-authorized as the single-instructor fallback required by E0-S2 task 3 and the risk register ("Voice rights unresolved → Select pre-authorized owner-recorded single-instructor fallback"). The owner records their own voice and declares the permitted use above, which is the strongest of the three acquisition routes in ADR-0001 §5.2. This authorization is for the recorded scope only; any use not covered here routes to the project owner/rightsholder per `docs/governance/ROUTING-TABLES.md` ("Voice use, consent, retention, or watermarking"). The recording itself and its checksums are produced at E0-S3; this record authorizes that recording in advance. — Ross Todd (project owner and rightsholder), 2026-08-23.
