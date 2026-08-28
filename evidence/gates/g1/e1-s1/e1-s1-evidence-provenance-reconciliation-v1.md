# E1-S1 Evidence Provenance Reconciliation v1

- Date/time and timezone: 2026-08-28, Europe/Berlin
- Candidate revision: branch `e1s1/workspace-ci-contract-baseline`, commits `7715657..HEAD`
  over merge base `53e03339853f3b7e0c7253039c24aca7fdc0d290`
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

This record supersedes no accepted report, and deliberately so. A digest inside an accepted
record is provenance, not a liveness claim: it names the exact bytes a conclusion was reached
against, and it stays correct when those bytes are later revised — `git show <blob>` still
produces the document the approver read. `evidence/README.md` requires supersession when an
accepted report is *changed*; none is changed here, and none needs to be.

What was missing is the index. E1-S1 revised seven governed documents that three accepted
records pin, and nothing said which pins had become historical or whether the conclusions resting on them
survived. A reader auditing `e0-s3-g0-qualification-decision-v3` today recomputes
`docs/adr/ADR-0002-…` and gets a digest the record does not contain, with no way to tell a
routine revision from a silently rewritten waiver. This record is that index, and
[`../../../../scripts/check-evidence-provenance.py`](../../../../scripts/check-evidence-provenance.py)
is what keeps the next one from having to be written by hand.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all four hold:

1. Every record the check still reads — accepted or awaiting approval — that pins a document
   this branch revised is listed below, with the digest it pins and the digest the document now
   carries.
2. For each, the conclusion the record reached is shown to be unaffected by the revision, by
   naming what the revision changed — not by asserting that it did not matter.
3. Drift that predates this branch's merge base is listed separately, with an owner, rather than
   folded into E1-S1's account or left undiscoverable.
4. A check refuses a future revision that leaves a record pinning bytes not accounted for here,
   so the set is mechanically known rather than remembered.

## Accounted provenance mismatches

