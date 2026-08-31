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

## E1-S3: qualifying the worker session

The four `t5_e1_` names in `DELIVERY-PLAN.md` §E1-S3 are acceptance criteria,
not `cargo test` functions — `grep 'fn t5_'` across `crates/` returns nothing,
because every `t5_` name in this project is discharged by an operator-run
instrument plus an evidence record citing its hashed output. E0-S3 used the same
shape; `evidence/gates/g0/e0-s3/e0-s3-g0-qualification-report-v1.md` is what one
looks like.

The instrument is a Rust example rather than a script here, because three of the
four criteria are about the *executor* driving a real worker. A Python harness
would re-implement the protocol client and then qualify the re-implementation
instead of the shipped path.

```text
cargo run --package study-tts-testkit --example worker-qualification -- \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

Every root is a required argument with no default. The governed two are named
here only as placeholders: `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
keeps their real locations out of Git, CI, and logs. The output root must not
already exist, so a rerun cannot overwrite the artifacts a previous result was
hashed from.

It prints one JSON object naming the worker bundle identity and each
criterion's verdict, exits non-zero if any failed, and **writes that object to
`<output root>/qualification-result.json`, reporting its SHA-256 on the last
line**. The file is the thing an evidence record cites: a result that existed
only in a terminal could not be hashed or cited, which an audit of the first
E1-S3 result recorded as a finding against it. Copy that file under
`evidence/gates/g1/e1-s3/` and cite it with the reported digest per
`evidence/README.md`; `scripts/check-evidence-provenance.py` verifies citations
with SHA-256, which is why the instrument reports that digest and no other.

A fifth criterion, `t5_e1_worker_survives_restart_and_starts_offline`, is not one
of the four `DELIVERY-PLAN.md` names. It is a helper criterion covering ADR-0001
§17.7's restart and offline requirements, which nothing shared between the fake
and the real worker exercised: both suites started one worker, rendered once and
dropped it. It runs the same
`run_worker_restart_contract_scenario` the T4 suite drives the protocol fake
through, so the two ends are exercised by one function.

**This is not run by `qualification.yml`, deliberately** — the same reason that
workflow gives for its own real-model steps: naming a governed root in a public
workflow file would put it into Git, and a scheduled run would touch artifacts
the rights policy keeps operator-controlled.

**Listening is not covered.** These criteria measure session behavior — one
model load per lifetime, protocol-only stdout, staging containment, a stable
bundle identity — and none of them listens to the audio. E1-S3 produces speech
for the first time, so a listening review is owed separately before any gate
that depends on it.
