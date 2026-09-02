# The review and accept cycle

How work in this repository goes from "the tests pass" to "a gate can rely on it".

Three things are reviewed, by three different kinds of reviewer, and they are easy to confuse:

| What | Who reviews it | What refuses it |
|---|---|---|
| **Code** | The automated gates | `cargo test`, Clippy, the conventions and provenance checks |
| **Audio** | A human, listening | Nothing mechanical. This is the point |
| **Evidence** | The accountable owner, at a gate | `scripts/check-evidence-provenance.py` |

An instrument may measure anything it can measure. It may never supply the second row.

---

## 1. Code: the gates

Run before claiming anything. `AGENTS.md` owns the list; this is it in order, fastest first.

```bash
cargo fmt --all -- --check
python3 scripts/check-rust-conventions.py
cargo clippy --offline --workspace --all-targets --all-features --locked -- -D warnings
cargo test --offline --workspace --all-targets --locked
cargo test --offline --workspace --doc --locked
python3 -m unittest discover --start-directory worker/tests
worker/.venv/bin/python -m unittest discover --start-directory worker/tests
(cd scripts/qualification && python3 -m unittest discover --start-directory tests)
python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'
python3 scripts/check-evidence-provenance.py
git diff --check
```

Doctests are a separate command because `--all-targets` excludes them. A check that did not run is
reported as not run, never as passed.

The worker suite runs **twice, under two interpreters, and both are required**. It is standard
library only, which is what lets `.github/workflows/ci.yml` run it with the system interpreter and
no installation step. But the worker's render path imports `numpy`, `soundfile`, and `torch`
unconditionally, so on that interpreter every test covering it is skipped — and a defect once
shipped through exactly that gap, with all sixty-one tests passing while the real worker died on
its first synthesis. `RenderPlumbingTests` drives the render with a stub model and the real
libraries; the second command is the only place it runs. `python3 -m unittest` reporting
`OK (skipped=2)` is the signal that it did not.

---

## 2. Qualification: what only the reference machine can answer

Some criteria cannot run in CI, because they need the real model, the governed roots, and the
restored `worker/.venv`. Those are the `t5_` names. `grep 'fn t5_'` across `crates/` returns
nothing on purpose: every one is an acceptance criterion discharged by an operator-run instrument
plus a record citing its hashed output.

```bash
cargo build --package study-tts-testkit --example worker-qualification

unshare --user --map-root-user --net \
  ./target/debug/examples/worker-qualification \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

The namespace is required: the instrument refuses to start unless `/proc/net/dev`
holds only `lo` and `/proc/net/route` contains no IPv4 route entries, so a filed
result is always one whose IPv4 egress was denied rather than one whose worker
said it had set some variables. Built outside the namespace and run inside it,
because a build may legitimately reach a network and a qualification run may not.

Writes `qualification-result.json` under the output root and prints its SHA-256. Copy it beside the
story record and cite that digest.

**Read it before committing it.** On a passing run it holds counts, criteria, and digests only. A
*failing* criterion can quote a path from the output root, and
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps governed locations out of Git.

The worker bundle identity alone needs no governed root:

```bash
cargo run --package study-tts-runtime --example worker-bundle-hash
```

### The model root is pinned in Git, and that is deliberate

`PINNED_MODEL_REVISION` and `DECLARED_MODEL_ARTIFACTS` in
`crates/study-tts-runtime/src/model_gate.rs` hold the qualified revision and the SHA-256 and byte
count of every artifact the backend loads. `WorkerConfiguration::for_bundle` hashes them before it
can return a launchable configuration, so a worker cannot start against weights this build has not
proven — and `WorkerTtsExecutor::start` refuses a worker that then reports a *different* revision,
which is what stops it loading a directory the gate never read. Hashing all four costs about two
seconds on the reference machine.

They live in Git rather than being read from the governed root's `bundle-manifest.json`, which
declares the same values. A digest list beside the weights is trust on first use: whoever can
replace the weights can replace the list. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps
governed *locations* and *bytes* out of the repository, not the checksums of public third-party
weights — and this does not extend to voice digests, which stay in the governed voice root.

**Changing them is a governed-backend change.** A new revision is ADR-0002's decision, taken by the
engineering and project owners per `docs/governance/ROUTING-TABLES.md` §Decision routing, and the
constants are updated from the new acquisition's `bundle-manifest.json` as part of it — never
edited to make a failing gate pass.

**The derived digest is also a synthesis-key input, from E1-S5.** It was not, and the reasoning for
that was recorded here: verification refuses unproven bytes outright, so a legitimate weights change
moves `PINNED_MODEL_REVISION` and the key with it, while an illegitimate one never renders. What
that leaves open is one case — a commit that edits `DECLARED_MODEL_ARTIFACTS` and does *not* move
`PINNED_MODEL_REVISION`. The gate proves the new bytes, the key stands still, and audio from the old
weights is reused for the new ones. `model_gate::model_artifacts_hash` closes it by making the key
follow the digests, which is issue #66. It needed an ADR-0001 §12.5 amendment, because that section
enumerates its key inputs and a record cannot add one:
`docs/adr/deviations/ADR-0001-D011-model-artifacts-key-input.md`, approved 2026-09-02.

---

## 3. Listening: the part no instrument may answer

ADR-0001 §17.5 makes a human listening review a gate condition. The machinery below exists to make
that judgment *honest* and *repeatable* — never to replace it.

### Render

```bash
cargo run --package study-tts-testkit --example listening-render -- \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

