# E1-S1 Provisional Contract Baseline Evidence v15

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v14`

## Scope and decision

This record exists for one reason: correcting statements that were untrue of the tree moved
three files `e1-s1-provisional-contract-baseline-v14` pins, so v14 can no longer be checked
against the working tree. It supersedes v14 for its controlled-record table. V14 remains the
immutable record of the bytes it read, and everything it concluded stands — nothing here
contradicts it. One of the three is a Rust file, and the change in it is a test's failure
message: no behavior, no published byte, and no identity moves.

**This record was accepted on 2026-08-30 and is the record in force.** While it stood
proposed, `python3 scripts/check-evidence-provenance.py` reported three mismatches, all v14's
and all expected: its pins of `docs/INDEX.md`, `crates/study-tts-core/src/lesson.rs`, and
`docs/governance/TRACEABILITY-MATRIX.md`, each moved by a correction this record records.
Accepting it cleared all three by superseding v14, which is no longer checked — exactly as v14
cleared v13's three. No reconciliation row was asked for, and a baseline record could not grant
one anyway.

### What moved, and why

`docs/INDEX.md`'s E1-S2 story-evidence row credited
`e1-s2-evidence-provenance-reconciliation-v3` with accounting for its eleven pins "read against
every round of `E1-S2-INTERFACE-CHANGE-002`". That record's own §Scope and decision says
otherwise: it was written after the fifth review and states that "the reading now covers all
five rounds of E1-S2". `E1-S2-INTERFACE-CHANGE-002` §Identification records six, and the sixth
landed after v3 was accepted, moving four of the eleven files v3 accounts for —
`crates/study-tts-core/src/lesson.rs`, `docs/architecture/WALKING-SKELETON.md`, `AGENTS.md`,
and `docs/INDEX.md` itself. The row now says what v3 read.

The correction withdraws nothing and grants nothing. All eleven pairs cite
`e1-s1-provisional-contract-baseline-v13`, which v14 superseded, so they suppress nothing
whatever reading justified them — which the same index row already recorded, and which v14
§Accounted provenance mismatches records in full. What the correction removes is a reader's
reason to believe a suppression was examined against bytes nobody had read.

The same change corrects two statements in
`evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md` — §Result criterion 6 and the
`check-evidence-provenance.py` row of §Verification run — both of which still described v14 as
`Proposed` and the branch as carrying three unaccounted mismatches that v14's acceptance had
already cleared. Both rows now describe the state this change leaves: red twice, both times on
E1-S1 baseline pins rather than on any accounting E1-S2 owes, and green once this record is
accepted.
That record is `Proposed` and no accepted record pins it, so those edits add no mismatch and
need no supersession; it stays `Proposed` until G1 for the reason its own §Open findings gives.

**Two further corrections moved the other two files, both from the E1-S2 audit against
`DELIVERY-PLAN.md` §E1-S2.** Neither changes behavior:

- `crates/study-tts-core/src/lesson.rs` — inside
  `t2_e1_unicode_and_protected_terms_survive_round_trip`, a failure message read
  ``expect("`{case}` must be valid")``. `expect` takes a `&str`, so the brace was literal and a
  refused case named none of the ten `HOSTILE_TEXT` entries it could have been. It is now
  ``unwrap_or_else(|error| panic!("`{case}` must be valid: {error}"))``, which names the case and
  the located refusal. Verified by blanking one entry and reading the panic before restoring it,
  which is the only way a message change is actually checked.
- `docs/governance/TRACEABILITY-MATRIX.md` — the `Canonical reviewed lesson only` row named its
  controls in prose and named no test, where sibling rows name theirs. It now names the five
  `DELIVERY-PLAN.md` §E1-S2 tests and the code paths behind the controls. That second half also
  closes a one-sided mirror: `SegmentRole`, `DeliveryStyle`, `MIN_RECALL_RESPONSE_MS`, and
  `MAX_RECALL_RESPONSE_MS` name ADR-0001 §3.2, §5.1, §8.1, and §13.2 in their own doc comments,
  and until now no standing document named them back — only
  `docs/architecture/E1-S2-INTERFACE-CHANGE-002.md`, a change record. `rust-comment` requires
  both ends, the way `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
  already names `lesson.rs` for the ceilings.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all four hold:

1. Every controlled record v14 pinned is checked again here from current bytes, with none
   dropped and none added.
2. No accepted predecessor is edited. v14's table is corrected by supersession, and every pin
   this record moves is named in §Scope and decision.
3. Each statement or message the change replaces is wrong about the tree and each replacement is
   right about it, read against the records and the code they describe rather than against
   memory, with the message change reproduced rather than assumed.
4. No public Rust signature, published schema, worker protocol version, dependency,
   worker-bundle identity, or audio byte moved for this record — the one Rust edit is a failure
   message inside a `#[cfg(test)]` module — and no check v14 ran is reclaimed here without being
   re-run.

## Result

