# E1-S1 Provisional Contract Baseline Evidence v14

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v13`

## Scope and decision

This record exists for one reason: the **twenty-second audit** changed three
files `e1-s1-provisional-contract-baseline-v13` pins —
`crates/study-tts-runtime/src/worker_protocol.rs`,
`crates/study-tts-runtime/src/lib.rs`, and
`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` — so v13 can no longer be
checked against the working tree. It supersedes v13 for its controlled-record table and
verification run. V13 remains the immutable record of the bytes it read, and
everything it concluded stands: nothing here contradicts it.

**This record was accepted on 2026-08-30 and is the record in force.** While it
stood proposed, `scripts/check-evidence-provenance.py` reported three
mismatches, all v13's and all expected: its pins of
`crates/study-tts-runtime/src/worker_protocol.rs` and
`crates/study-tts-runtime/src/lib.rs`, which the audit changed, and of
`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`, which the audit was appended
to. Accepting this record cleared them by superseding v13, which is no longer
checked; no reconciliation row was asked for, and a baseline record could not
grant one anyway.

`docs/INDEX.md` gains a pointer to this record in the same change, naming it the
record in force, so a reader who reaches v13 through the index also reaches what
superseded it. Its own pin under v13 is a pair
`e1-s2-evidence-provenance-reconciliation-v3` already accounts for, so that edit
adds no mismatch.

### What the twenty-second audit changed

The audit is recorded in `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`
§What the twenty-second audit closed.

1. **A worker response naming one voice profile twice was accepted silently.**
   `WorkerInitializationIdentities::voice_profile_hashes` deserialized through a
   derived `BTreeMap`, which keeps the last value for a repeated key, so an
   `initialized` frame binding one profile to two digests parsed and kept
   whichever the sender wrote last. `worker/study_tts_worker/protocol.py`'s
   `_distinct_keys` refuses a repeated name in every object it reads, and
   `worker_protocol.rs`'s module documentation lists that refusal among the
   rules it says both ends apply — so the two ends disagreed exactly where the
   documentation said they agreed. Closed by a `MapAccess` visitor on that
   field, refusing a repeated name beside the empty-set refusal it already
   carried.
2. **A published cache entry naming one generation parameter twice was reused.**
   `ArtifactProvenance::generation_parameters` deserialized the same way, so the
   earlier binding was gone before `load_validated` recomputed the key — and the
   key recomputes from what the map kept, which is the published one. An entry
   edited to carry a second spelling of a parameter it already records therefore
   derived its own key, passed every check, and was handed back as a hit, while
   the record beside the audio no longer said one thing about what produced it.
   Closed by the same deserializer, now shared as `crate::distinct_map`.

Both are the silent-overwrite class `E1-S2-INTERFACE-CHANGE-002`
§Identification item 12 closed for lesson `speakers`, in the two map-valued
fields this build parses. The audit's §The sweep records every other map-valued
field that was read and why none of them needs the deserializer; every other
object in a frame or a record is a struct, where `serde` already refuses a
repeated field.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. This record is accepted when
all of the following hold:

1. Every controlled record v13 pinned is checked again here from current bytes,
   with none dropped.
2. Each of the twenty-second audit's two closed defects is reproduced against
   the code that carried it.
3. No accepted predecessor is edited. The correction is made by supersession.
4. Formatting, Clippy, Rust conventions, the full offline workspace suite,
   doctests, documentation, published schemas, Python worker tests, script
   tests, qualification script tests, dependency policy, Markdown links, and
   diff hygiene all pass, and every check this record did not run is named as
   unrun rather than omitted.
5. The worker-bundle identity is recomputed and shown not to have moved.
6. No public Rust API signature, wire field, published schema, worker protocol
   version, dependency, or audio byte changed.

## Verification run

Run on the branch working tree at the time of writing: WSL2, Ubuntu 24.04,
CPython 3.12.3. **This is not the ADR-0002 reference-machine protocol**, and
where a figure depends on that protocol it is reported as not comparable rather
than compared.

| Check | Command | Result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Clean |
| Red regression, repeated voice profile | The test written first, against the derived `BTreeMap` still in place | Failed: the frame parsed and `voice_profile_hashes` held one entry carrying the *second* digest, `3333…3333`. The shared deserializer turns the same frame into a refusal |
| Red regression, repeated cache parameter | The test written first, against the derived `BTreeMap` still in place | Failed by returning a `ValidatedCachedArtifact` for the edited entry — the tampered record was accepted and its audio reused, which is the defect in its strongest form |
| Tests | `/usr/bin/time -f 'elapsed=%e seconds' cargo test --offline --workspace --all-targets --locked` | 308 passed, 0 failed, 0 ignored; 6.37 s wall on a warm build. An earlier run including the rebuild of the changed crate took 23.51 s; neither is a reference-machine measurement |
| Lints | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Clean |
| Doctests | `cargo test --offline --workspace --doc --locked` | 7 passed |
| Documentation | `cargo doc --offline --workspace --no-deps --locked` | Passed without warnings |
| Published schemas current | `cargo run --offline --locked -p study-tts-runtime --example generate-schemas` then `git diff --exit-code -- schemas/` | No diff. JSON Schema has no vocabulary for a repeated object name, so this refusal is not expressible there and the file is unchanged |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean |
| Probe compiles | `python3 -m compileall -q -f crates/study-tts-runtime/src/runtime_probe.py` | Clean |
| Python worker | `python3 -m unittest discover --start-directory worker/tests` | 44 passed, unchanged: the Python end already refused a repeated name and needed no edit |
| Repository scripts | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | 20 passed |
| Qualification scripts | `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'` | 21 passed |
| Dependency policy | `cargo deny check` | Advisories, bans, licenses, and sources passed |
| Markdown links | Repository-wide relative-link scan over `*.md` | Clean |
| Diff hygiene | `git diff --check` | Clean |
| Worker bundle identity, three runs | `cargo run --offline --quiet --locked -p study-tts-runtime --example worker-bundle-hash` | All three returned `75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3`, unchanged from v13. Wall times of 36.07, 10.39, and 12.00 s include `cargo run` overhead and are **not** measurements against the `ADR-0001-D004` band, which times the compiled example |
| Provenance | `python3 scripts/check-evidence-provenance.py` | Clean, zero mismatches. While this record stood proposed the check reported three, and the three expected ones: v13's pins of `crates/study-tts-runtime/src/worker_protocol.rs`, `crates/study-tts-runtime/src/lib.rs`, and `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`. Accepting this record superseded v13, which is no longer checked, and all three cleared. The table below was pinned by `--write` immediately before acceptance, which is the last point `repin_refusal` permits it, and this row was written from the run that followed |

