# Rights Record: rights-chatterbox-weights-v2

- Artifact type: model (weights, tokenizer, codec, and extractor dependency inventory)
- Owner or rightsholder: Resemble AI (published Chatterbox artifacts); dependency owners as recorded in the governed package-license inventory
- Source URI: https://huggingface.co/ResembleAI/chatterbox/tree/1b475dffa71fb191cb6d5901215eb6f55635a9b6
- Exact revision/checksum: model snapshot `1b475dffa71fb191cb6d5901215eb6f55635a9b6`; per-file checksums below; governed external bundle manifest SHA-256 `ff1c09d66f069ff4b797d520fa22cfd9c888a43796825c1525237689ef9ed24f`
- License or consent document URI/checksum: pinned model card at https://huggingface.co/ResembleAI/chatterbox/blob/1b475dffa71fb191cb6d5901215eb6f55635a9b6/README.md declares MIT, SHA-256 `c2c75c034eadc6595789724e6b8b3ffcc2025f0875785cafeb9b39e1514e64b6`; complete governed package-license inventory SHA-256 `184fb371bf3d05ed1abbf56e7c62e78363ad8181b8c731f14df60295e4a4e71f`
- Reviewer: Ross Todd (project owner; rights-review role)
- Review date: 2026-08-25
- Supersedes: `rights-chatterbox-weights-v1`, SHA-256 `f45cddee75b40f7ba443974acfde2654042b2b47f2bd2dffc7a08dcef862db30`

The predecessor remains immutable. This record completes its pinned-snapshot, license-declaration, safe-artifact, and checksum procedure.

## Artifact identities

| Artifact | Responsibility | Bytes | SHA-256 |
|---|---|---:|---|
| `s3gen.safetensors` | Speech codec/generator weights | 1,056,484,620 | `2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e` |
| `t3_cfg.safetensors` | Text-to-token model weights | 2,129,653,744 | `914cb1696f47527fe8852ca8f1fe1fa63cb34f76f9c715e84e067b744dd0da81` |
| `tokenizer.json` | English text tokenizer | 25,470 | `d71e3a44eabb1784df9a68e9f95b251ecbf1a7af6a9f50835856b2ca9d8c14a5` |
| `ve.safetensors` | Voice-encoder weights | 5,695,784 | `f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c` |

Only these four snapshot artifacts were acquired. Legacy model `.pt` files and the packaged `conds.pt` were not acquired. The extractor environment is Python 3.12.3 with `torch==2.6.0+cpu`, `torchaudio==2.6.0+cpu`, and `s3tokenizer==0.1.7`; its complete 64-package freeze has SHA-256 `7de742701305fd95810a46bf575dc3c18377e5c910f9f48159f256f3e4af48e2`. The installed `s3tokenizer` declares Apache 2.0 and its installed license file has SHA-256 `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`.

## Permitted scope

- Private use: Yes — owner-only private synthesis and voice qualification
- Commercial use: Not requested or approved by this project record
- Modification/voice cloning: Conditional extraction is permitted only from a reference covered by its own approved consent record
- Internal distribution: Owner-controlled machines only; weights and dependency artifacts never enter Git, ordinary CI, or exported packages
- External publication: Prohibited under the recorded project scope
- Geographic/audience limits: Owner only under the recorded project scope
- Watermark or attribution: Preserve and qualify Chatterbox PerTh watermark behavior under OQ-09 before G3; do not intentionally remove or bypass it

## Data handling

- Storage location: Governed access-controlled Linux-filesystem model root outside Git and ordinary CI
- Access: Project owner and owner-controlled qualification tooling only
- Retention: While the pinned snapshot backs a qualified or referenced build
- Backup: Approved encrypted and verified backup only; otherwise reacquire the immutable snapshot and verify every checksum
- Revocation/deletion procedure: An upstream terms change or rights incident disables new use and follows `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Revocation and incident handling

## Decision

- [x] Approved for recorded scope
- [ ] Restricted
- [ ] Review required
- [ ] Prohibited

Rationale and approver: Before download, the project owner verified the pinned model-card MIT declaration and recorded narrow acquisition approval. All four local artifacts match the immutable snapshot identities, safe safetensors loading succeeds, the complete installed-package RECORD inventory has no missing or mismatched hashed entry, and the exact local model loads with offline flags and zero observed network attempts. Approval is limited to the stated private owner-only scope and is not a universal legal conclusion about other model uses or distribution. — Ross Todd, 2026-08-25.
