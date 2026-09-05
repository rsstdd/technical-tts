# Risk, Open-Question, and Descope Register

## Open questions

Unresolved answers remain explicit. `TBD` does not authorize a default that expands scope or rights.

| ID | Question | Owner | Decision deadline | Blocking effect | Current disposition |
|---|---|---|---|---|---|
| OQ-01 | What exact filesystem location does `publish` write, and who consumes it? | Project owner | Before G3 | Blocks external-publication definition and M3 | TBD; private previews only |
| OQ-02 | Is version 1 single-machine, single-user, and local-filesystem only? | Project owner | Before G0 | Changes threat model and architecture | Proposed: yes |
| OQ-03 | Is version 1 English only? | Project owner | Before G0 | Changes normalizer, ASR, and corpus | Proposed: yes |
| OQ-04 | Which voice configuration has valid consent and permitted use? | Project owner/rightsholder | Before real voice use | Blocks G0 real-voice path | Resolved: `owner-fallback-v1`, acquired and approved under `rights-voice-owner-fallback-v2`; selected for version 1 by ADR-0001-D003 |
| OQ-05 | Which source content may be narrated or distributed? | Project owner/rightsholder | Before qualification corpus use | Blocks affected use and external publication | Classify each source |
| OQ-06 | What is the named reference machine and backup/recovery plan? | Engineering owner | Full-box machine before G3; backup before M3 | Blocks G3 performance qualification | Constrained development environment `reference-wsl2-d9d550f06b783405` measured; full-box deployment configuration not yet named; no qualified backup exists; accept single-machine risk with an eight-working-hour target to rebuild and rerun critical qualification before M3 |
| OQ-07 | Does failed ASR calibration block 1.0? | Project owner | Before corpus investment | Changes M3 scope | Resolved: no; ADR-0001-D001 requires recorded ASR triage, complete human review while ASR is unqualified, and explicit disclosure |
| OQ-08 | What release signing mechanism and key custody apply? | Project owner | Before G3 | Blocks M3 release process | TBD |
| OQ-09 | Is watermarking required by model terms, voice terms, or distribution policy? | Rights approver | Before G3 | Blocks final audio profile | TBD |
| OQ-10 | Who is the independent listener representative? | Project owner | Before G1 listening approval | Blocks independent listening gates | Unassigned |
| OQ-11 | Who owns human review when the project owner is unavailable? | Project owner | Before M2 | Blocks every gate needing a listening judgment | Resolved: no deputy; review waits for the project owner. `OQ-10` stays unassigned, and the accepted risk is that an unavailable owner stops acceptance rather than review passing to someone unnamed |
| OQ-12 | What cache budget applies to private preview? | Engineering owner | Before M2 | Bounds cache growth and the destructive prune E2-S5 owns | Resolved: none; the cache is unbounded and reclamation is an explicit operator prune. E2-S2 ships prune as report-only, and E2-S5 inherits no byte ceiling from this answer |
| OQ-13 | Where are private previews written and backed up? | Engineering owner | Before M2 | Sets recovery expectations for preview output | Resolved: written beneath the workspace `previews/<lesson>/packages/<generation>` root; not backed up. A lost preview is reproducible from its lesson and cache, so the exposure is time, consistent with `OQ-06`'s accepted single-machine risk |
| OQ-14 | Is a second engineer available for any independently schedulable track? | Project owner | Before M2 | Changes schedule parallelism and the M2 candidate date | Resolved: no; the project is solo and every track is serial. The T-CLI track stays separable in principle but is not independently scheduled, which is the condition the `Solo schedule overload` risk row already prices |

## Risk register

