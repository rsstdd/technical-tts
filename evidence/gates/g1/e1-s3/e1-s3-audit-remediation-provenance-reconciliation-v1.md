# E1-S3 Audit Remediation Provenance Reconciliation v1

- Date/time and timezone: 2026-08-31, Europe/Berlin
- Candidate revision: working tree on `fix/issue-59-retired-grant`, after the E1-S3 audit
  remediation
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

`e1-s3-worker-backend-provenance-reconciliation-v1` is Accepted and accounts for the thirty paths
E1-S3 had moved when it was written. The audit remediation that followed moved one further path
that `e1-s1-provisional-contract-baseline-v15` also pins, so before this record was accepted
`python3 scripts/check-evidence-provenance.py` exited `1` with exactly one unaccounted mismatch.

This record accounts for that one and for nothing else. It does **not** supersede the v1
reconciliation, which continues to account for its own thirty: the earlier grant is still correct,
and replacing it would re-open thirty settled questions to answer one new one.

## Accounted provenance mismatches

The single mismatch is cited by `e1-s1-provisional-contract-baseline-v15`.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `worker/tests/test_protocol.py` |

## Why it moved

`worker/study_tts_worker/protocol.py` gained a required `staging_root` field on
`initialize`, so `worker/tests/test_protocol.py` moved with it: every helper in that suite that
builds a well-formed `initialize` frame now carries the field, and the refusal cases carry it too
so that each one is still refused for the fault it names rather than for a missing field that is
now checked first.

The field itself is the fix for the fifth audit finding. Until it existed the worker was told one
output path and no root, so it could inspect only the spelling of the path it was handed;
`t5_e1_worker_output_cannot_escape_staging_root` therefore asserted a property neither end could
prove, which the qualification instrument and the E1-S3 story record both recorded as a
limitation. The worker now decides containment against the resolved parent of every assigned path,
and both of those records drop the limitation.

`docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` §Worker frames carries the change. It is folded
into the unreleased `e1.worker.2.0` rather than moved to a third major, on the same reasoning
`ADR-0001-D005` approved for E1-S1: `2.0` has never existed outside this working tree, no durable
artifact and no evidence record outside `Proposed` was written under the shape being corrected,
and both protocol ends, the fake, the tests, the fixtures and the generated schema move together.

## Verification

| Command | Result |
|---|---|
| `python3 scripts/check-evidence-provenance.py`, while this record was Proposed | Exit `1`, one unaccounted — the state this record was written to account for |
| `python3 scripts/check-evidence-provenance.py`, from acceptance | Exit `0` |
| `python3 -m unittest discover --start-directory worker/tests` | 60 passed |
| `cargo test --offline --workspace --all-targets --locked` | 359 passed |
| `cargo test --offline --workspace --doc --locked` | 8 passed |
| `cargo fmt --all -- --check` | Clean |
| `cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings` | Clean |

A Proposed record grants nothing, which is why the first row records exit `1`. The second row is
the state from acceptance.

## Approvals

Ross Todd holds both roles. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the rows stay separate for one signatory.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that `worker/tests/test_protocol.py` moved because `initialize` gained a required `staging_root`, that the change is folded into the unreleased `e1.worker.2.0` under `ADR-0001-D005`'s reasoning, and that the verification set passes against the moved bytes | 2026-08-31 |
| Project owner | Ross Todd | Approve — accept that no conclusion `e1-s1-provisional-contract-baseline-v15` rests on is withdrawn, and the stated limit that this accounts for one movement rather than re-taking v15's measurements | 2026-08-31 |
