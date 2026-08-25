# Routing Tables

## Decision routing

| Question | Decider | Consult | Record | Escalation deadline |
|---|---|---|---|---|
| Architecture or ADR-0001 invariant | Engineering owner | Project owner | New or amended ADR | Before implementation |
| MVP scope or milestone tradeoff | Project owner | Engineering owner | Delivery Plan amendment and issue update | Before sprint commitment |
| Worker protocol or public schema | Engineering owner | Every consuming track | Interface-change record | Before merge |
| Chatterbox/model revision | Engineering owner and project owner | Rights reviewer | ADR-0002 | Before download or qualification |
| Audio threshold, codec, or loudness reference | Engineering owner and listener representative | Project owner | ADR-0003 | Before production qualification |
| Voice use, consent, retention, or watermarking | Project owner/rightsholder | Engineering owner | ADR-0004 and rights record | Before real voice use |
| ASR release-control status | Engineering owner and human-review owner | Project owner | ADR-0005 | Before G3 exit |
| Source-content distribution rights | Project owner/rightsholder | Rights reviewer | Content-rights record | Before external publication |
| Security control or accepted vulnerability | Engineering owner | Project owner | Threat/risk record | Before release candidate |
| Descope | Project owner | Engineering owner | Ratified descope ladder and gate record | Before work is removed |
| Production publication | Project owner | All required approvers | Release checklist | M3 only |

## Work routing

| Work type | GitHub label | Primary path | Required validation |
|---|---|---|---|
| Governance and evidence | `track:governance` | `docs/governance/`, `evidence/` | Document consistency and approval record |
| Domain and durable state | `track:core` | `crates/study-tts-core/`, schemas | T1–T4, schema compatibility, property tests |
| Chatterbox worker | `track:worker` | `worker/`, runtime worker adapter | Shared contracts, offline real render, containment |
| Audio and package | `track:audio` | Runtime audio/export modules | Sample arithmetic, FFmpeg integration, listening |
| CLI and diagnostics | `track:cli` | `crates/study-tts-cli/` | CLI integration, JSON output, exit codes |
| Markdown authoring | `track:authoring` | Core compiler and fixtures | Parser properties, goldens, audit output |
| ASR verification | `track:verification` | Runtime verifier, verification schemas | Identity invalidation, calibration, seeded defects |
| Reliability and lifecycle | `track:runtime` | `crates/study-tts-runtime/` | Fault injection, resource governance, recovery |
| Qualification and release | `track:release` | `docs/operations/`, evidence | T6, rights, SBOM, clean-machine, rollback |

## Failure routing

For failures with structured runtime advice, this table is mirrored by
`crates/study-tts-runtime/src/error/mod.rs::BuildError::remedy` and the category `remedy`
methods under `crates/study-tts-runtime/src/error/`. The test
`error::tests::t1_e0_governed_remedy_mappings_are_exhaustive` pins owner, action, and routing-row
names with exhaustive matches so a new refusal cannot inherit advice without review.

| Failure | Immediate action | Owner | Publication effect |
|---|---|---|---|
| Invalid lesson or schema | Reject before worker startup | Core | Blocked |
| Voice consent/checksum mismatch | Refuse profile load | Project owner | Blocked |
| Worker protocol or containment failure | Terminate worker tree; preserve diagnostics | Worker/runtime | Blocked |
| Invalid or over-range audio | Quarantine unique attempt; bounded retry | Audio/runtime | Blocked for segment |
| ASR verifier failure | Preserve synthesis cache; stop at verification | Verification | Preview may use human authority; production blocked per profile |
| Human review finding | Record finding; retake or accept with authority | Human-review owner | Production blocked until resolved |
| State or checksum corruption | Refuse overwrite; run reconciliation | Runtime | Blocked |
| Missing rights classification | Restrict to permitted private scope | Project owner | External publication blocked |
| Failed release gate | Preserve candidate; create corrective issue | Gate owner | Production blocked |

## Artifact routing and retention

| Artifact | Location | Commit? | Authority |
|---|---|:---:|---|
| ADRs and policy | `docs/adr/`, `docs/governance/` | Yes | Reviewed Markdown |
| Schemas and safe fixtures | `schemas/`, `fixtures/` | Yes | Versioned source |
| Test reports without sensitive content | `evidence/tests/<run-id>/` | Conditional | Checksummed report |
| Qualification reports | `evidence/qualification/<gate>/<run-id>/` | Report only | Signed/checksummed index |
| Model weights and tokenizers | Managed external model root | No | URI and checksum record |
| Voice references and consent evidence | Restricted managed voice root | No raw audio | Consent record and checksum |
| ASR corpus audio | Governed external artifact location | No raw corpus | Corpus manifest |
| Cache, jobs, staging, quarantine | Local `data/` roots | No | Runtime manifests |
| Private previews | `previews/` runtime root | No | Package manifest and review record |
| Production bundle | Configured publish root | No binaries by default | Signed release manifest |

Never place source text, raw voice paths, voice recordings, model weights, secrets, or production audio in GitHub issues, pull requests, CI logs, or diagnostic bundles.
