# E0-S0 Walking Skeleton

## Status and purpose

The walking skeleton is the first executable integration contract. It proves that reviewed canonical lesson JSON can cross the provisional Rust boundaries, publish deterministic cached segment WAVs as atomic directory transactions, assemble exact PCM in Rust, invoke real FFmpeg without a shell, validate the encoded result with ffprobe, and atomically select an immutable private-preview package beneath `previews/<lesson-id>/packages/<manifest-blake3>/` through `previews/<lesson-id>/current.json`.

This path is deliberately smaller than G1. It pulls forward the ADR-0001 §12.3 durability primitives, provisional lesson-scoped ownership, cache-key serialization, and package publication journal needed to keep E0 artifacts transaction-safe. It does not claim the complete E2 job state machine, resume CLI, production schemas, approval records, audio conditioning, full provenance, or a complete output package. The validated `lesson_id` is the provisional E0 job and publication identity until the approved versioned job ID lands.

## Integration order

The required order is fixed because each stage consumes a validated artifact from the preceding stage:

1. Load and validate the provisional two-segment lesson fixture before any subprocess starts.
2. Derive a deterministic render plan and provisional synthesis cache keys.
3. Resolve and preflight FFmpeg and ffprobe, recording each resolved executable and version.
4. Canonicalize the workspace, create managed cache, job, quarantine, and preview roots, and verify containment.
5. Acquire `jobs/<lesson-id>/build.lock`, whose strict provisional record carries PID, Linux process-start identity, and creation metadata; refuse a live owner before reconciliation or output work.
6. Reconcile the strict provisional `jobs/<lesson-id>/publication.json` journal and validate any authoritative `current.json` without overwriting corrupt state.
7. Resolve each cache key under its bounded cross-process key lock. A miss writes and validates `audio.wav` plus `artifact.json` in one sibling staging directory, flushes both files and the directory, renames the directory without replacement, and flushes the shard directory. Abandoned attempts move to collision-free quarantine.
8. Recheck an immutable current package for the same plan, tool identities, and path-normalized FFmpeg and ffprobe argument-profile identities; a no-op rebuild returns it without assembly or encoding.
9. Concatenate cached PCM and declared silence in Rust into a transaction-local `lesson.wav` beneath `jobs/<lesson-id>/staging/<transaction>/`.
10. Invoke FFmpeg with a pinned discrete argument vector to encode transaction-local `lesson.m4a` from the master WAV.
11. Invoke ffprobe with discrete arguments and require one mono AAC stream.
12. Checksum both outputs and atomically write the transaction-local `manifest.json` with executable, version, executed-argument, normalized argument-profile provenance, and `release_status: private_preview`.
13. Flush every package file and the package directory, rename the complete directory to `previews/<lesson-id>/packages/<manifest-blake3>/` without replacement, then atomically replace and directory-sync `current.json`. The journal makes a crash after package durability but before selection finishable by the next build.

```mermaid
flowchart LR
    Lesson["Two-segment lesson JSON"] --> Validate["Validate reviewed input"]
    Validate --> Plan["Deterministic plan"]
    Plan --> Preflight["Preflight tools and managed paths"]
    Preflight --> Fake["Fake tone synthesis"]
    Fake --> Cache["Validated WAV cache"]
    Cache --> PCM["Rust PCM assembly"]
    PCM --> WAV["Master WAV"]
    WAV --> FFmpeg["Real FFmpeg AAC encode"]
    FFmpeg --> M4A["M4A"]
    M4A --> Probe["Real ffprobe validation"]
    WAV --> Manifest["Minimal manifest"]
    Probe --> Manifest
    Manifest --> Package["Immutable package generation"]
    Package --> Current["Atomic current.json selection"]
```

## Provisional boundary ownership

