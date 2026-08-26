# Milestone Capability and Approval Matrix

This matrix assigns delivery and approval responsibility. Capability timing remains controlled by the Delivery Plan.

| Capability | First required gate | Delivery owner | Approver | Evidence or test route |
|---|---|---|---|---|
| Fake end-to-end pipeline | G0a | Engineering owner | Project owner | E0-S0 T4 skeleton suite |
| Release profiles and fail-closed publication | G0 | Core owner | Project owner | E0-S1 schema/state tests and gate record |
| Voice, model, and source rights records and selected version 1 voice configuration | G0 | Project owner | Rightsholder/project owner | E0-S2 evidence, ADR-0001-D003, and enforcement tests |
| Reference environment and real Chatterbox viability | G0 | Engineering owner | Project owner | E0-S3 v1 measurement report, v2 progression decision, and ADR-0002 |
| Provisional seams and fakes | G0 | Engineering owner | Affected-track owners | E0-S4 contract suite |
| Reviewed canonical lesson JSON | G1 | Core owner | Engineering owner | E1-S2 tests |
| Published schema, scaffold, validation, example | G1 | CLI/core owners | Engineering owner | E1-S1/E1-S5 tests |
| Real persistent Chatterbox worker, pool size one | G1 | Worker owner | Engineering owner | E1-S3 shared and real contracts |
| Validated content-addressed synthesis cache | G1 | Runtime owner | Engineering owner | E1-S1/E1-S3 identity and cache tests |
| WAV, M4A, MP3, transcript, chapters, captions, manifest | G1 | Audio owner | Project owner | E1-S4 structural tests and package review |
| G1 interface freeze | G1 | Engineering owner | Affected-track owners | Freeze charter and contract results |
| Atomic resume and recovery | M2 | Runtime owner | Engineering owner | E2-S1 fault injection |
| Takes, retakes, and prune roots | M2 | Core/runtime owners | Engineering owner | E2-S2 tests |
| Preview audio conditioning and loudness | M2 | Audio owner | Human-review owner | E2-S3 tests and listening record |
| Structured run report | M2 | Runtime/CLI owners | Engineering owner | E2-S4 schema and redaction tests |
| MVP CLI and diagnostics | M2 | CLI owner | Project owner | E2-S5 command contract tests |
| Immutable human approval | M2 | Human-review owner | Project owner | E2-S6 approval integrity tests |
| General Markdown compilation | G3 | Authoring owner | Project owner | E3-S1 through E3-S4 tests/goldens |
| Compatibility and upgrade impact | G3 | Core owner | Engineering owner | E3-S5 matrix and dry-run tests |
| In-process ASR and separate verification identity | G3 | Verification owner | Engineering owner | E4-S0/E4-S1 qualification |
| Expected-ASR lattice and adjudication | G3 | Verification owner | Human-review owner | E4-S2 tests and promotion records |
| Calibrated ASR release control | G3 target | Verification owner | Human-review and project owners | E4-S3/ADR-0005 or amendment |
| Production verification state machine | G3 | Runtime/verification owners | Engineering owner | E4-S4 recovery/state tests |
| Frozen voice/style loudness references | G3 | Audio owner | Listener representative | E5-S1/ADR-0003 |
| Resource-governed parallel pool | G3 | Runtime/worker owners | Engineering owner | E5-S2 qualification |
| Retry, lifecycle, security, and verification recovery | M3 | Runtime owner | Engineering/security owner | E5-S3 through E5-S5 |
| Long-form soak and listening qualification | M3 | Engineering owner | Listener representative/project owner | E6-S1 T6 report |
| Supply chain, rights, SBOM, and advisories | M3 | Engineering/project owners | Project owner | E6-S2 release evidence |
| Clean install, operations, upgrade, and rollback | M3 | Engineering owner | Project owner | E6-S3 rehearsal |
| Production authorization and publish enablement | M3 | Project owner | Required role signatories | E6-S4 release record |

Roles map to Ross Todd during solo development unless another named person accepts the role. Independent-listener and rightsholder decisions remain separate even when the project owner coordinates them.