`scripts/check-evidence-provenance.py` recognizes these exact pairs and no others; neither this
record's existence nor a prose mention suppresses a mismatch. Every pair is justified below:
E1-S1's own revisions under §Affected accepted records, the unapproved E1-S1 chain under §Draft
records pinned to bytes this branch has since moved, and the rest under §Drift predating this
branch.

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
| `e1-s1-provisional-contract-baseline-v7` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v7` | `crates/study-tts-core/src/lesson.rs` |
| `e1-s1-provisional-contract-baseline-v7` | `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` |

## Superseded without supersession metadata

`scripts/check-evidence-provenance.py` removes a record from checking on a `- Supersedes:`
metadata line, or on a row in this table. Prose supersession never counts on its own, so a new
record must carry the metadata line. The six records below were accepted before that rule and
state their supersession in prose alone. They cannot gain the line: amending an accepted report
is what `evidence/README.md` forbids, and superseding six records to add one line each would be
churn against no reader's benefit. Each row was checked against the successor that claims it,
named beside it.

| Superseded record | Superseded by |
|---|---|
| `e0-s3-g0-qualification-report-v1` | `e0-s3-g0-qualification-decision-v2`, itself superseded by `-v3` |
| `e0-s4-provisional-contract-baseline-v1` | `e0-s4-provisional-contract-baseline-v2` |
| `e1-s1-provisional-contract-baseline-v1` | `e1-s1-provisional-contract-baseline-v2` |
| `e1-s1-provisional-contract-baseline-v2` | `e1-s1-provisional-contract-baseline-v3` |
| `e1-s1-provisional-contract-baseline-v3` | `e1-s1-provisional-contract-baseline-v4` |
| `e1-s1-provisional-contract-baseline-v4` | `e1-s1-provisional-contract-baseline-v5` |

Only the first column is read; the second is for the reader.

## Draft records pinned to bytes this branch has since moved

`e1-s1-provisional-contract-baseline-v5` is committed with a review table that is pending in
every row; `-v6` and `-v7` are drafts of the two audits that followed it. None of the three is
accepted, so none is retired by its successor: `evidence/README.md` gives an unapproved
superseding record no effect, and that rule is what stops a draft from retiring an accepted
report. The cost is that an unapproved chain accumulates mismatches it cannot clear on its own
authority, so each is accounted for here instead. Every pinned digest below was reproduced from
current bytes rather than described from memory; §Verification records how.

All three pin `docs/testing/TEST-DATA-MANIFEST.md` at
`1eaca00abe695c2ea08e9642b9f2fb6a9dfb55f4679eb99b4f14a34b8748b7ae`; it now hashes
`51c5ff77ba57747bb106a0a1733ee5814cef4de13b7d5e051bd317a16bd11525`. Three things moved it. The
manifest gained one row, for the `e1-s1-takes-unusable-lesson-id-v1` contract fixture; its
closing paragraph now states what `t3_e0_registered_fixture_checksums_match_test_data_manifest`
actually covers, which is committed bytes, so an unaccompanied generator change passes until the
fixtures are regenerated; and §What the tenth audit closed recorded the new checksum of
`fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` after it gained two identity-ceiling
cases. No row was removed and no row changed except that one checksum, so every fixture each
record reasoned about is still described by the same line, and none of the three reasoned about
that checksum's value. **All three conclusions stand on this document.**

### `e1-s1-provisional-contract-baseline-v5.md`

Also pins `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` at
`18d36edcbba3406ed35d1765411859434c826eee99ff884dfa4f851a5a9d547b`; it now hashes
`38027f7226cd0d6d38fef071296aeaeab832e544acc3e105b686b4129888a7d4`.

The seventh audit closed a limit that record listed as open. `worker/requirements.lock` gained
four resolution directives and one `--hash=sha256:` per index-supplied pin, so the bullet stating
that the lock "pins versions, not artifact hashes" became false, and the section it pointed at
was replaced. Leaving it would have left an interface-change record advertising a gap that no
longer exists and a cross-reference to a heading that no longer exists. The eighth and ninth
audits then appended their own sections.

This one is not like the pre-branch rows: the revision **does** change what the record describes,
because the lock's bytes are a worker-bundle hash input and the bundle identity moved with them.
What it does not change is v5's conclusion, which is that the E1-S1 contracts and identities are
recorded and tested. The moved identity is recorded in the change record itself, where a reader
looking for the current cache-key state is sent.

### `e1-s1-provisional-contract-baseline-v6.md`

Also pins `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` at
`20ef0fcba49cfbfc21678acd10e06b66ac5b93999fcc8391b8a77e3cdecaca50`, which no commit contains:
the working tree held those bytes between the seventh and the eighth audit and was never
committed there. Removing seven things from the current document reproduces that digest exactly. Four are
v7's audits: §What the eighth audit closed, §What the ninth audit closed, the sentence recording
that the CLI now reports the tested E1-S1 baseline, and the pointer superseding v6 by v7. Three
are the tenth audit's, named under §`e1-s1-provisional-contract-baseline-v7.md` below, and
removing those three alone reproduces the bytes v7 pinned — from which the four above reproduce
these. So all seven are the entire difference and every one is an audit that followed v6. v6's own §What the seventh audit closed
is byte-identical. **v6's account of the artifact-locked worker environment and the `e1-s1-v3`
bundle identity stands.**

Worth naming because the reconstruction was needed at all: a record may pin bytes no commit
carries, and then only a reconstruction can distinguish a routine revision from a rewritten
control. Pinning working-tree bytes is what makes that possible, and committing an audit's
inputs alongside its record is what makes it unnecessary.

### `e1-s1-provisional-contract-baseline-v7.md`

Also pins `crates/study-tts-core/src/lesson.rs` at
`88b781ad94239230c44b634058108435a0ddd3002b8827d8ba42f4793408d29d`; it now hashes
`4d44eaa34a517d965a20e2bb88e33d28ba7ad44d0574807d8c867e72874e2ece`. Two things moved it. One is
an added doc paragraph on the `language` field — that the published `pattern` is necessary but
not sufficient, that `LanguageTag` is the authority, and that a tag the schema admits may still
be refused. The other is §What the tenth audit closed repairing the `LESSON_SCHEMA_VERSION` doc
comment, which linked `AuthoredLesson::schema` after v7's own audit made that field private, so
`cargo doc` warned and the rendered link went nowhere; it names `AuthoredLesson::new` instead.
Reverting the link repair alone reproduces
`4ad9dfa9f3b2c4981fa76dab311a89a23bdb3edfbcd02ab17aa3a2c04af182c0`, and deleting the paragraph
from that reproduces the pinned digest exactly, so the two are the entire difference.

Both are comments. No item, signature, visibility, serialization, or schema output moves, and the
`AuthoredLesson::new` boundary and the private schema-metadata fields that v7 records are
untouched by either. **v7's conclusion stands** — the link repair is that record's own dangling
reference being cleaned up, not a change to what it concluded.

Also pins `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` at
`aa6b825796245d025a1fbbfa5ab3a7665639e68685845adf8cee69b5042a08d5`; it now hashes
`38027f7226cd0d6d38fef071296aeaeab832e544acc3e105b686b4129888a7d4`. The tenth audit added three
things: §What the tenth audit closed with its §What this moves, the narrowing of the §Version and
compatibility statement that no worker frame changed, and the pointer superseding v7 by v8.
Removing those three reproduces the pinned digest exactly, so they are the entire difference.
This was rerun after each revision of that section rather than asserted once.

This row is unlike every other one here. **v7's conclusion on the worker-bundle hash does not
stand, and was already false when v7 was written.** Its §Identity and compatibility impact says
the v6 hash "remains valid" because the bundle does not contain `study-tts-core`; the premise is
true and the conclusion is not, because `worker/study_tts_worker/protocol.py` and
`schemas/worker-protocol-v0.schema.json` are declared bundle inputs and the tenth audit had
already moved both. The row exists so the check can still read v7 until an accepted record
retires it, not because the conclusion survived. What v7 concluded about its own change stands:
schema metadata and CLI status text reach no bundle input.
[`e1-s1-provisional-contract-baseline-v8.md`](e1-s1-provisional-contract-baseline-v8.md) carries
the identity that holds now, and is the record to read for it.

## Documents this branch revised

| Document | Pinned digest | Current digest |
|---|---|---|
| `docs/adr/ADR-0002-model-hardware-voice-format-qualification.md` | `397dd2efa3094aca8c8f0aca11f67e44f4014ed0b0d018684fe06c24978c9b53` | `d52845a4a0b6029fd477f98b22a4e881311af7c4490694b75c4bd34dae93a5e7` |
| `docs/operations/REFERENCE-ENVIRONMENT.md` | `a673a4b1570df39d2458493a6ec3b033b0545ebe9aa9adcaca4ad51021cdfd50` | `478aab95750fb968b064f6aca8d1a2ecc22363a195b693a307691c0a43c0ca19` |
| `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | `57c477e8fe7d3b4058bf8a79b8bf5ba56c622e4ed2af2c528ed63e89d529f398` | `277a2acbe66595e6482d4a4510750f754af32ee45412a7f35297096ecd3392cc` |
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `b6a61564cd6642b0db8c0bd0435bff047b89082462c8ad76e6922abad51e7bd6` | `7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730` |
| `docs/architecture/WALKING-SKELETON.md` | `229c177a8a815c4130f9973677a1b274d3e6dd63350ffe36df8fbb344012e232` | `79bda366c253bec9ea3918920e9444cb50e30077076fac3948008cfcda268eac` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `6e68540ad601cab17457eb75781b299e1d80e44dabd41d31bbc968d91adc0e41` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `9daded5d8420b8a2a852e3a2fa64abb1251cfcdeb5a4e59395a2f41e520ece11` | `51c5ff77ba57747bb106a0a1733ee5814cef4de13b7d5e051bd317a16bd11525` |

