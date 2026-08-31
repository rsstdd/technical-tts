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
(cd scripts/qualification && python3 -m unittest discover --start-directory tests)
python3 -m unittest discover -s scripts/tests -p 'test_check_evidence_provenance.py'
python3 scripts/check-evidence-provenance.py
git diff --check
```

Doctests are a separate command because `--all-targets` excludes them. A check that did not run is
reported as not run, never as passed.

---

## 2. Qualification: what only the reference machine can answer

Some criteria cannot run in CI, because they need the real model, the governed roots, and the
restored `worker/.venv`. Those are the `t5_` names. `grep 'fn t5_'` across `crates/` returns
nothing on purpose: every one is an acceptance criterion discharged by an operator-run instrument
plus a record citing its hashed output.

```bash
cargo run --package study-tts-testkit --example worker-qualification -- \
    --bundle-root . \
    --model-root <governed model root> \
    --voice-root <governed voice root> \
    --output-root <fresh directory>
```

Writes `qualification-result.json` under the output root and prints its SHA-256. Copy it beside the
story record and cite that digest.

**Read it before committing it.** On a passing run it holds counts, criteria, and digests only. A
*failing* criterion can quote a path from the output root, and
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps governed locations out of Git.

The worker bundle identity alone needs no governed root:

```bash
cargo run --package study-tts-runtime --example worker-bundle-hash
```

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
- Takes are shuffled into `sample-NN.wav`. The mapping goes to `randomization-key.json`.
- `review-sheet.json` is written **pending**: five criteria and a disposition per sample, all
  `null`.

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
take's SHA-256, so a retake that renders new audio into the same directory cannot inherit the
previous review.

**Retake the review whenever the audio changes, not only when the text does.** Edge conditioning,
a model revision, a voice-profile change, or a threshold swap all produce different samples from
the same script.

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

```
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
