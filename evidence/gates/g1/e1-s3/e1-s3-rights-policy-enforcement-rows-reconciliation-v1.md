# E1-S3 Rights Policy Enforcement Rows Provenance Reconciliation v1

- Date/time and timezone: 2026-08-31, Europe/Berlin
- Candidate revision: working tree on `story/e1-s3-single-worker-cache`, worker bundle identity
  `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750`, after the fifth audit
  remediation
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

The fifth audit of E1-S3 found that the pre-load voice gate was bypassable through a profile
directory name that is not UTF-8, and that `WorkerConfiguration::for_protocol_fake` could be
pointed at the real worker over a governed root. Closing both mechanized three blocking rules that
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement did not name a test for — one of
them owed since the fourth remediation landed `admit_voice_root` without a row.

`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement therefore gained three rows. Two
accepted G0 records pin that file's digest.

This record accounts for that one path and for nothing else. It supersedes no record, withdraws no
conclusion, and grants no permission beyond suppressing the two mismatches named below.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `evidence_e0_model_and_voice_rights_records_complete_v3` | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` |
| `evidence_e0_source_provenance_use_and_distribution_classification_complete_v3` | `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` |

The digest moved from `f7a3fa1635242f0650e088293b0e6a7f490043cf359b0b7912356329453fa7dc` to
`e590ed3818b6822e520b8a0f9fd89f940b710203ef41db50217071432401456b`.

## Why it moved

Three rows were added to the §Enforcement table, and nothing else in the document changed.

| Rule now named | Enforced by | Why it was owed |
|---|---|---|
| Every profile beneath a governed voice root is gated before a worker may deserialize any of them | `t1_e1_a_revoked_profile_the_request_never_names_refuses_the_root` | The fourth remediation added `voice_gate::admit_voice_root` because the worker `torch.load`s every `conditionals.pt` in the root during `initialize`. That is a blocking rule with an executable protocol, so §Enforcement owed it a row and did not get one |
| A profile directory whose name is not UTF-8 refuses the whole governed root | `t1_e1_a_profile_name_that_is_not_utf8_refuses_the_root` | The fifth audit's first finding. The gate *skipped* such an entry on the recorded ground that no `profile_id` could equal an unspellable name; the worker reads the same name through Python's `surrogateescape` and a JSON record can carry the identical lone surrogate, so the two compare equal and the profile was loaded ungated |
| The protocol fake is never told where a governed root is | `t1_e1_the_protocol_fake_cannot_be_handed_a_governed_root` | The fifth audit's second finding. `for_protocol_fake` took a caller-chosen program and environment, so the gates could be skipped rather than merely deferred |

Every row names a test that exists and fails against the behavior it forbids. The first two were
red before they were green; the third was red against a constructor that returned `Ok` with no
check.

## What this does not change

- **No rule was weakened, narrowed, or removed.** The table only grew. `CLAUDE.md`
  §Non-negotiables forbids weakening a rights, consent, or checksum control, and the diff to this
  document adds three enforcement rows and touches nothing else.
- **No classification, consent record, or rights record moved.** Both citing records are about
  which artifacts have rights records and how sources are classified; neither conclusion depends
  on which tests §Enforcement names.
- **The two G0 records are not edited.** They stay accepted at the digests they were accepted
  against, as `evidence/README.md` requires.
- **The governed roots are unnamed here**, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
  §Storage and access.

## Verification

| Command | Result |
|---|---|
| `sha256sum docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` | `e590ed3818b6822e520b8a0f9fd89f940b710203ef41db50217071432401456b` |
| `python3 scripts/check-evidence-provenance.py`, before this record | Exit `1`, two unaccounted — the state this record is written to account for |
| `python3 scripts/check-evidence-provenance.py`, from acceptance | Exit `0`, no unaccounted mismatches |
| `cargo test --offline --workspace --all-targets --locked` | 390 passed, 0 failed |

The remaining gate results are recorded in the E1-S3 story record's fifth-remediation verification
section, taken against this change.

## Approvals

Signed. `scripts/check-evidence-provenance.py` counts a reconciliation record only when its status
reads `Accepted`, which is why the mismatches above stood open while this record was being written.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accepted — that three blocking rights rules now name the tests that enforce them, that one of the three was owed from the previous remediation rather than new, and that the policy document moved only by addition | 2026-08-31 |
| Project owner | Ross Todd | Accepted — that two accepted G0 rights records now cite a superseded digest of the rights policy, that neither record's conclusion depends on the rows added, and that neither record is edited | 2026-08-31 |
