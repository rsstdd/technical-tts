# E1-S1 Provenance Tooling Reconciliation v1

- Date/time and timezone: 2026-08-29, Europe/Berlin
- Accountable owner: Engineering owner
- Approver: Repository user
- Status: Accepted
- Supersedes: nothing

## Scope and decision

Twenty-one audits and thirteen baseline supersessions were spent on one
infrastructure story. The measurement behind this record is that
`e1-s1-provisional-contract-baseline-v13` pins 44 repository paths, 43 of which
moved during E1-S1, and that 25 of the story's 28 commits touched at least one
of them. A record pinning the files a story is actively building is stale by
construction, so supersession — which exists to correct a conclusion an approver
relied on — became the routine cost of committing.

Four changes address that, and they move three governed paths:
`scripts/check-evidence-provenance.py`, its tests, and `evidence/README.md`.
This reconciliation accounts only for those three record/path pairs. It changes
no measured result, approval, identity, or contract.

The repository user directed all four changes on 2026-08-29, including the
governed amendment to `evidence/README.md` §Provenance and the judgment call on
where the load-bearing/context line falls.

1. **Digests are transcribed by the script, not by hand.**
   `--write` re-pins every digest cell of a proposed record from current bytes.
   The record is named on the command line and never inferred. It refuses an
   accepted record, one declaring no status, a superseded one, a path outside
   `evidence/`, and a path that does not exist; and it leaves a row citing two
   paths alone, because one digest cannot name two files' bytes. This closes the
   defect class that produced v12's table, in which six digests were copied from
   a tree that had already moved.
2. **A record declaring `Proposed` is no longer checked.** A proposal is not in
   force, so nothing rests on its pins and there is no conclusion for a moved
   document to invalidate. The exemption is read from an explicit status only;
   a record declaring nothing stays checked, preserving the fail-open property
   `evidence/README.md` argues for.
3. **Pins are separated from context references.** A digest is an obligation, so
   it is spent only where a conclusion rests. A row carrying no trailing SHA-256
   is already outside the check, so this needs no new enforcement — which is
   why the distinction must be drawn deliberately.
4. **A story keeps one record, accepted at its gate.** Supersession returns to
   meaning that a conclusion was wrong.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all five hold:

1. Every newly reported mismatch appears exactly once below.
2. The old and replacement digests are reproduced from the named files.
3. The compatibility impact on `-v13`'s conclusions is stated.
4. The behavioral change is shown to remove no record from checking today, so
   no earlier conclusion is retroactively unprotected.
5. The repository user authorizes the amendment and these exact rows.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-provisional-contract-baseline-v13` | `scripts/check-evidence-provenance.py` |
| `e1-s1-provisional-contract-baseline-v13` | `scripts/tests/test_check_evidence_provenance.py` |
| `e1-s1-provisional-contract-baseline-v13` | `evidence/README.md` |

## Reproduced movement and conclusion impact

`scripts/check-evidence-provenance.py` was pinned at
`93ed8f9f7d1871ccc310c62f79bbc5b556878b45e6b80e634b5afd071dbf2946`
and now hashes
`f52c5cd5c4d7e879e38ef4ab133b0fe8ac52117c11d4406db5d2179d946f3a1b`.

`scripts/tests/test_check_evidence_provenance.py` was pinned at
`8a4e719d44b4a0ef121a66925681ba40d4084311363ef8f8dcc57642ee970f4a`
and now hashes
`650b3dad1968049dcde03794ab68ad07fc88dc0c72b0d92bc7a3d20623dfa0c1`.

`evidence/README.md` was pinned at
`31942b848f0435bdd63711ff1925973feef46aeb6608bee18a54ce17ffebbd7f`
and now hashes
`d28b0f752e84dd73b13c89bc07a4718c5b691054a4524b130abbc62b1e6dd052`.

`-v13` cited all three for the twenty-first audit's second closed defect — that
any accepted record could excuse its own provenance mismatch — and for the
convention restricting accounting to reconciliation records. **That restriction
is untouched.** `RECONCILIATION_TOKEN`, `accounted_mismatches`, and the
`## Accounted provenance mismatches` contract are unchanged, and
`test_an_ordinary_accepted_record_cannot_suppress_a_mismatch` still passes. So
the conclusion `-v13` reached against these bytes still holds against the new
ones.

No contract, wire shape, schema, error variant, refusal message, worker protocol
version, dependency, or audio byte moved. The worker-bundle identity is
unaffected: none of the three paths is a declared input in
`worker/bundle-manifest.json`, so it remains
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`.

**The `Proposed` exemption removes nothing from checking today.** The only two
records declaring `Proposed` are `-v6` and `-v7`, both already superseded and so
already unchecked. The count of checked records is 21 before and after. The
change therefore cannot retroactively unprotect an earlier conclusion; it takes
effect only on drafts written from here.

## A defect this work found in itself

The first implementation of `--write` swept every proposed record instead of
taking one named by its author. Run against a copy of this tree it would have
rewritten `-v6` and `-v7`: two abandoned drafts that are still formally live
proposals, because the only record superseding `-v6` is `-v7`, which is itself
proposed and so cannot supersede anything. One of them carries a mismatch the
fourteenth-audit reconciliation deliberately accounts for, which the sweep would
have erased without a word.

It was caught by running the sweep against a throwaway copy before ever running
it on the repository. Two things changed: the record is now named on the command
line and never inferred, and `repin_refusal` refuses a superseded record
outright. Naming the record is what actually closes it — a tool that decides for
itself which immutable history to rewrite is the same failure as a hand-copied
digest, with more reach.

`-v6` and `-v7` are left exactly as they are. They are evidence of what they
measured, and retiring them properly is a separate decision for the repository
user, not a side effect of a tooling change.

## Verification run

Run on the branch working tree, on the ADR-0002 reference machine: WSL2,
Ubuntu 24.04, CPython 3.12.3.

| Check | Command | Result |
|---|---|---|
| Red first | The five new tests added before `repin` and the exemption existed | 1 failure, 4 errors; the failure named the proposed record still being checked |
| Repository scripts | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | 20 passed, up from 12 |
| Exemption is inert today | Enumerated `records_to_check` before and after | 21 records checked in both; the only two `Proposed` records already carried accounted or retired status |
| Sweep defect, red | First `--write` design run against a throwaway copy of the tree | Would have rewritten `-v6` and `-v7`; design changed to a named record and a superseded refusal |
| `--write` end to end | A scratch proposed record pinning `deny.toml` at zeros, re-pinned, re-run, removed | Rewrote to `949b87a59898…` matching `sha256sum`; second run reported already current; refused `-v13` as `Accepted` and `-v7` as superseded |
| Provenance | `python3 scripts/check-evidence-provenance.py` | Recorded below, after this record was written |

Hosted CI, the reference-machine qualification workflow, real-model
qualification, ASR, and listening were not run. This record changes no audio
behavior or bytes.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Repository authority | repository user | Directed the four changes and authorized these three accounting rows | 2026-08-29 |
