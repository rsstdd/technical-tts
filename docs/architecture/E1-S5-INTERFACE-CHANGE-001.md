# E1-S5 Interface Change 001 — The authoring command line

## Identification

- Record ID: `E1-S5-INTERFACE-CHANGE-001`
- Status: **Proposed.** §Approval carries no signature. Nothing here may be read as accepted.
- Contract owner: T-CLI (the `study-tts` command surface)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-CLI, T-CORE
- Accepted ADR, if architectural: none. ADR-0001 §7.2 already names `clap` for "stable
  subcommands and non-interactive use", and `DELIVERY-PLAN.md` E1-S5 already assigns the two
  commands. This record declares a change of published surface, not a change of architecture.

`DELIVERY-PLAN.md` E1-S1 shipped a dependency-free status executable and said so in its own
record: `E1-S1-INTERFACE-CHANGE-001` §"What the ninth audit closed" states that the binary
"reports the tested E1-S1 contract baseline and names E1-S5 as the owner of product commands".
This is that story. The executable it describes is retired here, on the schedule that record
published.

## Version and compatibility

### The executable — status reporter → `study-tts lesson`

| | Before | After |
|---|---|---|
| Binary name | `study-tts-cli`, the package name by default | `study-tts`, declared by `[[bin]]` |
| Arguments | None accepted or parsed | `lesson new <lesson-id> --out <path>`, `lesson validate <path>` |
| Stdout on success | One fixed sentence | `created lesson scaffold: <path>` or `valid lesson: <path>` |
| Stderr | Always empty | The refusal, when there is one |
| Exit status | Always `0` | `0`, or `1` for every refusal |
| Dependencies | None | `clap`, `study-tts-core`, `study-tts-runtime` |

**Breaking contract**, and deliberately so: `t4_e1_status_executable_reports_the_contract_baseline`
pinned the old stdout byte for byte, and that test is removed rather than adapted. A test that
asserted the new sentence under the old name would claim continuity between two different
commands. No durable artifact, cached entry, schema, or identity is involved — the removed
contract was a sentence printed to a terminal, and nothing reads it but a person.

The binary rename is the visible half. `study-tts` is what `DELIVERY-PLAN.md` E1-S5 names in
`study-tts lesson new` and `study-tts lesson validate`, and a command an author types is a
published surface in a way a Cargo package name is not.

### `study-tts-runtime` public API — compatible extension

Three additions, no removals and no signature changes:

| Item | What it is |
|---|---|
| `load_lesson(&Path) -> Result<ValidatedLesson, BuildError>` | The read-and-validate step extracted from `build_preview_with_services`, which now calls it |
| `scaffold_lesson(&str, &Path) -> Result<(), BuildError>` | Writes a validated lesson scaffold, refusing to replace anything |
| `SCAFFOLD_VOICE_PROFILE` | The profile a scaffold binds its one speaker to |

`load_lesson` is an extraction, not a reimplementation, and that is its whole point: two
implementations of "is this lesson usable" would be free to disagree, and the one an author
consulted through `lesson validate` would be the one that did not matter. `build_preview` reads
the same bytes through the same bounded reader and the same `ValidatedLesson::from_json` it always
did.

`scaffold_lesson` owns no filesystem durability. It calls `durable::write_bytes_noreplace`, which
stays `pub(crate)` alongside every other function in that module, so the ADR-0001 §12.3 ordering
has exactly one implementation and the CLI has none. That ordering is proven for the new function
rather than assumed from the old one:
`t4_e1_durable_byte_publication_flushes_file_then_rename_then_parent` asserts file sync, then
rename, then parent sync through the same crash-injection seam
`t4_e0_durable_json_replacement_flushes_file_then_rename_then_parent` uses, and
`t4_e1_a_taken_destination_keeps_its_bytes_and_leaves_no_staged_file` asserts that a refusal
leaves the destination byte-identical, synchronizes no parent, and removes its staged sibling.

### `IoError` — new variant `DestinationExists`

A refusal for a publication that must not replace anything. It is raised in one place,
`scaffold_lesson`, and `build_preview`'s `# Errors` section says in as many words that it cannot
reach one: a build claims names inside a workspace it owns, where losing a publication race is
reported by the durable-state category that owns the record.

The variant is deliberately unrouted. `docs/governance/ROUTING-TABLES.md` §Failure routing
establishes no owner for an authoring refusal, and the person who chose the path is the only one
who can choose another, so the message carries that remedy rather than inventing a governed one.
`BuildError::Io` continues to return no `RemedyAdvice`, unchanged.

`IoError` is not `#[non_exhaustive]`, so a new variant is breaking for an external matcher. There
are none: the enum is consumed only inside this workspace, where
`t3_e1_every_documented_error_variant_is_named_by_its_errors_section` now holds three `# Errors`
sections against it rather than one.

### `durable::parent_of` — behavioral fix

`Path::parent` answers an empty path for a bare file name, which names the current directory
rather than a missing one. Staging into `""` fails with `ENOENT`, so `--out lesson.json` would have
been refused a destination that is perfectly writable. An empty parent now resolves to `.`;
`UnrootedDestination` still refuses a path with no parent at all.

