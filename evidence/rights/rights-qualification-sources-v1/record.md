# Rights Record: rights-qualification-sources-v1

- Artifact type: source content (qualification and release sources)
- Owner or rightsholder: Ross Todd (owner-authored material)
- Source URI: `fixtures/lessons/e0-s0-two-segment.json`, `fixtures/lessons/e0-s0-cache-identity.json` (committed); G0 smoke-test text (authored at E0-S3)
- Exact revision/checksum: Committed fixtures carry SHA-256 rows in `docs/testing/TEST-DATA-MANIFEST.md`, enforced by `t3_e0_registered_fixture_checksums_match_test_data_manifest`
- License or consent document URI/checksum: Owner-authored for this repository; no external license applies
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-23

## Classification

Each qualification and release source carries exactly one classification from `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Classification (mirrored by `SourceClassification` in `crates/study-tts-core/src/rights.rs`):

| Source | Classification | Transformation scope | Distribution scope |
|---|---|---|---|
| `lesson-two-segment-v1` | owner-authored | Synthesis and test mutation | Repository; private use |
| `lesson-cache-identity-v1` | owner-authored | Synthesis and test mutation | Repository; private use |
| G0 smoke-test text (E0-S3) | owner-authored | Synthesis only | Private use only; never published |
| Any future third-party source | rights review required until individually recorded | None until recorded | Blocked until recorded |

An unresolved classification blocks production release mechanically (`t4_e0_production_release_rejects_unresolved_content_rights_classification`); the product records classification and scope and does not encode a universal legal conclusion about any third-party material.

## Permitted scope

- Private use: Yes — private lesson rendering and qualification for the owner's own study use
- Commercial use: No
- Modification/voice cloning: Text transformation for synthesis permitted; not applicable to voice
- Internal distribution: Repository and owner devices
- External publication: Not granted by this record. Private use and publication rights are recorded separately: the ratified distribution scope is internal, owner use only, per `docs/governance/RELEASE-PROFILES.md` §4, and its reopening conditions are listed there
- Geographic/audience limits: Owner only
- Watermark or attribution: Not applicable to owner-authored text

## Data handling

- Storage location: Committed fixtures in Git; private lesson sources outside Git under access-controlled roots
- Access: Project owner; CI reads committed fixtures only
- Retention: Repository lifetime for committed fixtures; private sources per owner decision
- Backup: Git for committed fixtures; approved encrypted backup for private sources
- Revocation/deletion procedure: Owner-authored material has no external revocation path; removal follows `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: Every current qualification and release source is owner-authored, the least-encumbered classification, and each carries a checksum-verified manifest row. Future sources enter through this record's last classification row: they are rights-review-required until individually recorded, which the release pipeline enforces mechanically. Approver for any use not explicitly covered: project owner/rightsholder, consulting the rights reviewer, per `docs/governance/ROUTING-TABLES.md` ("Source-content distribution rights"). — Ross Todd, 2026-08-23.
