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
| Primary voice profile | TBD | TBD | TBD | TBD | Pending |
| Fallback owner voice | Ross Todd | TBD consent record | TBD | TBD | Pending |
| Qualification source corpus | TBD | TBD | TBD | N/A | Pending |
| External distribution | Project owner | TBD | TBD | TBD | Pending |

## Acceptance

Accept before real voice use beyond a lawful test profile and before any production publication. Each underlying record must be independently verifiable and referenced by checksum or governed identifier.

