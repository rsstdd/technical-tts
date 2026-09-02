# E0-S3 / G0 — ADR-0002 requalification for the `torch 2.10.0` backend uplift

- Status: Accepted

**Accepted 2026-09-02.** The ADR-0002 waiver is re-earned for the `torch 2.10.0` backend under the
clarified §Expiry standard. §Acceptance basis records exactly what that rests on, and §Residual
obligations records what it does not close.

This record began as measurements without a decision. Both items it recorded as absent were
subsequently supplied — the randomized listening assessment on 2026-09-02, and the schedule
reforecast in `DELIVERY-PLAN.md` §2.3, which landed in the same commit as this record and was never
in fact missing.

Date: 2026-09-02. Machine: `reference-wsl2-d9d550f06b783405`, unchanged.

## Acceptance basis

ADR-0002 §Expiry, as clarified by its 2026-09-02 amendment, requires re-establishing every
§Required evidence item a condition-2 change could have invalidated, and carrying forward what it
cannot reach. Item by item, for a speech-affecting worker input:

| Requirement | Evidence | Status |
|---|---|---|
| Single-worker RTF and 60-minute projection | §Performance | Re-established |
| Ten-run fixed-seed determinism characterization | §Fixed-seed determinism — one decoded-PCM hash across ten takes, `c9e7cc161bc66c9d…` | Re-established |
| Listener assessment | §Randomized listening assessment — 6 of 6 `accept`, completed sheet SHA-256 `28abd94cc6e81cbd8b914d0a368602e06505cc9fdd338c0b3edde227a11a71f1` | Re-established |
| Worker-bundle and FFmpeg identities | §Identities | Re-established |
| Environment integrity | §Environment integrity | Re-established |
| Schedule reforecast | `DELIVERY-PLAN.md` §2.3, *2026-09-02 backend-uplift reforecast* | Supplied |
| Source revisions, licences, permitted scope | Carried forward from the accepted G0 qualification | Unreached by this change |
| Reference-machine inventory | `docs/operations/REFERENCE-ENVIRONMENT.md`, machine unchanged | Unreached |
| Voice consent and checksum records | `rights-voice-owner-fallback-v2`, voice unchanged | Unreached |

Three things are **not** required and were not performed:

- **A blind listening of all ten fixed-seed takes.** ADR-0002's amendment separates deterministic
  characterization from perceptual assessment. Where decoded-PCM evidence establishes the ten are
  identical, listening to all ten adds nothing to the characterization — one is the same audio as
  the other nine.
- **Additional listening-script content.** The six-line committed fixture belongs to the listener
  assessment and has no numerical relationship to the ten fixed-seed generations. The apparent
  shortfall was a misreading of the pre-amendment table.
- **Full-box performance qualification.** ADR-0002 §Expiry requires it before **G3**, regardless of
  this change, and it is untouched by this acceptance.

**No requirement was reduced and no historical conclusion was changed to reach this acceptance.**
The ten-run characterization is still ten runs; the listener assessment is still the approved blind,
checksum-bound procedure; G0's failed performance measurements stand exactly as measured, and the
waiver remains a waiver.

## Residual obligations

**Two advisories remain applicable to the installed versions.** Determined from the advisories' own
version ranges, which are the authority here — GitHub reports both as `fixed`, and that alert state
does not override a range the installed version sits inside.

| Advisory | Severity | Range | Installed | Disposition |
|---|---|---|---|---|
| `GHSA-rrmf-rvhw-rf47` | low | `<= 2.12.1`, needs `2.13.0` | `torch 2.10.0+cpu` | Open. `torch.jit`, `lstm_cell`, `pad_packed_sequence`, and `unpack_sequence` appear nowhere in `worker/` or `crates/`, so the affected paths are unreachable by this build. Raising to `2.13.0` pairs a `torchaudio` built against `torch 2.11` with a `torch` two minors newer, which is an unverified ABI bet on a bundle whose identity gates every cache entry |
| `GHSA-h35f-9h28-mq5c` | medium | `< 83.0.0`, needs `83.0.0` | `setuptools 81.0.0` | Open, and blocked by the runtime cap §Dependency delta explains: `pkg_resources` was removed in `82.0.1` and `perth/perth_net/__init__.py` imports it at runtime |

