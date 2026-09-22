# E2-S5 Interface Change 001 — The `study-tts` command surface

## Identification

- Record ID: `E2-S5-INTERFACE-CHANGE-001`
- Status: **Proposed.** §Approval carries the rows a signature would fill. Until it is signed this
  record authorizes nothing; it describes a change that is already on `main`, and says so.
- Contract owner: T-CLI (the `study-tts` command surface)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CLI, T-CORE, T-AUDIO
- Accepted ADR, if architectural: none. ADR-0001 §7.3 names the commands and the seven exit-code
  classes; this record declares which of them ship, what their numbers are, and what the E1-S5
  surface's exit status became. It changes no architecture.

`E1-S5-INTERFACE-CHANGE-001` published `lesson new` and `lesson validate`, fixed their exit status
at "`0`, or `1` for every refusal", and recorded in its §Limits that "`DELIVERY-PLAN.md` E2-S5 owns
`--json` and stable numeric exit classes, and publishing either here would be a contract that story
did not choose." This is that story, and this is the record that clause delegated.

## Version and compatibility

### `study-tts lesson new` / `lesson validate` — exit status and `--json`

| | Before (`E1-S5-INTERFACE-CHANGE-001`) | After |
|---|---|---|
| Exit status | `0`, or `1` for every refusal | `0`, or the ADR-0001 §7.3 class of the refusal, per the table below |
| `--json` | Not accepted | Accepted, global; one envelope on standard output |
| Stdout on success | `created lesson scaffold: <path>` / `valid lesson: <path>` | Unchanged without `--json` |
| Stderr | The refusal | The refusal, plus a `try:` line when a routing row names a command |

**Breaking contract**, by the standard the E1-S5 record set for itself when it retired a pinned
stdout sentence: a caller that tested `$? -eq 1` breaks. A caller that tested nonzero does not,
which is why `1` is left unassigned below. There is no version number to move, because a command
surface carries none; the charter row is `lesson` / `3.1`, which is the document and is unchanged.
`--json` is a compatible extension of both commands.

### Exit-code classes — `crates/study-tts-cli/src/exit.rs`

ADR-0001 §7.3 fixes seven names and no numbers. The numbers are this record's, in the ADR's
listing order from `2`, so `1` stays what every shell and harness already reads as "it failed"
without claiming which way.

| Code | Class | What the operator does next |
|---|---|---|
| `0` | success | — |
| `1` | *unassigned* | Left to wrappers that never learned this table |
| `2` | invalid input | Fix what they typed or authored: lesson, takes, voice, rights, a publication claim, a path they named |
| `3` | missing dependency | Install or restore: a tool, a governed model artifact, a worker bundle |
| `4` | incompatible environment | The governed root cannot host the work: containment, durable state, a filesystem refusal |
| `5` | worker failure | The synthesis worker failed, was refused, or a cache entry it produced did not validate |
| `6` | audio-quality failure | Audio failed a structural or quality invariant, or a review finding stands |
| `7` | *reserved* — cancellation | Nothing in this build cancels; declared at `7` when something does, renumbering nothing |
| `8` | internal error | A defect in this build |

`ExitClass::of` classes a refusal by its governed `RemedyOwner` first — who acts on it — and by
`BuildErrorClass` only for a refusal with no governed remedy. One exception is decided before the
owner is asked: a `WorkerBundle` refusal is `3` whoever owns it, because every bundle refusal
routes to the worker-failure row and a worker that never started has not failed.
`t1_e2_every_refusal_class_maps_to_an_adr_exit_class` pins the class-level table with an exhaustive
`match`; `t4_e2_each_failure_class_has_declared_exit_code` pins `2` and `3` through the binary.

### The published commands

`lesson new`, `lesson validate`, `render`, `resume`, `retake`, `inspect`, `takes accept`,
`review`, `report`, `doctor`, `cache verify`, `cache prune`, `publish` — thirteen, of which the
first two were published by E1-S5. All accept `--json`.
`t4_e2_every_mvp_command_has_stable_structured_output` checks its own table against `--help`, so a
fourteenth command cannot land unlisted.

Where a spelling departs from ADR-0001 §7.3, the command's `--help` says how and why:
`takes accept` takes no `--segment`, because a takes document carries one selection per segment
and `ValidatedTakes` refuses a partial one; `retake --segment` takes `<segment-id>=<take>` rather
than the bare id, because a take is a synthesis-key input and a retake that chose its own number
could not be repeated by spelling it; `cache prune` accepts `--dry-run` and deletes nothing either
way (see §Limits). `invalidate`, `export`, and `lesson compile` are §7.3 names E2-S5 task 1 does
not list; `lesson compile` is E3-S1's.

### The `--json` envelope — stable, versioned, not published

One record per invocation: `output_version`, `command`, `outcome` (`succeeded` with `summary`, or
`refused` with `error_class`, `exit_code`, `message`, and `recovery_command` when a routing row
names one). `CLI_OUTPUT_VERSION` is `1.0-cli-output` and moves when a field is added or removed.