## Affected accepted records

Three accepted records pin one or more of the digests above. Each states what the revision
changed and why the record's conclusion is reached against unchanged substance.

`e0-s4-provisional-contract-baseline-v1.md` pins two of them and is deliberately absent: its own
successor records that "the v1 record and its checksums remain unchanged as historical
provenance", which is a supersession of exactly the kind this reconciliation says needs no
further action.

### `e0-s3-g0-qualification-decision-v3.md`

Current SHA-256 `6cf52a7b11338475ba8b13f0dc16862567658b1ea10436ef0ab19b5cedf11d91`. Pins
`docs/adr/ADR-0002-…` and `docs/operations/REFERENCE-ENVIRONMENT.md`.

Both revisions are in commit `407dfc2`, and both are pointer corrections made *because* v3
exists. ADR-0002 previously named `e0-s3-g0-qualification-decision-v2` as the superseding
progression decision and the original fixed-seed report as the fixed-seed evidence; it now names
v3 and `evidence_e0_fixed_seed_synthesis_determinism_is_characterized_v2`. `REFERENCE-ENVIRONMENT.md`
received the same two pointer corrections, plus a status sentence recording that E0-S4 has since
supplied the provisional contract baseline that v3 recorded as still outstanding.

Neither revision touches what v3 decided on. The measured `RTF` of `14.9804`, the cold projection
of `53,947.516` seconds, the waiver's development-only scope, its expiry, its compensating
controls, and the full-box obligation before G3 are all byte-identical across the revision; the
`REFERENCE-ENVIRONMENT.md` change says so in its own text. **The conditional pass stands.**

