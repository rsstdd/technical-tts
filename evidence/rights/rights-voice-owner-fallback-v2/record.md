# Rights Record: rights-voice-owner-fallback-v2

- Artifact type: voice (acquired owner-recorded single-instructor fallback and derived conditional)
- Owner or rightsholder: Ross Todd (speaker, recording owner, and voice rightsholder)
- Source URI: governed external artifact `voice://owner-fallback-v1/reference.wav`; user-selected recording take 4
- Exact revision/checksum: `reference.wav` BLAKE3 `b57455db4712257ab102af210098ef8b0592d03c296178640c6e47ef129c61db`, SHA-256 `1d6b2c247f9e66e23e9d27819920430993ae2296c138dd88a4b39a8f38b117e8`; `conditionals.pt` BLAKE3 `4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47`, SHA-256 `f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00`
- License or consent document URI/checksum: governed external artifact `voice://owner-fallback-v1/consent.json`, BLAKE3 `af0bab6abe6b7a514e6f78c8ea1e7f325db2776d2d4f30c9e574139d5b9b3efd`, SHA-256 `a46bdfc090a955227c5674c863aecc6e75ec8dd0cfa8778a2fe79779d64dcc6d`; profile SHA-256 `d17e73efd281af2dbdc0adf2e772dad856e30fb3bc572f4b7ce6459994b98de1`
- Reviewer: Ross Todd (project owner and rightsholder; roles held separately per `docs/governance/PROJECT-EXECUTION-CHARTER.md`)
- Review date: 2026-08-25
- Supersedes: `rights-voice-owner-fallback-v1`, SHA-256 `7453f28b1f14a24912bb472e801f09023a3afaad52db4c3a171dd1a0453dff38`

The predecessor remains immutable. This record completes its recording, selection, consent, conditional, checksum, and runtime-gate procedure.

## Permitted scope

- Private use: Yes — `private_synthesis` and `voice_qualification` for Ross Todd's owner-only use
- Commercial use: No
- Modification/voice cloning: Conditional extraction from this selected owner recording only; the profile remains one speaker and cannot be relabeled as Nadia, Tom, another person, or multiple speakers
- Internal distribution: Ross Todd's owner-controlled devices and qualification tooling only
- External publication: No; this record grants neither publication nor public distribution
- Geographic/audience limits: Owner only
- Watermark or attribution: Chatterbox watermark policy remains deferred to OQ-09 before G3; this does not authorize bypass or removal

## Data handling

- Storage location: Restricted managed voice root on the WSL2 Linux filesystem outside Git, ordinary CI, logs, and exported packages; directory mode `0700`, required-file mode `0600`
- Access: Project owner and owner-controlled profile-validation or qualification tooling only
- Retention: Retain the selected reference and conditional while this profile backs qualification or a referenced build; preserve rejected takes until the owner explicitly authorizes deletion
- Backup: Approved encrypted and verified backup only; otherwise no backup
- Revocation/deletion procedure: Setting `consent_status` to `revoked` disables new use immediately; deletion requires explicit owner authorization and follows `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: Ross Todd recorded and listened to the candidate takes, rejected take 1 as silence and take 3 as clipped, and explicitly selected take 4. The selected 10.833792-second WAV is mono 24 kHz 16-bit PCM with safe headroom. The pinned offline CPU extractor generated the conditional once; the artifact then reloaded through the exact Chatterbox `weights_only=True` path with zero network attempts. Strict `profile.json` and `consent.json` records carry the actual BLAKE3 identities, granted state, and only the two approved uses. The existing runtime accepted the real profile and then reached the deliberately later missing-FFmpeg refusal without synthesis. Any use outside this scope routes to the project owner/rightsholder. — Ross Todd, 2026-08-25.