Hosted CI and the protected reference-machine qualification workflow were not
run. Real-model qualification, ASR, and listening were not run; this audit
changes no audio behavior or bytes.

## The identity that did not move

No file `worker/bundle-manifest.json` declares was touched — the change is one
Rust deserializer and one Rust test — so the worker-bundle identity stays at
`75d563103eccc76616ce97b66e2d4648b2a258cda1118e6ffc9ccc20b9d2bab3` and
`WORKER_BUNDLE_IDENTITY_VERSION` stays at `e1-s1-v4`. §Verification run records
the three recomputations rather than resting on the argument.

No cache entry, plan, or take could have been written under the corrected
shape: nothing in the build reads `voice_profile_hashes` before E1-S3, and the
only process that emits an `initialized` frame is `fake-ndjson-worker`, which
builds the map from a Rust `BTreeMap`. That is also `ADR-0001-D005` condition 3,
which the audit's §Compatibility and identity impact records in full.

## Controlled records

Every row v13 pinned is checked again here, with none dropped, and every digest
is reproduced from current bytes by
`python3 scripts/check-evidence-provenance.py --write` rather than copied
forward. Two rows are added, both files this audit's second finding rests on and
neither of which v13 carried: `crates/study-tts-runtime/src/cache.rs`, where the
entry was reused, and `crates/study-tts-runtime/src/distinct_map.rs`, the
deserializer both fields now read through. Everything else the audit rests on —
`worker_protocol.rs`, `E1-S1-INTERFACE-CHANGE-001.md`, `ADR-0001-D005`, the
shared contract fixture, and `worker/tests/test_protocol.py` — are rows v13
already carried.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `13caab365274df74ec965e03c7eeb1bacdcf2c007346f471985a7722c3bc4d3a` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `5a86e5f560c13bfabb9359a5f400534d56d7d05324d3b98563373014b1dd7d42` |
| `docs/architecture/WALKING-SKELETON.md` | `3af7bb28462f92c083484cf7c4a1ff9c483b2be79156771e9ec9f4b5b1c6f1e0` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `cf458da1d5330ca0c7258005aaa354d55e410798dec0617856a0f2257633ef15` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `c4399a4567aa9da37fd9f15b598efba19bfe001978f6467392bfb192aa531491` |
| `crates/study-tts-runtime/src/worker_protocol.rs` | `b2d5744e8dc6ac55ae398a6258e45072582f5515b82c42259092c7408f1796e9` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `ebade17f66cac6ec290a8f5cebe0a2688960e47e5ea2303d51ca6c077ddc556b` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `9d94ed6cc3238234dbd8c4ac5e59b07d66f07d6ce514b3b88eea4366b8fbfcc1` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `7b49ab56a27056d56a5bd49df0f5c3ce07cc14ea54f6a7981f18f6ac9b2640c1` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `22aa6c6a38facfa28723ca55cfb04ccced3a2476c11d8459da56592317a0783c` |
| `crates/study-tts-testkit/src/json_schema.rs` | `a6c7f0760387af239e905ef730ba2a3eac40a1cca1709dcc0edf1c1d18a5191b` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `0d0fd31542b3535cb857707f64e5ff71ddcbc3bd8940c10b87ff451cf3fe29e1` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `4ae160526713e44f8d168b54236fc0a857defdf2349e447f794fd56ffc6700ed` |
| `schemas/worker-protocol-v1.schema.json` | `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` |
| `worker/AGENTS.md` | `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` |
| `worker/bundle-manifest.json` | `cba489f1d3922c64997fb65c0930a0d8bbb2d542903c435e88b24c0d65fcb2a9` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `09678090fa92b77585de6f05adfbc665ed95f8fc45116e5894cd161bbb7dc7e6` |
| `worker/study_tts_worker/worker.py` | `0777f9b16a41e1c2db00c445229c04b48328bae7fafc6001174846aca0fc8bbf` |
| `worker/tests/test_protocol.py` | `e2dfeb70bc0e0be4d9f7e5de26768e95268cb227b41b9fea8c889efaed889178` |
| `worker/tests/test_worker.py` | `682f2d24c7db45bc0bac90aa4d37de72238f456203b8f2b1a06c3fa6b5aa7113` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `5baf207afec480901d2fb8a4d627939bd73888aa491a14f92c415afcdfd036e3` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `f8ff5796dde3712c2f270ffadcc6e151320e4a7cb128a8c7d562367716f01556` |
| `crates/study-tts-runtime/src/error/mod.rs` | `64ffa8b990422c0e36e4a55bcb509dafb1e063925b12d64db8b08eca1dc046bf` |
| `crates/study-tts-runtime/src/lib.rs` | `6f71870f7f91313ce1a7969133d13f2a9195253b3008becbf645923c499a17c8` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `3ae6fe5e2f052f6febf6e41ebd44e21b55cbfb541e24bb3165f127fb01551cba` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `b85b819c29dd3fab9d45c3c5704007df9aa8af3a4930410c1da50d776909a6de` |
| `docs/INDEX.md` | `fbff9df7d3850162f9dc0facf2cc3df21d5aa0a1742419d9a3fa6e79aeddf6fd` |
| `.github/workflows/ci.yml` | `ff80cf2ec76731ab805c5ee6d5dad13c61b423359aac5f156508077be757cda3` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `19bae414902f6faf2feefac43ab4ba2f1071ae8b83aae6393ff5b40b3ed03050` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `b8986e5bf77a76ad95fdb9268b98559d347df32360b837fdc8183f8455159cb4` |
| `crates/study-tts-testkit/tests/schemas.rs` | `69846239bf4a9101d0e359c9f293797520204c5050e41914b830a1e3fce9add2` |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | `84ed5903193a95a4e8056cb6a7ae07f4ea17ca729f2f67846ec6bd26fe081957` |
| `scripts/check-evidence-provenance.py` | `f52c5cd5c4d7e879e38ef4ab133b0fe8ac52117c11d4406db5d2179d946f3a1b` |
| `scripts/tests/test_check_evidence_provenance.py` | `650b3dad1968049dcde03794ab68ad07fc88dc0c72b0d92bc7a3d20623dfa0c1` |
| `crates/study-tts-runtime/src/cache.rs` | `fbd81db2d214880383614471abc7f572dbbb22dffe9979b77b53cda6462c2013` |
| `crates/study-tts-runtime/src/distinct_map.rs` | `727bb6a2fa01a378adcfc9c01c7ba3bab903a952bf9b4ded8786fa628495892a` |
| `evidence/README.md` | `d28b0f752e84dd73b13c89bc07a4718c5b691054a4524b130abbc62b1e6dd052` |