| Boundary | Current owner | Stabilization point |
|---|---|---|
| Lesson parsing and validation | `study-tts-core::{AuthoredLesson, ValidatedLesson}` | E1-S1 and E1-S2 schemas |
| Render planning and synthesis identity | `study-tts-core::RenderPlan` | E1-S1 identity contracts |
| Synthesis port | `study-tts-runtime::SegmentSynthesizer` | Replaced by the E0-S4 asynchronous contract |
| Deterministic tone implementation | `study-tts-testkit::DeterministicToneWorker` | Extended by E0-S4 shared fakes |
| Durable filesystem primitives | `study-tts-runtime::durable` | Extended, not replaced, by E2-S1 job state and E5 containment |
| Provisional lesson and cache-key locks | `study-tts-runtime::locking` | Replaced by approved job identity and integrated executor ownership in E2-S1/E5 |
| Cache validation and atomic directory publication | `study-tts-runtime` | Extended with production worker artifacts and verification in E1-S3/E2-S1 |
| PCM concatenation and silence | `study-tts-runtime` | Extended in E1-S4 and E2-S3 |
| FFmpeg and ffprobe invocation | `study-tts-runtime` | Extended in E1-S4 without changing pinned arguments, preflight, provenance, or the no-shell rule |
| Immutable preview generations and `current.json` | `study-tts-runtime::preview` | Extended into the complete package and approval flow in E1-S4/E2 |
| Minimal manifest | `study-tts-runtime` | Replaced by the E1-S1 versioned manifest schema |
| Build refusal API | `study-tts-runtime::BuildError` and public category enums | Frozen with the other public Rust interfaces at G1 |

## Provisional resource ceilings

E0-S0 refuses unbounded authored input and FFmpeg-family execution within the
fixed envelope below. These values are security ceilings, not measured
performance budgets or backend segmentation limits. They remain fixed until a
configuration milestone owns them; this does not move or redefine the
configurable worker supervision assigned to E5.

| Resource | Provisional ceiling |
| --- | ---: |
| Canonical lesson JSON | 16 MiB UTF-8 bytes |
| Segments per lesson | 4,096 |
| `display_text` per segment | 64 KiB UTF-8 bytes |
| `spoken_text` per segment | 64 KiB UTF-8 bytes |
| Source references per segment | 256 |
| One source reference | 4 KiB UTF-8 bytes |
| Aggregate title and segment string/reference fields | 16 MiB UTF-8 bytes |
| Version probe deadline | 5 seconds |
| ffprobe deadline | 30 seconds |
| FFmpeg encode deadline | 30 minutes |
| Captured stdout per tool execution | 1 MiB |
| Captured stderr per tool execution | 1 MiB |

The lesson values mirror the constants in
`crates/study-tts-core/src/lesson.rs`; its public
`MAX_LESSON_JSON_BYTES` is also imported by
`crates/study-tts-runtime/src/pipeline.rs::read_lesson`. That reader performs a
nonblocking Unix open, requires the opened descriptor to be a regular file,
performs a metadata preflight, and then reads at most
`MAX_LESSON_JSON_BYTES + 1`. A FIFO therefore cannot block the open, and a
file that grows after preflight remains bounded. Oversized input returns
`LessonError::LessonJsonTooLarge`; a special file returns
`IoError::LessonNotRegularFile`. Both refusals precede planning, tool
inspection, workspace creation, and synthesis.

The tool values mirror `TOOL_OUTPUT_LIMIT_BYTES`, `VERSION_PROBE_POLICY`,
`FFPROBE_POLICY`, and `FFMPEG_ENCODE_POLICY` in
`crates/study-tts-runtime/src/process.rs`. The shared runner drains stdout and
stderr concurrently through nonblocking, cancellable capture workers. On Unix
it creates a dedicated process group. On Linux it also records both capture
pipe identities and terminates any process outside that group that retains a
pipe, so a descendant cannot escape cleanup with `setsid` and strand capture.
Direct-child exit observation precedes the otherwise blocking `Child::wait`,
and capture joins are attempted only after `JoinHandle::is_finished`. Either
kind of cleanup that exceeds its one-second observation window transfers the
owned handle to a dedicated background reaper before returning a typed
failure. Other targets retain a bounded direct-child fallback. Existing
nonzero-exit categories remain the owner of bounded stderr diagnostics.

The ceiling-to-test traceability is mechanized by the following exact test
names:

- Lesson JSON boundary and parse ordering:
  `t1_e0_lesson_json_byte_limit_accepts_the_boundary_and_precedes_parsing`.
- Segment-count boundary:
  `t1_e0_segment_count_limit_accepts_the_boundary_and_rejects_one_more`.
- Per-field UTF-8 byte boundaries:
  `t1_e0_spoken_text_limit_counts_utf8_bytes`,
  `t1_e0_display_text_limit_counts_utf8_bytes`, and
  `t1_e0_source_reference_limits_accept_boundaries_and_count_utf8_bytes`.
- Aggregate authored-text boundary:
  `t1_e0_programmatic_authored_text_limit_accepts_the_boundary`.
- Metadata-preflight growth protection:
  `t1_e0_bounded_lesson_reader_refuses_growth_after_metadata_preflight`.