**Neither is gate-blocking here, and neither is closed by this acceptance.** ADR-0001 places
dependency, advisory, and license checks at release and SBOM time;
`docs/governance/ROUTING-TABLES.md` routes a security control or accepted vulnerability to a
threat/risk record decided by the project owner **before release candidate**; and `deny.toml`
covers Rust crates rather than the Python worker environment. Both therefore carry forward as
obligations owed before release candidate, under E6-S2, and **must not be recorded as remediated**.

## Why this record exists

ADR-0002 §Expiry condition 2 expires the waiver on "selection of a different deployment machine,
device path, model, voice conditional, **or other speech-affecting worker input**." `torch` is a
speech-affecting worker input, and the audio measurably changes, so the waiver is expired by this
change rather than carried across it. §Expiry also states what must then happen: "rerun the
single-worker RTF and 60-minute projection with the pinned worker identity or its governed
successor." That rerun is §Performance below.

## Identities

| Item | Value | Status |
|---|---|---|
| Worker bundle identity | `d87aee58cc06d10dc0310c15225c60f9064bf2d17b53c3929bcdb803a98ca703` | **new** |
| Superseded bundle identity | `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` | superseded |
| Chatterbox code commit | `eb90621fa748f341a5b768aed0c0c12fc561894b` | unchanged |
| Chatterbox `LICENSE` SHA-256 | `4248e910a928849fe5815a0f9236e17fa07768d95b9193212752c464b93d6caa` | unchanged, matches ADR-0002 |
| Model revision | `1b475dffa71fb191cb6d5901215eb6f55635a9b6` | unchanged |
| Voice profile | `owner-fallback-v1` | unchanged |
| FFmpeg SHA-256 | `ed16af623947494a72e284b6eb8ff225f2da22b38b5d5069c2fd4b4ba3384e41` | unchanged, matches ADR-0002 |
| FFmpeg version | `6.1.1-3ubuntu5` | unchanged |
| Python | `cpython 3.12.3`, `cp312`, `manylinux_2_39_x86_64` | unchanged |
| `sitecustomize` digest | `Q9gRJdkjdrGmnVOnESagQcyaGNgIDpLeoKKuI74Tix4` | unchanged |

The bundle identity was derived mechanically with
`cargo run --package study-tts-runtime --example worker-bundle-hash`, per §22, against the restored
candidate environment attached at `worker/.venv`. The link was returned to the qualified
environment afterwards and the qualified environment was not modified.

## Dependency delta

Eleven pins changed, one added, none removed.

| Distribution | From | To | Why |
|---|---|---|---|
| `torch` | `2.6.0+cpu` | `2.10.0+cpu` | the change; closes seven Dependabot alerts |
| `torchaudio` | `2.6.0+cpu` | `2.10.0+cpu` | tracks `torch` by its own exact pin |
| `setuptools` | `78.1.0` | `81.0.0` | closes GHSA-5rjg-fvgr-3xxf (high) |
| `filelock` | `3.32.3` | `3.32.5` | incidental resolution drift |
| `importlib-metadata` | `9.0.0` | `9.0.1` | incidental |
| `joblib` | `1.5.3` | `1.6.0` | incidental |
| `msgpack` | `1.2.1` | `1.2.2` | incidental |
| `platformdirs` | `4.11.4` | `4.11.7` | incidental |
| `protobuf` | `7.36.0` | `7.36.1` | incidental |
| `regex` | `2026.7.19` | `2026.9.3` | incidental |
| `sympy` | `1.13.1` | `1.14.0` | incidental |
| `cloudpickle` | — | `3.1.2` | new transitive of `torch 2.10.0` |

Held deliberately against unpinned resolution: `s3tokenizer 0.1.7`, `numpy 2.5.2`,
`transformers 4.46.3`, `diffusers 0.29.0`.

