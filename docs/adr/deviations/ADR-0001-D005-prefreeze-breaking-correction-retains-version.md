# ADR-0001-D005 — A pre-G1 breaking correction retains its unreleased version

- **Status:** Approved
- **Date:** 2026-08-29
- **Controlling ADR and sections:** ADR-0001 §7.1 and §17, through
  `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
- **Requesting story:** E1-S1
- **Owner:** Engineering owner
- **Approver:** Ross Todd, engineering owner and project owner
- **Expiry:** G1. At the interface freeze this permission ends and the change classes apply
  unmodified.

## Approved deviation

Permit a **Breaking contract** change to a provisional seam to retain its version, rather than
increment the major and supply migration and rollback, when every one of the following holds:

1. The seam is still provisional under §G1 freeze — not yet frozen.
2. The version being retained was itself introduced by an unreleased breaking move within the
   same story, so no consumer ever saw the shape being corrected.
3. No durable artifact on disk, and no evidence record outside `Proposed`, was written under the
   shape being corrected.
4. Supervisor, fake, worker, tests, fixtures, and generated schema move in the same commit, in
   the order §Amendment rules before G1 already requires.
5. The correction and its reasoning are recorded in the story's interface-change record before
   the code lands.

Every other requirement of the **Breaking contract** row stands: impact report and owner
approval are unaffected. Only the major-version increment and the migration-and-rollback
procedure are waived, and only under all five conditions.

## The gap

`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes reads:

> | Breaking contract | Required field, semantic change, frame change | Major version, migration, impact report, owner approval |

That row is written for a contract someone depends on. Migration and rollback describe moving a
consumer from one released shape to another, and a major increment is what tells that consumer
its assumptions are void.

The document also says these seams "remain provisional until the real Chatterbox worker and real
package path pass the same contracts at G1", and its only stated pre-G1 rule is an ordering one:
"Before G1, every amendment updates its fake, fixtures, and shared suite before its consumers."
It never says what a breaking correction to an **unreleased, unfrozen** version requires. The
row and the pre-G1 paragraph are both true and do not meet.

The concrete case is the fifteenth E1-S1 audit, recorded in
`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`. `WorkerResponseFrame::Initialized` carried an
arbitrary string map, so a worker could report a successful load while naming no model revision,
no tokenizer revision, and no voice profile. The correction made those identities required. That
is a required-field change to `e1.worker.1.0` — and `e1.worker.1.0` is itself the breaking
version the first audit introduced, which nothing has ever consumed.

Incrementing to `e1.worker.2.0` would have created a version whose entire migration story is
"the 1.0 nobody used reported a load it had not performed", and would have left `1.0` standing
in the tree as a shape this project publishes and considers valid. Retaining the version is the
smaller lie only if it is *authorized*, which is what this record exists to decide.

## Why this is a record and not a paragraph

The decision was originally argued inline in
`docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`, three paragraphs above the §Amendment
rules section stating the rule it departs from. That document describes itself as mirroring the
governance document; a mirror that grants itself an exception is how a rule quietly stops being
the rule. Recording it here puts the decision where deviations are approved, in a file whose
status a reader can check, and leaves the baseline document mirroring rather than legislating.

Nothing about the fifteenth audit's engineering changes. What changes is that the permission is
reviewable, and that the next pre-G1 breaking correction has a condition list to meet rather
than a precedent to cite.

## Impact

- **Architecture and authority boundaries:** No change. This record adds no code and removes no
  control.
- **Schemas and interfaces:** No change to any published schema. `worker-protocol-v1.schema.json`
  and `WORKER_PROTOCOL_VERSION` stay as the fifteenth audit left them.
- **Synthesis, verification, and cache identities:** No field is added or removed and no identity
  moves. The bundle-hash consequences of the fifteenth audit are recorded in that audit's own
  §Compatibility and identity impact and are unaffected by whether this record is approved.
- **Security, rights, and privacy:** No control is waived. Condition 3 is what keeps this from
  reaching anything durable.
- **Tests and evidence:**
  `t3_e1_published_schema_required_fields_match_the_recorded_surface` in
  `crates/study-tts-testkit/tests/schemas.rs` is what makes a future required-field change
  visible at the point it is made, so a correction taken under this permission cannot be a
  silent one. The accepted E1-S1 evidence is
  `evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v11.md`.
- **Operations:** None.

## Alternatives considered

| Alternative | Reason rejected |
|---|---|
| Increment to `e1.worker.2.0` as the class table reads | Publishes a `1.0` this project never intends anyone to speak, and a migration procedure from a version with no consumers; the freeze at G1 would then carry two majors, one of them fictional |
| Leave the reasoning in `PROVISIONAL-CONTRACT-BASELINE.md` | A mirror document granting itself an exception to the document it mirrors, which is the defect this record closes |
| Amend §Change classes to add a pre-G1 column | Rewrites a governance rule to fit one case, and does it in the document with the widest reach; a bounded, expiring permission is the smaller instrument |
| Treat it as a compatible extension | False. A required field is not optional, and calling it one would make the class table describe something other than what happened |
| Do nothing and accept the inconsistency | The next pre-G1 breaking correction then cites this one as precedent, with no conditions attached to it |

## Compensating control and expiry

This permission expires at G1, where §G1 freeze takes over and every seam is frozen with a
migration procedure. Until then, condition 5 is the compensating control: the correction is
written into the story's interface-change record **before** the code lands, so the reasoning is
reviewable at the time rather than reconstructed afterwards.

If this permission is revoked before G1, the compensating action is to increment the worker
protocol to `e1.worker.2.0`, supply the migration and rollback the class table requires, and
retire `e1.worker.1.0` — mechanical work, since no consumer and no durable artifact depends on
either.

## Rollback

Supersede this record and take the revocation path above. Retain this decision and the v11
evidence as the history of the correction they approved; no cache entry or durable artifact is
re-keyed, relabeled, or deleted.

## Decision

- [x] **Approve**
- [ ] Reject
- [ ] Defer

Ross Todd holds both roles below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately,
which is why the two rows differ.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Approve — accept that `e1.worker.1.0` describes a shape that changed after the version existed, and that the five conditions are the whole of what bounds it | 2026-08-29 |
| Project owner | Ross Todd | Approve — accept a bounded, G1-expiring permission to retain a version across a pre-G1 breaking correction, in place of the major increment §Change classes requires | 2026-08-29 |