- Pre-work runtime ordering:
  `t4_e0_oversized_lesson_fails_before_tools_workspace_and_synthesis` and
  `t4_e0_lesson_fifo_fails_before_tools_workspace_and_synthesis`.
- Independent pipe overflow:
  `t4_e0_bounded_command_reports_the_stream_that_overflows`.
- Deadline enforcement:
  `t4_e0_bounded_command_times_out_with_an_injected_policy` and
  `t4_e0_deadline_includes_capture_setup_and_precedes_success`.
- Monitoring failure cleanup:
  `t4_e0_capture_thread_start_failure_terminates_and_reaps_child`.
- Non-reaping process-group observation:
  `t4_e0_exit_observation_keeps_process_group_leader_waitable`.
- Descendant cleanup:
  `t4_e0_timeout_terminates_and_reaps_the_process_group` and
  `t4_e0_successful_child_terminates_and_reaps_lingering_descendants`.
- Escaped pipe-holder containment:
  `t4_e0_timeout_terminates_escaped_descendant_retaining_capture_pipes`.

The process-executing T4 tests are intentionally colocated in
`crates/study-tts-runtime/src/process.rs` because their injected short policy
and capture-start failure seams are private implementation details. Moving
them to `study-tts-testkit` would widen production visibility solely for the
test harness; the tests still exercise real filesystem and `/bin/sh` process
boundaries and retain their T4 names and budget.

The word provisional is material. The fixture uses `schema_version: 0.1-skeleton`; lock, journal, and selection records use distinct internal `0.1-skeleton-*` versions with unknown-field rejection. None can be mistaken for the complete lesson, job, manifest, or publication schemas accepted in ADR-0001. Later stories may version or replace these contracts, but they must preserve this test path or update it in the same change so the end-to-end integration order remains executable.

New minimal preview manifests use `0.2-skeleton`, which requires normalized tool argument-profile identities. Reconciliation still accepts strict legacy `0.1-skeleton` manifests without those fields, but cannot reuse them as a matching tool-profile generation.

Before G1, the provisional flat `BuildError` was intentionally replaced by
transparent category variants with exact leaf refusals beneath them. This was a
source-breaking Rust pattern change, accepted while workspace consumers were
still migrated together: it preserved each failure distinction, message,
source chain, and operator remedy while making the category boundary explicit
before interface freeze. On the supported `x86_64-unknown-linux-gnu` target,
`size_of::<BuildError>()` measured 80 bytes before and 80 bytes after the
refactor, so the existing boxed cache fault remained the only boxed payload. The
baseline is enforced by
`error::tests::t1_e0_build_error_does_not_grow_during_category_refactor` in
`crates/study-tts-runtime/src/error/mod.rs`, using
`PRE_REFACTOR_BUILD_ERROR_SIZE_BYTES`; update this record and that constant
together. Structured remedy advice supplements the actionable messages. Rich
`miette` reports remain deferred until the product CLI diagnostics and
JSON-output contract exist.

## Permanent check

Run the T4 suite locally after restoring locked dependencies:

```bash
cargo test --offline -p study-tts-testkit --test walking_skeleton --locked
```

`cargo --offline` controls dependency resolution but does not prevent runtime network access. CI therefore compiles the test binaries as the normal runner user, enters a new Linux network namespace containing only loopback, verifies that no egress-capable interface exists, drops back to the runner user, and executes the prebuilt workspace binaries under a 60-second deadline. The deadline measures test execution rather than compilation. The test phase does not download models, access model artifacts, require a GPU, or reach a network service.

The `walking-skeleton` CI job is required to remain green through every later story. A contract change that breaks the integration order is incomplete until the fake, fixture, implementation, and this record agree again.

## Deferred from E0-S0

MP3, chapters, transcripts, captions, authoritative non-UTF-8 path representation and full provenance beyond the executed FFmpeg and ffprobe identities and normalized argument profiles, the complete job state machine and resume CLI, take selection, post-render ASR, loudness normalization, edge conditioning, cache pruning, and the production Chatterbox worker remain in their assigned G1 or later stories. E0 quarantine retains abandoned cache and package attempts but exposes no deletion workflow. Existing flat E0 preview files are legacy artifacts: new builds leave them intact and select only immutable packages through `current.json`. The minimal manifest is mechanically marked as a private preview, the production manifest loader rejects its schema version, and the publication entry point returns a typed refusal.