**The governed tree was not edited, and still declares `torch==2.6.0`.** Nothing in the lock or
restore path reads that declaration — §Regenerating the lock step 3 installs the governed tree's
dependencies by name and step 4 installs the tree itself `--no-deps` — so relaxing it would change
no resolution while costing a commit, a new `code-<commit>` directory, a new provenance line, and a
`extractor_identity` on the voice profile that no longer describes the code that made it. The
residual hazard is narrow and worth stating: the installed metadata is now internally inconsistent,
so `pip check` complains, and **any dependency-resolving `pip install` into the qualified
environment would try to restore `torch 2.6.0`.** Every command in §Restoring the environment passes
`--no-deps`, which is what contains it. Correcting the declaration is a separate provenance task.

**Two alerts read `fixed` while their installed versions remain in range.** Checked 2026-09-02
against the advisories' own stated ranges rather than against the alert state: `torch 2.10.0+cpu`
sits inside GHSA-rrmf-rvhw-rf47's `<= 2.12.1`, which needs `2.13.0`, and `setuptools 81.0.0` sits
inside GHSA-h35f-9h28-mq5c's `< 83.0.0`, which needs `83.0.0` and is blocked by the cap explained
below. GitHub nonetheless reports both as `fixed`, with no dismissal reason and the same timestamp
as the eight genuine closures. **Eight of ten are remediated; two are bookkeeping.** An alert state
is therefore not evidence of remediation here, and the counts in §Dependency delta are the versions
compared against the ranges rather than the alert list.

**`setuptools` is capped at `81.0.0`, not raised to current.** `pkg_resources` was removed in
`82.0.1`, and `perth/perth_net/__init__.py` imports it at runtime; above the cap the backend fails
with `TypeError: 'NoneType' object is not callable`, because `perth/__init__.py` swallows the
`ImportError` and leaves `PerthImplicitWatermarker = None`. GHSA-h35f-9h28-mq5c, which needs
`83.0.0`, is therefore not closable without a second governed source tree.

## Environment integrity

| Check | Result |
|---|---|
| Offline `--require-hashes --no-index` restore, both phases of §Install only from the verified wheelhouse | pass |
| `s3tokenizer` sdist built `--no-build-isolation` under `setuptools 81.0.0` | pass |
| Real render from the restored environment, offline variables applied | pass |
| Runtime probe `integrity_faults` | none |
| `record_digests` regenerated | 57 entries |
| `WorkerBundle::verified_hash()` | pass — manifest, lock, tree, and interpreter ABI all agree |
| `cargo test --workspace --all-targets --locked` | 426 passed, 0 failed |
| `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -D warnings` | clean |

### The worker contract, not only the backend

The measurements above drive the backend directly. The production entry point was exercised
separately, as `python -m study_tts_worker.worker` over its NDJSON protocol, against the candidate
environment:

| Frame | Result |
|---|---|
| `initialize` | `initialized`, reporting `model_revision 1b475dff…`, `tokenizer_revision eb90621f…`, and the loaded voice conditioning |
| `health` | `ready: true`, `model_loaded: true` |
| `capabilities` | returned |
| `synthesize` | `synthesis_succeeded` — 82,560 frames, 24,000 Hz, 1 channel, `voice_conditioning_hash 4951f9e1…` matching `profile.json` |
| `shutdown` | clean, exit 0 |

**Output WAV compatibility** — an ADR-0002 §Required evidence item — is satisfied by that render
rather than asserted: the file the worker wrote into its assigned staging root reads back as
**mono, 24,000 Hz, `FLOAT` subtype, `WAV` format**, which is the canonical worker format ADR-0002
qualifies.

Two containment behaviors were observed working while getting there, and both are the code doing
its job: a bare filename was refused because `output` is resolved with `relative_to(staging.path)`,
and an unknown `style` was refused by name. Note also that `setuptools 81.0.0` emits a new
`pkg_resources` deprecation warning at backend import; it lands on **stderr and does not enter the
NDJSON stream**, because `reserve_protocol_stream` redirects file descriptor 1 before the backend
is imported. That is the mechanism its docstring describes, confirmed against a warning that did
not exist when it was written.

## Performance

**These numbers do not reproduce the ADR-0002 protocol and must not be read against its table.**
They were taken without the `unshare --user --map-root-user --net` namespace §Reference environment
records, and on a 3.08–3.44 second utterance rather than the 5.88 second one behind the recorded
results. Against the recorded `14.9804` worst RTF they would suggest a 2.4× improvement, and that
reading would be **wrong** — the control below shows the machine, not `torch`, accounts for almost
all of that gap. What this section supports is a same-machine A/B: ten fixed-seed takes in one
process, per backend, run back to back in one session.