Worth naming, because it is the reason this record exists: `407dfc2` was itself a provenance
correction, and it invalidated the provenance of the record it was correcting toward.

### `e0-s3-audit-remediation-v2.md`

Current SHA-256 `8d7ebbfe9a6551a8e98c92d749b5d6cebc625de64232cca5f15d33604c71ed12`. Pins
`docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` in its controlled-record table.

The revision moves OQ-01's decision deadline from before G0 to before G3, and moves OQ-07 from
`Proposed: no` to `Resolved: no` on the authority of accepted deviation `ADR-0001-D001`. This
record's audit conclusions concern the WAV-variation analysis scripts, their tests, and the
chatterbox-weights and owner-fallback rights records. It cites the register as a controlled
document, not as support for a finding, and neither OQ-01 nor OQ-07 appears in its findings.
**The remediation conclusions stand.**

### `e0-s4-provisional-contract-baseline-v2.md`

Current SHA-256 `473f2713b00861cf6be98117118c377b99f26d765aa658e51df46f6e86bedf51`. Pins
`docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`, `docs/architecture/WALKING-SKELETON.md`,
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, and `docs/testing/TEST-DATA-MANIFEST.md`.

Both E0-S4 records are treated together, because one revision explains both. E1-S1 extended all
four documents: the change-control document gained the E1-S1 record's change classes, the two
architecture documents gained the identity, manifest-layout, and worker-bundle mirrors E1-S1
introduced, and the test-data manifest gained the E1-S1 fixture rows.

These revisions are **additive to** the E0-S4 records rather than corrective of them, and where
E1-S1 does supersede an E0-S4 interface it says so under its own authority: the `tts_executor`
move from `e0.tts-executor.0.1` to `e1.tts-executor.1.0` is recorded as a breaking change in
`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`, which
`e1-s1-provisional-contract-baseline-v5.md`, SHA-256
`eff542fda465a9e7074df440c23faf9ad3066c827fc05b266b35fcc897802d6e`, records and its still
unapproved successors carry forward. No digest is pinned for the change record here: it moves
with every audit, and §Draft records pinned to bytes this branch has since moved is where its
movement is accounted. **The E0-S4 records remain
the accurate account of the provisional contracts as E0-S4 left them**, which is all either
claims to be; E1-S1's own chain carries the current state.

## Drift predating this branch

Found while reconciling the above, listed because a check that fails on it would otherwise be
silently weakened to pass, and because the alternative is that it stays undiscoverable. Every
entry below pins a digest older than this branch's merge base `53e0333`, so none of it was caused
by E1-S1 and none of it is E1-S1's to close.

