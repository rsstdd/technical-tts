# Rights Record: rights-m2-lesson-sources-v1

- Artifact type: source content (M2 acceptance lesson input)
- Owner or rightsholder: Ross Todd (owner-authored repository material)
- Source URI: `fixtures/lessons/m2-durable-publication.source.md`
- Exact revision/checksum: SHA-256
  `6a78e19cb84c99a5d81ab147721e8b7a98c74495e5451bddfee606ebfb45ff1a`, 6,338 bytes; BLAKE3
  `35ede6cd2ef4558df93f553e4b44c39f0b561f2ee25e4427cbb25dd873d91d81`, which is the value
  `fixtures/lessons/m2-durable-publication.json` carries as its `source.content_hash`
- License or consent document URI/checksum: Not applicable. The document is written for this
  repository from its own governing documents, and quotes none of them
- Reviewer: Ross Todd (project owner and source rightsholder)
- Review date: 2026-09-06
- Supersedes: Nothing; this is the first record covering M2 acceptance lesson material

## Classification

| Source | Classification | Transformation scope | Distribution scope |
|---|---|---|---|
| `fixtures/lessons/m2-durable-publication.source.md` | owner-authored | Compilation into the lesson below | Owner-only private use; never published |
| `fixtures/lessons/m2-durable-publication.json` | owner-authored | Synthesis into a private-preview package | Owner-only private use; never published |

## Permitted scope

- Private use: Yes
- Commercial use: No
- Modification/voice cloning: Text may be synthesized; this record grants no voice rights
- Internal distribution: Owner devices and governed private evidence only
- External publication: No
- Geographic/audience limits: Owner only
- Watermark or attribution: Not applicable to owner-authored text

## Data handling

- Storage location: Committed to this repository; rendered audio under the governed
  private-preview output root outside Git
- Access: Project owner only
- Retention: Repository lifetime for the two committed documents; rendered audio per ADR-0004
  while cited by M2 evidence
- Backup: Approved encrypted and checksum-verified backup only
- Revocation/deletion procedure: Project-owner decision under the repository artifact policy

## Decision

**Signed 2026-09-06.** The material this record covers is classified, and `OQ-05` is resolved
for both documents.

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale, accepted as drafted — Ross Todd, project owner and source rightsholder, 2026-09-06:
The source document introduces
no third-party claim. It restates durability and atomicity rules this repository already fixes
in `docs/adr/ADR-0001-production-rust-study-guide-tts.md` §12.3 and implements in
`crates/study-tts-runtime/src/durable.rs`, in words written for this repository, and it quotes
neither. The lesson is compiled from it and adds no other material, so both carry the one
classification `OQ-05` requires. Nothing here grants a distribution scope the release profile
does not already limit to internal owner use.

The decision approves the scope this record states and no wider one: owner-only private use, no
commercial use, no external publication, and no voice rights. Rendered audio derived from this
material stays under the governed private-preview output root outside Git, per §Data handling.

*Entered by the engineering assistant at the project owner's explicit direction on 2026-09-06.
The digests above were re-pinned by commit `c0f373a`, which corrected a `rename(2)`/`EXDEV`
claim in the source; they name the corrected bytes, and the byte count moved with them.*
