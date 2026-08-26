# E0-S3 Qualification Tooling

This directory contains disposable, non-product tooling for the E0-S3 Chatterbox feasibility
spike. It does not define the production worker protocol, a product CLI command, a durable
schema, or cache behavior.

`chatterbox_spike.py` requires an already restored, frozen Python environment and already
acquired local artifacts. It refuses to run unless its roots are non-symlinked ext4 paths and
the process has only the loopback network interface. Run it through the required namespace:

```text
cargo build --locked -p study-tts-core --example qualification_blake3_file
```

The harness requires the resulting
`target/debug/examples/qualification_blake3_file` through `--blake3-executable`. This helper
uses the workspace-pinned `blake3` crate so voice artifacts are verified without adding a
second hashing implementation to the frozen Python environment.

```text
/usr/bin/time -v -o <private-time-report> \
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
