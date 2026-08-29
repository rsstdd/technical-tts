# E1-S1 Provisional Contract Baseline Evidence v10

- Status: Accepted
- Supersedes: `e1-s1-provisional-contract-baseline-v9`

## Scope and decision

This record supersedes `e1-s1-provisional-contract-baseline-v9`, SHA-256
`57225a4b20072dc7c5fc2ff7014ee80ff357cc0eb48e5b7e1bc4ef5a79edf2c0`, for its
controlled-record table and verification run. V9 remains the immutable record of
the bytes it read. Everything it concluded about the seventeenth audit stands.

It exists for three reasons: to **correct a false statement v9 carries**, to
record a **governance cleanup** across the ADRs and planning documents, and to
re-pin the records the repository owner's commit `9b66fd4` moved.

### V9 states something untrue, and this record says so

V9 §Deviations and limitations reads:

> Three ADRs — `ADR-0003`, `ADR-0004`, `ADR-0005` — carry a trailing-blank-line
> change in the working tree that this audit did not make and did not revert.
> None is pinned by an unsuperseded evidence record, so none affects provenance.

**The first sentence is wrong.** Those three files carried a
`- **Status:** Proposed` → `- **Status:** Accepted` change, not a whitespace
change. The error was the seventeenth audit's: the diff was inspected by its
tail rather than in full, and the status hunks at the head were never read. The
claim then went into a record that was accepted, which is why it is corrected
here rather than edited there.

The second sentence happens to be true, and for a reason worth stating because
it is not the one v9 implies. `ADR-0004` **is** cited by an active accepted
record — `evidence_e0_model_and_voice_rights_records_complete_v3` §Controlled
records pins it — but that exact pair is listed under §Accounted provenance
mismatches in the accepted
[`e1-s1-evidence-provenance-reconciliation-v1.md`](e1-s1-evidence-provenance-reconciliation-v1.md),
which excuses the path permanently. **`ADR-0004` is therefore unwatched by
`scripts/check-evidence-provenance.py`, by accepted design.** That is a standing
blind spot, not a defect in the script, and it is why a status change to a
governing ADR produced a green provenance run. It is recorded here so it is not
rediscovered as a bug.

### What the cleanup changed

