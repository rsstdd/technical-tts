# E1-S3 Protocol Docstring Identity Reconciliation v1

- Date/time and timezone: 2026-08-31, Europe/Berlin
- Candidate revision: working tree on `story/e1-s3-single-worker-cache`, after the fourth audit remediation
- Accountable owner: Engineering owner
- Approvers: Engineering owner, project owner, and T-AUDIO
- Status: Accepted
- Supersedes: nothing

## Scope and decision

The fourth audit of E1-S3 found `worker/study_tts_worker/protocol.py` naming
`schemas/worker-protocol-v1.schema.json` twice — a file the Accepted
`E1-S3-INTERFACE-CHANGE-001` deleted when it moved the worker frames to `e1.worker.2.0`. Correcting
it is a two-character edit inside a module docstring. It is also, unavoidably, a change to a
**declared worker-bundle input**: `worker/bundle-manifest.json` lists `protocol.py` and `worker.py`
among the eight paths `WorkerBundle::verified_hash` hashes.

The worker bundle identity therefore moved:

| | Identity |
|---|---|
| Superseded | `d66e84e4512e2249976523f2ce6a0acaecb7fa6a6494d2aba19b2e4081de37af` |
| Current | `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` |

**This record decides one thing:** that the 2026-08-31 listening review, taken against audio
rendered at the superseded identity, is carried forward rather than retaken. The T5 qualification
result is **not** carried forward — it was retaken at the current identity, and
`e1-s3-single-worker-synthesis-and-validated-cache-v1.md` §T5 qualification result carries only
that run.

## Accounted provenance mismatches

None. No accepted record pins either file at a digest, and
`scripts/check-evidence-provenance.py` exits `0` against this tree. This record exists because a
*bundle identity* moved, which the checker does not see and no accepted record accounts for.

| Citing record | Cited repository path |
|---|---|
| — | — |

## Why the review carries and the qualification did not

The two artifacts answer different questions, and only one of them is about the bytes that moved.

**The listening review is a judgment about audio.** The complete change to the bundle this round is
ten lines, and every one of them is inside a docstring:

```
worker/study_tts_worker/protocol.py | 4 ++--
worker/study_tts_worker/worker.py   | 8 ++++++++
```

`protocol.py` gains two `v1` → `v2` substitutions inside its module docstring and one attribute
docstring. `worker.py` gains a paragraph inside `_voice_conditioning`'s docstring, recording the
two-sided coupling with `voice_gate::admit_voice_root` that the fourth audit's first finding owes.
No statement, expression, constant, or signature changed. A docstring cannot reach a sample: it is
not read by the generation path, not passed to the model, and not written into a take. The six
reviewed WAVs are therefore the audio this bundle produces, and the reviewer's five criteria bear
on it unchanged.

**The T5 qualification result is a claim about a named bundle.** Its first criterion is
`t5_e1_worker_bundle_hash_matches_when_all_declared_bundle_inputs_match`, which is a statement
*about the identity itself*; carrying it forward would be citing a run that measured a bundle this
tree no longer holds. It was rerun, in a loopback-only network namespace, and the result filed.

**A retake was possible and was declined on cost, not on principle.** The worker reports
`deterministic_seed: false`, so re-rendering the six samples would produce different bytes, which
could not be compared with the reviewed set — the review would have to be taken again by a person,
on six samples, to conclude what this record concludes by reading a ten-line diff. The decision to
argue rather than re-listen is recorded here, signed, and open to challenge, which is the whole
reason it is written down instead of assumed.

## What this does not change

- **No control is weakened.** Nothing about consent, containment, checksums, offline enforcement,
  or recovery is relaxed by this record; it grants no suppression and withdraws no conclusion.
- **The sheet still binds to its audio.** `scripts/qualification/check_listening_review.py` re-hashes
  all six WAVs against the digests in `review-sheet.json` before it will reveal the randomization
  key, and it did. The review is bound to bytes, not to a bundle string, which is what makes this
  argument checkable at all.
- **The sheet records the superseded identity, and is not edited.** `review-sheet.json` names
  `d66e84e4…` because that is the bundle that rendered the audio the reviewer heard. Rewriting it to
  the current identity would make a completed sheet say a person listened to something they did
  not.

## Verification

| Check | Command | Result |
|---|---|---|
| Bundle diff is docstring-only | `git diff worker/` | 10 lines, all inside docstrings; read in full, not sampled |
| Worker bundle identity | `cargo run -p study-tts-runtime --example worker-bundle-hash` | `58f1a098b7f36ded6dd2c84a6dfdaf72e30d4f76fe217fa262ce3bb9162db750` |
| Worker suite, system interpreter | `python3 -m unittest discover --start-directory worker/tests` | 64 passed, 2 skipped |
| Worker suite, restored environment | `worker/.venv/bin/python -m unittest discover --start-directory worker/tests` | 64 passed, 0 skipped |
| Listening review binds to its audio | `python3 scripts/qualification/check_listening_review.py …/listening` | Exit `0`, six digests match |
| Evidence provenance | `python3 scripts/check-evidence-provenance.py` | Exit `0` |

**Not run, and not claimed.** No audio was re-rendered, and no listening review was retaken; that
is the decision this record makes rather than an omission. Hosted CI and the protected
reference-machine qualification workflow were not run.

## Approvals

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the rows stay separate for one signatory.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept that correcting a stale schema reference in a declared bundle input moved the worker bundle identity to `58f1a098…`, and that the T5 qualification result was retaken rather than carried | Accepted, 2026-08-31 |
| Project owner | Ross Todd | Accept that a listening review is carried across a bundle-identity move on a read diff rather than on a re-render, and that this is a precedent limited to a change with no executable content | Accepted, 2026-08-31 |
| Contract owner (T-AUDIO) | Ross Todd for T-AUDIO | Accept the 2026-08-31 listening review, taken at `d66e84e4…` on laptop built-in speakers, as the audio evidence for the candidate at `58f1a098…`, with the limitations its own §What this review does not cover states | Accepted, 2026-08-31 |