- The words come from `fixtures/listening/e1-s3-listening-script.json`, committed and registered in
  `docs/testing/TEST-DATA-MANIFEST.md`, so a retake reviews the same text and only the audio
  differs.
- **Every take is published through the cache before it is blinded**, so what a reviewer hears is
  the conditioned audio a build would assemble — padded, ramped, and validated — rather than the
  worker's raw output. An instrument that rendered straight to a file would hand the reviewer audio
  the conditioner had never seen, which is what this review exists to judge.
- The voice is resolved through the same rights gate a build passes, at
  `VoiceUse::VoiceQualification`: this material never reaches a lesson. **A governed `consent.json`
  whose `permitted_use` omits `voice_qualification` refuses the render**, and the consent record is
  what must change, not the request.
- **Every profile in the governed voice root is gated, not only the one that renders.** The worker
  deserializes all of them during `initialize`, so `WorkerConfiguration::for_bundle` runs
  `voice_gate::admit_voice_root` before it can hand back a launchable configuration. A revoked or
  altered profile left in the root refuses the run whether or not anything names it — move it out
  of the governed root, which is what the rights policy's revocation path asks for. **A profile
  directory whose name is not UTF-8 refuses the root too**, and is not skipped: the worker reads
  that name through Python's `surrogateescape` and would load the profile, so an entry Rust cannot
  spell is one it must refuse. Rename the directory.
- Takes are shuffled into `sample-NN.wav`. The mapping goes to `randomization-key.json`.
- `review-sheet.json` is written **pending**: five criteria and a disposition per sample, all
  `null`.
- **`--output-root` must not already exist.** The instrument refuses one that does, so a retake
  takes a new root and cannot overwrite the set an earlier review is still bound to.
- **No network namespace here**, unlike §2. This instrument asserts no offline property and
  produces audio for a person rather than evidence about a network, so wrapping it in `unshare`
  proves nothing. It does need `worker/.venv` restored, because it drives the real worker.
- **Render last.** Anything that changes conditioned bytes — `condition_edges`, the silence
  threshold, the model revision, a voice profile — makes an already-rendered set historical the
  moment it lands. A retake rendered before such a change is stale on arrival.

### Review

Play every sample. For each one answer all five criteria and set a disposition:

| Field | Meaning |
|---|---|
| `omissions_or_additions` | Words added or dropped against the written line |
| `pronunciation` | Anything said wrong |
| `voice_consistency` | Drift in timbre or identity |
| `pacing` | Rate, pauses, breath placement |
| `noise_or_artifacts` | Clicks, hum, glitches, truncation |
| `disposition` | `accept` or `reject` |

Also fill `reviewer`, `playback_environment`, and `reviewed_at`.

**Write `none` where you heard nothing. A blank is not a finding** — the renderer writes `null` to
mean "not yet answered", and `none` is how you say you listened and there was nothing there.

**`reject` is as complete an answer as `accept`.** The checker refuses an *unanswered* sample, never
an unfavourable one. A gate that only passed on approval would be pressuring the answer.

Do not open `randomization-key.json` first.

### Verify, then reveal

```bash
python3 scripts/qualification/check_listening_review.py <output root>/listening
```

It refuses, naming what to fix, when:

- a criterion or disposition is unanswered, or the review is unattributed;
- a sample's audio no longer hashes to what the sheet recorded;
- the key does not describe exactly the samples the sheet reviewed.

Only then does it print the mapping. Exit `0` means the review is complete **and still bound to the
bytes it was made against**.

### What the blinding does and does not enforce

Nothing stops someone opening the key early, and this project does not pretend otherwise — the
blinding is a discipline. What *is* mechanical is the binding: every judgment is recorded under a
take's SHA-256, so a review cannot be inherited by audio it was not made against. A retake cannot
even be rendered over the old set — the instrument refuses an `--output-root` that exists — so the
two sets sit side by side and the record says which one a verdict belongs to.

**Retake the review whenever the audio changes, not only when the text does.** Edge conditioning,
a model revision, a voice-profile change, or a threshold swap all produce different samples from
the same script.

### Retaking a review, and closing it out

A retake is owed whenever the conditioned bytes move. It is **not** owed for a change that cannot
reach audio, and the argument for that has to be written down and signed rather than assumed.

**What a retake does not drag with it.** The listening review and the `t5_` qualification result
answer different questions, so a retake of one is not automatically a retake of the other. But
deciding whether the qualification still holds is its own judgment, and there is one wrong way to
make it.