## Accounted provenance mismatches

This record accounts for nothing, and cannot: the twenty-first audit restricted
that power to reconciliation records, and this is a baseline record.

Accepting it does retire the eleven pairs
`e1-s2-evidence-provenance-reconciliation-v3` §Accounted provenance mismatches
carries, because every one of them cites `e1-s1-provisional-contract-baseline-v13`
and a superseded record is not checked. Those rows stop having an effect; the
reading that record gives of why each of those eleven files moved is unaffected
and stays the record of it.

## Deviations and limitations

- **This record re-pins the eleven files E1-S2 moved without re-reviewing
  E1-S2.** Pinning them from current bytes is what keeps this table from being
  stale on the day it is written — the defect v13 found in v12 — but the reading
  that justifies those eleven moves is
  `e1-s2-evidence-provenance-reconciliation-v3`, not this record. Accepting this
  record accepts the table as current bytes, not E1-S2 as reviewed here.
- **The audit's own §Approval is unsigned**, while this record is not.
  `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` §Approval still states the
  decisions it asks for and nothing more, and accepting this record does not
  grant them: this record attests bytes and a verification run, not the
  contract change that moved them.
- **Not run on the reference machine, and not to the `ADR-0001-D004`
  protocol.** The bundle identity is reproduced three times and agrees with
  v13, which is the claim made; the timings are not evidence about the band.