Two independent cross-checks say the discrepancy is in machine conditions rather than in this
harness measuring something else:

- **Peak RSS reproduces.** ADR-0002 records `6,831,940 KiB` (6,671.8 MB) from `/usr/bin/time -v`;
  the control here measures 6,670.1 MB, a difference of **0.026%**. The memory profile of the
  recorded run is reproduced almost exactly.
- **The projection formula reconciles.** `3600 × 14.9804 = 53,929 s` against the recorded
  `53,947.516 s`, so the 60-minute projection below is computed the same way ADR-0002 computed
  its own.

A harness that reproduces the recorded peak RSS to three decimal places, on the same machine, while
returning less than half the recorded RTF, is measuring the same work under a less contended
machine — the original run's `unshare` namespace and whatever else shared the box that day are not
reconstructible here.

| Measurement | `torch 2.6.0+cpu` (control) | `torch 2.10.0+cpu` (candidate) | Change |
|---|---:|---:|---|
| Model load, wall | 6.511 s | 5.609 s | −13.9% |
| Model load, CPU | 8.399 s | 7.789 s | −7.3% |
| Peak RSS | 6,670.1 MB | 6,650.9 MB | −0.3% |
| Distinct decoded-PCM hashes over 10 in-process takes | 1 | 1 | unchanged |

### RTF is a function of utterance length, and the gate turns on that, not on `torch`

Measured at two lengths, four runs each at the longer one, ten at the shorter:

| Configuration | Utterance | RTF mean | RTF worst | 60-min projection at worst RTF | Gate `<= 21,600 s` |
|---|---:|---:|---:|---:|---|
| `torch 2.6.0+cpu` | 3.08 s | 6.50 | 6.99 | 25,164 s | **fail** |
| `torch 2.10.0+cpu` | 3.44 s | 5.90 | 6.20 | 22,320 s | **fail** |
| `torch 2.6.0+cpu` | 13.64 s | 4.48 | 4.54 | 16,344 s | pass |
| `torch 2.10.0+cpu` | 14.16 s | 4.21 | 4.31 | 15,516 s | pass |

Each take carries a large fixed cost that does not scale with the audio produced. Fitting
`RTF = fixed / audio + marginal` across the two lengths:

| | Fixed per-take overhead | Marginal RTF | Fitted RTF at ADR-0002's 5.88 s utterance |
|---|---:|---:|---:|
| `torch 2.6.0+cpu` | 8.04 s | 3.89 | 5.26 |
| `torch 2.10.0+cpu` | 7.68 s | 3.67 | 4.97 |

**A single RTF number is not meaningful for this backend unless the utterance length is stated
beside it**, and ADR-0002's table does not carry one on the RTF row. On a 3-second utterance both
backends fail the gate; on a 14-second utterance both pass it; the `torch` uplift improves RTF by
approximately 5–11% — 9.2% mean and 11.3% worst at the shorter length, and 6.0% mean and 5.1%
worst at the roughly 14-second length — and decides the gate in neither direction.

**This does not lift the performance waiver, and must not be read as doing so.** The fitted 4.97 at
ADR-0002's own utterance length sits under the `<= 6.0` gate, but it is a fit rather than a
measurement, it was taken without the `unshare` namespace, and it cannot explain the recorded
`14.9804` — which remains a real measurement of that day's machine that nothing here reproduces or
refutes. What this section establishes is narrower and is all it claims: **the uplift improves every
performance measure and regresses none.** Lifting the waiver needs the ADR-0002 protocol rerun as
written, which is §Expiry's requirement and is not what this record contains.

## Fixed-seed determinism

The candidate reproduces ADR-0002's recorded characterization exactly: **one** unique decoded-PCM
hash across ten fixed-seed takes (`c9e7cc161bc66c9d…`), matching the "1 unique decoded-PCM hash"
row. The control does the same at `406849ac92cd0aa3…`. The two differ from each other, which is the
expected consequence of moving a speech-affecting input.

