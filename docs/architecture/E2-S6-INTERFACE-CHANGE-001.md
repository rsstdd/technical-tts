# E2-S6 Interface Change 001 — A person's approval becomes a document

## Identification

- Record ID: `E2-S6-INTERFACE-CHANGE-001`
- Status: **Accepted 2026-09-11.** Every row in §Approval is signed.
- Contract owner: T-CLI (`approval`, `preview_release`)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CLI, T-AUDIO, T-RUNTIME
- Accepted ADR, if architectural: **not architectural, and that was checked rather than assumed.**
  ADR-0001 §18 already ratifies human review as a precondition of an accepted build — "a
  **reviewed** 60-minute technical lesson builds", "no unapproved claim enters the lesson through
  compilation", "no omitted, duplicated, inserted, or materially mispronounced technical content
  survives **review**". This story gives that ratified requirement a shape; it does not move the
  authority boundary, so §Change classes calls it **breaking contract** rather than architectural.

## Version and compatibility

Two new published documents, neither replacing anything.

- Contract ID: `approval` — new at `1.0`, `schemas/approval-v1.schema.json`
- Contract ID: `preview_release` — new at `1.0`, `schemas/preview-release-v1.schema.json`
- Compatibility class: **breaking contract** — two formats enter the published catalogue, which
  `PUBLISHED_SCHEMAS` grows from eight to ten to record.
- Required/defaulted fields: eight required at the root of `approval`, four at the root of
  `preview_release`. No optional or defaulted field in either.
- Unknown-field behavior: refused. `deny_unknown_fields` on both.
- Unknown-value behavior: refused. `ApprovalDisposition` is a closed vocabulary with no
  `#[serde(other)]`: a disposition this build does not know is not an approval, and reading it as
  a rejection would be as wrong as reading it as an acceptance.
- Wire representation: JSON, written through the existing durable path — stage, fsync, rename,
  fsync parent. No new publication mechanism.

## Why the package does not hold either document

`ADR-0001` §12.1's `output/` tree names neither, exactly as it named no run report before
`E2-S4-INTERFACE-CHANGE-002` recorded why the package holds one. This record answers the same
question and reaches the opposite conclusion, for a reason specific to approval.

**The order forbids it.** Task 2 fixes that the manifest is final before review, and it has to be:
a reviewer cannot listen to a package that does not exist. Approval is therefore always *after*
publication. But a package directory is published by rename and **named by the BLAKE3 of its own
manifest**, so a file added afterwards either falsifies that name or mutates a published immutable
artifact. The run report could go inside the package because it is sealed *during* the write; an
approval cannot, because it is made after it.

Both documents therefore live under `previews/<lesson-id>/`, beside `current.json`:

```text
previews/<lesson-id>/
  current.json                       selection record, unchanged
  release.json                       preview_release, one per lesson
  approvals/<manifest-blake3>.json   approval, one per reviewed generation
  packages/<manifest-blake3>/        untouched by any of this
```

Approvals are keyed by manifest digest and accumulate. A rejected generation keeps its rejection:
the record is evidence that a generation was judged, and deleting it would erase the judgment.

## The cycle the M2 acceptance forbids, prevented by construction

`DELIVERY-PLAN.md`'s M2 acceptance requires human approval recorded "**without a checksum cycle**".

The references run one way only: `preview_release` → `approval` → `manifest` → the artifacts. No
document names anything upstream of itself, and **no type has a field that could**. That is the
whole mechanism. `t3_e2_release_record_references_manifest_and_approval_without_cycle` asserts it
on the field lists — an exhaustive destructure of `ApprovalRecord` and a check that no manifest
field name could hold an approval — rather than by scanning bytes for a digest, which would pass
for a cycle spelled any other way.

`E2-S4-INTERFACE-CHANGE-002` §"The reference runs one way only" established this discipline for
the manifest–report pair. This is the same rule applied to a chain of three.

## How a content change invalidates an approval, without comparing content

An approval is stored under the digest of the manifest it judged; a package directory is named by
that same digest. A rebuilt package is therefore a different name with no approval beside it, and
invalidation needs no comparison to happen — it is the absence of a file.

`approved_package` additionally checks that a stored approval's recorded digest equals the one
selecting it, refusing as `ApprovalManifestMismatch`. That is belt to the braces: it catches a
record whose file was renamed, the only way the two could disagree.

`t4_e2_content_change_invalidates_prior_approval` proves it through a retake, and rejects the wrong
implementation this repository would otherwise reach for — **an approval keyed by lesson**, which
would silently carry a reviewer's judgment of one recording onto a different one.

## What task 7 cannot mean

"Require completed approval before private-preview completion" cannot gate *writing* a package:
approval requires a package to listen to, so under that reading nothing could ever be built. Task 2
settles the order. The gate is therefore on **declaring a preview finished**, which is
`publish_preview_release` — it refuses `PreviewNotApproved` when the selected generation carries no
accepting approval.

Recorded as an interpretation, not a discovery. If the owner intends the broader reading, it is
unimplementable as written and the task needs rewording rather than a different design.

## Identity effect

| Identity | Verdict |
|---|---|
| Synthesis, verification, plan, takes, cache | **None move.** No key input is read or written |
| Package | **Does not move**, and that is the point. If approving changed package identity, the reviewer would sign one identity and the tree would hold another |
| Transaction | **Does not move.** Approval happens after publication, outside any transaction |