- **Hosted CI and protected qualification remain unrun** local follow-up
  evidence at their existing gates. `ADR-0001-D005` and its G1 expiry are
  unchanged by this audit, which is taken under it.
- **The refusal is `WorkerFrameError::Malformed`, not a variant of its own.**
  The audit records why; a reader who expects one distinct variant per violated
  invariant should read that section rather than infer an oversight.
- **Three rows this record pins moved for E1-S2's sixth review, not for this
  audit.** `crates/study-tts-core/src/lesson.rs` gained
  `LessonError::MalformedSourceContentHash` and the classifier that raises it;
  `AGENTS.md` and `docs/INDEX.md` gained the sentence and the row that describe
  it. All three are recorded in
  `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md` §Identification items 13 and
  14, accepted on the same day as this record, and none of them is a control
  this audit's conclusions rest on. Pinning them from current bytes is what
  keeps this table from being stale on the day it is written; reading them is
  `e1-s2-canonical-lesson-workflow-v1`'s.
- **`crates/study-tts-core/src/lesson.rs` moved once more, for a seventh E1-S2
  review, and this table pins the later bytes.** That review replaced the
  hand-written variant count in
  `t1_e1_each_lesson_invariant_has_a_distinct_error` with one derived from
  `LessonError`'s own declaration, because a count kept by hand agrees with any
  enum: a variant added together with its `field_of` arm left the assertion
  passing while no case exercised the refusal. It is a test-only change inside a
  `#[cfg(test)]` module — no behavior, no published format, and no identity
  moves — and like the rows above it is not a control this audit's conclusions
  rest on. It is pinned here rather than left stale because `repin_refusal`
  permits `--write` only while a record declares `Proposed`, so the choice was
  to pin current bytes or to accept a table already known to be wrong. Reading
  that change is `E1-S2-INTERFACE-CHANGE-002`'s, not this record's.
- **Two comment-only edits in the tree are not this audit's.**
  `crates/study-tts-core/src/identity.rs` and `crates/study-tts-core/src/lesson.rs`
  carry uncommitted prose corrections authored outside this work: the first
  replaces a stale claim that `sample_context` resolves a conditioning hash for
  one speaker and none for the other, which E1-S2 made untrue when it gave both
  speakers distinct digests; the second replaces a rendering example that read as
  stray backticks. Both were read against the code they describe and are
  accurate, and neither changes behavior — but the `lesson.rs` row below pins
  bytes that moved for a reason belonging to E1-S2 rather than to this audit,
  which the two bullets above record, and `identity.rs` is not a pinned row at
  all.
- **v13's open items are carried, not closed.** `declared_superseded_ids` still
  lets a retired record grant, and the 0.90-second bundle-hash run v13 reports
  is still unexplained. This audit touched neither.

## Review

Ross Todd holds every role below.
`docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
project and requires each approval to name its role and accepted risk
separately. **Every row is signed on 2026-08-30: each states the decision this
record asked for and the date it was made.**

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept that refusing a repeated voice-profile name narrows `e1.worker.1.0` under `ADR-0001-D005` rather than moving it, on the five conditions the audit records | 2026-08-30 |
| Engineering owner | Ross Todd | Accept one shared deserializer for both fields and the two T1 tests, each reproduced red, on the 308-test suite, and accept that the refusals stay `WorkerFrameError::Malformed` and `CacheEntryFault::UnparseableArtifact` | 2026-08-30 |
| Project owner | Ross Todd | Accept that this record corrects v13's table by supersession with no predecessor edited, and that accepting it retires the eleven accounted pairs citing v13 | 2026-08-30 |
| Worker owner | Ross Todd for T-WORKER | Accept that the worker-bundle identity is unmoved, reproduced three times off the reference-machine protocol, with hosted-CI and protected qualification reproduction still owed before G1 | 2026-08-30 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that no published cache entry is invalidated — none can carry a repeated name — that an entry edited after publication is now refused rather than reused, and that nothing reads `voice_profile_hashes` before E1-S3 | 2026-08-30 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-30 |
