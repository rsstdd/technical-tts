# Development Workflow

## Bootstrap location

Run development under Ubuntu 24.04 in WSL2. Keep the repository, Rust target, Python environment, models, voices, caches, fixtures, jobs, and generated output on the Linux filesystem.

## Start a story

1. Read the GitHub story, ADR-0001, Delivery Plan section, and routed policies.
2. Confirm the definition of ready and dependencies.
3. Move the story to In Progress and assign it.
4. Create a focused branch.
5. Add the first failing test or create the governed evidence protocol.
6. Implement the smallest coherent vertical behavior.
7. Keep the walking skeleton green.

## Local verification order

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Run the narrowest affected test first. Run broader checks after the target behavior is green. Product commands in AGENTS.md are authoritative only after their implementation exists.

The permanent end-to-end integration check is:

```bash
cargo test --offline -p study-tts-testkit --test walking_skeleton --locked
```

Its stage order, provisional boundaries, and deferred capabilities are recorded in [`../architecture/WALKING-SKELETON.md`](../architecture/WALKING-SKELETON.md). Every later story must keep this check green.

## Pull request

- Use the repository pull-request template.
- Keep one story per pull request unless a documented atomic dependency makes separation unsafe.
- Link the story, ADR requirement, tests, evidence, and rollback.
- Identify schema, cache identity, migration, rights, security, and listening effects.
- Do not commit model weights, voices, private lessons, production audio, caches, or generated environments.
- Update the story's test list in DELIVERY-PLAN.md in the same commit that adds, renames, or removes a test. A list that
lags the code is worse than no list, because it reads as authoritative.

## Review order

1. Correctness and acceptance criteria.
2. Architectural invariants and authority boundaries.
3. Failure handling, recovery, containment, and resource bounds.
4. Test quality and deterministic fixtures.
5. Compatibility, provenance, and documentation.
6. Maintainability and unnecessary abstraction.

## Completion

Update issue checkboxes and attach verified results. If work remains, create a linked issue with an owner and acceptance criteria. Do not close a story with hidden follow-up work.
