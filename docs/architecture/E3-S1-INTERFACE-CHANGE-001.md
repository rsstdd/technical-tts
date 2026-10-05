# E3-S1 Interface Change 001 — Markdown compilation and `study-tts lesson compile`

## Identification

- Record ID: `E3-S1-INTERFACE-CHANGE-001`
- Status: **Proposed.** §Approval carries the rows a signature would fill. Until it is signed this
  record authorizes nothing.
- Contract owner: T-AUTHORING (the source-document surface)
- Engineering owner: Engineering owner
- Affected-track reviewers: T-AUTHORING, T-CORE, T-CLI
- Accepted ADR, if architectural: none. ADR-0001 §9.1 names the parser and the nine source blocks,
  §9.2 bounds compilation to deterministic operations, §7.2 names `pulldown-cmark`, and §7.3
  spells `lesson compile`. This record declares what ships of that, and the four readings §9
  leaves open.

## Version and compatibility

### `study-tts lesson compile` — new published command

```text
study-tts lesson compile <source.md> --out <lesson.json> --lesson-id <id>
    [--language <tag>] --speaker <name>=<voice-profile> [--voice <name>]
```

**Compatible extension.** A third `lesson` subcommand beside `new` and `validate`;
`E1-S5-INTERFACE-CHANGE-001` published those two and `E2-S5-INTERFACE-CHANGE-001` published the
exit-code table this command reports through. Nothing existing changes shape. `--json` comes from
the global flag, and a compile refusal exits `2`, invalid input, with no new exit-class mapping —
see §`BuildError`.

### `study-tts-core` — new public API

Compatible extension: `SourceBlock`, `SourceBlockKind`, `CompileError`, `CompileRequest`,
`MAX_SOURCE_BLOCKS`, `PRONUNCIATION_FENCE`, `parse_source`, `compile_lesson`.

### `study-tts-runtime` — new public API

Compatible extension: `compile_source`. It sits beside `scaffold_lesson` in `authoring.rs`
because it reads and writes files, which is the boundary `crates/AGENTS.md` §Routing table puts
here rather than in core; the parse and the lesson assembly are core's.

**`BuildError` gains `Compile(Box<CompileError>)`** whose `class()` is the existing
`BuildErrorClass::Lesson`. A distinct variant so a test can assert the exact refusal, and the
existing class because a Markdown document this build cannot compile is invalid authored input —
which is what that class means and what ADR-0001 §7.3's `invalid input` says to do about it. A new
class would have bought a seventeenth row in `ExitClass::of_class`, the `CLASSES` table in
`exit.rs`, `expected_durable_state_remedy`'s neighbours, and
`t1_e0_governed_remedy_mappings_are_exhaustive`, for a refusal the operator answers identically.
Boxed to hold `BuildError`'s size under the baseline
`t1_e0_build_error_does_not_grow_during_category_refactor` pins.

### The dependency

`pulldown-cmark 0.13.4` with `default-features = false`, which is ADR-0001 §7.2's named candidate:
"Markdown parsing — `pulldown-cmark` or equivalent — Structural parsing rather than regular
expressions." §9.1 forbids the alternative outright, and the hand-written alternative is a
CommonMark implementation, which is not a reasonable thing to write here.

Two crates enter `Cargo.lock`: `pulldown-cmark 0.13.4` (MIT) and `unicase 2.9.0`
(MIT OR Apache-2.0). Default features are off, which excludes the `html` writer and
`pulldown-cmark-escape`; this build renders no HTML. `cargo deny check` reports
**advisories ok, bans ok, licenses ok, sources ok**, against `deny.toml`'s four allowed licenses.
CI restores before it goes offline — `.github/workflows/ci.yml` runs `cargo fetch --locked` and
every later step is `--offline` — so a locked dependency is compatible with the offline policy.

## The four readings §9 leaves open

### 1. A heading emits no spoken segment

§9.2 requires headings be converted "into section metadata rather than reading 'heading level
two'". **Lesson `3.1` has no section field** — its complete property set is `$schema`, `language`,
`learning_objectives`, `lesson_id`, `schema_version`, `segments`, `source`, `speakers`, `title`.

