# ADR-0004: Voice, Content, and Retention Policy

- **Status:** Proposed; rights records incomplete
- **Owner/approver:** Project owner/rightsholder
- **Engineering reviewer:** Engineering owner
- **Depends on:** ADR-0001, E0-S2, E6-S2

## Decision to be completed

Approve voice profiles, consent scopes, watermark requirements, source-content classifications, distribution scope, access, retention, revocation, deletion, backup, and incident handling.

## Non-negotiable controls

- Public-figure cloning is prohibited.
- A cloned voice requires a consent record, reference checksum, permitted-use scope, and build audit event.
- Private use and publication rights are separate.
- Unresolved content rights block external publication.
- Raw voices, private content, model artifacts, and generated audio do not enter Git or ordinary CI.
- Revocation disables new use and triggers an affected-artifact review.

## Approval table

| Record | Owner/rightsholder | Scope | Retention | Watermark | Status |
|---|---|---|---|---|---|
| Primary voice profile | TBD (`rights-voice-nadia-v1`, `rights-voice-tom-v1`) | TBD; no lawful source acquired | TBD | TBD | Review required; fallback owner voice is the authorized substitute |
| Fallback owner voice | Ross Todd (`rights-voice-owner-fallback-v1`) | Private synthesis, internal owner use only | Per consent scope; raw reference outside Git under the restricted voice root | Deferred to OQ-09 (before G3) | Pre-authorized per E0-S2 task 3 |
| Qualification source corpus | Ross Todd (`rights-qualification-sources-v1`) | owner-authored; private qualification use | Repository lifetime for committed fixtures; ADR-0005 for external corpora (`rights-asr-corpora-v1`) | N/A | Approved for recorded scope |
| External distribution | Project owner | Internal, owner use only per `docs/governance/RELEASE-PROFILES.md` §4 | Release artifacts per this policy's retention defaults | TBD (OQ-09) | Recorded; reopening conditions in RELEASE-PROFILES §4 |

## Acceptance

Accept before real voice use beyond a lawful test profile and before any production publication. Each underlying record must be independently verifiable and referenced by checksum or governed identifier.