**No existing artifact is invalidated, stranded, migrated, or rebuilt.** A package published before
this story is exactly as valid after it; it simply has no approval, which is true and was always
true. Nothing needs requalification: qualification measures the worker, and no synthesis input
moves.

## Impact

- Synthesis identities affected: none. Verification identities affected: none.
- Plan, takes, or package identities affected: **none.**
- Consumers and commands affected: **none yet.** E2-S5 (#18) owns the `review` and `approve`
  commands; this story deliberately adds no CLI surface, because #18 task 1 owns it.
- Fakes and shared suites affected: none. No fake writes or reads an approval.
- Fixtures and schemas affected: two schemas published; `PUBLISHED_SCHEMAS` 8 → 10;
  `PUBLISHED_REQUIRED_SURFACE` gains two rows; four contract fixtures added with their SHA-256
  rows in `docs/testing/TEST-DATA-MANIFEST.md`.
- Existing cached artifacts affected: **none.** No cache entry records either contract.
- Published packages or accepted takes affected: **none.**
- Rights: an approval names a **reviewer identity and role** — the first person-identifying field
  in a published document here. `RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access permits
  "approval records" in the repository by name. The record carries a name and a role and no path,
  so issue #82's redaction rule has nothing to reach.

## What the checklist gained, and why it is versioned

`docs/operations/PREVIEW-REVIEW-CHECKLIST.md` is now `1.0`, mirrored by
`study_tts_core::PREVIEW_REVIEW_CHECKLIST_VERSION` and pinned two-sided by
`t1_e2_checklist_version_matches_the_checklist_document`.

The version is what makes an approval legible. That document exists because "the criteria drifted"
across four listening reviews using four different criteria sets, and an approval that did not say
which set it answered would reintroduce exactly that.

Two corrections came with it. **"Protected terms" is now a per-segment criterion** — E2-S6 task 1
names it and the phrase appeared nowhere in the file. And §Scope said the package was "the six
artifacts a preview generation writes", which E2-S4 made seven; a reviewer checking a package
against a stale count is the kind of error a checklist exists to prevent.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: **not applicable** — no fake or shared
  contract suite touches an approval.
- Migration procedure: **none.** Both formats are new; nothing exists to migrate.
- Rollback procedure: revert. Stored approvals become files nothing reads; no package, cache entry,
  or job is affected, because none of them ever referenced one.
- Compatibility evidence: the two valid contract fixtures prove schema and parser agree; the two
  uppercase-digest fixtures prove each schema refuses at the exact field that is wrong.
- Mapped tests and qualification rerun: the E2-S6 named tests. **No requalification.**
- Walking skeleton result: **82 passed**, recorded at implementation; re-run at approval.

## The charter gap this record widens

`G1-FREEZE-CHARTER.md` §The inventory is derived promises every `*_SCHEMA_VERSION` constant appears
as a frozen row or in §Deliberately not frozen. `APPROVAL_SCHEMA_VERSION` and
`PREVIEW_RELEASE_SCHEMA_VERSION` appear in neither, joining `RUN_REPORT_SCHEMA_VERSION`.

**No row was added, deliberately.** A frozen row at a version no signature has made effective is
what `E2-INTERFACE-CHANGE-001` §What this signature moves declined to write, and writing one here
would contradict that a week later. The charter's §Status records all three by name instead.
Signing this record and `E2-S4-INTERFACE-CHANGE-001` closes the whole gap.

## Open questions

**G-A — whether an approval should record per-segment findings.** It records the decision, not the
findings: the checklist a reviewer filled and the evidence record citing it hold those, and
duplicating them here creates a second place for them to disagree. Tasks 3 and 4 ask only for the
manifest checksum and checklist version. If a consumer is later found to need the findings
mechanically, this is where that is decided.

**G-B — whether `ReleaseStatus` should gain a third variant.** It has two, `PrivatePreview` and
`ProductionRelease`, and its own doc says a private preview is "not verified, **approved**, or
releasable" — a word the type cannot express now that approval exists. Left alone here because it
is a frozen-vocabulary change serving no criterion this story owns.
`t3_e2_private_preview_cannot_claim_production_verification` pins what matters meanwhile: an
approved, released preview still cannot claim production.

## Approval

**Every row below is signed.** Each records a decision a role made and the date it was made.

| Role | Decision sought | Status |
|---|---|---|
| Contract owner (T-CLI) | Accept `approval` `1.0` and `preview_release` `1.0` as published documents, and the decision-not-findings shape |Accepted — Ross Todd, 2026-09-11 |
| Engineering owner | Accept two new published schemas, the empty migration, and that no identity moves |Accepted — Ross Todd, 2026-09-11 |
| Affected-track reviewer (T-AUDIO) | Accept that neither document enters the package, and that package identity is unchanged by approval |Accepted — Ross Todd, 2026-09-11 |
| Affected-track reviewer (T-RUNTIME) | Accept the gate placement in `publish_preview_release` and the interpretation of task 7 |Accepted — Ross Todd, 2026-09-11 |
| Project owner | Accept `PREVIEW-REVIEW-CHECKLIST.md` `1.0` as the checklist `DELIVERY-PLAN.md` §5 requires by M2 |Accepted — Ross Todd, 2026-09-11 |

- Effective version and date: `approval` `1.0` and `preview_release` `1.0`, effective 2026-09-11.

## Amendments

| Date | Amendment | Approval |
|---|---|---|
