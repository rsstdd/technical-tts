# E1-S1 Eleventh-Audit Provenance Reconciliation v1

- Date/time and timezone: 2026-08-28, Europe/Berlin
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted
- Supersedes: nothing

## Scope and decision

The eleventh E1-S1 audit moves four governed records after the accepted
`e1-s1-evidence-provenance-reconciliation-v1` and the still-unapproved
provisional-baseline drafts v5-v7 pinned earlier bytes. This supplement names
only the seven new record/path pairs. It neither edits nor supersedes the
accepted reconciliation: once approved, both records remain active and their
accounting rows are combined by
[`../../../../scripts/check-evidence-provenance.py`](../../../../scripts/check-evidence-provenance.py).

The engineering and project owners authorized acceptance on 2026-08-28. Per
[`../../../README.md`](../../../README.md), its exact accounting rows now have
mechanical effect without changing the accepted reconciliation they supplement.

## Acceptance criterion

Accepted when all four hold:

1. Every mismatch newly reported after the eleventh audit appears exactly once
   below as the citing record and cited repository path the checker reads.
2. The pinned and replacement digests are reproduced from the named files.
3. Each movement is explained against the conclusion the citing record reached.
4. The engineering owner and project owner approve the account; no accepted
   evidence is amended in place.

## Accounted provenance mismatches

| Citing record | Cited repository path |
|---|---|
| `e1-s1-evidence-provenance-reconciliation-v1` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e1-s1-evidence-provenance-reconciliation-v1` | `docs/testing/TEST-DATA-MANIFEST.md` |
| `e1-s1-provisional-contract-baseline-v5` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |
| `e1-s1-provisional-contract-baseline-v6` | `docs/operations/WORKER-ENVIRONMENT.md` |
| `e1-s1-provisional-contract-baseline-v6` | `crates/study-tts-runtime/src/worker_bundle.rs` |
| `e1-s1-provisional-contract-baseline-v7` | `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` |

## Reproduced movement and conclusion impact

`PROVISIONAL-CONTRACT-BASELINE.md` was pinned at
`7a0f57b0fb67cf58f875ca72700312c7571bcdb25f530a1fe799fc7264aff730`
by the accepted reconciliation and drafts v5-v7. It now hashes
`28e9a50c703f44e2d5bb747626a7c9b62a43b9deb91b0bcb51f2c8d9d81a2b7b`.
The movement records the breaking `e1.worker.1.0` baseline, ASCII request-ID
rule, and complete six-method protocol. Those changes replace the old worker
frame baseline deliberately; they do not alter the earlier reports' results
for synthesis identities, artifact-bound worker locks, lesson construction, or
CLI status.

`TEST-DATA-MANIFEST.md` was pinned by the accepted reconciliation at
`51c5ff77ba57747bb106a0a1733ee5814cef4de13b7d5e051bd317a16bd11525`
and now hashes
`047711828f3165610dc47225089f3415a7d9724afb36b5574310456b5538da7d`.
The moved rows record the renamed worker-protocol major, the six-frame fake
session, and the shared Unicode request-ID case. Fixture provenance, rights,
sensitivity, and retention remain unchanged.

`WORKER-ENVIRONMENT.md` was pinned by v6 at
`2687c8f4a1fa52984e67dc572a38721a12308f1a9f28408d25fd9a7669a5036c`
and now hashes
`13099072317bc4d936c028ff61cb6d442dbfee202eabd7abe6342a0501fefbc4`.
Its protocol-schema input path moved from v0 to v1. The v6 conclusion that the
restored environment matches the artifact-bound lock is unchanged; the worker
bundle identity changes because the declared input bytes changed.

`worker_bundle.rs` was pinned by v6 at
`cf92deb45adc906e442829c494aa7af0c193eba0d7e9b979876df1cc35148c22`
and now hashes
`e5daa9592c0dbd80e40b33c831d1e454c01473940c60f03c55b042618ff23b82`.
The lock checks v6 qualified remain present. The protocol schema path and
`WORKER_BUNDLE_IDENTITY_VERSION` move so old bundle identities cannot be reused
under the breaking protocol.

## Verification

Run after recording approval:

```text
python3 scripts/check-evidence-provenance.py
```

On 2026-08-28 the command passed with no unaccounted mismatch after authorized
acceptance. Before acceptance, the same command reported exactly the seven
pairs above; the in-memory acceptance simulation also passed before this
immutable record was published.

## Review

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | engineering owner | Approved the seven exact accounting rows | 2026-08-28 |
| Project owner | project owner | Approved the compatibility-impact account | 2026-08-28 |
