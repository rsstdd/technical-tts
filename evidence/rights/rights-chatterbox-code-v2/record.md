# Rights Record: rights-chatterbox-code-v2

- Artifact type: model (inference code)
- Owner or rightsholder: Resemble AI (Chatterbox open-source repository)
- Source URI: https://github.com/resemble-ai/chatterbox/releases/tag/v0.1.2
- Exact revision/checksum: release `v0.1.2`, Git commit `eb90621fa748f341a5b768aed0c0c12fc561894b`; governed external bundle manifest SHA-256 `ff1c09d66f069ff4b797d520fa22cfd9c888a43796825c1525237689ef9ed24f`
- License or consent document URI/checksum: MIT at https://raw.githubusercontent.com/resemble-ai/chatterbox/eb90621fa748f341a5b768aed0c0c12fc561894b/LICENSE, SHA-256 `4248e910a928849fe5815a0f9236e17fa07768d95b9193212752c464b93d6caa`
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-25
- Supersedes: `rights-chatterbox-code-v1`, SHA-256 `f14a83ae2aea65cb9100c29fddaeccf28c28274db85165cfc3d9f2fb6a43a2d7`

The predecessor remains immutable. This record completes its pinned-revision and verified-license procedure.

## Permitted scope

- Private use: Yes — owner-only private synthesis and voice qualification
- Commercial use: Not requested or approved by this project record; the upstream MIT grant is broader than the deliberately narrow project scope
- Modification/voice cloning: Code modification is permitted under MIT; voice cloning remains governed separately by an approved voice-consent record
- Internal distribution: Owner-controlled machines only under the recorded project scope
- External publication: Not approved by this record; reopening requires project-owner review under `docs/governance/RELEASE-PROFILES.md` §4
- Geographic/audience limits: Owner only under the recorded project scope
- Watermark or attribution: Retain the upstream copyright and MIT license notice with copies or substantial portions of the code

## Data handling

- Storage location: Governed access-controlled Linux-filesystem model root outside Git and ordinary CI
- Access: Project owner and owner-controlled qualification tooling only
- Retention: While the pinned revision backs a qualified or referenced build; superseded revisions remain until unreferenced
- Backup: Approved encrypted and verified backup only; otherwise reacquire the immutable revision and re-verify it
- Revocation/deletion procedure: MIT is irrevocable for received copies; a rights incident disables new use and follows `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: Before acquisition, the project owner verified that official release `v0.1.2` resolves to the recorded commit, reviewed the exact MIT license bytes, and recorded acquisition approval limited to owner-only private synthesis and voice qualification. The installed `chatterbox-tts==0.1.2` license file has the same SHA-256, and every hashed installed-package RECORD entry verifies. No lesson or TTS output was rendered during E0-S2. Any use outside the recorded scope routes to the project owner/rightsholder per `docs/governance/ROUTING-TABLES.md`. — Ross Todd, 2026-08-25.
