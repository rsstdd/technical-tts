# E1-S1 Evidence Provenance Reconciliation v2

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Candidate revision: branch `e1s1/workspace-ci-contract-baseline`, working tree at the
  twenty-first audit
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: `e1-s1-evidence-provenance-reconciliation-v1`

## Scope and decision

This record supersedes `e1-s1-evidence-provenance-reconciliation-v1`, SHA-256
`2094e0b52087cbd5208f024715e119911a0dd712a21abd074b7202fd471b2b81`, for its accounting table and
its verification run. V1 remains the immutable record of the bytes it read, and every
justification it gives — §Affected accepted records, §Draft records pinned to bytes this branch
has since moved, §Drift predating this branch — stands as written and is not restated here.

Two things forced a successor rather than a second reconciliation beside v1.

**The twenty-first audit removed a power v12 had been using.** Until that audit,
`scripts/check-evidence-provenance.py` honored an `## Accounted provenance mismatches` section in
*any* accepted record, contrary to its own docstring and to `evidence/README.md` §Provenance.
`e1-s1-provisional-contract-baseline-v12` carried three rows under that heading and they were
taking effect. With the restriction in place they take none, and two of them describe real
mismatches that still need an accounted home. They are added below. The third named v1 itself and
is not carried: a superseded record is not checked, so it needs no excuse.

**V1's own table of pinned-versus-current digests was pinning documents.** Its §Affected accepted
records table is written `| Document | Pinned digest | Current digest |`, and the checker pins
every backticked repository path that precedes the *final* full-length SHA-256 in a row. The
"current digest" column therefore became seven live pins recording what those documents happened
to hash on 2026-08-28 — a snapshot that any later revision invalidates, which is what
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` then did. The neighboring §Drift table
already avoided this by writing digests truncated (`38527e02b8ef…`), which no pin matches. This
record states no full-length digest except the one supersession pin above, so it cannot strand
itself the same way.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all four hold:

1. Every row v1 accounted for is carried forward or shown to be unnecessary, with none dropped
   silently. Three are shown unnecessary and dropped with the evidence for it.
2. The rows stranded by the twenty-first audit's restriction are accounted for here, in a record
   whose identifier makes it a reconciliation record.
3. No row is redundant: removing any one produces exactly one refusal, and it names that row.
4. `python3 scripts/check-evidence-provenance.py` exits `0` against the working tree this record
   describes.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no others; neither this
record's existence nor a prose mention suppresses a mismatch. Twenty-six rows are v1's,
unchanged and justified there. Two are new. Three of v1's are dropped, and §The three rows this
record drops says why.

| Citing record | Cited repository path |
|---|---|
| `e0-s3-g0-qualification-decision-v3` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` |
| `e0-s3-g0-qualification-decision-v3` | `docs/operations/REFERENCE-ENVIRONMENT.md` |
| `e0-s3-audit-remediation-v2` | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` |
| `e0-s4-provisional-contract-baseline-v2` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e0-s4-provisional-contract-baseline-v2` | `docs/architecture/WALKING-SKELETON.md` |
| `e0-s4-provisional-contract-baseline-v2` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e0-s4-provisional-contract-baseline-v2` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `evidence_e0_descope_ladder_is_ratified` | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` |
| `evidence_e0_open_questions_have_gate_aligned_deadlines_and_owners` | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` |
| `evidence_e0_milestone_matrix_has_one_owner_and_gate_per_requirement` | `docs/governance/MILESTONE-CAPABILITY-MATRIX.md` |
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` |
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `docs/adr/ADR-0004-voice-content-and-retention-policy.md` |
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `evidence/rights/rights-chatterbox-code-v2/record.md` |
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `evidence/rights/rights-chatterbox-weights-v2/record.md` |
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `evidence/rights/rights-voice-owner-fallback-v2/record.md` |
| `evidence_e0_source_provenance_use_and_distribution_classification_complete_v3` | `docs/governance/RELEASE-PROFILES.md` |
| `evidence_e0_source_provenance_use_and_distribution_classification_complete_v3` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e0-s3-g0-qualification-decision-v3` | `evidence/gates/g0/e0-s3/e0-s3-audit-remediation-v2.md` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/chatterbox_spike.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_chatterbox_spike.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/analyze_wav_variation.py` |
| `e0-s3-audit-remediation-v2` | `scripts/qualification/tests/test_analyze_wav_variation.py` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` |

### The three rows this record drops

V1 accounted for three pairs citing `e1-s1-provisional-contract-baseline-v7`. None of them
suppresses anything: `-v8` carries `- Status: Accepted` and
``- Supersedes: `e1-s1-provisional-contract-baseline-v7` ``, so `-v7` is retired by metadata and
is not checked at all. The rows were dead when v1 was written, and v1's §Verification — which
reports that every one of its 28 rows produced exactly one refusal when removed — did not catch
them. This record's sweep did, and §Verification below reports the result rather than the claim.

Dropping them is what v1 §Open findings directs for exactly this case. `-v5` and `-v6` are a
different matter and keep their rows: `-v6` is superseded only by the *proposed* `-v7`, and `-v5`
only by `-v6`, so an unapproved successor retires neither and both are still checked.

### The two rows this record adds

`e1-s1-provisional-contract-baseline-v5` and `-v6` each pin
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` at
`7367c36faea9…`, which the eighteenth audit moved to `cf458da1d533…` when it revised the
mechanization list. Both records are part of the unapproved `-v5`/`-v6`/`-v7` chain v1 §Draft
records already describes: unapproved, so not retired by any successor, and therefore
accumulating mismatches they cannot clear on their own authority. This is the same class v1
accounted for, not a new one, and v1's open finding on that chain is carried forward below
unchanged.