| ID | Risk | Probability | Status | Impact | Mitigation | Trigger/owner |
|---|---|---:|---|---:|---|---|
| R-01 | CPU-only Chatterbox misses a performance gate | High | Accepted through G2 | High | ADR-0002 permits development progression on the constrained WSL2 allocation; retain measured estimates and qualify the full-box configuration before G3 | Full-box result missing, `RTF > 6.0`, or cold 60-minute projection `> 21,600` seconds; engineering owner |
| R-02 | Model or voice rights block intended use | Medium | Open | Critical | Rights records and owner-recorded fallback | Missing or incompatible permission; project owner |
| R-03 | Chatterbox output WAV is unsupported or inconsistent | Low | Open | High | Variant round-trip test and bounded decoder fallback | G0 compatibility failure; engineering owner |
| R-04 | Model output is not byte deterministic | High | Open | Medium | First-valid-artifact-wins cache and retained artifact reconstruction | Determinism evidence; engineering owner |
| R-05 | WSL2 resource topology causes oversubscription | Medium | Open | High | Physical-core detection, conservative fallback, RAM/core validation | Resource-gate failure; runtime owner |
| R-06 | Native ASR integration delays G3 | Medium | Open | Medium | Keep ASR outside M2 and preserve human-review authority | E4-S0 estimate breach; project owner |
| R-07 | ASR gates fail | Medium | Open | High | Improve verifier or amend release control to full human review | Any ADR-0005 class failure; project owner |
| R-08 | Content or voice data leaks through logs/artifacts | Low | Open | Critical | Redaction defaults, governed roots, CI fixture policy | Any leak; security owner |
| R-09 | Filesystem corruption or interrupted writes lose work | Low | Open | High | Atomic publication, checksums, reconciliation, fault injection | Integrity failure; runtime owner |
| R-10 | Solo-engineer review blind spot | Medium | Open | Medium | Independent listener, explicit role sign-offs, contract suites | G1 and M3; project owner |
| R-11 | Upstream model/tool drift invalidates evidence | Medium | Open | High | Pin identities, qualification checksums, upgrade impact report | Any dependency change; engineering owner |
| R-12 | Backlog process consumes delivery capacity | Medium | Open | Medium | One source of truth, generated views, minimum required records | Weekly review; project owner |
| R-13 | No qualified backup reference machine exists | Not estimated | Accepted | High | Preserve committed/redacted evidence and pinned inventories; target eight working hours to rebuild and rerun critical qualification on replacement hardware before M3 | Primary loss or replacement; engineering owner |

## Ratified descope ladder

Apply the first sufficient step. Never descope safety, correctness, rights, integrity, recovery, or truthful release status.

| Order | Permitted descope | Earliest gate affected | Preserved invariant |
|---:|---|---|---|
| 1 | Reduce example and qualification lesson variety | G1/M2 | Real end-to-end path remains |
| 2 | Ship M2 with one approved owner-recorded voice | M2 | Consent and checksum controls remain |
| 3 | Keep pool size one through G3 | G3 | Resource governance and production qualification remain before M3 |
| 4 | Defer nonessential CLI convenience commands | M2/G3 | Validate, preview, inspect, resume, retake, accept, and doctor remain |
| 5 | Limit Markdown support to explicitly documented constructs | G3 | Unsafe transformations warn or fail |
| 6 | Defer calibrated ASR release control through an ADR amendment | M3 | Every production segment receives immutable human review |
| 7 | Delay external publication and ship private-only 1.0 | M3 | Rights and release-status honesty remain |

**Ratification:** Ratified 2026-08-23 by the project owner. Scope removal is authorized only by applying the first sufficient step in ladder order.

Descope step 2 was applied on 2026-08-25 through approved ADR-0001-D003. Version 1 uses the one
approved owner-recorded voice. Nadia and Tom remain `Review required` and outside the version 1
critical path; no rights, identity, or two-speaker capability is implied.

| Role | Name | Decision | Date | Notes |
|---|---|---|---|---|
| Project owner | Ross Todd | Ratified | 2026-08-23 | — |
| Engineering owner | Ross Todd | Ratified | 2026-08-23 | Same person during solo development |
