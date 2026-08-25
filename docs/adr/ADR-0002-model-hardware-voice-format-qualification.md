# ADR-0002: Model, Hardware, Voice, and Format Qualification

- **Status:** Proposed; awaiting G0 evidence
- **Owner:** Engineering owner
- **Approver:** Project owner
- **Depends on:** ADR-0001, E0-S2, E0-S3

## Decision to be completed

Qualify the exact Chatterbox code, model, tokenizer, codec, worker bundle, voice profile, canonical worker output format, FFmpeg conversion identity, and WSL2 reference hardware. ADR-0001 already selects standard Chatterbox; this record determines whether the pinned configuration meets the accepted gates.

## Required evidence

- exact source revisions, artifact URIs, checksums, licenses, and permitted scope;
- reference-machine inventory;
- offline real render;
- model load time and peak RAM;
- pool-size-one RTF and six-hour projection;
- output WAV compatibility;
- ten-run fixed-seed determinism characterization;
- voice consent/checksum and listener assessment;
- worker-bundle and FFmpeg identities;
- schedule reforecast.

## Decision table

| Item | Candidate | Evidence | Decision |
|---|---|---|---|
| Chatterbox revision | Standard Chatterbox `v0.1.2`, code commit `eb90621fa748f341a5b768aed0c0c12fc561894b` | `rights-chatterbox-code-v2`; governed bundle manifest SHA-256 `ff1c09d66f069ff4b797d520fa22cfd9c888a43796825c1525237689ef9ed24f` | Pending E0-S3 qualification |
| Model/tokenizer/codec | `ResembleAI/chatterbox` snapshot `1b475dffa71fb191cb6d5901215eb6f55635a9b6`; safe `s3gen`, `t3_cfg`, and `ve` safetensors plus `tokenizer.json`; Python 3.12.3, PyTorch/Torchaudio 2.6.0 CPU, `s3tokenizer==0.1.7` | `rights-chatterbox-weights-v2`; all per-file and dependency-inventory checksums recorded there | Pending E0-S3 qualification |
| Voice profile | Acquired owner-recorded single-instructor `owner-fallback-v1`, BLAKE3 `b57455db4712257ab102af210098ef8b0592d03c296178640c6e47ef129c61db`; derived conditional BLAKE3 `4951f9e1fb8a665321b2a31c0eb1691e318378bbf892aef44bb9e85b23598e47`. `nadia-v1`/`tom-v1` remain unavailable and at Review required | `rights-voice-owner-fallback-v2`; offline weights-only conditional reload and existing Rust runtime gate pass | Pending E0-S3 voice and listening qualification |
| Reference hardware | TBD | TBD | Pending |
| Canonical worker format | 24 kHz mono float WAV per ADR-0001 | TBD | Pending validation |
| Supported FFmpeg identity | TBD | TBD | Pending |

## Acceptance

Accept only after every G0 exit gate passes. If performance fails, reopen hardware or backend feasibility before E1 implementation continues. Byte nondeterminism does not invalidate content-addressed reuse because the cache retains the first valid artifact, but it must be recorded.