The reading taken: a heading is recorded as a source block, the **first** heading supplies the
lesson's `title`, and no heading emits a segment. Nothing is spoken that should not be, and the
heading's text and position stay traceable through its `block-NNN` reference.

The alternative was a new optional section field, lesson `3.1` → `3.2`: a compatible extension
that moves `schemas/lesson-v3.schema.json`, the charter's `lesson` row, and the fixtures. It is
not taken because the narrow reading satisfies §9.2 without moving a published contract, and
because `learning_objectives` already carries the lesson-level framing a section heading would.
**If a later story needs section metadata, this is the record that declined to add it.**

### 1b. A link records its label as well as its destination

§9.1 names the block "link label and destination", and both halves are recorded: the block is held
open from `Tag::Link` to its end so the label accumulates into it. The label **also** stays in the
prose that holds it, because a link's label is part of the sentence a listener hears and dropping
it from the paragraph would silently delete words.

Recorded because the first implementation did not do this. It pushed the link block at the opening
tag with an empty `text`, so the destination was recorded and the label never was — half of what
§9.1 states — and the test that existed asserted only the destination, agreeing with the narrower
behavior. `t1_e3_a_link_records_its_label_and_its_destination` now asserts both halves and the
sentence.

### 2. The table-linearization policy

§9.1 says a table is "converted to a reviewable linear representation" and stops. The policy,
which `study-tts-core/src/markdown.rs::BlockBuilder::append_table_row` names in return:

> Row-major. Each cell is named by its column header, cells joined by `, `, and each row ends with
> `.`. The header row names the columns and is not itself a row.

So §9.3's own example table linearizes as
`Source: charCodeAt(0), Context: API memorization, Spoken result: char code at, with index zero.`

**A cell with no value is refused** — `CompileError::EmptyTableCell`. There is deliberately no
ragged-row refusal: `pulldown-cmark` pads a short row to the header's width before this build sees
it, so raggedness arrives as an empty cell, and detecting the original would mean reading the
source line, which §9.1 forbids. An empty cell linearizes to a column name followed by nothing,
which §9.2's "emit no text it cannot derive" bars. There is likewise no headerless-table refusal:
a pipe table's first row *is* its header in CommonMark, so the refusal was unreachable.
`t1_e3_table_cell_with_no_value_is_refused` established both before the variants were replaced.

### 3. The pronunciation-directive syntax

§9.1 names "explicit pronunciation directive" as a source block and defines no syntax; nothing
else in the repository mentions one. The syntax taken: **a fenced block whose info string is
`pronounce`**, mirrored by `PRONUNCIATION_FENCE`.

It is valid CommonMark, so every other tool renders it as a code block rather than mangling it;
the fence's info string is something the parser already hands this build; and §9.1 calls a
directive a *block*, which this is. The alternatives were an inline extension such as
`[term]{pronounce="..."}`, which is not CommonMark and would need hand-lexing inside text events —
close to the regex stripping §9.1 forbids — and a sidecar file, which splits one authored decision
across two files and overlaps E3-S2's project lexicon.

**A directive is extracted and recorded, never applied.** Applying one is normalization, which is
ADR-0001 §9.3's and `DELIVERY-PLAN.md` E3-S2 task 6's lexicon.

### 4. Where a compilation's non-Markdown values come from

The lesson schema requires six top-level fields and nine per segment; Markdown carries at most
`title`, the two texts, and the block references. ADR-0001 §7.4 governs the rest, and
**its hierarchy is implemented nowhere and owned by no story** — issue #95 now carries it.