## Superseded without supersession metadata

Carried forward from v1 unchanged, and reproduced rather than referenced because the checker
reads this table to decide what to skip. It is load-bearing: `- Supersedes:` metadata retires
eighteen records on its own, and these six are retired by nothing else. The six were accepted
before
`- Supersedes:` metadata was required and state their supersession in prose alone. They cannot
gain the line: amending an accepted report is what `evidence/README.md` forbids.

| Superseded record | Superseded by |
|---|---|
| `e0-s3-g0-qualification-report-v1` | `e0-s3-g0-qualification-decision-v2`, itself superseded by `-v3` |
| `e0-s4-provisional-contract-baseline-v1` | `e0-s4-provisional-contract-baseline-v2` |
| `e1-s1-provisional-contract-baseline-v1` | `e1-s1-provisional-contract-baseline-v2` |
| `e1-s1-provisional-contract-baseline-v2` | `e1-s1-provisional-contract-baseline-v3` |
| `e1-s1-provisional-contract-baseline-v3` | `e1-s1-provisional-contract-baseline-v4` |
| `e1-s1-provisional-contract-baseline-v4` | `e1-s1-provisional-contract-baseline-v5` |

Only the first column is read; the second is for the reader.

## Open findings

V1's two open findings carry forward unchanged in substance. The third is new.

| Finding | Severity | Owner | Required action | Deadline |
|---|---|---|---|---|
| The E1-S1 baseline chain `-v5` and `-v6` is unapproved, so six of its rows are accounted for rather than retired. `-v7` left the chain when `-v8` was accepted, and its three rows are dropped above | Not blocking for E1-S1 | Engineering owner | Approve the chain, or the record that supersedes it. An accepted successor removes `-v5` and `-v6` from checking by metadata, and the six rows should then be dropped | Before G1 gate review |
| Pre-branch drift is unreconciled, and the rights-record rows are unreviewed | Blocking for G1 acceptance; not blocking for E1-S1 | Engineering owner | Reconcile each row: confirm the revision left the record's conclusion intact, or supersede the record. Then drop its row from §Accounted provenance mismatches | Before G1 gate review |
| `declared_superseded_ids` reads §Superseded without supersession metadata from any record whose status is Accepted, including one an accepted record has since superseded. A retired record therefore keeps a grant power, which is the defect class the twenty-first audit closed for `accounted_mismatches` and did not close here | Not blocking; no wrong entry exists today | Engineering owner | Filter that scan to active accepted records, or record why a retired record should keep the power. This record carries the table forward so the answer changes nothing today | Before G1 gate review |

## Verification

Ubuntu 24.04 under WSL2 on 2026-08-29, against the working tree this record describes:

- `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` — pass, 12 tests.
- `python3 scripts/check-evidence-provenance.py` — pass, exit `0`.
- The sweep was run over v1's 29 rows before any were dropped: 26 removals each produced exactly
  one refusal naming that row, and the three `e1-s1-provisional-contract-baseline-v7` rows each
  produced none. Those three are dropped above on that evidence. Re-run over the 28 rows this
  record carries, every removal produced exactly one refusal naming that row, so no carried row
  is redundant and none suppresses more than the pair it names.
- The claim that this record's §Superseded without supersession metadata table is load-bearing
  was checked rather than assumed: `superseded_ids` returns eighteen records and
  `declared_superseded_ids` adds exactly the six that table names.
- The 26 carried rows were extracted from v1's own table programmatically rather than retyped,
  and v1's digest above was produced by `hashlib.sha256` over its bytes on the date above.

## Decision

- [x] **Provenance reconciled for E1-S1 through the twenty-first audit; pre-branch drift and the
      unapproved draft chain routed to the open findings above**
- [ ] Supersede the affected records

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept the carried rows, the two added rows, and the open finding this record raises against `declared_superseded_ids` rather than closes | 2026-08-29 |
| Project owner | Ross Todd | Accept that v1 is retired for its table and verification run while its justifications remain the reference, pinned above by digest | 2026-08-29 |
