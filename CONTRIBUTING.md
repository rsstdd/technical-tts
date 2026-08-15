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

See [`docs/operations/DEVELOPMENT-WORKFLOW.md`](docs/operations/DEVELOPMENT-WORKFLOW.md) for the complete workflow.