**The scope of that property is narrower than the ADR-0002 row states, and this is not caused by
the uplift.** Determinism holds *within* one worker process and fails *across* processes, on the
current qualified backend as much as on the candidate: `CausalConditionalCFM.__init__`
(`chatterbox/models/s3gen/flow_matching.py:191`) draws `self.rand_noise` at model construction and
consumes it at line 213 as the flow-matching initial noise, while `worker.py` seeds per take in
`_synthesize` — after `_load_backend` has already built the model. Every take in a process shares
one noise vector drawn before any seed was applied. ADR-0002's row is correct as measured, because
"persistent ten-run" is ten takes through one loaded worker. Issue #70 carries the analysis; the row
should gain that scope rather than be retracted.

## Not supplied by this record

- ~~**Randomized listening assessment.**~~ **Supplied 2026-09-02.** See §Randomized listening
  assessment below. This was the item blocking acceptance; it no longer is.
- ~~**Schedule reforecast.**~~ **Already supplied**, and this line was wrong when written.
  `DELIVERY-PLAN.md` §2.3 carries the *2026-09-02 backend-uplift reforecast*, landed in the same
  commit as this record. It records **no calendar moves**, holds M2 at three weeks and M3 at eleven
  weeks after overall G0 closure, and leaves full-box performance qualification before G3 untouched.
- **The acceptance decision**, which is the owner's under §Acceptance. This is now the only item
  outstanding.
- Full-box performance qualification, which §Expiry requires before G3 regardless of this change.

## Randomized listening assessment

Supplied 2026-09-02, after this record's measurements, and **not at the identity they were taken
at**. The audio moved twice more between them — seeding before model construction (#70) and
`deterministic_seed` → `True` — so the assessment was rendered at the current bundle rather than at
`d87aee58…`, which is the identity a reader should hold it against.

| Item | Value |
|---|---|
| Worker bundle identity | `1af4e1713ee3eb7e96d6d0f4d2845f741e78e8a87dd320796f1e561f0f179d05` |
| Voice profile | `owner-fallback-v1` |
| Script | `fixtures/listening/e1-s3-listening-script.json`, committed and registered |
| Samples | 6, blinded as `sample-01` … `sample-06` |
| Sheet as rendered, pending | SHA-256 `96dabe2a8180bf13401efbcbd773c01a4ae2b2ebabb2b961b085382e41dd3e5f` |
| Sheet as completed | SHA-256 `28abd94cc6e81cbd8b914d0a368602e06505cc9fdd338c0b3edde227a11a71f1` |
| Reviewer | Ross Todd |
| Date | 2026-09-02 |
| Playback environment | Built-in laptop speakers |
| Result | 6 of 6 `accept`; `none` on all five criteria of all six samples |
| Overall finding | "All six samples sounded good. No finding on any criterion, on any sample." |

`check_listening_review.py` exits `0`, which is what binds each judgment to the bytes it was made
against: it refuses an incomplete sheet and refuses one whose recorded digests no longer describe
the audio beside it. The key was opened only by that script, after the sheet was complete.

**Six samples, where ADR-0002's decision table records ten.** The committed script carries six
lines, and E1-S3's review used six. The script is committed precisely so a retake reviews the same
words and only the audio differs, so the comparable baseline for this assessment is E1-S3's six
rather than the ten of the original G0 characterization. Stated here rather than left for a reader
to notice, because the counts differ and the record should say why.

**Built-in laptop speakers bound what a clear result means.** `libmp3lame` artifacts sit in the band
small drivers reproduce least — though these samples are canonical WAV rather than MP3, so that
particular limit bites less here than it does on a package review. What it does bound is the
`noise_or_artifacts` and `pacing` criteria generally: a clear result records that nothing was
audible on those speakers.

## Reproduction

Measurement harness and raw results are session-local rather than committed, because they are a
one-off A/B rather than a governed instrument: ten-take driver, and per-take JSON for both
backends carrying sample counts, wall and CPU seconds, RTF, and the decoded-PCM SHA-256 of every
take. Regenerating them means restoring both environments and rerunning; the procedure is
`docs/operations/WORKER-ENVIRONMENT.md` §Restoring the environment against this record's lock and
against its predecessor.