This reaches `write_json_atomically` as well as the new function. No current caller passes a bare
relative name — every one resolves a path beneath a workspace root it was given — so the fix
changes no existing behavior and removes a trap the first relative caller would have hit.
`t1_e1_a_destination_with_no_directory_component_stages_in_the_current_directory` holds all three
answers: a bare name, a relative name that keeps its parent, and a path with no parent that is
still refused. It asserts on `parent_of` rather than by publishing a relative path, because the
current directory is process-wide state and these tests run concurrently.

## Impact

- **Synthesis identities:** none. No synthesis-key input moves.
- **Verification identities:** none.
- **Plan, takes, or package identities:** none. `load_lesson` returns the same `ValidatedLesson`
  from the same bytes.
- **Schemas:** none. The published lesson schema is unchanged at `3.1`; `cargo run --example
  generate-schemas` rewrites `schemas/` byte-identically.
- **Existing cached artifacts and published packages:** unaffected.
- **Fakes and shared suites:** unchanged in behavior, extended in coverage.
  `t4_e1_the_real_job_repository_passes_the_shared_contract` runs
  `run_job_repository_contract_scenario` against `FileSystemJobRepository` directly. Parity had
  been inferred until now: the scenario ran only against `InMemoryJobRepository`, and the real
  repository appeared only inside `t4_e0_walking_skeleton_uses_only_published_seams`, wrapped in a
  recorder that observes calls rather than checking the contract.
- **Fixtures:** none added, none changed. `fixtures/lessons/e1-s4-three-segment.json` is cited by
  `docs/operations/AUTHORING.md` as the reviewed worked example and is neither copied nor edited,
  because an accepted evidence record refers to the package rendered from it.
- **Dependencies:** `clap 4.6.6` with `derive`, and its tree — `clap_builder`, `clap_derive`,
  `clap_lex`, `anstream`, `anstyle`, `anstyle-parse`, `anstyle-query`, `anstyle-wincon`,
  `colorchoice`, `heck`, `is_terminal_polyfill`, `once_cell_polyfill`, `strsim`, `utf8parse`.
  Fifteen crates enter `Cargo.lock`; `cargo deny check` reports advisories, bans, licenses, and
  sources all clean.
- **Consumers and commands:** the `study-tts` binary, which is the only consumer of the added
  runtime API.

## Delivery and recovery

- **Fake and shared-suite update before consumers:** no seam changed shape, so no fake needed
  updating. The shared job-state suite gained its real implementation before this record was
  written.
- **Migration:** none required of any stored artifact. A person who invoked the status executable
  invokes `study-tts lesson --help` instead.
- **Rollback:** revert the commit. Nothing durable was written under a changed contract, so
  nothing has to be reconciled afterwards.
- **Compatibility evidence:** the full CI-equivalent suite, run locally and reported per step.
  `cargo run --example generate-schemas` leaves `schemas/` unchanged, which is what shows no
  published schema moved.
- **Walking skeleton:** `cargo test --offline --workspace --all-targets --locked` passes,
  `walking_skeleton` included. T4 measured over a warm build at 13.1 s, 13.2 s, and 15.0 s against
  its five-minute budget, by `cargo test --offline --workspace --all-targets --locked -- t4_`.

## Limits this change does not close

- **The scaffold is `approved` as written.** That is what lets
  `t4_e1_scaffolded_lesson_renders_through_the_walking_skeleton` hear the shape of a lesson without
  a manual approval step, and it means the field records a starting value rather than a review.
  `docs/operations/AUTHORING.md` §3 carries the rule an author must apply instead; nothing
  mechanizes it.
- **A diagnostic's independence from lesson text is asserted for validation refusals, not
  proved for every input.** `t4_e1_a_refusal_never_quotes_the_lesson_text_it_read` drives a
  segment-level refusal and shows the spoken text is absent. `serde` can quote a *mistyped* value
  in a shape refusal — a lesson that put prose where a number belongs — and no test bounds that
  today.
- **No product render command exists.** `docs/operations/AUTHORING.md` §5 sends an author to
  `cargo run --package study-tts-testkit --example package-render`, which is an instrument rather
  than a product surface.
- **Every refusal exits `1`.** `DELIVERY-PLAN.md` E2-S5 owns `--json` and stable numeric exit
  classes, and publishing either here would be a contract that story did not choose.
- **This record accepts no story.** `DELIVERY-PLAN.md` E1-S5's own evidence is not opened here,
  and cannot be until the worker seeding of issue #70 and the requalification and fresh listening
  behind it close: those move the worker-bundle hash and therefore every synthesis key and plan
  hash, so an E1-S5 evidence record written first would be stale the moment it was written.

## Approval

**No row below is signed.** Each records a decision a role is being asked for, and nothing here is
accepted until the row carries a name and a date.

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md` permits that for
a personal project and requires each approval to name its role and accepted risk separately, which
is why the rows stay separate for one signatory.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept retiring the E1-S1 status executable and its pinned stdout, and renaming the binary to `study-tts` | Pending |
| Contract owner (T-CLI) | Accept `lesson new` and `lesson validate` as the published E1-S5 surface, a single nonzero exit for every refusal, and `clap` plus its fifteen-crate tree | Pending |
| Contract owner (T-CORE) | Accept `load_lesson`, `scaffold_lesson`, and `SCAFFOLD_VOICE_PROFILE` as compatible additions, `IoError::DestinationExists` as an unrouted refusal, and that no schema or identity moves | Pending |
| Engineering owner | Accept the `parent_of` behavioral fix reaching `write_json_atomically`, and the limits recorded above | Pending |

- Effective version and date: not effective; `Proposed`.
