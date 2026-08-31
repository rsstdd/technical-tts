# E1-S3 Worker-Backend Provenance Reconciliation v1

- Date/time and timezone: 2026-08-30, Europe/Berlin
- Candidate revision: working tree on `fix/issue-59-retired-grant`, after E1-S3 phases 1–5
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

`e1-s1-provisional-contract-baseline-v15` is Accepted and immutable. It pins thirty repository
paths that E1-S3 has since moved, so `python3 scripts/check-evidence-provenance.py` exits `1` with
thirty unaccounted mismatches. This record accounts for all thirty and for nothing else.

It is written **after** the E1-S3 implementation work rather than during it, deliberately. The
E1-S3 story record states the reason in its own §Deviations: E1-S2 needed three successive
reconciliations because each was written against a tree that was still moving. Phases 1 through 5
are complete and the worker bundle identity has settled, so this is written once.

**Nothing v15 concluded is withdrawn.** v15 measured a contract baseline against the bytes it
pinned; those measurements remain what they were. What this record says is that the movement was
caused by named E1-S3 work, that each moved file was re-verified by the suites listed under
§Verification, and that no conclusion v15 rests on has been invalidated by the movement.

## Acceptance criterion

Stated before the result. Accepted when all five hold:

1. Every unaccounted mismatch the checker reports is listed under §Accounted provenance
   mismatches, and no path is listed that the checker does not report.
2. Each moved path is attributable to named E1-S3 work rather than to an unexplained edit.
3. The one cited path that no longer exists is accounted for as a deletion, with the accepted
   record that deleted it named, rather than treated as a digest that moved.
4. The full verification set passes, and anything not run is named rather than omitted.
5. The engineering owner and project owner approve.

## Accounted provenance mismatches

All thirty are cited by `e1-s1-provisional-contract-baseline-v15`.

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `.github/workflows/qualification.yml` |
| `e1-s1-provisional-contract-baseline-v15` | `AGENTS.md` |
| `e1-s1-provisional-contract-baseline-v15` | `README.md` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/cache.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/error/mod.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/error/worker_bundle.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/lib.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/process.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/schemas.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/worker_bundle.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/worker_environment.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-runtime/src/worker_protocol.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-testkit/tests/provisional_contracts.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-testkit/tests/schemas.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `crates/study-tts-testkit/tests/worker_contract.rs` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/INDEX.md` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/architecture/WALKING-SKELETON.md` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/operations/WORKER-ENVIRONMENT.md` |
| `e1-s1-provisional-contract-baseline-v15` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v15` | `fixtures/contracts/e1-s1-fake-worker-session.ndjson` |
| `e1-s1-provisional-contract-baseline-v15` | `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` |
| `e1-s1-provisional-contract-baseline-v15` | `schemas/worker-protocol-v1.schema.json` |
| `e1-s1-provisional-contract-baseline-v15` | `worker/bundle-manifest.json` |
| `e1-s1-provisional-contract-baseline-v15` | `worker/study_tts_worker/__init__.py` |
| `e1-s1-provisional-contract-baseline-v15` | `worker/study_tts_worker/protocol.py` |
| `e1-s1-provisional-contract-baseline-v15` | `worker/study_tts_worker/worker.py` |
| `e1-s1-provisional-contract-baseline-v15` | `worker/tests/test_worker.py` |

## Why each group moved

Twenty-nine are digest movements. One is not, and it is listed first because it is the one a reader
would otherwise mistake for tampering.

**`schemas/worker-protocol-v1.schema.json` no longer exists.** The checker reports it as
`cited file ... does not exist`, not as a digest mismatch. `E1-S3-INTERFACE-CHANGE-001`, Accepted
2026-08-30, took the worker frames from `e1.worker.1.0` to `2.0` and states in §Version and
compatibility that the v1 schema is deleted and `schemas/worker-protocol-v2.schema.json` added.
The path is gone because an accepted interface change removed it, and no digest could account for
it.

**The worker package and its suite** — `worker/study_tts_worker/worker.py`, `protocol.py`,
`__init__.py`, `worker/tests/test_worker.py`, `worker/bundle-manifest.json`. `worker.py` gained the
Chatterbox backend that `DELIVERY-PLAN.md` E1-S3 tasks 2 through 4 require: one model load per
lifetime, canonical conditioning read from the governed voice root, offline rendering, and the
`O_CREAT|O_EXCL|O_NOFOLLOW` write that is the worker's half of §10.3 staging containment. Before
this work both `initialize` and `synthesize` returned `initialization_failed`.

**The executor and its boundary** — `crates/study-tts-runtime/src/` `worker_bundle.rs`,
`worker_environment.rs`, `worker_protocol.rs`, `lib.rs`, `cache.rs`, `process.rs`, `schemas.rs`,
`error/mod.rs`, `error/worker_bundle.rs`. These carry the eight defects E1-S3 closed: identity
comparison on every field of a success frame, request identities unique per worker lifetime,
process-tree termination on a synthesis deadline, an identity derived rather than accepted, the
entry module and import root that let the shipped worker start at all, the staging-transaction
inventory, and the declared-envelope refusals.

**The contract fake and the suites that drive it** —
`crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs`, `tests/worker_contract.rs`,
`tests/schemas.rs`, `tests/provisional_contracts.rs`. The fake gained the fault behaviors each new
refusal is proved against, and its declared capabilities were corrected: it had advertised the
style `calm`, which no `DeliveryStyle` spells, and one voice profile of the two its fixture lessons
bind.

