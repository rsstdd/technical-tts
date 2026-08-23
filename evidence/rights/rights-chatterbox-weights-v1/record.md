# Rights Record: rights-chatterbox-weights-v1

- Artifact type: model (weights, tokenizer, codec)
- Owner or rightsholder: Resemble AI (published Chatterbox weight artifacts)
- Source URI: https://huggingface.co/ResembleAI/chatterbox
- Exact revision/checksum: TBD — pinned at first download (E0-S3): the project owner records the artifact revision and a SHA-256 checksum per file here and in ADR-0002 before qualification
- License or consent document URI/checksum: Per Resemble AI's published weight terms at the pinned revision; URI and checksum recorded at verification. The weights ship with the PerTh watermark; watermark policy is OQ-09 (before G3)
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: Pending — see Decision

## Permitted scope

- Private use: Expected yes under the published terms, subject to verification below
- Commercial use: Not requested; out of the recorded scope (`docs/governance/RELEASE-PROFILES.md` §4)
- Modification/voice cloning: Conditional-extraction from consented references only; every cloned voice requires its own consent record per ADR-0001 §15.3, never a weight-license claim
- Internal distribution: Owner-only; weights never enter Git, CI, or exported packages
- External publication: Prohibited; weights are never distributed
- Geographic/audience limits: Per the published terms at verification
- Watermark or attribution: PerTh watermark behavior recorded and measured under OQ-09 before G3

## Data handling

- Storage location: Access-controlled Linux-filesystem root outside Git, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access
- Access: Project owner only; CI never receives real model weights
- Retention: While the pinned revision is the qualified backend
- Backup: Only via an approved encrypted, verified backup process; otherwise re-download and re-verify against the pinned checksums
- Revocation/deletion procedure: On upstream terms change or rights incident, disable new use and follow `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [ ] Approved for recorded scope
- [ ] Restricted
- [x] Review required
- [ ] Prohibited

Rationale and approver: License identity drafted from the publicly known publication; neither the terms nor the artifacts have been fetched or verified from this offline environment. Verification procedure: at first download (E0-S3) the project owner reads the terms at the pinned revision, records their URI and checksum, records per-file artifact checksums, and flips this decision to Approved in a superseding entry. This record is complete as to structure and scope; the checksum rows are deliberately deferred to acquisition. No real render occurs before approval. Approver for uncovered uses: project owner/rightsholder per `docs/governance/ROUTING-TABLES.md`. — Ross Todd, pending verification.