| Criterion | Result |
|---|---|
| 1. Table re-pinned whole | Met. Every row v14 carries is reproduced below by `python3 scripts/check-evidence-provenance.py --write` from current bytes; none dropped, none added. |
| 2. Correction by supersession | Met. v14 is untouched. This record declares `- Supersedes:` and takes effect only on acceptance; §Scope and decision names all three moved pins and why each moved. |
| 3. Statements read against their sources | Met. `e1-s2-evidence-provenance-reconciliation-v3` §Scope and decision states the five-round reading in its own words; `E1-S2-INTERFACE-CHANGE-002` §Identification numbers six rounds; the four moved files were compared against the commit that added v3 rather than inferred from the round they were named in; v14's §Status and §Review carry `Accepted` and six signed rows. For the two audit fixes: `expect` takes a `&str`, so the brace could not have interpolated, and the replacement was run against a deliberately blanked case to see the panic name it — a message asserted by a green test is a message nobody has read. The sibling rows of the traceability row were read for the convention it now follows. |
| 4. Nothing else moved | Met. Three prose edits across two Markdown documents, plus one failure message inside a `#[cfg(test)]` module. No public signature, published schema, worker protocol version, dependency, worker-bundle identity, or audio byte moves, and the suite count is unchanged at 308 because no test is added, renamed, or removed. §Verification run records what was run and names what was not. |

## Verification run

Run from the repository root on the branch working tree: WSL2, Ubuntu 24.04, CPython 3.12.3.
**This is not the ADR-0002 reference-machine protocol**, and this record makes no measurement
claim that would need it.

| Check | Command | Result |
|---|---|---|
| Provenance | `python3 scripts/check-evidence-provenance.py` | Clean, zero mismatches. While this record stood proposed the check reported three, and the three expected ones: v14's pins of `docs/INDEX.md`, `crates/study-tts-core/src/lesson.rs`, and `docs/governance/TRACEABILITY-MATRIX.md`. Accepting this record superseded v14, which is no longer checked, and all three cleared. The table below was pinned by `--write` immediately before acceptance, which is the last point `repin_refusal` permits it, and this row was written from the run that followed |
| Formatting | `cargo fmt --all -- --check` | Clean |
| Rust conventions | `python3 scripts/check-rust-conventions.py` | Clean, exit 0 |
| Lints | `cargo clippy --workspace --all-targets --all-features --offline --locked -- -D warnings` | Clean |
| Tests | `cargo test --workspace --offline --locked --all-targets` | 308 passed, 0 failed — unchanged from v14, as a message-only edit requires |
| Doctests | `cargo test --workspace --offline --locked --doc` | 7 passed |
| Failure message reproduced | One `HOSTILE_TEXT` entry blanked, the round-trip test run, then restored | The panic named the case and the refusal: ```precomposed` must be valid: `<test lesson>` at `/segments/0/spoken_text`: segment `seg-0001` has empty spoken_text``. Green again after restoring |
| Script tests | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | Pass |
| Diff hygiene | `git diff --check` | Clean |
| Relative links | Every relative Markdown link in `docs/INDEX.md`, `evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md`, and this record resolved against the tree | Clean |

Not re-run, and not reclaimed: documentation, published schemas, the Python worker suite,
qualification-script tests, `cargo deny`, the walking skeleton against real FFmpeg, and the
worker-bundle identity. The one Rust byte that moved is a failure message inside a
`#[cfg(test)]` module — it reaches no schema, no bundle input, and no encoder — and v14
§Verification run remains the record of those checks. Hosted CI, the protected qualification
workflow, real-model qualification, ASR, and listening were not run.

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
| `crates/study-tts-core/src/lesson.rs` | `ca38caa5ec2fefc502bc3f1ecb5a39ec62bd8a4331c648ffb9f48c8972ea10bc` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `f8ff5796dde3712c2f270ffadcc6e151320e4a7cb128a8c7d562367716f01556` |
| `crates/study-tts-runtime/src/error/mod.rs` | `64ffa8b990422c0e36e4a55bcb509dafb1e063925b12d64db8b08eca1dc046bf` |
| `crates/study-tts-runtime/src/lib.rs` | `6f71870f7f91313ce1a7969133d13f2a9195253b3008becbf645923c499a17c8` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `3ae6fe5e2f052f6febf6e41ebd44e21b55cbfb541e24bb3165f127fb01551cba` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `b85b819c29dd3fab9d45c3c5704007df9aa8af3a4930410c1da50d776909a6de` |
| `docs/INDEX.md` | `148c7040dd09db681719876e83539cfe5d9421dac5debe72546d923427a33d9f` |
| `.github/workflows/ci.yml` | `ff80cf2ec76731ab805c5ee6d5dad13c61b423359aac5f156508077be757cda3` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `19bae414902f6faf2feefac43ab4ba2f1071ae8b83aae6393ff5b40b3ed03050` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `f9eb7f1a193f38eac3ed91e3ae6753c886827db6a374ae0b94b2f1d2796c028b` |
| `crates/study-tts-testkit/tests/schemas.rs` | `69846239bf4a9101d0e359c9f293797520204c5050e41914b830a1e3fce9add2` |
| `docs/adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md` | `84ed5903193a95a4e8056cb6a7ae07f4ea17ca729f2f67846ec6bd26fe081957` |
| `scripts/check-evidence-provenance.py` | `f52c5cd5c4d7e879e38ef4ab133b0fe8ac52117c11d4406db5d2179d946f3a1b` |
| `scripts/tests/test_check_evidence_provenance.py` | `650b3dad1968049dcde03794ab68ad07fc88dc0c72b0d92bc7a3d20623dfa0c1` |
| `crates/study-tts-runtime/src/cache.rs` | `fbd81db2d214880383614471abc7f572dbbb22dffe9979b77b53cda6462c2013` |
| `crates/study-tts-runtime/src/distinct_map.rs` | `727bb6a2fa01a378adcfc9c01c7ba3bab903a952bf9b4ded8786fa628495892a` |
| `evidence/README.md` | `d28b0f752e84dd73b13c89bc07a4718c5b691054a4524b130abbc62b1e6dd052` |

