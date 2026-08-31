# ADR-0001-D008 — Worker shutdown contains the process group and what it enumerated, not the full tree

- **Status:** Approved
- **Date:** 2026-08-31
- **Controlling ADR and sections:** ADR-0001 §10.3, "parent owns process lifetime and terminates
  the full child process tree"
- **Requesting story:** E1-S3
- **Owner:** Engineering owner
- **Approver:** Ross Todd, engineering owner and project owner
- **Expiry:** E5-S4, whose `t4_e5_a_descendant_that_leaves_its_process_group_is_still_contained`
  is the check this permission ends at. `DELIVERY-PLAN.md` §Story E5-S4 carries the task.

## Approved deviation

Permit E1-S3 to ship worker shutdown whose containment is the **union of a process-group kill and
a set of recorded pidfds**, rather than the full child process tree ADR-0001 §10.3 requires.

## The gap

`WorkerClient::shutdown` enumerates the worker's descendants once — before the worker is asked to
leave, which is the last moment `/proc/<pid>/task/*/children` still names them — then asks the
worker to leave, waits for it to exit without reaping it, and signals the process group it was
spawned as leader of. Both halves are real and each covers what the other misses:

- the group kill reaches a descendant started **after** the enumeration, because the leader is
  still unreaped and POSIX keeps a process group ID unusable while the group has a member;
- the recorded pidfds reach a descendant that left the group **before** the enumeration.

Their union is still not the tree. A descendant that calls `setsid()` in the window between the
enumeration and its parent's exit is in no group this build owns and appears in no `/proc` entry
the exit left behind, so nothing can name it. `wait_for_containment` then proves the group empty
and every recorded pidfd dead, and reports success, because those are the only two things it can
observe.

The residual is not hypothetical and is not obscure: a double-fork daemonization is the ordinary
way a process detaches, and any library the worker imports may do it.

## Why it is not closed in E1-S3

| Alternative | Reason rejected now |
|---|---|
| Spawn the worker into a PID namespace | Needs `CLONE_NEWPID` through `Command::pre_exec`, and `unsafe_code = "forbid"` is set workspace-wide in the root `Cargo.toml`. Lifting that is itself an ADR-0001 decision, not a remediation |
| Wrap the spawn in `unshare --pid --fork` | Replaces the bundle interpreter as the spawned program. `WorkerConfiguration::for_bundle` derives the worker bundle identity from the program it launches, so the identity every cache key is built on would describe a wrapper rather than the bundle. It also adds a util-linux runtime dependency and needs unprivileged user namespaces |
| A cgroup v2 `cgroup.kill` | The correct mechanism, and the one E5-S4 should reach for. It needs a delegated writable cgroup subtree, which `docs/operations/REFERENCE-ENVIRONMENT.md` does not require the reference machine to provide |
| `PR_SET_CHILD_SUBREAPER` | Reparents orphaned descendants to this process so they can be waited on, but it is process-global state a library may not set on its caller's behalf, and it identifies reparented children only by their arrival, not by which worker they came from |

Every one of these is a design decision with its own blast radius. A remediation round is the
wrong place to take one, which is the same reason the model half of the second audit's sixth
finding was not folded into a remediation either.

## Conditions

1. The residual is stated in the story record's §Limits, in the terms above, and not as a
   rounding of "the tree is contained".
2. Nothing claims full-tree containment. `WorkerClient::shutdown`'s own documentation says what it
   reaches, and `t4_e1_a_descendant_started_during_shutdown_is_contained` is named for the case it
   actually proves — a descendant started after enumeration that stays in the group.
3. No test is written to assert the escape is contained. It is not, and a test shaped to pass
   against the current behavior would be the weakened control `CLAUDE.md` §Non-negotiables forbids.

## What this does not permit

- It does not permit the *enumerated* or *in-group* halves to regress. Both are covered by
  `t4_e1_a_gracefully_shut_down_worker_leaves_no_descendant_behind` and
  `t4_e1_a_descendant_started_during_shutdown_is_contained`.
- It does not extend to tool subprocesses outside the worker session; `process::terminate` is
  unchanged and this deviation makes no claim about it.
- It does not survive a worker pool. ADR-0001 §10.1 allows a pool size above one after `doctor`
  verifies the budgets; a pool multiplies the number of unowned escapees and the owner should
  revisit this before E5-S2 rather than inherit it.

## Rollback

Nothing durable depends on the deviation: it describes what shutdown fails to reach, and no
artifact, identity, or published format records it. It ends when E5-S4 lands a containment
mechanism that names a descendant which has left every group this build owns — at which point
this record is superseded rather than amended.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately,
which is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that shutdown's containment is the union of a process-group kill and recorded pidfds, that `wait_for_containment` reports success on evidence that cannot see a `setsid` escapee, and that closing it needs a mechanism this workspace's `unsafe_code = "forbid"` and reference environment do not currently allow | 2026-08-31 |
| Project owner | Ross Todd | Approve — accept an ADR-0001 §10.3 deviation through G1 in exchange for E1-S3 shipping with the containment it does have, on the conditions that the residual is stated rather than rounded away and that no test is shaped to pass against it; accept that the closure is scheduled in E5-S4 and must be revisited before any worker pool above size one | 2026-08-31 |
