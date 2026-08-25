# ADR-0001-D002 — Select the single-instructor voice configuration

- **Status:** Approved
- **Date:** 2026-08-25
- **Raised by:** Ross Todd
- **Approved by:** Ross Todd (project owner and fallback-voice rightsholder; roles recorded
  separately under `docs/governance/PROJECT-EXECUTION-CHARTER.md`)
- **Affects:** ADR-0001 §5.1, §5.2, §18, and §19; `DELIVERY-PLAN.md` E0-S2 and E0-S3;
  `docs/governance/RELEASE-PROFILES.md` §3

## The contradiction

ADR-0001 permits an approved single-instructor fallback when the two-speaker format is not
viable, and the Delivery Plan risk response selects the owner-recorded fallback when no lawful
Nadia or Tom source is available at G0. ADR-0001 §5.2 and §19 nevertheless say real
qualification must wait for two accepted production profiles.

No lawful Nadia or Tom source was available at the G0 deadline. Their rights records remain
`Review required` and fail closed. The owner-recorded `owner-fallback-v1` profile is acquired,
consented, checksummed, safely loadable, and approved for owner-only private synthesis and voice
qualification. Requiring unavailable second-party voices after selecting that lawful fallback
would leave E0-S2's accepted risk response unusable.

## Decision

Version 1 selects the approved owner-recorded single-instructor configuration before E0-S3.
Nadia and Tom are outside the version 1 critical path and are not required for E0-S2 closure,
the E0-S3 real-model spike, M2, or version 1 production qualification. Their existing records
remain `Review required`; no source, consent, identity, or capability is implied.

The single-instructor configuration has these constraints:

- every spoken turn uses the one approved `owner-fallback-v1` profile;
- learner questions become instructor-voiced rhetorical prompts as ADR-0001 §5.1 prescribes;
- the system must not relabel the owner profile as Nadia, Tom, a second speaker, or multiple
  speakers;
- the selected profile, consent scope, conditionals, and synthesis identity remain subject to
  every existing checksum, offline, cache, listening, and release gate;
- the current authorization remains owner-only private synthesis and voice qualification, with
  no publication, distribution, or commercial use.

A future two-speaker format requires a new accepted decision plus separately approved,
provenance-ready voice profiles. It cannot enter through configuration drift or relabeling.

## Rationale

This applies the already ratified single-instructor descope while preserving the controls that
matter: one standard Chatterbox backend, reviewed text, truthful speaker identity, explicit
consent, immutable profile identity, offline rendering, and human qualification. It removes an
unavailable format dependency rather than weakening a rights or integrity gate.

The tradeoff is explicit: version 1 gives up two-speaker dialogue, the Nadia/Tom recognizability
criterion, and the dialogue-credibility measurement. Naturalness, fatigue, instructional
prosody, voice consistency, and long-form listening remain qualification requirements for the
selected owner voice.

## Amendment to ADR-0001 §5.2

For version 1, replace the requirement that qualification wait for two accepted production
profiles with this rule:

> Qualification may begin with the selected single-instructor fallback after its source,
> consent, reference, conditionals, extractor identity, and permitted scope are approved and
> checksummed. A two-speaker configuration still requires two independently approved profiles.

The remaining profile-integrity and safe-conditional-loading requirements of §5.2 are unchanged.

## Amendment to ADR-0001 §18 and §19

For version 1, interpret the voice acceptance criterion as follows:

> The selected owner voice remains recognizable and consistent throughout the lesson, and the
> build truthfully uses the approved single-instructor format.

The Phase 0 implementation uses and qualifies `owner-fallback-v1`. The two-speaker dialogue gate
is not applicable to this selected format. Any future Nadia/Tom qualification reopens §5.2 and
the dialogue gate through a superseding accepted decision.

## Rollback

This decision can be superseded after two lawful voice profiles are acquired. Existing
single-instructor references, conditionals, evidence, cache entries, and outputs remain
identified by their original hashes and are not relabeled or deleted. A two-speaker build uses
new profile and synthesis identities and reruns every affected qualification and listening gate.

## Verification

- `rights-voice-owner-fallback-v2` records the acquired voice, consent, hashes, and scope.
- `evidence_e0_model_and_voice_rights_records_complete_v3` records the fallback selection and
  passing runtime gate.
- `rights-voice-nadia-v1` and `rights-voice-tom-v1` remain `Review required`.
- E0-S3 must render only with the exact selected owner profile and must not claim Nadia/Tom or
  two-speaker qualification.
- `voice_identity_and_format` remains a required production gate, with its criterion amended in
  `docs/governance/RELEASE-PROFILES.md`.
