# Rights Record: rights-voice-owner-fallback-v2

- Artifact type: voice (owner-recorded single-instructor fallback)
- Owner or rightsholder: Ross Todd (speaker and recording rightsholder)
- Source URI: `governed://technical-tts/voices/owner-fallback/v1`
- Exact revision/checksum: reference WAV SHA-256 `1d6b2c247f9e66e23e9d27819920430993ae2296c138dd88a4b39a8f38b117e8`, BLAKE3 `b57455db4712257ab102af210098ef8b0592d03c296178640c6e47ef129c61db`; conditionals SHA-256 `f3dbb5c5ae882079cdfde6dbd599d78ba82347f717414b2f74920080d7785f00`, BLAKE3 `4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47`
- License or consent document URI/checksum: governed `consent.json`, SHA-256 `a46bdfc090a955227c5674c863aecc6e75ec8dd0cfa8778a2fe79779d64dcc6d`; profile SHA-256 `d17e73efd281af2dbdc0adf2e772dad856e30fb3bc572f4b7ce6459994b98de1`
- Reviewer: Ross Todd (project owner and rightsholder; roles recorded separately)
- Review date: 2026-08-25
- Supersedes: `rights-voice-owner-fallback-v1`; the v1 record remains immutable

## Permitted scope

- Private use: Yes — owner-only private synthesis and voice qualification
- Commercial use: No
- Modification/voice cloning: Conditional extraction and synthesis from the owner's recording only; the profile remains single-speaker
- Internal distribution: Owner devices and governed private evidence only
- External publication: No
- Geographic/audience limits: Owner only
- Watermark or attribution: PerTh policy remains open as OQ-09 before G3; qualification does not remove or bypass it

## Data handling

- Storage location: Restricted governed voice root on the qualified Linux filesystem; never Git, CI, fixtures, or exported packages
- Access: Project owner only
- Retention: Minimum needed for the recorded consent scope and reconstruction evidence
- Backup: Approved encrypted and checksum-verified backup only; no unapproved backup exists
- Revocation/deletion procedure: Setting `consent_status` to `revoked` disables new use; deletion requires the project-owner record defined by the artifact policy

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: The owner recorded the reference, selected it for the single-instructor
fallback, and signed a permitted-use declaration for `private_synthesis` and
`voice_qualification`. The reference is 24 kHz mono PCM; the precomputed conditional was loaded
through Chatterbox's `weights_only=True` path under the pinned extractor identity, and both files
match the identities above. Approval is restricted to the exact owner-only scope. — Ross Todd,
project owner and rightsholder, 2026-08-25.