| Accepted record | Document | Pinned digest | Current digest |
|---|---|---|---|
| `evidence_e0_descope_ladder_is_ratified.md` | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | `38527e02b8ef…` | `277a2acbe665…` |
| `evidence_e0_open_questions_have_gate_aligned_deadlines_and_owners.md` | `docs/governance/RISK-OPEN-QUESTIONS-DESCOPE.md` | `38527e02b8ef…` | `277a2acbe665…` |
| `evidence_e0_milestone_matrix_has_one_owner_and_gate_per_requirement.md` | `docs/governance/MILESTONE-CAPABILITY-MATRIX.md` | `02be58df06fd…` | `bc7bba939488…` |
| `evidence_e0_model_and_voice_rights_records_complete_v3.md` | `docs/adr/ADR-0002-…-qualification.md` | `86ac449598a6…` | `d52845a4a0b6…` |
| `evidence_e0_model_and_voice_rights_records_complete_v3.md` | `docs/adr/ADR-0004-voice-content-and-retention-policy.md` | `f87f60fe9938…` | `da44e625de4e…` |
| `evidence_e0_model_and_voice_rights_records_complete_v3.md` | `evidence/rights/rights-chatterbox-code-v2/record.md` | `e2850972d83c…` | `4cd78b5f2790…` |
| `evidence_e0_model_and_voice_rights_records_complete_v3.md` | `evidence/rights/rights-chatterbox-weights-v2/record.md` | `ea0a1424cb5e…` | `538c3fcc3716…` |
| `evidence_e0_model_and_voice_rights_records_complete_v3.md` | `evidence/rights/rights-voice-owner-fallback-v2/record.md` | `9ed07599bae9…` | `75868ec5e044…` |
| `evidence_e0_source_provenance_use_and_distribution_classification_complete_v3.md` | `docs/governance/RELEASE-PROFILES.md` | `6d52a9be71c3…` | `476201ec14e1…` |
| `evidence_e0_source_provenance_use_and_distribution_classification_complete_v3.md` | `docs/testing/TEST-DATA-MANIFEST.md` | `2a15c2130dee…` | `a0f79979f713…` |
| `e0-s3-g0-qualification-decision-v3.md` | `e0-s3-audit-remediation-v2.md` | `bcda43efddca…` | `8d7ebbfe9a65…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/chatterbox_spike.py` | `e90124eecc94…` | `6f4c432f53cf…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/tests/test_chatterbox_spike.py` | `37fe190bc7e8…` | `cad0a451115c…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/analyze_wav_variation.py` | `3ddff01a9012…` | `454be8ec3aec…` |
| `e0-s3-audit-remediation-v2.md` | `scripts/qualification/tests/test_analyze_wav_variation.py` | `fe833de1174b…` | `4c7460523b5a…` |

Each of the four qualification-script rows was verified to predate the merge base rather than
assumed to: `git show 53e0333:<path>` and the working tree hash alike, so this branch did not
touch them. They are the sharpest of the pre-branch rows, because a qualification script whose
bytes moved after an audit pinned them is the case the pinning exists to catch — the audit's
conclusions were reached against code that is no longer what runs.

The rights-record rows deserve comparable attention: `evidence_e0_model_and_voice_rights_records_complete_v3`
is the record that establishes lawful model and voice use, and it pins three rights records that
have since moved. Those are the rows where the pinned substance could bear on the conclusion.

## Open findings

| Finding | Severity | Owner | Required action | Deadline |
|---|---|---|---|---|
| The E1-S1 baseline chain `-v5`, `-v6`, and `-v7` is unapproved, so five of its rows are accounted for rather than retired | Not blocking for E1-S1 | Engineering owner | Approve the chain, or the record that supersedes it. An accepted successor removes `-v5` and `-v6` from checking by metadata, and the five rows should then be dropped | Before G1 gate review |
| Pre-branch drift above is unreconciled, and the rights-record rows are unreviewed | Blocking for G1 acceptance; not blocking for E1-S1 | Engineering owner | Reconcile each row: confirm the revision left the record's conclusion intact, or supersede the record. Then drop its row from §Accounted provenance mismatches | Before G1 gate review |

## Verification

Ubuntu 24.04 under WSL2 on 2026-08-28:

- `python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'` — pass,
  11 tests covering approval, explicit supersession, exact reconciliation coverage, and missing
  repository citations.
- `python3 scripts/check-evidence-provenance.py` — pass, exit `0`, against the working tree this
  record describes. The same command refused with 6 unaccounted mismatches before the five E1-S1
  chain rows and this record's own stale manifest snapshot were corrected.
- Each of the 28 rows in §Accounted provenance mismatches was removed in turn and the check
  re-run: every removal produced exactly one refusal, and it named that row. So no row is
  redundant, and none suppresses more than the pair it names.
- The two reconstructions are reproduced, not asserted. Deleting §What the eighth audit closed,
  §What the ninth audit closed, the CLI sentence, and the v6-to-v7 pointer from
  `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` hashes to v6's pinned
  `20ef0fcba49c…`; deleting the added `language` doc paragraph from
  `crates/study-tts-core/src/lesson.rs` hashes to v7's pinned `88b781ad9423…`. Both were checked
  against a copy; neither file was modified.
- Every digest in this record was produced by `sha256sum` or `hashlib.sha256` against the bytes
  on the date above, and each pinned-digest column was read out of the citing record rather than
  recomputed.

## Decision

- [x] **Provenance reconciled for E1-S1; pre-branch drift routed to the open finding above**
- [ ] Supersede the affected records

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accepted | 2026-08-28 |
| Project owner | Ross Todd | Accepted | 2026-08-28 |
