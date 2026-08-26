# Rights Record: rights-chatterbox-weights-v2

- Artifact type: model (weights, tokenizer, and codec inputs)
- Owner or rightsholder: Resemble AI
- Source URI: https://huggingface.co/ResembleAI/chatterbox/tree/1b475dffa71fb191cb6d5901215eb6f55635a9b6
- Exact revision/checksum: revision `1b475dffa71fb191cb6d5901215eb6f55635a9b6`; per-file checksums below
- License or consent document URI/checksum: [model card at the pinned revision](https://huggingface.co/ResembleAI/chatterbox/resolve/1b475dffa71fb191cb6d5901215eb6f55635a9b6/README.md), declaring MIT, SHA-256 `c2c75c034eadc6595789724e6b8b3ffcc2025f0875785cafeb9b39e1514e64b6`
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-25
- Supersedes: `rights-chatterbox-weights-v1`; the v1 record remains immutable

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `s3gen.safetensors` | 1,056,484,620 | `2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e` |
| `t3_cfg.safetensors` | 2,129,653,744 | `914cb1696f47527fe8852ca8f1fe1fa63cb34f76f9c715e84e067b744dd0da81` |
| `tokenizer.json` | 25,470 | `d71e3a44eabb1784df9a68e9f95b251ecbf1a7af6a9f50835856b2ca9d8c14a5` |
| `ve.safetensors` | 5,695,784 | `f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c` |

## Permitted scope

- Private use: Yes for the recorded owner-only qualification and private synthesis scope
- Commercial use: Not requested; outside this record
- Modification/voice cloning: Conditioning only from a separately consented owner voice
- Internal distribution: Owner-only; weights never enter Git, CI, or output packages
- External publication: Weights are not distributed
- Geographic/audience limits: Owner only under the recorded project scope
- Watermark or attribution: PerTh behavior remains subject to OQ-09 before G3; qualification does not remove or bypass it

## Data handling

- Storage location: Governed Linux-filesystem model root outside Git
- Access: Project owner only
- Retention: While the revision is qualified or referenced by evidence/builds
- Backup: Approved encrypted and checksum-verified backup, otherwise reacquire and reverify every file
- Revocation/deletion procedure: Disable new use on a terms change or incident and follow the repository artifact policy

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: The pinned model card and artifact inventory were acquired and verified
before rendering. Every executable weight is SafeTensors, the tokenizer is JSON, the recorded
checksums match the local bytes, and no legacy model `.pt` or packaged `conds.pt` is present.
Approval is limited to owner-only private qualification and synthesis. — Ross Todd, project
owner and rights reviewer, 2026-08-25.