The three statuses were reverted to `Proposed`. Five documents already said so
and were left untouched, which is what confirms the revert was the correct
direction rather than one of two defensible readings: `docs/INDEX.md`,
`AGENTS.md`, `README.md`, `DELIVERY-PLAN.md` §754, and
`docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §45. Each ADR had also kept its
Proposed-era status tail — "Accepted; awaiting calibration", "Accepted; awaiting
E4 evidence" — and none had gained the date, approver, or decision table that
`ADR-0002` carries for a real acceptance.

Four documents were stale rather than wrong and were brought up to date:

| Document | Was | Now |
|---|---|---|
| `docs/INDEX.md` | named v5 as the current E1-S1 evidence | names this record and its chain, says what it corrects, and points at the reconciliation records beside them |
| `AGENTS.md`, `README.md` | "production schemas … **not** implemented" | E0-S4 and E1-S1 named; the claims still true are kept unchanged |
| `docs/testing/TEST-STRATEGY.md` | naming rule written `t<tier>_<epic>_` | `t<tier>_e<epic>_`, plus the contract and tier rules that were enforced but unwritten |
| `docs/governance/TRACEABILITY-MATRIX.md` | no row for `ADR-0001-D004` | one, as its own §Maintenance rule requires |

`.claude/skills/rust-review/SKILL.md` gained the durable half of the lesson
in §Commit `9b66fd4` below. §Visibility & architecture already required a
two-sided mirror to *exist*; it now requires a reviewer to read both sides and
check they **agree**, because a code condition weaker than the rule its own
document states is the finding that survives a mirror being present. §Traps that
have bitten this repo carries the concrete case: a moved function is not an
unchanged function, and proving a refactor preserved behavior answers "did I
break it", never "was it right".

That file is outside `scripts/check-evidence-provenance.py`'s watched
directories — `REPOSITORY_DIRECTORIES` covers `.github`, `crates`, `docs`,
`evidence`, `fixtures`, `schemas`, `scripts`, and `worker`, not `.claude` — so
it is recorded here in prose rather than pinned. A row for it would look
controlled and be silently ignored, which is worse than no row.
`docs/INDEX.md` §Start here names it as the code-review standard, so it is a
governed document that provenance does not watch. This is the same class of gap
as the `ADR-0004` accounting above.

`docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` gained an §Audit index: one
row per audit, what it closed, and whether it moved the bundle identity, read
from each section's own §What this moves. Every section is kept verbatim,
because superseded evidence cites them by name. The index also states that
audits 1–13 name `schemas/worker-protocol-v0.schema.json` correctly for when
they were written, and that the eleventh audit renamed it — which resolves the
only nonexistent path cited anywhere in the repository.

### Commit `9b66fd4` repaired a control that was weaker than its own document

Commit `9b66fd4`, "fix(runtime): enforce locked startup code ownership", landed
the seventeenth audit's module split together with a fix the audit should have
found and did not.

`check_startup_modules_are_accounted` treated a startup module as accounted for
when **any installed distribution** owned it — `module.owner.is_some()`. The
rule it implements had been written the other way since the fourteenth audit:
`docs/operations/WORKER-ENVIRONMENT.md` §Nor are their bytes, nor the modules
`site` imports by name says a module that executes "must be accounted for —
owned by a **locked** distribution's `RECORD`, or declared in
`worker/bundle-manifest.json`." An unlocked distribution shipping a
`sitecustomize` therefore satisfied the check while running code the lock never
named, in the process whose identity the check exists to describe.

**This is a code↔document mirror that had come apart, not a tightening beyond
spec.** Naming it that way matters: the document was already ratified, so the
repair restores the stated control rather than adding to it, and `ADR-0001-D004`
needs no amendment because the behavior D004 authorizes is the documented one.

The commit fixes both halves. `check_startup_modules_are_accounted` now takes
the lock's pins and requires the owner to be among them, and
`t4_e1_an_unaccounted_startup_module_is_refused_and_an_inert_one_is_not` drives
`[None, Some("unlocked-package")]` and asserts a locked owner *is* accounted
for — which is why the suite still reports 283 while covering a case it did not
cover before.

**The seventeenth audit reviewed this exact function and missed it.** The split
moved `check_startup_modules_are_accounted` verbatim and verified that behavior
had not changed, which was true and was the wrong question: a review that reads
a module against `rust-review` and `ponytail` and does not read it against the
document it mirrors will pass code that is weaker than its own rule. The two
existing tests both used an ownerless module, so the suite agreed. A reviewer of
this record should treat the seventeenth audit's coverage of `worker_environment`
as verified for structure and unverified for document conformance.

The verification run below was taken **after** that commit, so it covers the
committed code rather than the state the seventeenth audit measured.

## Acceptance criterion

Stated before the result, per `evidence/README.md`. V9's sixteen criteria carry
forward and were re-run. This record adds five, and is accepted when all
twenty-one hold:

17. The false statement in v9 is quoted, named as wrong, and corrected, with the
    reason it was made rather than only the fact that it was.
18. The three ADR statuses read `Proposed`, and the five documents that assert
    so are unmodified — verified by inspection, not by editing them to agree.
19. No relative link in any Markdown file is broken, and no active document
    cites a test name or repository path that does not exist. Historical
    references in superseded records and in the audit log are exempt and are
    identified as such.
20. The worker-bundle identity is unchanged from v9. No document edit may move
    it, and a change here would mean a bundle input was edited by mistake.
21. Every check v9 named passes on the committed code, with the provenance check
    reported honestly whatever its result.

## Controlled records

Every row v9 pinned is checked again here, with none dropped.

| Record | SHA-256 |
|---|---|
| `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` | `581c22ad07a0152eaa50c6f3cb25dc64654e3d3dffc9998a19c3b280563662c4` |
| `docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` | `e91244cf0dfb608dd0c0fb4207be9c4b1d209ddf2576d53c623020d8f160c4f1` |
| `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md` | `5d57eb15cb723a30fde82ae66843d5f2e14bd47a06ec061ef5931b1910e5fe8e` |
| `docs/architecture/WALKING-SKELETON.md` | `3cf0bde8e50913d41df3faf680af9d3013585214830231ec886fa01fd54b76cc` |
| `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` | `7367c36faea96c0bf18fb60b30ab23d6c994540f795c3f6963a1d8f86e981b53` |
| `docs/testing/TEST-DATA-MANIFEST.md` | `ec658d4018a543bb1779c641f4a6a51486a25cb24c8722c4fa3d8b96ef75dd96` |

The changed implementation records are pinned separately:

| Record | SHA-256 |
|---|---|
| `crates/study-tts-runtime/src/worker_protocol.rs` | `0c88ffbe69ba2fee3e8dc7b72191ce76bd2bc29ad44f4cc6fe7f5ba0af4da04e` |
| `crates/study-tts-runtime/src/schemas.rs` | `67815bd15c8cacb3c3a2dcf0738caac00d702bfaf13fe356e683eee65c2d5c17` |
| `crates/study-tts-runtime/src/worker_bundle.rs` | `e20732d22714ad6597e86f7ce3fc9b52a99de8d4776d264f97d08933851f08a7` |
| `crates/study-tts-runtime/src/worker_environment.rs` | `2609ae05b35d5ecc67b43b8f11dc04b0f80bfcdf70e23b54f2c72c731c4bef5e` |
| `crates/study-tts-runtime/src/runtime_probe.py` | `28e4b2128ee1632f735cb9b5dc66c46ab5734936b338ed034e04bcee01b24816` |
| `crates/study-tts-runtime/src/process.rs` | `166687371829e2181e5bd969a7da4814decf58e72e01960960b3888f20f96a88` |
| `crates/study-tts-testkit/src/bin/fake-ndjson-worker.rs` | `d6785d52d6714f53247d4b036d6cc4f021cf50a4b0e7a8546d786488e0d27bf0` |
| `crates/study-tts-testkit/src/json_schema.rs` | `97ba85b3fb0a057a088d634faab5d8a0cdf5c717bf9029f564f54d008572dbed` |
| `crates/study-tts-testkit/tests/worker_contract.rs` | `ebcf0783ad1b24b7e12ce6c1f665762c3eadde1a37583fada0d174cff14d464e` |
| `crates/study-tts-testkit/tests/provisional_contracts.rs` | `13db8042a1e889d30b83cbc4e62c8b779c6fed4082c357facab6bcfa65141044` |
| `schemas/worker-protocol-v1.schema.json` | `01b13fce85d2da99e64c8b5cf9df02fe0dcd8a1039f7085ea76e22be815e1e9d` |
| `worker/AGENTS.md` | `a4ffc7943a6fd2e1a0c4549a74b53980167528d7f5f51145517b55ca1475fadb` |
| `worker/bundle-manifest.json` | `2135f785f47f6e9bc21ef6e9d95e8b67b990c7f689c9f32c01aace55a0dd46a4` |
| `worker/study_tts_worker/__init__.py` | `ec6c3f2b5b286ce8a3845ea874536ccc9cf4cf490ac5cd38b9b3036a90ede19c` |
| `worker/study_tts_worker/protocol.py` | `da7baa5c48d6038c3537e6414614de9beedcdf2098abd74d5a70d105814b4c98` |
| `worker/study_tts_worker/worker.py` | `0777f9b16a41e1c2db00c445229c04b48328bae7fafc6001174846aca0fc8bbf` |
| `worker/tests/test_protocol.py` | `405e9c41787b6784374146b695e166ff2b9de5828ba259826e7078f99149a6fd` |
| `worker/tests/test_worker.py` | `682f2d24c7db45bc0bac90aa4d37de72238f456203b8f2b1a06c3fa6b5aa7113` |
| `fixtures/contracts/e1-s1-fake-worker-session.ndjson` | `a9f506941a72b6b3df7a02052550e59c81f1cc78563e495a2fb420466893ab9d` |
| `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` | `5644a6b9ce17379ec4aacaeaf869ec25568b6a4d1507d5f47d742f53d0ca5cbb` |
| `crates/study-tts-core/src/lesson.rs` | `b9370a7a08ef3bc1c1338a62e1126300cc0bb97a89d0a89c4d6dcfb7c88025d9` |
| `crates/study-tts-runtime/src/error/worker_bundle.rs` | `05aabe1d5f72208a77284fcdb7c3ff4e27d8cd55a8364df420232c262228ae2d` |
| `crates/study-tts-runtime/src/error/mod.rs` | `e0f305c82d6ffd5e33b0b66ccd30ee7d8e040158a8125095db1e9700c2eba094` |
| `crates/study-tts-runtime/src/lib.rs` | `9ee3fd43ac856b2a48154d1a7c18736cb3c147d72677eaaf8332aeca4b218d32` |
| `docs/operations/WORKER-ENVIRONMENT.md` | `fca66abe1a0cfaef9e95d8c5792a48e03d4d98c6bc2676134ec4d81fcee55afc` |
| `docs/adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md` | `50f8684a38a10a6c87dea9d1c3eb4cb189a13517a2231e2af1d4019aa08821bb` |
| `docs/INDEX.md` | `a1a2e39dd3e63d3b87e943b4dbf04cf5fe24fedd7edaa00b84517fe886708183` |
| `.github/workflows/ci.yml` | `46d04e26233013a37cf5abe960d0854bfe265280fea1213b3e3556a0c0212b79` |
| `.github/workflows/qualification.yml` | `2e66deced0e6bbf56149ddf8d0aa705464f8b16d9298960e92841659de833cdf` |
| `AGENTS.md` | `a561d78d628eba447d7013589f141a58fbc31118f0142955c710e78c90bcf8cf` |
| `README.md` | `cc269b5257519c81cdbe0eeb38aba6e2fa9bd836cfde6475a698c4d309deac63` |
| `docs/testing/TEST-STRATEGY.md` | `44a146e35a8224e93bc2207474222864333cba7bbb27e359737ae0a2854468ec` |
| `docs/governance/TRACEABILITY-MATRIX.md` | `e920ca58e8f345912ea7df5f067c29d3549bc3140063714b69e3c53d16cf49dc` |

Four rows are new — `AGENTS.md`, `README.md`, `TEST-STRATEGY.md`, and
`TRACEABILITY-MATRIX.md` — so this cleanup is itself controlled rather than
being the one change nothing watches. `worker_bundle.rs`,
`worker_environment.rs`, `runtime_probe.py`, and `error/worker_bundle.rs` move
for commit `9b66fd4`; `E1-S1-INTERFACE-CHANGE-001.md` and `docs/INDEX.md` move
for this cleanup. **No `worker/` file and no `schemas/` file moved.**

## Verification run

Ubuntu 24.04 under WSL2 on 2026-08-29, cargo 1.97.1, CPython 3.12.3, and
FFmpeg 6.1.1 on `PATH`, against commit `9b66fd4` plus this cleanup:

- `cargo test --offline --workspace --all-targets --locked` — pass, 283 tests,
  the same count v9 recorded. No test was added, removed, disabled, or
  weakened by this cleanup, which changes no code.
- `cargo run --example worker-bundle-hash` —
  `6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`, identical
  to v9. Criterion 20.
- Every relative Markdown link in the repository resolves: 0 broken.
- Test-name citation scan: no active document cites a name that does not exist.
  The names that do not resolve are `DELIVERY-PLAN.md` forward references to
  unimplemented stories, plus three renamed and one deleted name cited only by
  superseded records and by the passages that record the rename and deletion.
- Repository-path citation scan: one nonexistent path,
  `schemas/worker-protocol-v0.schema.json`, cited ten times in the audit log as
  correct history and now explained at the top of that document.
- `cargo fmt --all -- --check`, `cargo clippy --offline --workspace
  --all-targets --all-features --locked -- -D warnings`, `taplo fmt --check`,
  `cargo doc --offline --workspace --no-deps --locked`,
  `python3 scripts/check-rust-conventions.py`, and `git diff --check main` —
  pass, no warnings.
- `cargo deny check` — `advisories ok, bans ok, licenses ok, sources ok`.
- `python3 -m unittest discover --start-directory worker/tests` — pass, 42.
- `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` — pass, 11.
- `python3 -m unittest discover -s scripts/qualification/tests -p 'test_*.py'`
  — pass, 21.
- `python3 scripts/check-evidence-provenance.py` — pass, no unaccounted
  mismatch. This record's acceptance retires v9, whose rows for the six files
  commit `9b66fd4` and this cleanup moved were the only failures; every row
  pinned here was recomputed from current bytes after the last edit.

## Deviations and limitations

- **Two governed documents sit outside provenance coverage.** `ADR-0004` is
  excused by an accepted accounting row, per §Scope and decision, and
  `.claude/skills/rust-review/SKILL.md` is outside `REPOSITORY_DIRECTORIES`
  entirely. Both are named by `docs/INDEX.md` as authorities. Neither gap is
  closed here: the first would mean narrowing an accepted reconciliation row
  from a path to a digest pair, and the second would mean widening the script's
  watched set, which is a change to a control rather than to a document.
- **`ADR-0004` is outside provenance coverage**, per §Scope and decision. Any
  future change to it is equally unwatched. Closing that would mean narrowing
  the accepted accounting row to a digest pair rather than a path, which is a
  change to an accepted reconciliation record and is not made here.
- **Nothing mechanically ties a routing-row name to the table it names.** Every
  baseline since v8 has recorded this. A check that reads
  `docs/governance/ROUTING-TABLES.md` and refuses an unknown row remains the
  durable fix and is still not written.
- The fifteenth-, sixteenth-, and seventeenth-audit contract, engineering,
  worker/runtime, and affected-track reviews have not occurred, and this record
  does not grant them.
- This was a developer-machine run. The reference machine must reproduce the
  bundle hash before G1, and the self-hosted qualification workflow was not
  dispatched.
- The tier-duration CI step has still not run in CI; its numbers in v9 are a
  local rehearsal.
- V8's `Status: Accepted` over a §Review table of four Pending decisions is
  preserved, not repaired. V8 is immutable and superseded, so nothing reads it.
- Markdown Prettier was not available. Structure and relative links were checked
  mechanically instead.
- Real-model qualification, ASR, and listening were not run. This change reaches
  no speech backend and alters no audio bytes.

## Review

Ross Todd holds every role below.
`docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for a personal
project and requires each approval to name its role and accepted risk
separately, which is why each row records a different acceptance.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept that this record changes no contract: no wire shape, schema, or published format moved, and the three ADRs return to the status five other documents already asserted | 2026-08-29 |
| Engineering owner | Ross Todd | Accept the ADR revert, the four planning-document corrections, the audit index, and the review rule added to `rust-review`, on the unchanged bundle hash and the 283-test suite as the evidence no behavior moved | 2026-08-29 |
| Project owner | Ross Todd | Accept this record, including its correction of the false statement v9 carries and its account of the two provenance blind spots that remain open | 2026-08-29 |
| Worker/runtime owner | Ross Todd for T-WORKER | Accept commit `9b66fd4` as the repair of a code-weaker-than-document defect rather than a new control, and accept that the seventeenth audit's review of `worker_environment` is verified for structure and unverified for document conformance | 2026-08-29 |
| Affected-track reviewers | Ross Todd for T-RUNTIME | Accept that no runtime behavior changed in this record's own scope | 2026-08-29 |
