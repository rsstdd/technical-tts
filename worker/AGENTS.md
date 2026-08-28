# Worker instructions

Substance lives in [`../docs/operations/WORKER-ENVIRONMENT.md`](../docs/operations/WORKER-ENVIRONMENT.md),
which owns the bundle manifest, the lock procedure, and the offline rules. This file records only
what must be true before an edit here, because the consequences of getting it wrong are not local.

## The rule that makes this directory different

**Every file listed in `bundle-manifest.json` is a synthesis-key input.** ADR-0001 §12.5 derives the
worker-bundle hash from them, and that hash names every cache entry in the project. Editing one is
therefore a decision to invalidate the cache, not a routine change — and *failing* to declare one
you added is worse, because the hash then describes a bundle that is not this one.

Two obligations follow, and both are enforced rather than remembered:

1. **Declare every `.py` file you add here, imported or not.** The check walks the directories
   named in `import_roots` and refuses any `.py` file beneath one that `inputs` does not declare
   (`t1_e1_a_module_under_an_import_root_the_manifest_omits_is_refused`, and
   `t1_e1_a_module_in_a_subpackage_of_an_import_root_is_declared` for one a level down). A file
   nothing imports is either dead code or a dynamic import: delete it or declare it.
2. **Do not narrow the manifest to make the check pass.** The floor is Rust's, not the manifest's:
   `REQUIRED_BUNDLE_INPUTS` and `REQUIRED_IMPORT_ROOT` in
   `crates/study-tts-runtime/src/worker_bundle.rs` refuse a manifest that drops the lockfile, the
   launcher, the protocol schema, the entrypoint, or the package root before a byte is hashed.

**Dynamic imports are fine.** The check is deliberately over the directory rather than over
`import` statements, because a module reached through `importlib`, `__import__`, or a
parenthesized multi-line `from ... import (...)` is one a textual scan does not see — and the
identity must not depend on *how* a module is reached. There is no review rule here to forget.

## Tests live outside the bundle

`worker/tests/` is deliberately not under `study_tts_worker/` and not declared in
`bundle-manifest.json`: a test module inside the bundle would be a synthesis-key input, so every
test edit would invalidate the cache. For the same reason they use `unittest` from the standard
library — `requirements.lock` is a declared input, and a test dependency added to it would change
the bundle hash. CI runs them with `python3 -m unittest discover --start-directory worker/tests`.

## Boundaries

- **stdout is protocol only, and the descriptor is taken to make that true.**
  `protocol.reserve_protocol_stream` duplicates stdout for `protocol.write_frame` and points file
  descriptor 1 at stderr, so a stray `print` — or a native library writing to fd 1 from inside a
  model load — is a diagnostic rather than bytes between two frames
  (`ProtocolStreamTests` in `tests/test_worker.py`).
- **stderr is diagnostics**, and carries no source text and no voice path (ADR-0001 §16).
- **A refused frame must never be an exit.** `protocol.read_request` converts everything the
  parser can raise — including the `ValueError` and `RecursionError` that are not
  `JSONDecodeError` — into a `FrameError` the supervisor reads as a failure frame. A process that
  died on a hostile frame would take every queued request with it
  (`HostileFrameTests` in `tests/test_worker.py`).
- **No model is loaded in this build.** `synthesize` refuses with `initialization_failed` naming
  E1-S3. Do not add a placeholder tone: the cache would publish it under a key claiming a real
  model produced it. `AGENTS.md` forbids shipping a stub as though it were implemented.
- **Offline is applied, not merely configured.** `worker.main` loads and validates `launcher.json`,
  then calls `worker._apply_offline_environment` before reserving the protocol descriptor. A
  backend must be imported *inside* `main`, after both operations — `huggingface_hub` and
  `transformers` read those variables as their modules load, and a module-level import would run
  before either.

## Environment

`worker/.venv` is gitignored as both a directory and a link, and on the reference machine it is a
**link** to an environment kept outside the checkout: `actions/checkout` cleans with `git clean -ffdx`, which removes ignored
files, so an environment built inside the workspace does not survive to the next qualification run.
The operations document owns the restore procedure, including the `--no-deps` install, the VCS
install that makes `pip` record which revision of the governed tree it checked out, and the rule
that every `.pth` in the environment belongs to a distribution the lock pins.
Model code and weights never enter Git, CI, fixtures, or logs
([`../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](../docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md)).


