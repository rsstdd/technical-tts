# Project Execution Charter

- **Project:** `study-tts`
- **Architecture authority:** ADR-0001 and its approved deviations
- **Delivery authority:** Delivery Plan Version 3
- **System of work:** [GitHub Project 1](https://github.com/users/rsstdd/projects/1)
- **Repository:** [rsstdd/technical-tts](https://github.com/rsstdd/technical-tts)
- **Initial operating model:** one engineer, one project owner, local WSL2 reference machine

## Objective

Deliver a private, human-approved five-minute technical study-guide package at M2, then qualify
production version 1 under ADR-0001 and its approved deviations. Speed comes from the walking
skeleton, fakes, explicit seams, and narrow work in progress. It does not come from skipping
tests, validation, rights checks, or recovery behavior.

## Release profiles

| Profile | Permitted output | Required authority | Prohibited claim |
|---|---|---|---|
| Engineering | Fixtures and diagnostic artifacts | Engineer | Usable preview or production release |
| Private preview | Package under `previews/` with `release_status: private_preview` | Human review record | Production approved, published, or externally distributable |
| Release candidate | Candidate package with unresolved gate visibility | Gate review | Production approved |
| Production | Package written through the production `publish` path | Every M3 gate and project-owner authorization | None beyond recorded distribution scope |

Unknown release statuses are invalid. The production path must fail closed when evidence is missing, stale, unsigned, or inconsistent with the build manifest.

## Milestone ownership

| Gate | Accountable owner | Engineering evidence owner | Approval | Exit artifact |
|---|---|---|---|---|
| G0a Skeleton | Engineering owner | Engineering owner | Project owner | Passing CI run and skeleton manifest |
| G0 Feasibility | Project owner | Engineering owner | Project owner | Completed ADR-0002 draft and G0 gate record |
| G1 Vertical slice | Engineering owner | Engineering owner | Project owner and human reviewer | Three-segment package, contract report, interface-freeze charter |
| M2 Private MVP | Project owner | Engineering owner | Project owner and human reviewer | Five-minute package and immutable approval record |
| G3 Production candidate | Engineering owner | Engineering owner | Project owner | Candidate evidence index and unresolved-gate report |
| M3 Version 1.0 | Project owner | Engineering owner | Project owner, rights approver, listener representative | Signed release checklist and checksummed bundle |

For a personal project, Ross Todd may hold project-owner, engineering-owner, legal-review, security-review, and QA roles, but each approval must name the role and accepted risk separately. The human listener may not be the engineer for final long-form qualification unless the risk is explicitly accepted.

## Capability gates

The complete capability matrix remains in the Delivery Plan. The following rules control interpretation:

1. A later implementation does not retroactively satisfy an earlier feasibility gate.
2. A passing fake does not qualify the real worker.
3. A private preview never satisfies production publication.
4. Advisory or uncalibrated ASR never replaces required human review.
5. A gate passes only when its evidence is immutable, linked, and reproducible from recorded inputs.

## Work-in-progress policy

- Before E0-S4, keep one engineering story active because interfaces are provisional.
- After versioned fakes and seams exist, one active story per independent track is permitted.
- A solo engineer still keeps one implementation story active at a time. Evidence collection may proceed in parallel.
- Review, blocked, and external-evidence work does not consume the implementation slot, but each item needs an owner and next action.
- Fix a broken walking skeleton before starting unrelated implementation.

## Definition of ready

A story is ready when:

- prerequisites and controlling decisions are resolved;
- its acceptance criteria and named tests are understood;
- required fixtures, rights, environments, and fakes are available;
- paths, schemas, and interfaces it consumes have owners;
- no open question can materially change its implementation;
- rollback and failure visibility are defined.

## Definition of done

A story is done only when:

- every task and acceptance criterion is complete;
- deterministic behavior was developed red-green-refactor;
- required tests pass at their declared tier;
- evidence uses the written protocol and records provenance;
- no test is disabled, weakened, or silently reclassified;
- schemas, fixtures, documentation, and examples agree;
- security, containment, recovery, and observability affected by the change are verified;
- the walking skeleton remains green;
- the GitHub story contains links to the change, test results, and evidence;
- unresolved work is a new linked issue rather than hidden follow-up text.

## Sign-off matrix

| Artifact | Required role | Deadline |
|---|---|---|
| MVP capability and use matrix | Project owner | Before E1 |
| Descope ladder and open-question register | Project owner | Before G0 |
| Interface baseline | Engineering owner | E0-S4 |
| Interface freeze charter | Engineering owner | G1 |
| Model, hardware, voice, and format qualification | Engineering and project owner | G0/G3 as recorded in ADR-0002 |
| Production audio profile | Engineering and listener representative | Before production qualification |
| Rights and retention policy | Project owner/rightsholder | Before real voice use and M3 |
| ASR calibration decision | Engineering and human-review owner | Before ASR becomes a release control |
| Threat model and dependency/license inventory | Engineering owner | Before release candidate |
| Release and rollback record | Engineering and project owner | M3 |

## Deviation rule

No schedule pressure authorizes an architectural deviation. A proposed deviation uses the ADR-deviation template, identifies affected stories and tests, states rollback, and receives approval before implementation. Emergency containment may stop work or disable publication; it may not silently change durable behavior.
