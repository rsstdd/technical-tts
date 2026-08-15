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
| Chatterbox revision | TBD | TBD | Pending |
| Model/tokenizer/codec | TBD | TBD | Pending |
| Voice profile | TBD | TBD | Pending |
| Reference hardware | TBD | TBD | Pending |
| Canonical worker format | 24 kHz mono float WAV per ADR-0001 | TBD | Pending validation |
| Supported FFmpeg identity | TBD | TBD | Pending |

## Acceptance

Accept only after every G0 exit gate passes. If performance fails, reopen hardware or backend feasibility before E1 implementation continues. Byte nondeterminism does not invalidate content-addressed reuse because the cache retains the first valid artifact, but it must be recorded.

