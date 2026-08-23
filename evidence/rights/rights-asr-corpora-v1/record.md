# Rights Record: rights-asr-corpora-v1

- Artifact type: corpus (ASR calibration corpora: `asr-clean-corpus-v1`, `asr-seeded-defects-v1`)
- Owner or rightsholder: Verification owner (Ross Todd); constituent audio rights recorded per source at admission
- Source URI: Governed external artifact location (outside Git), per `docs/testing/TEST-DATA-MANIFEST.md`
- Exact revision/checksum: Checksum manifest recorded at corpus admission (ADR-0005); no corpus exists yet
- License or consent document URI/checksum: Recorded per constituent source at admission; seeded-defect audio is derived only from the approved clean corpus
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-23

## Permitted scope

- Private use: Yes — internal ASR calibration and verification only
- Commercial use: No
- Modification/voice cloning: Defect seeding of the approved clean corpus is permitted; voice cloning from corpus audio is prohibited (a corpus inclusion is never consent, ADR-0001 §5.2)
- Internal distribution: Verification tooling on owner-controlled machines only
- External publication: No — corpora and their derivatives are never published
- Geographic/audience limits: Owner only
- Watermark or attribution: Per constituent source terms

## Data handling

This section records the access, retention, deletion, and backup rules E0-S2 task 5 requires; `docs/testing/TEST-DATA-MANIFEST.md` and ADR-0004 reference this record.

- Storage location: Access-controlled Linux-filesystem root outside Git, never `/mnt/c`; CI never receives corpus audio for ordinary pull requests
- Access: Project owner and verification tooling only
- Retention: While the calibration that used the corpus backs an accepted release; delete superseded corpora after their calibration evidence is superseded
- Backup: Only via an approved encrypted, verified backup process; otherwise re-acquire from the recorded sources and re-verify against the checksum manifest
- Revocation/deletion procedure: On a constituent-source rights incident, disable calibration use, identify affected calibrations by checksum manifest, and follow `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling; deletion is explicit and audited

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: This record approves the governance rules for ASR corpora before any corpus exists, so admission (ADR-0005) starts from recorded access, retention, deletion, and backup rules rather than improvising them. It does not approve any specific audio; each constituent source is classified and recorded at admission, and calibration use is blocked without that record ("No calibration use without record"). — Ross Todd, 2026-08-23.
