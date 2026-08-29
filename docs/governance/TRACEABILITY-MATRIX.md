# ADR-to-Story-to-Validation Traceability

The Delivery Plan and GitHub stories contain the exhaustive named-test lists. This matrix proves that every material ADR-0001 concern has an owning story and validation route. Story closure must link the exact run or evidence record.

| ADR requirement | Delivering stories | Primary validation | Gate |
|---|---|---|---|
| End-to-end boundary order | E0-S0 | `t4_e0_skeleton_produces_wav_m4a_and_minimal_manifest` | G0a |
| Release profiles fail closed | E0-S1, E2-S6, E6-S4 | `t3_e0_private_profile_cannot_report_production_release`; gate review | G0/M2/M3 |
| Voice and source rights | E0-S2, E6-S2 | rights records; unapproved-profile and unresolved-content tests | G0/M3 |
| Single-instructor version 1 voice selection | E0-S2, E0-S3, E6-S1 | ADR-0001-D003; exact-profile smoke render; voice-consistency and long-form listening evidence | G0/M3 |
| Real Chatterbox viability and offline use | E0-S3, E1-S3 | smoke render, offline qualification, measured constrained-environment RTF plus ADR-0002 waiver | G0/G1 |
| Full-box Chatterbox performance | E0-S3, E5-S2, E6-S4 | single-worker RTF and 60-minute projection on the named deployment configuration | G3/M3 |
| Synthesis determinism characterization | E0-S3 | `evidence_e0_fixed_seed_synthesis_determinism_is_characterized` | G0 |
| Replaceable versioned worker seam | E0-S4, E1-S1, E1-S3 | shared fake/real contract suite | G0/G1 |
| Canonical reviewed lesson only | E1-S2, E3-S3 | review-state rejection; display/spoken audit | G1/G3 |
| Separate synthesis and verification identities | E1-S1, E4-S1 | field-sensitivity properties; ASR-only invalidation | G1/G3 |
| Worker-bundle hash | E1-S1, E1-S3 | owned-input invalidation and reproducible bundle tests | G1 |
| Worker environment matches the lock before an identity is returned | E1-S1 | ADR-0001-D004; the seventeen `worker_environment` T4 tests, sixteen pinning refusals and one driving the probe on a real interpreter | G1 |
| Pre-G1 breaking correction retains an unreleased version only under bounded conditions | E1-S1 | ADR-0001-D005; `t3_e1_published_schema_required_fields_match_the_recorded_surface`; accepted E1-S1 v11 evidence | G1 expiry |
| Persistent single worker for MVP | E1-S3 | one-load-per-lifetime and protocol tests | G1 |
| Atomic validated synthesis cache | E1-S3, E2-S1 | cache-hit validation, crash-boundary fault injection | G1/M2 |
| Managed-path containment | E1-S3, E5-S4 | traversal, symlink, staging-escape tests | G1/M3 |
| Rust-owned PCM assembly | E1-S4, E2-S3 | exact sample count, edge conditioning, join tests | G1/M2 |
| Canonical master and independent exports | E1-S4 | structural validation and lossy-source rejection | G1 |
| Atomic job state and recovery | E2-S1, E5-S5 | interruption at every state/write boundary | M2/M3 |
| Explicit takes and prune roots | E2-S2 | stale-base rejection, manifest propagation, prune protection | M2/G3 |
| Preview loudness versus frozen production references | E2-S3, E5-S1 | provisional/frozen separation and unrelated-retake stability | M2/G3 |
| Structured telemetry and run report | E2-S4 | schema, redaction, reconciliation, bounded growth | M2 |
| Immutable human approval | E2-S6 | checksum/staleness/reviewer identity tests | M2 |
| Deterministic Markdown compilation | E3-S1, E3-S2, E3-S3 | parser properties, goldens, idempotence, audit | G3 |
| Protected-term segmentation | E3-S4 | split-prevention and pronunciation-registry tests | G3 |
| Version and upgrade impact | E3-S5 | compatibility matrix and stale-selection dry run | G3 |
| Post-render in-process ASR | E4-S0, E4-S4 | pool unload, cached-audio verification, no Chatterbox invocation | G3 |
| Expected-ASR lattice and explicit promotion | E4-S2 | alternatives, uncalibrated-term review, promotion audit | G3 |
| ASR numerical gates | E4-S3 | clean corpus and seeded-defect qualification | G3/M3 |
| Distinct Rendered/Verified/Published states | E4-S4, E6-S4 | state-transition and unresolved-finding tests | G3/M3 |
| Frozen voice/style loudness references | E5-S1 | calibration provenance and stable unrelated gain decisions | G3 |
| Resource-governed worker pool | E5-S2 | actual parallelism, CPU/RAM oversubscription rejection | G3/M3 |
| Bounded retries and process cleanup | E5-S3 | timeout, cancellation, orphan, retry-budget tests | M3 |
| Security boundary | E5-S4, E6-S2 | hostile inputs, malformed frames, threat and dependency review | M3 |
| Long-form reliability | E6-S1 | 45–60 minute soak, listening, drift, resource trend gates | M3 |
| Reproducible and reversible release | E6-S3 | clean install, bundle verification, rollback rehearsal | M3 |
| Production authorization | E6-S4 | complete gate index and publish refusal tests | M3 |

## Maintenance rule

When an ADR requirement, story, or named test changes, update this matrix in the same pull request. A row may point to evidence instead of an automated test only when the Delivery Plan classifies the result as evidence and supplies a written protocol.
