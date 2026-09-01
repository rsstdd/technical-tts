# ADR-0001-D006 — Worker environment lock verification cost and instrument

- **Status:** Approved
- **Date:** 2026-08-30
- **Controlling ADR and sections:** ADR-0001 §12.5 and §24
- **Supersedes:** `ADR-0001-D004-worker-environment-lock-verification`
- **Requesting story:** E1-S3
- **Owner:** Engineering owner
- **Approver:** Ross Todd, project owner
- **Expiry:** None proposed; this restates a measurement and its instrument, and waives nothing

## Proposed deviation

Carry `ADR-0001-D004` forward unchanged in substance — `WorkerBundle::verified_hash` still refuses
rather than returns a bundle identity when the attached interpreter disagrees with
`worker/requirements.lock` — and change only how its cost is measured and stated:

1. **CPU time is the banded measure**, and wall time is recorded beside it as context rather than
   as evidence.
2. **The bands are retaken** on the reference machine against a freshly built example.

Nothing about the check itself changes. No input is added to the hash, `WORKER_BUNDLE_IDENTITY_VERSION`
does not move, and no identity, cache entry, or published artifact is affected.

## The gap

D004 §Measured cost banded **wall time only**, at 3.43–3.62 s. `.github/workflows/qualification.yml`
times the step so "a run that has drifted far from it is the signal that the number needs retaking",
and `docs/operations/WORKER-ENVIRONMENT.md` made the environment comparison a precondition on
returning an identity. Together those make a timing the one instrument that says whether the
precondition ran.

**The instrument does not answer that question on this machine.** The reference environment is WSL2,
whose wall clock intermittently loses seconds mid-run. Measured here on 2026-08-30, 24 consecutive
runs of the freshly built example against the restored locked environment, fully warm:

| Measure | Value |
|---|---:|
| CPU time (`%U` + `%S`), whole `verified_hash` | **3.02 – 3.19 s** |
| Wall time, runs the clock did not corrupt | 3.25 – 3.42 s |
| Wall time, corrupted runs | 0.77 – 0.80 s |
| Corrupted runs | 3 of 24 |

Every corrupted run returned the correct identity and spent CPU time **inside** the band above. A
process cannot consume four times more CPU than elapsed on this workload — a Rust parent, one
`python -I -S` child, and two I/O-bound pipe threads — so the fast runs are the clock being wrong,
not the comparison being skipped. The page cache is not the explanation either: a fully warm run
still costs ~3.3 s of wall time and a cold one costs 14.01 s, so cache warmth moves the number the
other way.

This is the finding `e1-s1-provisional-contract-baseline-v13` recorded as unexplained — "a sixth
consecutive run returned the same identity in 0.90 s … a run an order of magnitude faster than the
authorized band is the shape a skipped comparison would also have, and this record does not claim
to have distinguished the two" — and which v14 and v15 carried forward untouched. It is the same
artifact, reproduced deliberately rather than observed once, and CPU time is what distinguishes the
two cases v13 could not.

## Impact

- **Architecture and authority boundaries:** No change. The check is unchanged and still reaches the
  outside world only through `WorkerBundle::verified_hash`.
- **Schemas and interfaces:** No change.
- **Synthesis, verification, and cache identities:** No change. No field is added or removed and the
  digest is bit-for-bit what §12.5 specifies. The checked-in bundle's identity is
  `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`, unchanged from v13.
- **Security, rights, and privacy:** Strengthened, and no control is waived. A timing can no longer
  be read as evidence that a fail-closed precondition ran.
- **Tests and evidence:** No test changes. `evidence/gates/g1/e1-s3/e1-s3-single-worker-synthesis-and-validated-cache-v1.md`
  §Preflight carries the 24-run table and the eight-run table that preceded it.
- **Operations:** `.github/workflows/qualification.yml` records `%U` and `%S` rather than bare
  `time`, and `docs/operations/WORKER-ENVIRONMENT.md` §This is the part of the check that costs
  something states the rule.
- **Schedule and scope:** None. E1-S3 does not wait on this decision; a refusal to approve leaves
  D004's wall band in force and returns issue #60 to open.

## Measured cost

Taken on the ADR-0002 reference environment (WSL2, Ubuntu 24.04, CPython 3.12.3) on 2026-08-30,
24 consecutive runs of `./target/debug/examples/worker-bundle-hash` — the compiled example, not
`cargo run`, whose build overhead D004 already excluded — against the restored locked environment:

| Measure | Value |
|---|---:|
| CPU time, whole `verified_hash` | 3.02 – 3.19 s |
| Wall time, uncorrupted runs | 3.25 – 3.42 s |
| Of which `RECORD` digest verification | ~1.5 s, unchanged from D004 |
| Bytes read for those digests | 1,263 MiB across 31,704 files |

The cost is paid **once per build**, not once per segment, so the check remains affordable as
written and no memoization is proposed. D004's wall band of 3.43–3.62 s is retired rather than
carried: this machine's uncorrupted wall time now sits just below it, which is the drift D004's own
workflow comment said should trigger a retake.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Keep D004's wall band and note the artifact in prose | Leaves the instrument unable to answer the one question it is asked, and leaves the next fast run to be re-investigated from scratch |
| Have the probe report files compared and bytes read, and print them | Strongest evidence, but it adds reporting code to the identity path for a question CPU time already answers with no new code |
| Use a monotonic clock inside the example | Measures the same corrupted clock source; `%U` and `%S` are accounted by the kernel per process and are not affected |
| Amend D004 in place | `docs/adr/` requires ADRs to supersede explicitly rather than silently contradict, and the band is what changed |

## Compensating control and expiry

No expiry is proposed, because nothing is waived. If this record is rejected, D004 stands with its
wall band, and the limitation must then be stated explicitly: a timing on this machine cannot
distinguish a fast run from a skipped precondition, and issue #60 returns to open rather than being
closed on the reproduction.

## Rollback

Restore `time` in `.github/workflows/qualification.yml`, revert the two paragraphs in
`docs/operations/WORKER-ENVIRONMENT.md`, and reinstate D004 as the controlling record. No artifact
is invalidated and no identity moves.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a
personal project and requires each approval to name its role and accepted risk separately, which is
why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept the retaken bands — CPU 3.02–3.19 s, wall 3.25–3.42 s — and that CPU time is the banded measure because the reference machine's wall clock corrupts roughly one run in eight | 2026-08-30 |
| Project owner | Ross Todd | Accept D006 superseding D004 on the reasoning and rollback recorded above, closing the finding v13 raised and v14 and v15 carried | 2026-08-30 |