## Accounted provenance mismatches

This record accounts for nothing, and cannot: the twenty-first audit restricted that power to
reconciliation records, and this is a baseline record.

Accepting it retires nothing further. The eleven pairs
`e1-s2-evidence-provenance-reconciliation-v3` carries were already retired when v14 superseded
v13, and they cite v13 rather than v14, so superseding v14 leaves them exactly as they are.

## Deviations and limitations

- **This record re-pins every file v14 pinned without re-reviewing the work that moved them.**
  Pinning from current bytes is what keeps the table from being stale on the day it is written
  — the defect v13 found in v12 — but the readings stay with their own records: v14's for the
  twenty-second audit, `e1-s2-evidence-provenance-reconciliation-v3`'s for the eleven files
  E1-S2 moved, and `E1-S2-INTERFACE-CHANGE-002`'s for the sixth and seventh reviews that v14
  §Deviations and limitations records pinning. Accepting this record accepts the table as
  current bytes, not that work as reviewed here.
- **`docs/architecture/E1-S2-INTERFACE-CHANGE-002.md` §Delivery and recovery carried the same
  overstatement, and this record does not correct it.** It says v3 "extends the reading to every
  round of this record", which the record's six numbered rounds make untrue for the same reason
  the index row was. That record was accepted and signed on 2026-08-30, and its own §Approval
  says a further correction "amends this record from outside, in a successor" — so the
  correction is `docs/architecture/E1-S2-INTERFACE-CHANGE-003.md`, written in the same change
  and `Proposed`. It is an architecture amendment, not an evidence re-pin: accepting this record
  accepts neither it nor the correction it makes.
- **The traceability row records routes; it mechanizes nothing.** Naming `SegmentRole`,
  `DeliveryStyle`, and the two recall constants in
  `docs/governance/TRACEABILITY-MATRIX.md` gives the mirror its second end, so a `grep` from
  either side reaches the other — but nothing fails if a vocabulary drifts from ADR-0001. The
  ceilings have the same property. A `t3_e1_*` test in the shape of
  `t3_e0_required_gates_match_the_release_profile_document` was considered and not written: the
  ADR states the roles as prose across three sections rather than as a transcribable list, so
  such a test would pin one hand-written table to another. Whoever moves those vocabularies next
  owns proposing it.
- **The E1-S2 audit that found these two defects is not re-reviewed here.** This record pins the
  bytes its fixes produced and attests the checks re-run over them; the reading of E1-S2 against
  `DELIVERY-PLAN.md` §E1-S2 belongs to `e1-s2-canonical-lesson-workflow-v1`, which stays
  `Proposed` until G1.
- **Not run on the reference machine.** No measurement is claimed; §Verification run names
  every check that was not re-run rather than omitting it.
- **v14's open items are carried, not closed.** `declared_superseded_ids` still lets a retired
  record grant, and the 0.90-second bundle-hash run v13 reports is still unexplained.

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that
for a personal project and requires each approval to name its role and accepted risk
separately, which is why the rows stay separate for one signatory. **Every row is signed on
2026-08-30: each states the decision this record asked for and the date it was made.**

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Project owner | Ross Todd | Accept that a false statement in `docs/INDEX.md` about what an accepted reconciliation read is corrected in place, that the pin it moved is corrected by superseding v14 with no predecessor edited, and that the same class of overstatement in `E1-S2-INTERFACE-CHANGE-002` stays open and owed before G1 | 2026-08-30 |
| Engineering owner | Ross Todd | Accept that the table below is current bytes; that the only Rust edit is a failure message inside a `#[cfg(test)]` module, reproduced against a deliberately broken case rather than assumed, leaving the suite at 308; and that fmt, conventions, Clippy, the workspace suite, and doctests were re-run here while every other check is cited to v14 rather than reclaimed | 2026-08-30 |