It is deliberately **not** a published schema: no `schemas/` file, no `PUBLISHED_SCHEMAS` entry,
no `G1-FREEZE-CHARTER.md` row, and a plain constant rather than a `*_SCHEMA_VERSION`, which the
charter's §The inventory is derived would oblige a row for. The reason is the one the plan review
on issue #18 settled: a schema is a promise to a consumer this project does not have, and a golden
fixture (`fixtures/contracts/e2-s5-cli-output-refusal.json`, pinned by
`t3_e2_cli_json_output_matches_its_committed_golden`) is the promise it can keep. Fields carry
IDs, hashes, timings, states, and error classes — `RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and
access's list — and nothing a refusal refused.

### `study-tts-runtime` public API

Compatible extensions: `diagnose`, `Finding`, `Verdict`; `accept_current_takes`;
`current_run_report`, `current_package_manifest`, `resolved_roots`; `retained_lesson_path`;
`tools::inspect_with_flag`; `manifest::recorded_selections` and `RecordedSegment::audio_blake3`;
`ToolOperation::HostProbe`; `DurableStateError::NoJobToRetake`.

**Breaking, in-repo only:** `VoiceProfileError::MissingVoiceRecord`, `VoiceRecordNotRegularFile`,
and `VoiceChecksumMismatch` carry `profile_id: String` and `record: &'static str` where they
carried `profile_dir: PathBuf` and `path: PathBuf`, and `VoiceRecordUnreadable` is added for a
record that exists and cannot be read. The reason is ADR-0001 §14 and
`RIGHTS-DATA-ARTIFACT-POLICY.md:40`: a refusal's `Display` is what `study-tts` prints by default,
and the old fields put the raw voice-reference path into it.
`t1_e2_no_voice_profile_refusal_names_a_path` and the path assertion in
`t4_e0_voice_checksum_mismatch_blocks_use` pin the new shape. The only matchers were the testkit's
and are updated in the same change. The remedy row is unchanged.

`JobRepository::load` no longer creates the job directory it asks about, and `preview::roots`
gained a non-creating `resolved_roots`; both are behavioral fixes with no signature change.

## Impact

- **Synthesis identities:** none. No synthesis-key input moves.
- **Verification identities:** none.
- **Plan, takes, or package identities:** none. `takes accept` writes a `TakesDocument` at the
  existing `TAKES_SCHEMA_VERSION`; it is the first thing in the tree that writes one, and a package
  built against it records `take_selection_source` `explicit`, which is the identity the M2 records
  named as unavailable.
- **Consumers and commands:** the `study-tts` binary is the only consumer of the added runtime API.
  Scripts that tested for exit `1` are the consumers of the breaking half.
- **Fakes and shared suites:** unchanged. `ReportingJobs` decorates `JobRepository` and delegates
  every method; no seam moved.
- **Fixtures and schemas:** `fixtures/contracts/e2-s5-cli-output-refusal.json` added with its
  `TEST-DATA-MANIFEST.md` row. `schemas/` is byte-identical; `git diff --exit-code -- schemas/`
  is the check.
- **Existing cached artifacts, published packages, accepted takes:** unaffected.
- **Governance documents:** `ROUTING-TABLES.md` §Failure routing gains the paragraph naming
  `recovery.rs` as the second side of its rows. `check-evidence-provenance.py` passes.

## Delivery and recovery

- **Fake and shared-suite update before consumers:** no seam changed shape.
- **Migration:** a script that tested `$? -eq 1` tests `-ne 0`, or the class it means.
- **Rollback:** revert the E2-S5 commits. Nothing durable was written under a changed contract; a
  takes document written by `takes accept` is a valid document at an unchanged version and needs
  no reconciliation.
- **Compatibility evidence:** the full CI-equivalent suite, reported per step at each commit.
- **Mapped tests:** the seven named tests in `DELIVERY-PLAN.md` §E2-S5, all present; plus
  `t3_e2_cli_json_output_matches_its_committed_golden`, `t1_e2_every_refusal_class_maps_to_an_adr_exit_class`,
  `t1_e2_every_recovery_row_names_a_row_the_routing_table_has`,
  `t4_e2_doctor_verifies_voice_profiles_and_names_no_path`,
  `t4_e2_an_accepted_takes_document_is_published_durably`.
- **Walking skeleton:** passes, 84 tests.

## Limits this change does not close

- **`cache prune` deletes nothing.** E2-S5 task 6 asks for "dry-run by default and explicit
  destructive confirmation"; the first is delivered and pinned, the second has no destructive mode
  to guard. The project owner decided on 2026-09-21 that the destructive mode ships under issue
  #94, ahead of E5-S4 task 5, and `DELIVERY-PLAN.md` §E2-S5 carries that as an Open item.
- **Cancellation has no producer.** Code `7` is reserved, not declared.
- **The voice-path redaction is proved at the runtime boundary, not through the binary.** A render
  reaches the voice gate only after the bundle and model gates, and the model gate needs the
  governed weights, which no offline test holds. The CLI prints the runtime's `Display` unchanged,
  so the runtime test is the proof; a CLI case is owed when a fixture model root exists.
- **`WorkerBundle` refusals route to the worker-failure row.** The exit class is corrected on the
  CLI side and `resume` is withheld for them; whether they deserve their own routing row is a
  `ROUTING-TABLES.md` question this record leaves open.
- **This record accepts no story.** `DELIVERY-PLAN.md` §E2-S5's close is the playbook's, once this
  record and the prune decision above are settled.

## Approval

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept that `lesson validate`'s refusal exit moves from `1` to its §7.3 class, and the thirteen commands as the published E2-S5 surface | Proposed |
| Contract owner (T-CLI) | Accept the exit-code table above, `--json` as a global flag, and the envelope as stable-and-versioned rather than published | Proposed |
| Contract owner (T-CORE) | Accept the `VoiceProfileError` field change and `VoiceRecordUnreadable` as the price of keeping raw voice-reference paths out of default output | Proposed |
| Contract owner (T-AUDIO) | Accept `takes accept` as the first writer of a takes document, at an unchanged `TAKES_SCHEMA_VERSION` | Proposed |
| Engineering owner | Accept the limits recorded above, including that `cache prune`'s destructive mode is issue #94's | Proposed |
