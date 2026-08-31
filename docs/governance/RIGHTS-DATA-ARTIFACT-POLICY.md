# Rights, Data, and Artifact Policy

## Scope

This policy covers model code and weights, voices and consent, source content, ASR corpora, generated audio, caches, logs, diagnostic bundles, and release artifacts. It records engineering controls and approvals; it is not a universal legal determination.

## Classification

Every nontrivial input receives one classification before use:

- owner-authored;
- public domain;
- permissively licensed;
- commercially or privately licensed;
- consented voice reference;
- evaluation-only;
- rights review required;
- prohibited.

This list is mirrored by `SourceClassification` in `crates/study-tts-core/src/rights.rs`. The two must agree, and changing either requires a policy amendment rather than an edit.

Private use and external distribution are separate permissions. A record permitting private narration does not permit publication.

## Required records

| Artifact | Required fields | Blocking rule |
|---|---|---|
| Model code/weights | source URI, exact revision, checksum, license text/checksum, permitted scope, reviewer | No real render without approved record |
| Tokenizer/codec | identity, revision, checksum, license | No qualified bundle without record |
| Voice reference | owner/rightsholder, consent scope, checksum, retention, revocation path, reviewer | Profile load fails closed |
| Source lesson | source, author/rightsholder, classification, transformation scope, distribution scope | Unresolved external distribution blocks publish |
| ASR corpus | artifact URI, checksum manifest, license, access, retention, defect provenance | No calibration use without record |
| Generated release | source and voice record IDs, build manifest, permitted audience, retention | No publication beyond recorded scope |

## Storage and access

- Keep raw voices, model weights, ASR corpora, private lessons, and generated audio outside Git.
- Store them under access-controlled Linux-filesystem roots, never `/mnt/c`, unless a separate approved backup process encrypts and verifies them.
- Store only identifiers, checksums, redacted paths, license metadata, and approval records in the repository.
- Logs contain hashes, IDs, timings, states, and error classes. They exclude full source text, spoken text, and raw voice-reference paths by default.
- CI uses safe, licensed, non-sensitive fixtures. CI never receives real voice references or model weights for ordinary pull requests.

## Retention defaults

These defaults apply until ADR-0004 records approved values:

| Data | Default |
|---|---|
| Staging | Delete after successful atomic publication; preserve failed attempts in quarantine |
| Quarantine | Retain until reviewed; deletion is explicit and audited |
| Synthesis cache | Retain while referenced by accepted takes or published manifests |
| Verification evidence | Retain with the build or until superseded under approved retention policy |
| Raw voice references | Minimum access and retention required by consent scope |
| Private previews | Project-owner managed; never automatically published |
| Release artifacts | Retain manifest, checksums, approval, and reconstruction references |

## Enforcement

Each blocking rule above that has an executable protocol is enforced by a named test; the remainder are enforced by review and evidence records.

| Rule | Enforced by |
|---|---|
| Profile load fails closed without a consent record | `t4_e0_missing_voice_consent_blocks_profile_load` |
| An unapproved voice profile enters neither preview nor production | `t4_e0_unapproved_voice_profile_cannot_enter_preview_or_production` |
| A voice checksum mismatch refuses profile use | `t4_e0_voice_checksum_mismatch_blocks_use` |
| A required voice record that is not a regular file refuses profile load, so a checksum cannot be satisfied by a link to a file outside the profile directory | `t4_e0_voice_records_that_are_not_regular_files_are_refused` |
| An unresolved content classification blocks production release | `t4_e0_production_release_rejects_unresolved_content_rights_classification` |
| A use outside a consent record's `permitted_use` scope is refused | `t1_e0_uses_outside_the_recorded_consent_scope_are_refused` |
| A permitted-use value outside the recorded vocabulary is rejected when the consent record is parsed | `t3_e0_unknown_permitted_use_values_are_rejected` |
| Every profile beneath a governed voice root is gated before a worker may deserialize any of them, not only the profile a request names | `t1_e1_a_revoked_profile_the_request_never_names_refuses_the_root` |
| A profile directory whose name is not UTF-8 refuses the whole governed root, because the worker reads that name through `surrogateescape` and would still load it | `t1_e1_a_profile_name_that_is_not_utf8_refuses_the_root` |
| The protocol fake is never told where a governed root is, so no configuration this build makes can start a worker over one that was never gated | `t1_e1_the_protocol_fake_cannot_be_handed_a_governed_root` |

## Revocation and incident handling

Consent revocation or a rights incident immediately disables new use of the affected profile or source. Preserve the audit record, identify affected packages by checksum and manifest reference, quarantine unpublished outputs, and follow the approved withdrawal decision. Deletion of material evidence requires project-owner authorization and a record of what was removed.

