# Authoring a Lesson

This is the loop `study-tts` supports today: **scaffold → edit → review → validate → preview**.
It covers only what the build implements. Where a step is still done by hand, this document says
so rather than describing a command that does not exist.

The two commands are `study-tts lesson new` and `study-tts lesson validate`. Both are
`crates/study-tts-cli/src/main.rs`, and both call `study-tts-runtime` for every decision:
`scaffold_lesson` in `crates/study-tts-runtime/src/authoring.rs` writes the scaffold, and
`load_lesson` in `crates/study-tts-runtime/src/pipeline.rs` checks a document — the same function
`build_preview` uses, so a lesson this guide calls valid is one a render accepts.

## 1. Scaffold

```
study-tts lesson new <lesson-id> --out <path>
```

The lesson ID is the lesson's stable identity and also names its output directory, so it must be
a portable identifier: letters, digits, `_`, `-`, and `.`, starting with anything but a dot. A
space is refused, and refused *before* anything is written.

What the scaffold contains, and why:

| Field | Value | Why this and not something else |
|---|---|---|
| `$schema`, `schema_version` | The current published lesson schema | So an editor with JSON Schema support checks the document as you type |
| `title` | The lesson ID | A fabricated prose title would be lesson content this build wrote and you never reviewed |
| `language` | `en` | An ADR-0001 §12.5 synthesis-key input, so it cannot be left to a default |
| `speakers` | One `instructor`, bound to `owner-fallback-v1` | `docs/adr/deviations/ADR-0001-D003-single-instructor-fallback.md` selects the single-instructor configuration and forbids relabelling the owner profile as a second speaker. `SCAFFOLD_VOICE_PROFILE` in `crates/study-tts-runtime/src/authoring.rs` is the code end of that decision |
| `segments` | `seg-0001` explanation, `seg-0002` recall prompt, `seg-0003` answer | ADR-0001 §3.4's default study sequence, reduced to the shortest run that exercises a role invariant |
| `pause_after_ms` | 400, 2000, 600 | The recall prompt's 2 000 ms sits inside the 1 500–4 000 ms response interval ADR-0001 §8.2 and §13.2 bound |
| `style` | `calm_explanatory` | The one style the worker declares and ADR-0001 §13.4 has a frozen loudness reference for |
| `editorial` | `true` | The placeholder lines are this build's own words, and ADR-0001 §8.2 lets a segment cite nothing only when it is marked editorial |
| `review_status` | `approved` | So the scaffold renders as written and you can hear the shape before you write a lesson |

Two consequences of that last row are yours to carry, because no tool enforces them:

- **The placeholder text is not content.** Every `display_text` and `spoken_text` line says
  "Replace this line with…". A scaffold rendered unchanged is a demonstration, never a lesson.
- **`approved` on a scaffolded segment records nothing.** It is a starting value that makes the
  render path work. The moment you edit a segment, §3 applies.

The command never replaces a file. If the destination exists it refuses and leaves both files
alone; parent directories are not created, because choosing where a lesson lives is not a
decision this command should make for you. The scaffold is written owner-readable only (`0600`),
through the same durable publication the rest of the build uses — staged, synchronized, renamed
into place, and the directory synchronized after.

## 2. Edit

Edit the JSON directly. The rules that are not obvious from the schema:

- **Segment IDs are stable.** A segment ID is part of the synthesis cache key's addressing and is
  what a takes document names to select a retake. Renaming `seg-0002` does not rename a segment;
  it deletes one and adds another, discarding whatever was recorded against the old identity.
  Insert `seg-0002a` between two segments rather than renumbering the ones after it.
- **`spoken_text` is what is synthesized; `display_text` is what a reviewer reads.** Only the
  first reaches a cache key. Keep them the same unless the spoken form genuinely differs — an
  expanded abbreviation, a spelled-out symbol — and then make the difference deliberate.
- **Every segment either cites source material in `source_refs` or sets `editorial: true`.**
  ADR-0001 §8.2 admits no third option. `editorial` means the words are the author's own, not
  that the citation is pending.
- **A recall prompt must leave a response interval**, between 1 500 and 4 000 ms. The two
  refusals are separate on purpose: one is answered by lengthening the pause, the other by
  shortening it.

## 3. Review

- **An edited segment returns to `needs_review`.** Change the text and change the status in the
  same edit. `approved` means a person read that text and approved *it*; leaving the status
  behind while the text moves makes the field a claim about a segment that no longer exists.
- **Approval comes after review, and from a person.** No command in this build sets
  `review_status`, and none should: `crates/study-tts-core/src/lesson.rs` refuses synthesis of an
  unapproved segment precisely so that approval cannot be an accident.
- `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` governs the wider cycle this sits inside.

## 4. Validate

```
study-tts lesson validate <path>
```

Success prints `valid lesson: <path>` and exits zero. A refusal writes nothing to stdout, exits
nonzero, and names where the problem is:

```
refused `lessons/my-lesson.json`
  field: /segments/1/pause_after_ms
  segment: seg-0002
  reason: segment `seg-0002` is a recall prompt with a 900 ms pause, leaving less than the 1500 ms response interval ADR-0001 §8.2 requires
```

`field` is an RFC 6901 JSON Pointer; a whole-document refusal has no pointer and prints none.
`segment` is there because a pointer names a *position*, and a segment that moved is still found
by its identity.

Diagnostics never quote your lesson text. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
§Storage and access excludes spoken text from diagnostics, so the pointer and the segment ID are
how you find the offending value — the refusal will not read it back to you.

## 5. Preview

There is no product render command. `DELIVERY-PLAN.md` E1-S5 covers authoring only, and inventing
a `study-tts render` here would publish a surface a later story has to keep. To hear a lesson,
use the instrument that produced the reviewed G1 package:

```
cargo run --package study-tts-testkit --example package-render \
  --bundle-root <worker bundle> \
  --model-root <governed model root> \
  --voice-root <governed voice root> \
  --lesson <your lesson> \
  --output-root <a path that does not exist>
```

It drives your lesson through `build_preview` with the real worker attached — the path a
production build takes, rather than a harness that assembles its own package — and prints the
BLAKE3 of each of the seven artifacts it wrote. The roots are governed, so they are named on the
command line and never defaulted into a committed file; `--output-root` must not already exist,
so a rerender cannot overwrite the package a review was taken against.

## The worked example

`fixtures/lessons/e1-s4-three-segment.json` is the reviewed three-segment lesson the G1 package
was rendered and listened to from. Read it as the example of a finished lesson: three segments
with an explanation, a recall prompt, and its answer; real `source_refs`; a `source` block
recording what it was compiled from; and learning objectives.

It is a registered fixture. `docs/testing/TEST-DATA-MANIFEST.md` records it and
`evidence/gates/g1/e1-s4/e1-s4-minimal-package-generation-v1.md` cites the package rendered from
it, so copy it to start a lesson of your own and leave the fixture itself alone — editing it
changes what an accepted evidence record refers to.

## What this guide does not cover

Parent-directory creation, interactive prompting, Markdown or prose compilation into lessons, a
product preview or render command, `--json` output, and stable numeric exit classes. Every
refusal exits `1` today; `DELIVERY-PLAN.md` E2-S5 owns the machine-readable surface, and giving
it one here would be a contract that story did not choose.
