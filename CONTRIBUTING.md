# Contributing

Read ADR-0001, the Delivery Plan, AGENTS.md, and the documentation index before changing cross-cutting behavior. The accepted architecture is binding unless a new decision record explicitly supersedes it.

## Workflow

1. Select a ready GitHub story and assign it.
2. Follow the project playbook and one-story pull-request rule.
3. Develop deterministic behavior with red-green-refactor.
4. Keep the offline walking skeleton green.
5. Run the required checks and attach evidence.
6. Complete the pull-request template and story checklist.

Do not commit models, voices, private content, generated audio, caches, credentials, or raw qualification corpora. Do not weaken validation, containment, rights, checksum, offline, or recovery controls to make a test pass.

## Refusal updates

When adding or changing a typed refusal:

1. Put the leaf variant in the narrowest owning error category; do not add a
   duplicate flat `BuildError` variant.
2. Keep outer artifact/path context separate from reusable inner faults, and
   use only the runtime's `io_error` and `audio_error` helpers for generic path
   enrichment.
3. Preserve an actionable `Display` message naming the artifact, invariant,
   and governed remedy owner where one exists.
4. Add structured remedy advice only for an owner established by
   `docs/governance/ROUTING-TABLES.md`; otherwise return no advice.
5. Update exact nested-variant, conversion, source-chain, path, message, and
   remedy tests without renaming a Delivery Plan test.
6. Measure `size_of::<BuildError>()`; do not increase it or add boxing without
   recording the measured reason.

See [`docs/operations/DEVELOPMENT-WORKFLOW.md`](docs/operations/DEVELOPMENT-WORKFLOW.md) for the complete workflow.
