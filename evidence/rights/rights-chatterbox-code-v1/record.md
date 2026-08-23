# Rights Record: rights-chatterbox-code-v1

- Artifact type: model (inference code)
- Owner or rightsholder: Resemble AI (Chatterbox open-source repository)
- Source URI: https://github.com/resemble-ai/chatterbox
- Exact revision/checksum: TBD — pinned at first acquisition (E0-S3); recorded here and in ADR-0002 before download or qualification per `docs/governance/ROUTING-TABLES.md`
- License or consent document URI/checksum: MIT license per the repository's published `LICENSE` file, as publicly known at authoring time; URI and checksum of the license text at the pinned revision recorded at verification
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: Pending — see Decision

## Permitted scope

- Private use: Yes under MIT, subject to verification below
- Commercial use: Not requested; out of the recorded scope (`docs/governance/RELEASE-PROFILES.md` §4: internal, owner use only)
- Modification/voice cloning: Code modification permitted under MIT; voice cloning governed separately by voice consent records, never by this code license
- Internal distribution: Yes under MIT, within the recorded owner-only scope
- External publication: Not requested; blocked until the distribution scope in RELEASE-PROFILES §4 is reopened
- Geographic/audience limits: None asserted by MIT; scope limited to the owner regardless
- Watermark or attribution: Retain the upstream copyright and license notice with any copy of the code

## Data handling

- Storage location: Cloned under an access-controlled Linux-filesystem root outside Git, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access
- Access: Project owner only
- Retention: While the pinned revision is the qualified backend; superseded revisions retained until their builds are unreferenced
- Backup: Only via an approved encrypted, verified backup process; otherwise re-clone from the pinned revision
- Revocation/deletion procedure: MIT is irrevocable for received copies; on a rights incident, follow `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [ ] Approved for recorded scope
- [ ] Restricted
- [x] Review required
- [ ] Prohibited

Rationale and approver: The license identity is drafted from Resemble AI's published repository, but this record was authored in an offline environment and the license text has not been verified against a pinned revision. Verification procedure: the project owner pins the revision at first acquisition (E0-S3), reads the `LICENSE` file at that revision, records its URI and SHA-256 checksum above, and flips this decision to Approved in a superseding entry. No real render occurs before approval per the policy's blocking rule ("No real render without approved record"). Approver for any use not explicitly covered: project owner/rightsholder per `docs/governance/ROUTING-TABLES.md` ("Chatterbox/model revision"). — Ross Todd, pending verification.
