# E1-S3 Interface Change 003 — A timeout reports the tree it could not contain

## Identification

- Record ID: `E1-S3-INTERFACE-CHANGE-003`
- Status: **Accepted, 2026-09-01.** §Approval records the decision each role made and the date it
  was signed.
- Contract owner: T-WORKER (`tts_executor`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-WORKER
- Accepted ADR, if architectural: not applicable. This implements ADR-0001 §10.3 as written. No
  authority boundary moves.

[`E1-S3-INTERFACE-CHANGE-001.md`](E1-S3-INTERFACE-CHANGE-001.md) and
[`E1-S3-INTERFACE-CHANGE-002.md`](E1-S3-INTERFACE-CHANGE-002.md) are **Accepted** and are not
edited. This record stands beside them, in the numbering E1-S2 established.

The sixth audit of this story found that `WorkerClient::request` discarded the result of the
cleanup it runs on a deadline:

> `crates/study-tts-runtime/src/worker_client.rs:320` is `let _ = self.shutdown();`. `shutdown()`
> has already taken the child and the ownership state, so if termination, reaping, or containment
> inspection fails, that failure is unrecoverable *and* unreportable — the caller sees a bare
> `BackendError::Timeout`.

ADR-0001 §10.3 makes the parent responsible for terminating the full child process tree on a
deadline. A tree that survived that termination holds a model resident and the staging directory
open for the life of the build, and it was the one outcome the refusal could not name.

The remediation added a field. **It did so without this record**, which is what the seventh audit
found and what this record exists to correct; §Version and compatibility states the consequence
plainly rather than presenting the change as though it had been classified at the time.

## Version and compatibility

### `TtsExecutor` — `e1.tts-executor.3.0` retained under `ADR-0001-D005`

- Contract ID: `TTS_EXECUTOR_CONTRACT_VERSION`
- Old version: `e1.tts-executor.3.0`
- New version: `e1.tts-executor.3.0` — retained
- Compatibility class: **Breaking contract**
- Required/defaulted fields: one required field added; no default
- Unknown-field behavior: unchanged, and not applicable — `BackendError` is never deserialized
- Wire or Rust representation changed: Rust only

`BackendError::Timeout` gains a required `containment_failure: Option<String>`:

```rust
Timeout {
    request_id: String,
    timeout_ms: u64,
    containment_failure: Option<String>,
},
```

`BackendError` is named in this document's §Baseline inventory row for `tts_executor` as part of
that contract's public representation, so a required field is a **Breaking contract** under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes — the same classification `E1-S3-INTERFACE-CHANGE-002` applied to its own
`edge_conditioning` field.

**The field is `Option`, and that is not what makes it compatible.** `None` is a fact about the
worker — the tree *was* proven gone — not an absent value standing in for one. A caller
constructing or destructuring the variant must still account for it, which is what a required
field means here.

### Why the version is retained

`ADR-0001-D005` permits a pre-G1 breaking correction to retain its unreleased version under five
conditions. They are argued individually, because D005 is a condition list to meet and not a
precedent to cite:

| Condition | Holds? |
|---|---|
| 1. The seam is still provisional under §G1 freeze | **Yes.** G1 is not reached; §Baseline inventory records the executor as frozen "only after the capacity-one Chatterbox adapter passes the suite" |
| 2. The retained version was introduced by an unreleased breaking move **within the same story** | **Yes.** `E1-S3-INTERFACE-CHANGE-001`, Accepted 2026-08-30, moved `e1.tts-executor.2.0` → `3.0` inside E1-S3 |
| 3. No durable artifact, and no evidence record outside `Proposed`, was written under the shape being corrected | **Yes.** `BackendError` is never serialized; no fixture under `fixtures/` names the contract version; the only evidence record describing this shape, `e1-s3-single-worker-synthesis-and-validated-cache-v1`, is `Proposed` |
| 4. Supervisor, fake, worker, tests, fixtures, and generated schema move in the same commit | **Yes.** No schema, worker, or fixture is reached at all; the supervisor, the fake's behaviors, and the shared suite move together |
| 5. The correction and its reasoning are recorded in the story's interface-change record **before the code lands** | **No — and this record is the remedy.** The field landed on 2026-09-01 with no record. Nothing was released and no consumer exists, so the condition can still be satisfied in substance before acceptance, but it was not satisfied in sequence |

Condition 2 is the one that decides this. `E1-S3-INTERFACE-CHANGE-002` applied the same test to
`CACHE_SCHEMA_VERSION` and **failed** it, because that constant moved to `1.0` in E1-S1 rather than
in this story, and took the major increment instead. Here the version being retained is this
story's own, so no consumer ever saw `BackendError::Timeout` without the field.

Condition 5 is recorded as unmet rather than argued away. The remedy is this record; the lesson it
carries is that a change to a type named in §Baseline inventory needs the classification step even
when the diff is one field.

### What is not a contract change

`ShutdownFailure`, introduced in `crates/study-tts-runtime/src/worker_client.rs` in the same work,
is `pub(crate)`. It separates ADR-0001 §10.3's subject — the process tree — from §17.7's subject —
the bytes the worker left on standard output — so the timeout path reports only the first. It
appears in no public signature, and `WorkerTtsExecutor::shutdown` still returns
`Result<(), BackendError>` with the message it returned before.

## Impact

- **Synthesis identities affected:** none. §Baseline inventory records `contract_version` as one of
  the two `BackendDescriptor` fields that does **not** reach a synthesis key, so no cache key and no
  plan hash moves. Nothing in `worker/` is touched, so the worker bundle identity
  `3e1f487cf259cd5b17bdeea16845c14426dbbded76f47732dd06b02198003747` stands, and with it the T5
  qualification result and the pending listening set.
- **Verification identities affected:** none.
- **Plan, takes, or package identities affected:** none.
- **Consumers and commands affected:** `WorkerClient::request` constructs the variant;
  `worker_contract.rs` destructures it. Both destructuring sites already used `..`.
- **Fakes and shared suites affected:** `fake-ndjson-worker` gains three behaviors —
  `hang-on-synthesis-leaving-bytes`, `hang-on-synthesis-escaping-containment`, and the
  `escapee-holding-stdout` the second one starts. `FakeTtsExecutor` and
  `run_tts_executor_contract_scenario` are unchanged.
- **Fixtures and schemas affected:** none. `BackendError` is not a wire format and no published
  schema covers it.
- **Existing cached artifacts affected:** none.
- **Published packages or accepted takes affected:** none.
- **Rights and privacy:** no change. The field carries a containment diagnostic — counts and
  process facts — and never a path or a governed location.

## Delivery and recovery

- **Fake and shared-suite update completed before consumers:** the fake's behaviors and the two T4
  tests land with the supervisor change, in the order §Amendment rules before G1 requires. There is
  no fixture or wire format to move first.
- **Migration procedure:** none required, and none possible to need — no durable artifact carries
  this shape.
- **Rollback procedure:** deletion. Remove the field and the `ShutdownFailure` split together; the
  two T4 tests and the T1 rendering test go with them.
- **Compatibility evidence:** `t1_e1_a_timeout_reports_a_containment_failure_beside_it` pins both
  renderings, so a contained timeout still reads exactly as it did before the field existed.
- **Mapped tests and qualification rerun:** E1-S3 executor and worker suites. No qualification rerun:
  the worker bundle is untouched, and the T5 criteria are statements about the worker's behavior at
  an identity this change does not move.
- **Walking skeleton result:** passes as part of `cargo test --workspace --all-targets`.

## Limits this change does not close

- **The escape itself is still not contained.** `ADR-0001-D008` records that a descendant which
  leaves its process group is reachable by neither the group kill nor a recorded pidfd, and this
  change does not narrow that. What it adds is that when such an escape holds the worker's standard
  output open, the refusal now says so instead of reporting a bare deadline.
- **Not every containment failure is observable.** An escapee that holds nothing is still silent,
  which is precisely what `ADR-0001-D008` §Approved deviation permits until E5-S4's
  `t4_e5_a_descendant_that_leaves_its_process_group_is_still_contained`.
- **A reader join can outlast the refusal.** `shutdown` joins its reader threads after draining the
  epilogue, so a process holding the protocol pipe delays the caller for as long as it holds it. The
  new T4 test bounds its own escapee for that reason. Bounding it in the supervisor is not attempted
  here.

## Approval

**Every row below is signed, on 2026-09-01.** Each records a decision a role was asked for and has
now made.

Ross Todd holds every role listed. [`../governance/PROJECT-EXECUTION-CHARTER.md`](../governance/PROJECT-EXECUTION-CHARTER.md)
permits that for a personal project and requires each approval to name its role and accepted risk
separately, which is why the rows stay separate for one signatory.

This acceptance covers the contract this record describes. It does **not** accept
`evidence/gates/g1/e1-s3/e1-s3-single-worker-synthesis-and-validated-cache-v1.md`, which stays
`Proposed` until G1.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept a required field on `BackendError::Timeout` as a **Breaking contract** that retains `e1.tts-executor.3.0` under `ADR-0001-D005`, and accept that condition 5 was met only after the fact | Accepted — Ross Todd, 2026-09-01 |
| Contract owner (T-WORKER) | Accept that `contract_version` reaches no synthesis key, so no identity, cache entry, or qualification result moves with this change, and accept the three limits recorded above | Accepted — Ross Todd, 2026-09-01 |

- Effective version and date: **2026-09-01.** `e1.tts-executor.3.0` retained;
  `SYNTHESIS_IDENTITY_VERSION` `e1-s2-v1`, `CACHE_SCHEMA_VERSION` `2.0`,
  `CACHE_PUBLICATION_CONTRACT_VERSION` `e0.cache-publication.2.0`, and `e1.worker.2.0` unchanged.
