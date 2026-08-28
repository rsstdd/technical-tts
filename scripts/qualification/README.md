# E0-S3 Qualification Tooling

This directory contains disposable, non-product tooling for the E0-S3 Chatterbox feasibility
spike. It does not define the production worker protocol, a product CLI command, a durable
schema, or cache behavior.

`chatterbox_spike.py` requires an already restored, frozen Python environment and already
acquired local artifacts. It refuses to run unless its roots are non-symlinked ext4 paths and
the process has only the loopback network interface. First build the BLAKE3 helper:

```text
cargo build --locked -p study-tts-core --example qualification_blake3_file
```

The harness requires the resulting
`target/debug/examples/qualification_blake3_file` through `--blake3-executable`. This helper
uses the workspace-pinned `blake3` crate so voice artifacts are verified without adding a
second hashing implementation to the frozen Python environment.

The E0-S3 harness authenticates the governed acquisition approval, bundle manifest, voice
profile, and consent record against reviewed SHA-256 values embedded in the harness before it
accepts their identity or approval fields. A different bundle or voice requires superseding
governed approval records and an explicit harness update. Generated output is limited to 60
seconds per run and ten minutes across one invocation.

Then run the harness through the required namespace. Set `PYTHONHASHSEED` to the same value as
`--seed` before the Python interpreter starts; the E0-S3 fixed-seed run uses `42`:

```text
PYTHONHASHSEED=42 /usr/bin/time -v -o <private-time-report> \
  unshare --user --map-root-user --net \
  <qualification-python> scripts/qualification/chatterbox_spike.py <arguments>
```

Pass `--help` for the complete argument list. Input text, voice material, generated audio, and
machine-readable raw results belong under the governed external evidence root and never in Git.
The output root must not exist; a rerun receives a new root rather than overwriting prior
evidence.

The harness creates a randomized listening directory. Review the numbered WAV files using
`listening/review-sheet.json` before opening `listening/randomization-key.json`. A human reviewer
must fill every criterion; the harness deliberately records listening as pending. Preserve the
generated pending sheet unchanged. Publish the completed review as a new checksum-linked file in
the governed evidence root, then open the key and link both artifacts from a superseding evidence
record or addendum.

## Running the tests for this tooling

```text
cd scripts/qualification && python3 -m unittest discover --start-directory tests
```

Twenty-one tests across the three modules. They need none of what the harness
needs — no restored environment, no governed artifacts, no network namespace —
because they exercise the parsing, checksum, and refusal logic rather than a
render. The `cd` is required: `tests/` is not a package, so discovery from the
repository root reports `Start directory is not importable` instead of running
them.

`.github/workflows/ci.yml` runs `worker/tests` and not these, because this
directory is disposable E0-S3 spike tooling rather than a product path. That
makes the command above the only thing standing between these tests and rot, so
run it whenever you change a script here.
