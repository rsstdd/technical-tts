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

Build it first, then run the binary through the required namespace. `cargo run`
inside `unshare` would resolve and compile under it, and a build is the one part
of this that legitimately reaches a network:

```text
cargo build --package study-tts-testkit --example worker-qualification

unshare --user --map-root-user --net \
  ./target/debug/examples/worker-qualification \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

**The namespace is required, not advised.** The instrument reads `/proc/net/dev`
and `/proc/net/route` before it creates the output root and refuses unless the
only interface is `lo` and no IPv4 route exists, which is the same check
`validate_network_isolation` makes for the E0-S3 harness above. ADR-0001 §17.7
asks the worker to operate without network access, and until this existed the
criterion read that off the worker's own diagnostics: `_apply_offline_environment`
prints the variables it applied, which proves the worker configured
`huggingface_hub` and `transformers` and proves nothing about the backend, a
transitive dependency, or a socket. Flags are a request; a namespace with no
IPv4 route is an IPv4 denial. The interfaces, IPv4 route count, and namespace
inode are recorded in the result, so a record can read the measured isolation
off the artifact.

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

**Listening is not covered by these criteria.** They measure session behavior —
one model load per lifetime, protocol-only stdout, staging containment, a stable
bundle identity — and none of them listens to the audio. E1-S3 produces speech
for the first time, so the review below is owed separately before any gate that
depends on it.

## E1-S3: the listening review

Rendered by a second example, for the same reason the qualification instrument
is one: the takes must come through `WorkerTtsExecutor`, the path production
uses.

```text
cargo run --package study-tts-testkit --example listening-render -- \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

The words are `fixtures/listening/e1-s3-listening-script.json`, committed and
registered in `docs/testing/TEST-DATA-MANIFEST.md` so a retake reviews the same
text and only the audio differs. The voice profile is read from the governed
voice root rather than named here.

It renders one take per line, copies them to shuffled `sample-NN.wav` under
`<output root>/listening/`, and writes two records beside them:

- `review-sheet.json`, **pending** — five criteria and a disposition per sample,
  all `null`. The instrument records no verdict, because the verdict is the one
  thing it exists to ask a human for.
- `randomization-key.json` — which line produced which sample.

Answer every criterion for every sample before opening the key. Write `none`
where nothing was heard; a blank is not a finding. Then:

```text
python3 scripts/qualification/check_listening_review.py <output root>/listening
```

That refuses an incomplete sheet, refuses one whose recorded digests no longer
match the audio beside it, and only then prints the mapping. Publish the
completed sheet under the governed evidence root and cite it by SHA-256.

`--output-root` must not already exist: a retake takes a new root, so it cannot
overwrite the set an earlier review is still bound to. Unlike the qualification
instrument above, this one needs **no network namespace** — it asserts no
offline property and produces audio for a person rather than evidence about a
network. What to update once the checker passes is in
`docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §3, under *Retaking a review, and
closing it out*, rather than repeated here.

**What the blinding does and does not enforce.** Nothing stops an operator
reading `randomization-key.json` early, and the instrument does not pretend
otherwise — the blinding is a discipline. What *is* mechanical is the binding
between a judgment and the bytes it was made against: every finding is recorded
under a take's SHA-256, so a review cannot be inherited by audio it was not made
against — and a retake cannot be rendered over the old set at all, because the
instrument refuses an `--output-root` that exists.

**Retake the review whenever the audio changes**, not only when the text does.
`ADR-0001-D007`'s edge conditioning pads and ramps every segment, so a build
that changes it produces different samples from the same script.
