# Documentation Index

This index routes work to the controlling document. ADR-0001 and explicit accepted amendments
control architecture. The Delivery Plan controls scope, sequence, gates, named tests, and
evidence. Documents below explain execution without superseding either authority.

## Start here

| Need | Document |
|---|---|
| Architecture and production invariants | [`adr/ADR-0001-production-rust-study-guide-tts.md`](adr/ADR-0001-production-rust-study-guide-tts.md) and its accepted amendments |
| Approved epics, stories, tasks, tests, gates, and schedule | [`../DELIVERY-PLAN.md`](../DELIVERY-PLAN.md) |
| How work is governed and accepted | [`governance/PROJECT-EXECUTION-CHARTER.md`](governance/PROJECT-EXECUTION-CHARTER.md) |
| Capability ownership and approval | [`governance/MILESTONE-CAPABILITY-MATRIX.md`](governance/MILESTONE-CAPABILITY-MATRIX.md) |
| Ownership, decisions, escalation, and artifact routing | [`governance/ROUTING-TABLES.md`](governance/ROUTING-TABLES.md) |
| ADR requirement to story and validation mapping | [`governance/TRACEABILITY-MATRIX.md`](governance/TRACEABILITY-MATRIX.md) |
| Risks, open questions, and authorized descoping | [`governance/RISK-OPEN-QUESTIONS-DESCOPE.md`](governance/RISK-OPEN-QUESTIONS-DESCOPE.md) |
| Rights, consent, retention, and artifact handling | [`governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](governance/RIGHTS-DATA-ARTIFACT-POLICY.md) |
| Investigated voice sources and admission status | [`governance/VOICE-CANDIDATE-REGISTER.md`](governance/VOICE-CANDIDATE-REGISTER.md) |
| Interface freeze and changes after G1 | [`governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md) |
| GitHub Project operating rules | [`governance/GITHUB-PROJECT-PLAYBOOK.md`](governance/GITHUB-PROJECT-PLAYBOOK.md) |
| Test tiers and TDD workflow | [`testing/TEST-STRATEGY.md`](testing/TEST-STRATEGY.md) |
| Test artifact provenance | [`testing/TEST-DATA-MANIFEST.md`](testing/TEST-DATA-MANIFEST.md) |
| Qualification and evidence records | [`testing/EVIDENCE-AND-QUALIFICATION.md`](testing/EVIDENCE-AND-QUALIFICATION.md) |
| Daily development workflow | [`operations/DEVELOPMENT-WORKFLOW.md`](operations/DEVELOPMENT-WORKFLOW.md) |
| Code style rules applied to all code | [`../.claude/skills/clean-code/SKILL.md`](../.claude/skills/clean-code/SKILL.md) |
| Code review standard and severity scale | [`../.claude/skills/rust-review/SKILL.md`](../.claude/skills/rust-review/SKILL.md) |
| Executable E0-S0 integration order and provisional seams | [`architecture/WALKING-SKELETON.md`](architecture/WALKING-SKELETON.md) |
| E0-S4 provisional contract IDs, fakes, suites, and stabilization | [`architecture/PROVISIONAL-CONTRACT-BASELINE.md`](architecture/PROVISIONAL-CONTRACT-BASELINE.md) |
| E0-S4 cache/package audit amendment | [`architecture/E0-S4-INTERFACE-CHANGE-001.md`](architecture/E0-S4-INTERFACE-CHANGE-001.md) |
| E1-S1 synthesis and verification identity amendment | [`architecture/E1-S1-INTERFACE-CHANGE-001.md`](architecture/E1-S1-INTERFACE-CHANGE-001.md) |
| Reference-machine qualification record | [`operations/REFERENCE-ENVIRONMENT.md`](operations/REFERENCE-ENVIRONMENT.md) |
| Worker bundle manifest, lock procedure, and offline behavior | [`operations/WORKER-ENVIRONMENT.md`](operations/WORKER-ENVIRONMENT.md) |
| E0-S3 G0 feasibility decision | [`../evidence/gates/g0/e0-s3/e0-s3-g0-qualification-decision-v3.md`](../evidence/gates/g0/e0-s3/e0-s3-g0-qualification-decision-v3.md) |
| E0-S4 provisional contract evidence | [`../evidence/gates/g0/e0-s4/e0-s4-provisional-contract-baseline-v2.md`](../evidence/gates/g0/e0-s4/e0-s4-provisional-contract-baseline-v2.md) (v1 retained as historical provenance) |
| E1-S1 controlled-record digests, superseding the E0-S4 table | [`../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v12.md`](../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v12.md), which supersedes [`…-v11.md`](../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v11.md) and through it v10 down to v1 for the table and verification run. v12 is the accepted current record; it carries the audit 20 environment-probe remediation — the `-S` bootstrap, manifest layout `1.2` authenticating installed `RECORD`s, and the corrected interpreter attach step. Three provenance-reconciliation records sit beside them and account for named digest movements the superseded drafts pin |
| Upgrade and compatibility procedure | [`operations/UPGRADE-RUNBOOK.md`](operations/UPGRADE-RUNBOOK.md) |
| Release, recovery, and rollback | [`operations/RELEASE-AND-ROLLBACK.md`](operations/RELEASE-AND-ROLLBACK.md) |
| Threat model | [`security/THREAT-MODEL.md`](security/THREAT-MODEL.md) |

## Decision records

| ADR | Status | Acceptance condition |
|---|---|---|
| ADR-0001 | Accepted | Architectural authority |
| [`ADR-0001-D001`](adr/deviations/ADR-0001-D001-asr-release-condition.md) | Approved amendment | ASR triage is required; failed calibration keeps ASR advisory rather than blocking release |
| [`ADR-0001-D002`](adr/deviations/ADR-0001-D002-constrained-development-performance-gate.md) | Approved amendment | Constrained-development performance waiver expires before G3 |
| [`ADR-0001-D003`](adr/deviations/ADR-0001-D003-single-instructor-fallback.md) | Approved amendment | Version 1 selects the approved owner-recorded single-instructor configuration |
| [`ADR-0001-D004`](adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md) | Approved amendment | Environment-lock verification is a precondition on the worker-bundle identity, not an input to it |
| [`ADR-0001-D005`](adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md) | Approved amendment | A pre-G1 breaking correction to an unreleased seam version may retain that version instead of incrementing the major; expires at G1 |
| [`ADR-0002`](adr/ADR-0002-model-hardware-voice-format-qualification.md) | Accepted with scoped waiver | Full-box performance qualification before G3 |
| [`ADR-0003`](adr/ADR-0003-production-audio-quality-profile.md) | Proposed evidence record | Audio calibration and listener approval |
| [`ADR-0004`](adr/ADR-0004-voice-content-and-retention-policy.md) | Proposed evidence record | Rights, consent, and retention approval |
| [`ADR-0005`](adr/ADR-0005-asr-calibration-and-release-control.md) | Proposed evidence record | Measured ASR gates or explicit amendment |

ADR-0002 approves ADR-0001-D002's constrained-development performance waiver.

## Templates

Use the templates in [`templates/`](templates/) for gate reviews, evidence, listening review, rights records, ADR deviations, and releases. A blank template is not approval. Completed records belong under `evidence/` or the governed external artifact location defined by the relevant policy.

Interface amendments use
[`templates/INTERFACE-CHANGE-TEMPLATE.md`](templates/INTERFACE-CHANGE-TEMPLATE.md)
and remain provisional until the G1 freeze record is approved.
