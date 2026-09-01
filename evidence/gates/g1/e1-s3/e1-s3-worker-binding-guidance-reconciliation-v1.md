# E1-S3 Worker Binding Guidance Provenance Reconciliation v1

- Date/time and timezone: 2026-09-01, Europe/Berlin
- Candidate revision: working tree on `story/e1-s3-single-worker-cache`, worker bundle identity
  `3e1f487cf259cd5b17bdeea16845c14426dbbded76f47732dd06b02198003747`, after the sixth audit
  remediation
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

The sixth audit of E1-S3 found `worker/AGENTS.md` still instructing that **no model is loaded in
this build** and that `initialize` and `synthesize` both refuse with `initialization_failed`. That
was true of the refusal stub E1-S1 shipped and has been false since the Chatterbox backend landed.

It matters more than an ordinary stale sentence because this is a **nested `AGENTS.md`**: it is
binding guidance for future work under `worker/`, so an instruction describing a build that no
longer exists propagates into reviews and can license a regression toward the stub.

Correcting it moves the file's digest, which the accepted
`e1-s1-provisional-contract-baseline-v15` pins.

This record accounts for that one path and for nothing else. It supersedes no record, withdraws no
conclusion, and grants no permission beyond suppressing the mismatch named below.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v15` | `worker/AGENTS.md` |

The digest moved from `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` to
`e4b52c001218ead8bda4eacb292d12fcb441502c731f09ee19b629d505d285c3`.

## Why it moved

One bullet changed, from a description of the refusal stub to a description of the shipped worker:
the model is loaded once per lifetime by `_load_backend`, `synthesize` renders through it into the
assigned staging root, and every success reports the model, tokenizer/codec, worker, and
voice-profile identities.

**The prohibition inside that bullet is kept verbatim and deliberately.** "Do not add a placeholder
identity or tone" is the reason the bullet exists — the cache would publish a placeholder under a
key claiming a real model produced it — and it is as binding now as it was against the stub. The
correction narrows the bullet to a false factual claim and leaves the rule standing.

## What this does not change

- **No control is weakened.** `CLAUDE.md` §Non-negotiables forbids weakening a validation,
  containment, rights, checksum, consent, offline, or recovery control; this edit removes a
  statement of fact that had become untrue and retains every instruction in the bullet.
- **`e1-s1-provisional-contract-baseline-v15` is not edited.** It stays accepted at the digest it
  was accepted against, as `evidence/README.md` requires. Its subject is the E1-S1 provisional
  contract baseline, which does not depend on which build the worker guidance describes.
- **The worker bundle identity is unaffected by this file.** `worker/AGENTS.md` is not among the
  eight declared inputs in `worker/bundle-manifest.json`. The identity did move this round, but for
  `worker/study_tts_worker/worker.py`, and that move is recorded in the story record rather than
  here — nothing was carried forward across it, so it needs no reconciliation.

## Verification

| Command | Result |
|---|---|
| `sha256sum worker/AGENTS.md` | `e4b52c001218ead8bda4eacb292d12fcb441502c731f09ee19b629d505d285c3` |
| `python3 scripts/check-evidence-provenance.py`, before this record | Exit `1`, one unaccounted — the state this record is written to account for |
| `python3 scripts/check-evidence-provenance.py`, from acceptance | Exit `0`, no unaccounted mismatches |

The remaining gate results are recorded in the E1-S3 story record's sixth-remediation verification
section, taken against this change.

## Approvals

Signed. `scripts/check-evidence-provenance.py` counts a reconciliation record only when its status
reads `Accepted`, which is why the mismatch above stood open while this record was being written.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accepted — that nested binding guidance under `worker/` now describes the shipped backend rather than the removed refusal stub, and that the placeholder prohibition inside the corrected bullet is retained unchanged | 2026-09-01 |
| Project owner | Ross Todd | Accepted — that an accepted E1-S1 baseline record now cites a superseded digest of `worker/AGENTS.md`, that its own conclusions do not depend on that file's contents, and that it is not edited | 2026-09-01 |