E3-S1 implements **levels 1 and 5 only**: command-line options for `lesson_id`, `language`, and a
repeatable `--speaker <name>=<voice-profile>` in the shape `--retake <segment-id>=<take>` already
takes; compiled safe defaults for `role` (`explanation`), `style` (`calm_explanatory`), and
`pause_after_ms` (`75`, the committed fixtures' value). Levels 2 through 4 — lesson profile,
project configuration, user configuration — are **not implemented**, and this story deliberately
set no file format for them: doing so inside a parser story would fix the shape of a configuration
system by precedent without the owner deciding it.

`speakers` has no default at all, and that is the load-bearing one: a voice profile reaches every
synthesis key under §12.5, so a compiler that defaulted it would be choosing a voice. When several
speakers are declared and `--voice` names none of them, the request is refused by
`CompileError::SpeakerNotChosen` — a named refusal rather than a borrowed `IoError`, because
nothing failed to read and an operator told their document was unreadable would look in the wrong
place. A speaker **bound twice** is refused by `CompileError::SpeakerDeclaredTwice`, which is the
invariant `LessonError::DuplicateSpeaker` already enforces on a lesson document, enforced one
layer earlier because the command line declares speakers before a document exists. Collecting
the bindings into a `BTreeMap` keeps the last one silently, which would let argument order decide
a voice. A document that read but is not UTF-8 is `CompileError::SourceNotUtf8`, carrying the
offset of the first invalid byte. All three were first written as a borrowed `IoError` with a
hand-built `io::Error`, which named a file that read without error; the review passes that found
them are why each is now a refusal about the thing that actually failed.
`role` and `style` are uniform per document because their vocabularies are seventeen and four
closed values and no Markdown structure implies any of them; a per-block guess would invent the
review metadata an author is meant to assign.

## The conflict this story surfaced, and what it did about it

**ADR-0001 §9.2 requires "reviewable draft segments"; `ValidatedLesson` refuses any segment that
is not `approved`.** The gate is stricter than §8.2's own text, which bars unreviewed material
from "a production build" — it refuses at every validation. So a compiled draft is not a valid
lesson in this build, and both ways to make it one are wrong: emitting `approved` asserts a review
nobody performed, and loosening the gate would let an unapproved lesson reach a render.

What `compile_source` does instead: it validates the bytes it is about to write and accepts
**exactly one** outcome besides success — `LessonError::UnapprovedSegment`, which is the author's
remaining work and what `study-tts lesson validate` tells them. Every other refusal is a defect in
the compiler and refuses the write. **The gate is not weakened.** The compiler stops claiming its
output has passed it, and `t4_e3_compile_writes_a_draft_whose_only_refusal_is_its_review` pins
both halves: compiling exits `0`, validating the result exits `2` naming `review_status`.

**This is the finding a reader should carry forward.** §8.2's text and its implementation disagree
about *when* approval is required, and E3-S2's golden harness and E3-S3's audit will meet the same
wall the moment they want to render compiled material. Resolving it — by amending §8.2, by gating
approval at the build boundary instead of the parse boundary, or by leaving it as it stands — is a
decision this record raises and does not take.

## Impact

- **Synthesis identities:** none. No synthesis-key input moves. `source_refs` is excluded from
  every identity, which `identity.rs`'s field-sensitivity table proves by mutating it to
  `block-002` and asserting the key does not move.
- **Verification, plan, takes, package, transaction identities:** none move.
- **Published schemas:** none move. The compiler emits lesson `3.1` and `generate-schemas` leaves
  `schemas/` byte-identical.
- **Existing cached artifacts, published packages, accepted takes, approvals:** unaffected. A
  compiled lesson's `spoken_text` is new text with no prior key to invalidate.
- **Consumers and commands:** `study-tts lesson compile` is new. `study-tts lesson --help` now
  lists three subcommands, which `t4_e1_help_lists_only_the_implemented_lesson_commands` pins.
- **Fakes and shared suites:** unchanged. No seam moved.
- **Fixtures:** none added. Every test writes its source document into its own temporary root, so
  no `docs/testing/TEST-DATA-MANIFEST.md` row is owed.
- **Version constants:** **none declared.** `G1-FREEZE-CHARTER.md` §The inventory is derived
  obliges a frozen row or a §Deliberately not frozen row for every `*_SCHEMA_VERSION`,
  `*_CONTRACT_VERSION`, `*_IDENTITY_VERSION`, `*_PROTOCOL_VERSION`, and `*_RENDERER_VERSION` in
  `crates/*/src/*.rs`; this story declares no such constant, so the charter needs no amendment.

## Delivery and recovery

- **Fake and shared-suite update before consumers:** no seam changed shape.
- **Migration:** none. Nothing durable was written under a changed contract.
- **Rollback:** revert the commits and remove two entries from `Cargo.lock`. A compiled lesson on
  disk stays a valid lesson `3.1` document and needs no reconciliation.
- **Mapped tests:** the five named tests in `DELIVERY-PLAN.md` §E3-S1 —
  `t2_e3_arbitrary_unicode_markdown_never_panics`,
  `t1_e3_each_supported_block_type_is_classified`,
  `t1_e3_fenced_code_is_never_merged_into_prose`,
  `t2_e3_source_references_are_stable_across_recompilation`,
  `t1_e3_unsupported_structure_is_reported_not_dropped` — plus
  `t1_e3_a_compiled_segment_is_never_marked_approved`,
  `t1_e3_a_compiled_lesson_names_every_declared_speaker`,
  `t1_e3_a_heading_emits_no_spoken_segment`,
  `t1_e3_a_table_linearizes_by_column_header`,
  `t1_e3_table_cell_with_no_value_is_refused`,
  `t1_e3_a_link_records_its_label_and_its_destination`,
  `t1_e3_a_document_past_the_block_ceiling_is_refused`,
  `t4_e3_an_unchosen_speaker_among_several_is_refused_without_writing`,
  `t4_e3_several_speakers_compile_when_one_is_chosen`,
  `t4_e3_a_speaker_bound_twice_is_refused_without_writing`,
  `t1_e3_a_source_document_that_is_not_utf8_is_refused_as_a_compile_problem`,
  `t1_e3_a_pronunciation_directive_is_extracted_not_spoken`,
  `t1_e3_block_references_are_numbered_in_document_order`,
  `t4_e3_compile_writes_a_draft_whose_only_refusal_is_its_review`, and
  `t4_e3_compile_never_overwrites_an_existing_lesson`.
- **Walking skeleton:** unaffected by construction; nothing on the render path moved.

## Limits this change does not close

- **No normalization.** `spoken_text` equals `display_text` on every compiled segment. ADR-0001
  §9.3 and E3-S2 own the difference, and emitting them equal keeps the document valid against §8.2
  while leaving the gap for the story that creates it.
- **One segment per prose block.** §9.2's "split material into reviewable draft segments" is
  satisfied at the block boundary. Splitting for length, protected terms, and stable child IDs are
  E3-S4's.
- **A fenced code block emits no segment.** §9.2 asks that code be preserved "separately from
  conceptual prose"; whether code is read literally or explained is §9.3's question and E3-S2's.
  So code is recorded, traceable, and silent.
- **A block reference is positional.** Inserting a block renumbers every block after it, so a
  reference traces a claim to a block *in one compilation* rather than across edits.
  `source.content_hash` names which compilation. A content-derived identity would be stable under
  insertion and would move on edit, trading one problem for another; E3-S5 is where a stronger
  guarantee would belong.
- **§7.4 levels 2 through 4 are unimplemented.** Issue #95.
- **The §8.2 approval conflict is raised, not resolved.** See above.

## Approval

Ross Todd holds every role listed. `docs/governance/PROJECT-EXECUTION-CHARTER.md:39` permits that
for a personal project and requires each approval to name its role and accepted risk separately.

| Role | Decision sought | Status |
|---|---|---|
| Project owner | Accept `lesson compile` as published surface, and that a compiled lesson is a draft an author must approve | Proposed |
| Contract owner (T-AUTHORING) | Accept the table-linearization policy and the `pronounce` fenced-block directive syntax as the authored surface | Proposed |
| Contract owner (T-CORE) | Accept the new `study-tts-core` API, `BuildError::Compile` reusing `BuildErrorClass::Lesson`, and that no schema or identity moves | Proposed |
| Contract owner (T-CLI) | Accept §7.4 levels 1 and 5 as the field sources, with levels 2–4 left to issue #95 | Proposed |
| Engineering owner | Accept `pulldown-cmark 0.13.4` and `unicase 2.9.0` with `cargo deny` clean, and the limits recorded above including the §8.2 conflict | Proposed |
