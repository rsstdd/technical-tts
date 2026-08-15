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

## Revocation and incident handling

Consent revocation or a rights incident immediately disables new use of the affected profile or source. Preserve the audit record, identify affected packages by checksum and manifest reference, quarantine unpublished outputs, and follow the approved withdrawal decision. Deletion of material evidence requires project-owner authorization and a record of what was removed.

