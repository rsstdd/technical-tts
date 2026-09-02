# Gate Review: G1 — g1-gate-review-v1

- Date: 2026-09-02, Europe/Berlin
- Candidate revision: `story/e1-s4-minimal-package-generation` at the E1-S5 implementation
- Candidate artifact/manifest checksum: package identity
  `3dbc3415d84a08177d7fe2e0b0b791a854b9d0309ffb8986424ce09b07b78fe6`, at worker bundle identity
  `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05`
- Accountable owner: Engineering owner
- Approvers: Engineering owner and project owner
- Status: Accepted

## Scope and criteria

`DELIVERY-PLAN.md` §E1-S5 states G1 acceptance as four conjuncts. They are taken here in the order
the plan writes them, and each is answered by a record rather than by this one.

| Requirement | Story | Test/evidence ID | Result | Artifact link/checksum |
|---|---|---|---|---|
| A reviewed three-segment lesson renders through real Chatterbox and produces a complete private-preview package | E1-S4, re-rendered at E1-S5 | `e1-s5-canonical-json-authoring-v1` §Listening material | Pass | package `3dbc3415d84a08177d7fe2e0b0b791a854b9d0309ffb8986424ce09b07b78fe6`, seven artifacts, 14.960 s master, `private_preview` |
| The lesson is authorable through the published schema and scaffold | E1-S5 | `t4_e1_scaffolded_lesson_validates_without_manual_repair`, `t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton`, `t1_e1_validation_error_names_the_offending_field_path` | Pass | `E1-S5-INTERFACE-CHANGE-001`, Accepted 2026-09-02 |
| Fake and real implementations pass shared contracts | E1-S1, E1-S3, E1-S4, E1-S5 | `run_tts_executor_contract_scenario`, `run_cache_contract_scenario`, `run_package_writer_contract_scenario`, `run_job_repository_contract_scenario` | Pass | See §The seam that closed last |
| The G1 interfaces freeze through a versioned charter | E1-S5 | `docs/architecture/G1-FREEZE-CHARTER.md` | Pass | Accepted 2026-09-02; eighteen contracts, twenty-one versioned constants accounted for |
| Human listening review (ADR-0001 §17.5) | E1-S5 | `e1-s5-canonical-json-authoring-v1` §Review result | Pass | Taken 2026-09-02, built-in laptop speakers, no finding on any of five criteria, disposition `accept` |
| Reference-machine qualification, offline | E1-S3, requalified at E1-S5 | six `t5_e1_` criteria | Pass | [`e1-s5-requalification-result.json`](e1-s3/e1-s5-requalification-result.json), SHA-256 `bebee3e0b2c5e0bbe6586ef65d2a5918f57537088d25535477c2097a98b8d4c0` |

## The seam that closed last

"Fake and real implementations pass shared contracts" was the conjunct with a genuine gap in it,
and the gap was not visible from the suite's own results. Three of the four seams ran their shared
scenario against both a fake and the real adapter. The **job repository** ran it only against
`InMemoryJobRepository`; `FileSystemJobRepository` appeared only inside
`t4_e0_walking_skeleton_uses_only_published_seams`, wrapped in a recorder that observes calls
rather than checking a contract. Parity was inferred rather than shown, and an inference passes
every test written against it.

`t4_e1_the_real_job_repository_passes_the_shared_contract` closes it, on the shape
`t4_e1_the_real_package_writer_passes_the_shared_contract` used for the identical gap at E1-S4.
The executor's real side is exercised by the T5 instrument on the reference machine rather than in
CI, which is where a real worker can exist.

## What moved to reach this gate

Three changes moved every synthesis key in the project, all of them deliberately before the freeze
rather than after it, because `ADR-0001-D005`'s pre-freeze permission expires here and each would
otherwise need a full **Breaking contract** migration.

| Change | Record | Issue |
|---|---|---|
| Seeding before model construction | `E1-S3-INTERFACE-CHANGE-004`, Accepted 2026-09-02 | #70 |
| `model_artifacts_hash` as a synthesis-key input | `E1-S5-INTERFACE-CHANGE-002` with `ADR-0001-D011`, both Accepted 2026-09-02 | #66 |
| `deterministic_seed` → `True` | `E1-S5-INTERFACE-CHANGE-005`, Accepted 2026-09-02 | #70 |

The worker bundle identity moved twice in the process — `3e1f487c…` → `2206e9c8…` → `1af4e171…` —
and the E1-S4 package and its listening review became historical with the first of those. That
record is accepted and was **not** edited; what it attests remains true of the artifacts it names.
The re-render and the fresh review in `e1-s5-canonical-json-authoring-v1` are what describe the
candidate this gate accepts.

## Open findings

| Finding | Severity | Owner | Required action | Deadline |
|---|---|---|---|---|
| `worker/pyproject.toml` is a declared bundle input though ADR-0001 §12.5 does not list it | Minor | Engineering owner | Remove it from `worker/bundle-manifest.json` `inputs` at the next change that moves the worker-bundle identity for its own reasons, per `G1-FREEZE-CHARTER` §The open §12.5 question. `check_requirements_match_lock` makes the failure mode unreachable meanwhile | Next identity move |
| Fourteen evidence citations name file versions never committed | Minor | Project owner | Issue #71's reconciliation, and `check_evidence_citations.py` into CI with `fetch-depth: 0` once it is accepted. Gates nothing here: every affected record is superseded or reconciled, and `check-evidence-provenance.py` passes | E2 |
| Reproducibility is measured for one environment, one seed, one sentence | Minor | Engineering owner | Widen the criterion's corpus if a later story depends on reproducibility beyond cache-key stability | E5-S2 |
| ADR-0003, ADR-0004, ADR-0005 remain `Proposed` | Minor | Project owner | Each awaits evidence from E2-S3, E5-S1, E6-S2, or E4 and carries a "Decision to be completed" section. None gates G1 | Their own epics |

## Decision

- [x] Pass
- [ ] Conditional pass, permitted only when the controlling plan allows it
- [ ] Fail

Decision rationale: every conjunct of `DELIVERY-PLAN.md` §E1-S5's G1 acceptance is met by a named
record, the ADR-0001 §17.5 listening review was taken by a person against the candidate's own bytes,
and the interfaces are frozen by an accepted charter whose inventory is derived mechanically rather
than curated. The four open findings above are recorded rather than waived; none is a condition on
this pass, and each names an owner and an occasion.

**What this gate does not certify.** It is a private-preview gate. Nothing here claims production
release: `release_status` is `private_preview`, the production gates of
`docs/governance/RELEASE-PROFILES.md` §3 are unimplemented, and `validate_production_manifest`
refuses. Listening was taken once, on built-in laptop speakers, with the bounds
`e1-s5-canonical-json-authoring-v1` §What this review will not cover states.

| Role | Name | Decision | Date |
|---|---|---|---|
| Engineering owner | Ross Todd | Pass | 2026-09-02 |
| Project owner | Ross Todd | Pass | 2026-09-02 |
| Contract owner (T-CORE) | Ross Todd for T-CORE | Pass | 2026-09-02 |
| Contract owner (T-WORKER) | Ross Todd for T-WORKER | Pass | 2026-09-02 |
| Contract owner (T-AUDIO) | Ross Todd for T-AUDIO | Pass | 2026-09-02 |
| Contract owner (T-CLI) | Ross Todd for T-CLI | Pass | 2026-09-02 |
