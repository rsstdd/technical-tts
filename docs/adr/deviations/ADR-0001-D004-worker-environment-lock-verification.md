# ADR-0001-D004 — Worker environment lock verification

- **Status:** Approved
- **Date:** 2026-08-29
- **Controlling ADR and sections:** ADR-0001 §12.5 and §24
- **Requesting story:** E1-S1
- **Owner:** Engineering owner
- **Approver:** Ross Todd, project owner
- **Expiry:** None proposed; this is an extension of a production invariant, not a waiver of one

## Proposed deviation

Permit `WorkerBundle::verified_hash` to **refuse rather than return** a bundle identity when the
interpreter attached at `worker/.venv/bin/python` disagrees with `worker/requirements.lock`. The
comparison covers the installed distribution set and versions, the PEP 610 provenance recorded
for the one governed-source distribution, the per-file SHA-256 each locked distribution records
in its own `RECORD` together with the `worker/bundle-manifest.json` declaration that authenticates
those records, and the `.pth`, `sitecustomize`, and `usercustomize` startup code that runs before
any declared input is read.

ADR-0001 §12.5 names the hash's inputs exhaustively and this deviation **adds none**. Every input
that reaches the digest is still exactly the list §12.5 gives, and
`WORKER_BUNDLE_IDENTITY_VERSION` does not move. What is added is a precondition on returning the
identity at all, which §12.5 does not describe.

## The gap

§12.5 states the guarantee this deviation exists to keep:

> Any change to executable project code, locked dependencies, protocol interpretation,
> inference-affecting launch settings, or runtime ABI invalidates synthesis without relying on a
> maintainer-controlled revision marker.

`worker/requirements.lock` reaches the identity as **bytes**. Hashing them proves what the file
*says* and nothing about the environment it describes. A `torch` upgraded in place, or a
`chatterbox-tts` the configured index satisfied at the same version, leaves every declared input
byte-identical and every cache key where it was — while the audio changes. Under §12.5 as
written, "locked dependencies" changed and synthesis was not invalidated.

A version is also a claim about which release was resolved, not about what its files hold, which
is why the comparison reaches the per-file digests rather than stopping at the version. And a
`.pth` runs at interpreter startup whether or not anything imports its distribution, so an
environment can execute code the lock never named while every pin still matches.

## Impact

- **Architecture and authority boundaries:** No change. The check lives in
  `crates/study-tts-runtime/src/worker_environment.rs`, which names this record in return, and
  reaches the outside world only through `WorkerBundle::verified_hash`.
- **Schemas and interfaces:** No change. No published schema and no wire format is affected.
- **Synthesis, verification, and cache identities:** No field is added or removed. The digest is
  bit-for-bit what §12.5 specifies; `6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`
  was the checked-in bundle's identity both before and after this check existed, and it moved to
  `f9711a21f3e046d53c7c617e9308893c9c0240badec0d3656487fe2796c6dc2a` only because
  `worker/bundle-manifest.json` — a declared input — gained the `record_digests` this check reads.
- **Security, rights, and privacy:** Strengthened, and no control is waived. The refusal never
  prints the governed source URL, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`; it names
  the commit to reinstall from instead.
- **Tests and evidence:** Nineteen T4 tests in `worker_environment` pin the refusals;
  `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` runs the probe against a real
  interpreter and a real `.dist-info` rather than a stand-in, and
  `t4_e1_interpreter_startup_code_cannot_edit_what_the_probe_reports` runs it against an
  interpreter carrying a hostile `.pth`.
- **Operations:** A restored, locked `worker/.venv` becomes a precondition for deriving a bundle
  identity. `docs/operations/WORKER-ENVIRONMENT.md` §Restoring the environment is the procedure,
  and `.github/workflows/qualification.yml` is where it runs.
- **Schedule and scope:** None. Nothing in E1-S1 or E1-S3 waits on this decision; a refusal to
  approve removes the check rather than blocking work.

## Measured cost

Taken on the ADR-0002 reference environment (WSL2, Ubuntu 24.04, CPython 3.12.3) on 2026-08-29,
five consecutive runs of `cargo run --package study-tts-runtime --example worker-bundle-hash`
against the restored locked environment:

| Measure | Value |
|---|---|
| Wall time, whole `verified_hash` | 3.43–3.62 s |
| Of which `RECORD` digest verification | 1.50 s |
| Bytes read for those digests | 1,263 MiB across 31,704 files |

The cost is paid **once per build**, not once per segment: the bundle identity is one input to
every cache key, derived once and reused. At that scale the check is affordable as written, so no
memoization is proposed. `.github/workflows/qualification.yml` times the step on every
qualification run, which is what would make a drift from these numbers visible.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Hash the lockfile bytes only, as §12.5 literally describes | Leaves the gap above open: a dependency upgraded in place keeps its cache key and changes the audio |
| Add the installed set to the hash as a new input | Would make the identity depend on tolerated extras — the reference machine's pre-commit tooling among them — and rebuild the cache on changes that cannot reach the worker |
| Skip `RECORD` digests and compare versions only | A version says which release was resolved, not what its files hold; an in-place edit is exactly the case the lock cannot see |
| Trust `RECORD` without declaring it in the manifest | `RECORD` is installed beside the files it pins, so editing a module and its `RECORD` line is one action that leaves the distribution self-consistent. The claims have to be stated from outside the environment, and the lock's artifact hashes cannot do it: nothing this build can ask the interpreter reports the artifact a distribution came from |
| Run the probe under `-I` alone | `site.main` then executes every `.pth` file and imports `sitecustomize` before the script runs, so the startup code this check exists to report gets to edit the report. `-S` suppresses both, at the cost of the script repeating the prefix half of `site.venv` |
| Ignore `.pth` and `sitecustomize` | Both execute before any declared input is read, so an environment could run code the lock never named while every pin matched |
| Defer the whole check to E1-S3, where the real worker gives it a caller | The control is correct and tested now; deferring would delete working fail-closed code and reintroduce it, and E1-S3 would inherit an identity that had never been checked |

## Compensating control and expiry

No expiry is proposed, because nothing is waived and no risk is being carried. If this record is
rejected, the compensating control is the reverse of the usual one: the check is removed, and
ADR-0001 §12.5's guarantee about "locked dependencies" reverts to what hashing the lockfile's
bytes can support — which must then be stated as a known limitation rather than left implied.

## Rollback

Delete `crates/study-tts-runtime/src/worker_environment.rs` and
`crates/study-tts-runtime/src/runtime_probe.py`, reduce `WorkerBundle::verified_hash` to
`WorkerBundle::hash` plus the runtime-ABI probe, and remove the environment sections from
`docs/operations/WORKER-ENVIRONMENT.md`. No artifact is invalidated and no identity moves:
the digest never depended on the check.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md`
permits that for a personal project and requires each approval to name its role
and accepted risk separately, which is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Accept the measured 3.4–3.6 s per build, and the operational precondition that a restored `worker/.venv` is required before a bundle identity can be derived | 2026-08-29 |
| Project owner | Ross Todd | Accept the environment-lock precondition as governed scope extending ADR-0001 §12.5, on the reasoning and rollback recorded above | 2026-08-29 |