**The worker bundle identity is not a qualification identity.** It hashes the eight declared inputs
in `worker/bundle-manifest.json` — the schema, the launcher, the lockfile, and the Python package.
**No Rust source is among them**, and `qualification-result.json` records only that hash, the
network isolation, and the criteria. Nothing in it covers the runtime that drove the session. So an
unmoved `worker-bundle-hash` proves the *worker* is the one that was qualified and nothing at all
about the executor:

```bash
cargo run --package study-tts-runtime --example worker-bundle-hash
```

That matters because four of the five criteria are statements about the Rust side driving the
worker, not about the worker alone — one model load per lifetime, restart and offline start, output
contained in the staging root, and a clean protocol channel. `worker-qualification` drives them
through `WorkerConfiguration::for_bundle` and `WorkerTtsExecutor`, so a change to the executor, the
worker client, process supervision and containment, staging resolution, or protocol handling can
change exactly what those criteria measure while the bundle hash sits still.

Cache publication and edge conditioning are the useful counter-example: the instrument synthesizes
against a **placeholder cache key the executor never reads**, so neither is on the qualified path,
and a change to either is a listening-review question rather than a qualification one. That is the
shape a reachability argument takes — a claim about what the instrument actually drives, checkable
by reading it.

**Re-qualify whenever a change is reachable from the qualified session path**, whichever language it
is in, and say so in the record. The bundle hash answers criterion 1 and the question "is this the
same worker"; it is not evidence for the other four. A change that cannot reach that path — a
lesson-gate refusal, a CLI flag, documentation — needs no re-run, and the argument for that belongs
in the record rather than in an unmoved hash.

Closing this properly means the result recording a runtime identity of its own, so the comparison
is mechanical rather than a judgment repeated each time. Until it does, the judgment is the
operator's and has to be written down.

**After the checker exits `0`,** the review is complete but nothing has been recorded yet. The
story record is `Proposed` until its gate, so this is an edit in place rather than a reconciliation:

| Update | With |
|---|---|
| The listening-material section | The new output location, every sample's SHA-256, the completed sheet's digest, and **the bundle identity that rendered it** |
| The review-result section | The dispositions and any findings, plus what the review does not cover |
| The listening acceptance criterion | `Pending` → `Met` |
| Any limitation saying a retake is owed | Closed, naming the set that closed it |
| Any approval row deferring on audio | Now decidable |

Then re-run `python3 scripts/check-evidence-provenance.py`, because a record that cites a document
by digest has just been edited.

**An accepted record that carried an earlier review forward is spent, not wrong.** Its argument
stood for the change it was written about; a later change to the audio is outside it. Do not edit
it — §4 is the rule, and a new record beside it or superseding it is the mechanism.

---

## 4. Evidence: Proposed, then Accepted

A record is written **Proposed**. It grants nothing in that state — this is the whole mechanism, so
resist reading a Proposed record as a decision.

| Status | Meaning |
|---|---|
| `Proposed` | Written, not in force. Grants nothing |
| `Accepted` | In force. Its grants apply |
| Superseded | Replaced by a record that names it |

To accept one: set `- Status: Accepted` exactly, and fill the approvals table with a role, a name,
a decision that **names the risk accepted**, and a date. One person may hold both roles on a
personal project — `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits it and requires the rows
to stay separate, because the two roles accept different risks.

**Never edit an accepted record.** Write a new one that supersedes it, or — when the earlier grants
still stand and only something new needs accounting — a second record beside it.

### A story record is accepted at its gate, not when the story ends

`evidence/README.md` is explicit about this, and it is the rule most likely to be short-circuited
by a green test run. A story record stays `Proposed` while it accumulates findings, and is accepted
at the gate it serves.

### When provenance fails

Records pin the SHA-256 of the documents they rely on. Editing such a document makes
`check-evidence-provenance.py` exit `1`. Three ways out, in order of preference:

1. **Recompute and re-pin** — if the record is still `Proposed`, it is a draft and may be re-pinned.
2. **Write a reconciliation record** — a record whose id contains `reconciliation`, listing each
   moved path under `## Accounted provenance mismatches` and explaining why it moved. It suppresses
   those mismatches **only once accepted**.
3. **Supersede** the record whose citation moved.

A prose mention suppresses nothing. Neither does a Proposed reconciliation.

---

## 5. The order

```text
code gates ──► qualification ──► listening render ──► human review ──► checker
                    │                                                     │
                    └──────────────► transcribe both into the record ◄────┘
                                              │
                                     accept at the gate
```

Qualification comes before the listening render because it reports the worker bundle identity the
listening record must cite. The human review comes before the checker because the checker's job is
to verify a review, not to conduct one.

---

## Related

- `scripts/qualification/README.md` — the operator procedure for each instrument
- `evidence/README.md` — record layout, supersession, and the provenance rule
- `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` — what a version move costs
- `docs/governance/PROJECT-EXECUTION-CHARTER.md` — who may approve what
