# Rights Record: rights-qualification-sources-v2

- Artifact type: source content (E0-S3 qualification input)
- Owner or rightsholder: Ross Todd (owner-authored repository material)
- Source URI: `governed://technical-tts/e0-s3/qualification-input/v1`
- Exact revision/checksum: UTF-8 input SHA-256 `d4452958cea237afe27e574c4c8a9429fabe3a809e1be342d1749c6ac25266dc`, 69 bytes
- License or consent document URI/checksum: Derived verbatim from the two approved `spoken_text` values in `fixtures/lessons/e0-s0-two-segment.json`, whose manifest checksum is governed by `lesson-two-segment-v1`
- Reviewer: Ross Todd (project owner and source rightsholder)
- Review date: 2026-08-25
- Supersedes: `rights-qualification-sources-v1`; the v1 record remains immutable

## Classification

| Source | Classification | Transformation scope | Distribution scope |
|---|---|---|---|
| `chatterbox-smoke-v1` input | owner-authored | Synthesis and private qualification only | Owner-only private use; never published |

## Permitted scope

- Private use: Yes
- Commercial use: No
- Modification/voice cloning: Text may be synthesized; this record grants no voice rights
- Internal distribution: Owner devices and governed private evidence only
- External publication: No
- Geographic/audience limits: Owner only
- Watermark or attribution: Not applicable to owner-authored text

## Data handling

- Storage location: Governed Linux-filesystem qualification input root outside Git
- Access: Project owner only
- Retention: With the E0-S3 raw evidence while the qualification report is retained
- Backup: Approved encrypted and checksum-verified backup only
- Revocation/deletion procedure: Project-owner decision under the repository artifact policy

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: The qualification input introduces no new claim. It is the exact two
already approved owner-authored fixture sentences, joined in source order and copied to the
private evidence root. The SHA-256 above binds the reviewed bytes sent to Chatterbox. — Ross
Todd, project owner and source rightsholder, 2026-08-25.