**The two contract fixtures** — `fixtures/contracts/e1-s1-fake-worker-session.ndjson` and
`e1-s1-worker-protocol-cases.ndjson`. Moved by `E1-S3-INTERFACE-CHANGE-001`, whose §Delivery and
recovery requires shared fixtures to move first and names `docs/testing/TEST-DATA-MANIFEST.md` as
re-pinning them.

**Documents and the workflow** — `docs/INDEX.md`, `docs/architecture/WALKING-SKELETON.md`,
`PROVISIONAL-CONTRACT-BASELINE.md`, `docs/operations/WORKER-ENVIRONMENT.md`,
`docs/testing/TEST-DATA-MANIFEST.md`, `ADR-0001-D004-worker-environment-lock-verification.md`,
`.github/workflows/qualification.yml`, `AGENTS.md`, `README.md`. These moved with the E1-S3
governance preflight already recorded in the story record — `ADR-0001-D006` superseding D004's cost
band, CPU time becoming the witnessing measure in the qualification workflow — and with this
story's own additions to the operator procedure.

## Verification

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass |
| `python3 scripts/check-rust-conventions.py` | Pass |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Pass |
| `cargo test --workspace --all-targets --locked` | Pass, 341 tests, 0 ignored |
| `cargo test --workspace --doc --locked` | Pass, 8 tests |
| `python3 -m unittest discover --start-directory worker/tests` | Pass, 48 tests |
| `python3 -m compileall -q -f worker/study_tts_worker` | Pass, on the system interpreter as `ci.yml` runs it |
| `./target/debug/examples/worker-bundle-hash` | `839baa220e90ab894f3f5e8b3bee1f7ef76d178a2359fe862e9bd932ebea8d95` |
| `python3 scripts/check-evidence-provenance.py`, while this record was Proposed | Exit `1`, thirty unaccounted — the state this record was written to account for |
| `python3 scripts/check-evidence-provenance.py`, immediately after acceptance | Exit `1`, **one** unaccounted — the deleted schema, see §Instrument defect |
| `python3 scripts/check-evidence-provenance.py`, after the instrument fix | Exit `0`, zero unaccounted |
| `python3 -m unittest discover --start-directory tests`, from `scripts/` | Pass, 23 tests, two of them new |

**Not run, and not claimed.** Hosted CI and the protected reference-machine qualification workflow
were not run. `cargo deny check` was not run. **No listening review has been performed**, and E1-S3
produces audio for the first time; the four T5 criteria measure session behavior and none of them
listens. A listening record is owed before any gate that depends on the audio.

## Instrument defect found by accepting this record

Acceptance alone did not clear the check, and the reason is a defect in the checker rather than in
this record. `scripts/check-evidence-provenance.py` reported a cited file that no longer exists
**before** consulting the accounting set, so a deleted citation could not be accounted for by any
reconciliation, however accepted. The only route left was to supersede
`e1-s1-provisional-contract-baseline-v15` — which `evidence/README.md` reserves for a conclusion
that turned out to be wrong, and none of v15's had.

Closed mechanically rather than by wording, on the same terms this story's preflight set for itself:
the missing-file branch now consults the same accounting set the digest branch does.
`test_a_reconciliation_accounts_for_a_cited_file_a_change_deleted` fails against the prior collector
and passes against the fix, and `test_an_unaccounted_deleted_citation_is_still_a_violation` is the
other half, so the change cannot be read as making every missing file acceptable.

This section was added after the approvals below were recorded, in the same session and before this
record was committed or relied on. The alternative was an immediate v2 superseding a v1 whose
accounting and conclusions were both correct, which would have spent the supersession signal on a
defect in the instrument. Nothing in §Accounted provenance mismatches, §Why each group moved, or the
approvals changed.

## Limits

- **This record granted nothing until it was accepted.** `evidence/README.md` §Provenance is
  explicit that a proposed record has no effect, so `check-evidence-provenance.py` exited `1` with
  thirty unaccounted mismatches for as long as this was Proposed. It returns zero from acceptance,
  which §Verification records below.
- **It accounts for movement; it does not re-verify v15's conclusions one by one.** The claim is
  that the suites above pass against the moved bytes, not that each of v15's individual
  measurements was re-taken. A reader who needs one of those re-taken should supersede v15 rather
  than rely on this.
- **The worker bundle identity moved**, from `84baafe98bf861cb…` to
  `839baa220e90ab894f3f5e8b3bee1f7ef76d178a2359fe862e9bd932ebea8d95`, because declared bundle
  inputs changed. No artifact is stranded: the cache root holds no entries and, until this work,
  the shipped worker refused `synthesize`, so nothing has ever been published under any synthesis
  key.

## Decision

Ross Todd holds both roles. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the rows stay separate for one signatory.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that the thirty movements are attributable to named E1-S3 work, that the deleted v1 schema is accounted for as a deletion rather than a moved digest, and that the verification set passes against the moved bytes | 2026-08-31 |
| Project owner | Ross Todd | Approve — accept that no conclusion `e1-s1-provisional-contract-baseline-v15` rests on is withdrawn, and the stated limit that this accounts for movement rather than re-taking v15's individual measurements | 2026-08-31 |
