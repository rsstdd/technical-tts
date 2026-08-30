# E1-S1 Provisional Contract Baseline Evidence v15

- Status: Proposed
- Supersedes: `e1-s1-provisional-contract-baseline-v14`

## Scope and decision

This record exists for one reason: correcting a false statement in `docs/INDEX.md` moved a
file `e1-s1-provisional-contract-baseline-v14` pins, so v14 can no longer be checked against
the working tree. It supersedes v14 for its controlled-record table. V14 remains the immutable
record of the bytes it read, and everything it concluded stands — nothing here contradicts it,
and this record reviews no code, because no code moved for it.

**While this record stands `Proposed`, `python3 scripts/check-evidence-provenance.py` reports
one mismatch — v14's pin of `docs/INDEX.md` — and accepting this record clears it by
superseding v14, exactly as v14 cleared v13's three.** A proposed record grants nothing until
then, so the branch carries that one mismatch in the meantime.

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
`Proposed` and the branch as carrying three unaccounted mismatches. v14 was accepted on
2026-08-30 and the check exits zero on the bytes those rows now describe. That record is
`Proposed` and no accepted record pins it, so those edits add no mismatch and need no
supersession; it stays `Proposed` until G1 for the reason its own §Open findings gives.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. Accepted when all four hold:

1. Every controlled record v14 pinned is checked again here from current bytes, with none
   dropped and none added.
2. No accepted predecessor is edited. v14's table is corrected by supersession, and the moved
   pin is the only reason this record exists.
3. Each statement the change replaces is false of the tree and each replacement is true of it,
   read against the records they describe rather than against memory.
4. No Rust source, published schema, worker protocol version, dependency, worker-bundle
   identity, or audio byte moved for this record, and no check v14 ran is reclaimed here
   without being re-run.

## Result

| Criterion | Result |
|---|---|
| 1. Table re-pinned whole | Met. Every row v14 carries is reproduced below by `python3 scripts/check-evidence-provenance.py --write` from current bytes; none dropped, none added. |
| 2. Correction by supersession | Met. v14 is untouched. This record declares `- Supersedes:` and takes effect only on acceptance. |
| 3. Statements read against their sources | Met. `e1-s2-evidence-provenance-reconciliation-v3` §Scope and decision states the five-round reading in its own words; `E1-S2-INTERFACE-CHANGE-002` §Identification numbers six rounds; the four moved files were compared against the commit that added v3 rather than inferred from the round they were named in; v14's §Status and §Review carry `Accepted` and six signed rows. |
| 4. Nothing else moved | Met. The change is three prose edits across two Markdown documents. §Verification run records what was run and names what was not. |

## Verification run

Run from the repository root on the branch working tree: WSL2, Ubuntu 24.04, CPython 3.12.3.
**This is not the ADR-0002 reference-machine protocol**, and this record makes no measurement
claim that would need it.

| Check | Command | Result |
|---|---|---|
| Provenance | `python3 scripts/check-evidence-provenance.py` | One mismatch, the expected one: v14's pin of `docs/INDEX.md`. It clears when this record is accepted and v14 is no longer checked. The table below was pinned by `--write` immediately before this row was written, which is the last point `repin_refusal` permits it |
| Script tests | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | Pass |
| Diff hygiene | `git diff --check` | Clean |
| Relative links | Every relative Markdown link in `docs/INDEX.md`, `evidence/gates/g1/e1-s2/e1-s2-canonical-lesson-workflow-v1.md`, and this record resolved against the tree | Clean |

Not re-run, and not reclaimed: formatting, Clippy, the Rust workspace suite, doctests,
documentation, published schemas, the Python worker suite, qualification-script tests,
`cargo deny`, and the worker-bundle identity. No byte those checks read moved for this record,
and v14 §Verification run remains the record of them. Hosted CI, the protected qualification
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
| `crates/study-tts-core/src/lesson.rs` | `5baf207afec480901d2fb8a4d627939bd73888aa491a14f92c415afcdfd036e3` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `f8ff5796dde3712c2f270ffadcc6e151320e4a7cb128a8c7d562367716f01556` |
| `crates/study-tts-runtime/src/error/mod.rs` | `64ffa8b990422c0e36e4a55bcb509dafb1e063925b12d64db8b08eca1dc046bf` |
| `crates/study-tts-runtime/src/lib.rs` | `6f71870f7f91313ce1a7969133d13f2a9195253b3008becbf645923c499a17c8` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `3ae6fe5e2f052f6febf6e41ebd44e21b55cbfb541e24bb3165f127fb01551cba` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `b85b819c29dd3fab9d45c3c5704007df9aa8af3a4930410c1da50d776909a6de` |
| `docs/INDEX.md` | `6bfe9a5111d7a504a8c2ccd33be83b8ab1b50b3e49f94203a948de262625269c` |
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
- **Not run on the reference machine.** No measurement is claimed; §Verification run names
  every check that was not re-run rather than omitting it.
- **v14's open items are carried, not closed.** `declared_superseded_ids` still lets a retired
  record grant, and the 0.90-second bundle-hash run v13 reports is still unexplained.

## Review

Ross Todd holds every role below. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that
for a personal project and requires each approval to name its role and accepted risk
separately, which is why the rows stay separate for one signatory. This record stands
`Proposed` until every row is signed.

| Role | Name | Decision sought | Date |
|---|---|---|---|
| Project owner | Ross Todd | Accept that a false statement in `docs/INDEX.md` about what an accepted reconciliation read is corrected in place, that the pin it moved is corrected by superseding v14 with no predecessor edited, and that the same class of overstatement in `E1-S2-INTERFACE-CHANGE-002` stays open and owed before G1 | 2026-08-30 |
| Engineering owner | | Accept that the table below is current bytes, that no code, schema, identity, or audio byte moved for this record, and that the checks v14 ran are cited rather than reclaimed | Pending |
