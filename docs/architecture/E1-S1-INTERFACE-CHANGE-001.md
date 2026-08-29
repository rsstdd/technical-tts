# E1-S1 Interface Change 001 — Complete Synthesis and Verification Identities

## Identification

- Record ID: `E1-S1-INTERFACE-CHANGE-001`
- Contract owner: T-CORE
- Engineering owner: engineering owner
- Affected-track reviewers: T-WORKER, T-AUDIO, and T-RUNTIME consumers
- Accepted ADR, if architectural: not applicable; this implements ADR-0001 §12.5
  as written and changes no authority boundary

## Audit index

This record is append-only: each audit added a section rather than editing an
earlier one, because superseded evidence records cite those sections by name and
are immutable. The table is the way in. Every section below is kept verbatim.

The last column is the question a reader usually arrives with, since a moved
worker-bundle identity re-keys every cache entry written after it. It is read
from each section's own §What this moves rather than inferred.

| Audit | What it closed | Bundle identity |
|---:|---|---|
| 1 | The E1-S1 baseline itself: canonical serialization, the synthesis and verification identities, digest-typed frame identities, and worker frames moved to breaking `e1.worker.1.0` with extension `1.1` | baseline, `e1-s1-v1` |
| 2 | The installed environment compared against `worker/requirements.lock`, and the manifest layout published as a `const` | moves, `v1`→`v2` |
| 3 | `.pth` startup hooks reported, and refused where the lock does not account for them | moves |
| 4 | An unreadable `worker/launcher.json` reported as a startup error rather than a parse failure | moves |
| 5 | Frame and session labelling separated, so one frame is no longer described as a session of them | moves |
| 6 | The worker-protocol schema gained `$id` | moves |
| 7 | The lock required to state its package sources, artifact kinds, and one artifact hash per index-supplied pin | moves, `v2`→`v3` |
| 8 | Lesson schema metadata made private, so `$schema` and `schema_version` cannot be rewritten after validation | no move |
| 9 | The CLI crate stopped identifying itself as the E0-S0 placeholder, with a process-level test pinning what it reports | no move |
| 10 | Correlation identities and refusal messages bounded at both protocol ends; two references that pointed at nothing repaired | moves |
| 11 | Three disagreements about what a complete worker protocol is; the schema input became `worker-protocol-v1` | moves, `v3`→`v4` |
| 12 | `threads` narrowed to `NonZeroU32` and `minimum: 1`, refusing a value no application could honor | moves |
| 13 | The refusal-boundary confidentiality defect: no rejected value, field name, or interpreter exception text reaches a failure frame | moves |
| 14 | Manifest layout `1.1`, declared startup modules, and per-file `RECORD` digest verification | moves |
| 15 | Typed initialization identities; the product worker fails closed instead of reporting a successful load of nothing | moves, to `6b0a3c14…` |
| 16 | The fake refuses a requested worker-bundle identity other than its own | no move |
| 17 | `ADR-0001-D004` authorizing the environment precondition, the `worker_bundle`/`worker_environment` split, the probe extracted to a `.py` file, sixteen tier corrections, and tier-duration reporting in CI | no move |
| 18 | The version retention in audit 15 moved out of a mirror document into `ADR-0001-D005`, and the required-field surface of every published schema put under test | no move |
| 19 | Audits 15–16 approved, the authoritative 60-second T4 deadline restored, and the fake-worker contract harness bounded with timeout cleanup | no move |
| 20 | The environment probe bootstrapped under `-S`, installed `RECORD`s authenticated against manifest layout `1.2`, and the interpreter attach step made to refuse a real directory | moves, manifest layout `1.1`→`1.2` |

Audits 1 through 13 name `schemas/worker-protocol-v0.schema.json`, which is
correct for the time each was written. The eleventh audit's breaking change
renamed it to `schemas/worker-protocol-v1.schema.json`, which is the only such
file in the tree today.

## Version and compatibility

- Contract ID: `tts_executor`
- Old version: `e0.tts-executor.0.1`
- New version: `e1.tts-executor.1.0`
- Compatibility class: breaking
- Required/defaulted fields: `BackendDescriptor::synthesis_identity` and
  `BackendDescriptor::deterministic_seed` are removed. Eight required fields
  replace them: `worker_bundle_hash`, `model_repository`, `model_revision`,
  `tokenizer_revision`, `languages`, `determinism_class`, `seed`, and
  `generation_parameters`. `SynthesisRequest` gains a required `language` and a
  required `take`, and `SynthesisReport` replaces its two loose identity strings
  with the `SynthesisContext` the executor actually used. Nothing is defaulted, because
  every one of them is a speech-affecting input and a default would silently
  name a cache entry that does not describe the audio in it.
- Unknown-field behavior: unchanged; every project-owned JSON boundary keeps
  strict Serde deserialization and rejects unknown fields and enum values.
- Wire or Rust representation changed: Rust representation, durable cache
  keys, and worker frames. `worker_frames` moves from `e0.worker.0.1` with
  extension `e0.worker.0.2` to breaking baseline `e1.worker.1.0` with extension
  `e1.worker.1.1`. The new major collects the accumulated semantic narrowings,
  adds the required `health` method, and makes correlation IDs bounded ASCII.

Two other project-owned documents are versioned in the same change, and each is
recorded here rather than as its own record because neither exists
independently of the identity work.

### Lesson document: `0.1-skeleton` → `1.0` → `1.1`

`0.1-skeleton` is not a `<major>.<minor>` version at all, so this build refuses
it as *malformed* rather than as an old version. That is the honest report:
nothing can tell which layout that label described.

- **`1.0` — breaking.** `language` becomes required. ADR-0001 §12.5 makes the
  language a synthesis-key input, so it cannot be defaulted, and a new required
  field is a breaking change under
  [`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
  §Change classes. This is the increment the E0-S4 audit found missing: the
  previous version of this record made `language` required while leaving
  `LESSON_SCHEMA_VERSION` at `0.1-skeleton`, so previously valid documents began
  failing under a version that had not moved.
- **`1.1` — compatible extension.** The optional `$schema` link is added, with
  absent as its declared default. A `1.0` document therefore remains valid, and
  `t3_e1_compatible_minor_extension_is_accepted` reads a committed `1.0` fixture
  to prove it. When the link *is* present it must name the schema for the
  version the document declares; a link to another schema is refused, because it
  means the document was checked against rules this build does not apply.

The rule itself is `SchemaVersion::accepted_by` in
`crates/study-tts-core/src/schema.rs`: a different major is refused, a newer
minor is refused, and an older minor of the same major is read with the defaults
its extensions declared. Refusing forward and accepting backward is what lets an
author keep a document that predates an extension while never letting this build
guess at one it has not seen.

### Cache artifact: `0.1-skeleton` → `1.0`

`artifact.json` gains a required `provenance` object recording the identities
the worker reported. `CACHE_SCHEMA_VERSION` is itself an ADR-0001 §12.5
synthesis-key input, so this moves every cache key a second time within E1-S1.
Entries written under the old version are refused as incompatible rather than
read, which is the intended effect: an entry with no provenance cannot answer
the question the provenance was added to answer.

## Impact

- Synthesis identities affected: **all of them.** The identity moved from six
  fields under `identity_version: "e0-s0-v1"` to the complete ADR-0001 §12.5
  input set under `e1-s1-v1`, the byte form moved to the canonical form in
  `study-tts-core/src/canonical.rs`, the language became a checked and
  case-normalized `LanguageTag`, and `CACHE_SCHEMA_VERSION` moved to `1.0`.
- Verification identities affected: none existed before this change.
  `VerificationContext`, `VerificationKey`, and `VerificationIdentityRecord` are
  new, under `VERIFICATION_IDENTITY_VERSION = "e1-s1-v1"`.
- Worker-bundle identities affected: **all of them.**
  `WORKER_BUNDLE_IDENTITY_VERSION` moved to `e1-s1-v2` when the derivation
  stopped accepting a caller-supplied input list and began reading
  `worker/bundle-manifest.json` and walking the import roots it declares to
  check that the list is complete. It moved again to `e1-s1-v3` when the
  derivation began refusing a lock without explicit sources, artifact kinds,
  and one artifact hash per index-supplied pin.
- Plan, takes, or package identities affected: every plan hash changes, because
  `PlannedSegment` gains `take` and the plan is hashed canonically. The takes
  document is new at `1.0`; no takes file exists to migrate.
- Consumers and commands affected: `study-tts-runtime`'s preview orchestration
  and `study-tts-testkit`'s fakes. No product command exists at E1-S1; the CLI
  reports the tested E1-S1 baseline until E1-S5 adds product commands.
- Fakes and shared suites affected: `FakeTtsExecutor` reports the descriptor's
  own context rather than two unrelated digests. This is not cosmetic — the
  cache now recomputes the key from the report and refuses a mismatch, so the
  previous fake could not have published at all.
- Fixtures and schemas affected: both lesson fixtures move to `1.1` and gain
  `$schema`; four lesson fixtures, one worker-session fixture, and three audio
  fixtures are added; every affected row in
  [`../testing/TEST-DATA-MANIFEST.md`](../testing/TEST-DATA-MANIFEST.md) carries
  new SHA-256 values. `schemas/` is populated with all seven documents.
- Existing cached artifacts affected: none exist outside developer working
  trees. Any that do are not reused, because their keys no longer match — which
  is the intended effect rather than a side effect.
- Published packages or accepted takes affected: none exist.

## What this change closes

The previous revision of this record acknowledged that the E1-S1 baseline was
incomplete. Each acknowledged gap is now closed, and each is closed by a named
test rather than by assertion:

| Gap | Closed by |
|---|---|
| Published lesson schema accepted any string for `$schema`, `schema_version`, `language`, and portable identifiers | `enum`, `const`, `pattern`, and length constraints derived from `LESSON_SCHEMA_VERSION`, `schema_uri`, `is_portable_id`, and `MAX_LANGUAGE_TAG_BYTES`; `t3_e1_the_published_lesson_schema_refuses_the_invalid_fixtures` |
| Bundle hash trusted the manifest's declared Python runtime and platform ABI | `WorkerBundle::verified_hash` probes the interpreter at `WORKER_INTERPRETER_PATH`; `t4_e1_an_interpreter_disagreeing_with_the_manifest_is_refused` |
| Cache hits compared the recorded key but never re-derived it from the recorded provenance | `CacheEntryFault::ProvenanceKeyMismatch`; `t1_e1_an_entry_whose_provenance_does_not_derive_its_key_is_refused` |
| Worker frame parameters were unchecked below the top level, and the frame ceiling was applied after the line had been buffered | `worker/study_tts_worker/protocol.py` per-method shapes and `read_line`; `worker/tests/test_protocol.py` |
| No `worker/` environment or lock procedure | `worker/pyproject.toml`, `worker/requirements.lock`, `worker/launcher.json`, `worker/study_tts_worker/`, and [`../operations/WORKER-ENVIRONMENT.md`](../operations/WORKER-ENVIRONMENT.md) |
| No `schemas/` | Seven generated schemas; `t3_e1_generated_schemas_match_checked_in_files` |
| Lesson schema not published at a stable path, no `$schema` in lessons | `schemas/lesson-v1.schema.json`; `t3_e1_published_lesson_schema_validates_every_example` |
| None of the named schema/version tests | `t3_e1_unknown_major_version_is_rejected`, `t3_e1_compatible_minor_extension_is_accepted` |
| Bundle hash derived from a caller-supplied list | `worker/bundle-manifest.json` plus a walk of every declared import root; `t1_e1_a_module_under_an_import_root_the_manifest_omits_is_refused` |
| Executor provenance ignored at publication | `AudioError::SynthesizerIdentityMismatch`; `t1_e1_audio_reported_under_other_identities_is_not_published` |
| Language accepted as any nonblank string, absent from the request | `LanguageTag`, `BackendDescriptor::languages`, `BackendValidationError::UnsupportedLanguage` |
| No separate reference-machine workflow | `.github/workflows/qualification.yml`, split from a fast `.github/workflows/ci.yml` |

## What the E1-S1 audit closed

An audit of this change before acceptance found nine further gaps, seven of them
Major. They are recorded here rather than folded silently into the table above,
because six of the seven describe a control that *looked* enforced and was not —
which is the class of defect this record exists to make visible.

| Gap | Closed by |
|---|---|
| The qualification workflow checked out with `actions/checkout`'s default clean, whose `git clean -ffdx` deletes the ignored `worker/.venv` the job then required | The qualified environment lives outside the checkout and is linked in at `WORKER_INTERPRETER_PATH`; `.github/workflows/qualification.yml` refuses rather than builds one, and [`../operations/WORKER-ENVIRONMENT.md`](../operations/WORKER-ENVIRONMENT.md) §Restoring the environment owns the procedure |
| `verified_hash` accepted any caller-chosen interpreter, so the ABI check could be satisfied by a Python that never runs the worker | The interpreter is no longer a parameter; `WORKER_INTERPRETER_PATH` is resolved beneath the bundle root |
| The manifest decided its own completeness: an empty `import_roots` switched the walk off, and a dropped `worker/launcher.json` left an inference-affecting input outside a hash that still succeeded | `REQUIRED_BUNDLE_INPUTS` and `REQUIRED_IMPORT_ROOT`, applied before any input is read; `t1_e1_a_manifest_omitting_a_required_input_is_refused`, `t1_e1_a_manifest_declaring_no_import_root_is_refused` |
| A bounded, syntactically valid JSON frame could end the worker process: `json.loads` raises `ValueError` past CPython's 4300-digit integer limit and `RecursionError` on deep nesting, neither of which is `JSONDecodeError` | Both converted to `FrameError`, plus `MAX_JSON_NUMBER_DIGITS` and `MAX_JSON_NESTING_DEPTH`; `HostileFrameTests.test_the_worker_answers_the_frame_after_a_hostile_one` proves the process survives to answer the next frame |
| Every published digest accepted any string although the parser requires 64 lowercase hexadecimal characters, and `schema_version` and `protocol_version` were unconstrained | `BLAKE3_HEX_PATTERN` on every digest, `SCHEMA_VERSION_PATTERN`, and `enum`/`const` version constraints derived from the values each parser accepts; `t3_e1_the_published_digest_pattern_accepts_exactly_what_the_parser_does` |
| A persisted plan carried no document version, so the E2 loader would arrive at files that never said what they were | `RenderPlan::schema_version` and `PLAN_SCHEMA_VERSION`; `t3_e1_the_published_plan_schema_describes_what_the_planner_writes` |
| Only lesson fixtures proved schema and parser agree; the other six formats had none | Valid and invalid fixtures for takes, verification, and job, invalid fixtures for plan and manifest, and a coverage assertion over `PUBLISHED_SCHEMAS`; `t3_e1_every_published_format_has_an_example_its_schema_and_parser_both_accept`, `t3_e1_every_published_format_refuses_a_document_at_the_field_that_is_wrong`, `t4_e1_the_published_manifest_schema_describes_what_a_package_writes` |
| Protocol frames used ordinary `sys.stdout`, so any Python or native write to descriptor 1 could corrupt NDJSON, and the launcher's offline environment was read but never applied | `protocol.reserve_protocol_stream` and `worker._apply_offline_environment`, both before any backend import; `ProtocolStreamTests` and `OfflineEnvironmentTests` in `worker/tests/test_worker.py` |
| The documented restore installed `chatterbox-tts` from the lockfile, letting the index satisfy the pin so the later governed install found the requirement already satisfied and did nothing | The index install excludes that one line, and the governed install is followed by a PEP 610 `direct_url.json` provenance check; [`../operations/WORKER-ENVIRONMENT.md`](../operations/WORKER-ENVIRONMENT.md) §Verify the provenance |
| `CanonicalValue::object` kept the last of two duplicate keys, so a typo in a cache-identity literal silently dropped a speech-affecting input | The duplicate is a panic with the field named; `t1_e1_a_repeated_identity_field_is_refused_rather_than_collapsed` |
| `LanguageTag` documented the RFC 5646 `langtag` production but omitted extlang and accepted a repeated variant, which §2.2.5 forbids | The accepted subset is named and the parser matches it; `t1_e1_tags_outside_the_accepted_grammar_are_refused` covers `zh-yue` and four repeated-variant spellings |

Two of these move a published boundary and are noted for the reader who is
comparing this record against the previous revision. `verified_hash` lost its
`interpreter` parameter, which is a breaking change to a signature no consumer
outside this workspace has yet. `RenderPlan` gained a first field, which changes
`plan-v1.schema.json` and not `plan_hash`: the hash names the segments to be
synthesized, and a document layout is not one of them, so no cache key moves for
this reason.

### The two the audit left open

That audit closed its findings but deliberately left two items for a decision
rather than resolving them in place. Both are now closed, and both are recorded
here because each touches a published boundary.

| Item | Closed by |
|---|---|
| `worker_bundle_hash` and `voice_profile_hash` were `String` on the worker frames, so `worker-protocol-v0.schema.json` published no digest constraint for either and a frame naming a truncated identity parsed cleanly | `WorkerBundleHash` and the new `VoiceProfileHash` on `InitializeParameters` and `WorkerResponseFrame::SynthesisSucceeded`, carrying `BLAKE3_HEX_PATTERN` into the published schema; `t1_e1_a_frame_naming_an_identity_that_is_not_a_digest_is_refused` |
| The Python end accepted any string for the same field, so a rule the Rust end enforced could still be sent past it | `protocol.blake3_hex` on `worker_bundle_hash`; `ParameterShapeTests.test_an_identity_that_is_not_a_digest_is_refused` |
| `validate_package` read two manifest layouts while `manifest-v0.schema.json` described one, and nothing held those two facts together | `t3_e1_the_published_manifest_schema_names_every_layout_it_describes`, which holds the published `const` against the layouts the parser dispatches on. (Closed at the time by a `ManifestLayout` enum; §What the second audit closed replaced it with two constants for the reason recorded there.) |

The worker-frame typing is the one to look at twice, because it narrows what a
frame may carry. It is classified as a **compatible patch** rather than a
breaking contract, and the argument is that no conforming frame changes:

- The wire shape is identical. Both fields were and remain JSON strings; the
  value objects serialize through `into = "String"`.
- The contract already required a digest. Both fields are documented as
  identities, the shared contract test asserted `is_blake3_hex` on them before
  this change, and the cache refuses a report whose identities do not recompute
  its key. The only frames newly refused are ones that could never have
  published.
- No durable artifact is affected. Frames are transient, and the one committed
  session fixture already carries real digests.

The later protocol audit applies the stricter reading: narrowing a field's
accepted values is a semantic change and therefore breaking under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes. The worker protocol therefore moves to `e1.worker.1.0` and its
extension to `e1.worker.1.1`, together with the session fixture, the published
schema, the Python constants, and
[`PROVISIONAL-CONTRACT-BASELINE.md`](PROVISIONAL-CONTRACT-BASELINE.md).

Holding the schema against the parser moves no boundary:
`manifest-v0.schema.json` regenerates with the same `schema_version` `const`,
because the layout it publishes and the string it pins are both unchanged.

## What the second audit closed

A second audit of this story raised six Major findings and one Minor. All seven
are closed here. They are recorded in this document rather than in a new one
because none of them changes the *classification* of E1-S1's interface change —
each closes a gap between what a boundary claimed and what it enforced.

| Finding | Closed by |
|---|---|
| The Python and Rust ends of the worker protocol implemented different contracts: Python refused the `e0.worker.0.2` extension version outright, accepted a `trace_context` under `e0.worker.0.1`, accepted an empty `request_id`, kept the last of two duplicate keys, and — having no integer width — accepted values `serde_json` refuses | Both ends now enforce the same version set, extension gate, identity rule, duplicate-key rule, and field widths, and both are driven from one committed file: `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson`, read by `t3_e1_both_protocol_ends_decide_the_committed_cases_alike` and by `SharedContractCaseTests` in `worker/tests/test_protocol.py` |
| Published schemas described `u32`, `u64`, and `usize` through `format` annotations with no `maximum`, so every published document admitted integers its Rust parser refuses; two wire fields were `usize`, whose width is the reader's pointer size | `publish_integer_bounds` writes the `maximum` each fixed-width format implies, applied to every document at publication rather than per field; `InitializeParameters::threads` and `WorkerCapabilities::max_text_bytes` became `u32` and `u64`; `t3_e1_every_published_numeric_field_declares_the_range_it_accepts` |
| Fields named `*_hash` and `*_blake3` were unrestricted `String`s in the verification, job, and manifest records, so those published schemas admitted malformed digests — and `VerificationContext::key_for` derived a well-formed key from a malformed profile hash, which `VerificationIdentityRecord::validate` then accepted | `VerificationProfileHash`, `VoiceConditioningHash`, `ManifestDigest`, `ToolProfileHash`, and a parseable `PlanHash`, at every recorded-digest boundary those three schemas publish; the hand-written `is_blake3_hex` checks and their four `DurableStateError` variants are deleted, because the refusal now carries each digest type's own remedy routing. Malformed fixtures for all three formats: `e1-s1-verification-malformed-profile-hashes.json`, `e1-s1-job-malformed-digests.json`, `e1-s1-manifest-malformed-digests.json` |
| `worker.py` copied every entry of the launcher's `offline_environment` into `os.environ`, so `worker/launcher.json` — a declared bundle input, and therefore a file that reads as governed — could set `PYTHONPATH` for the backend import one statement later. An adversarial check injected `PYTHONPATH=/tmp/injected` | `LAUNCHER_SHAPE` describes the launcher's complete shape and refuses an unknown field at either level, and the apply loop runs over `REQUIRED_OFFLINE_ENVIRONMENT` and `OPTIONAL_OFFLINE_ENVIRONMENT` rather than over the file; `LauncherShapeTests` in `worker/tests/test_worker.py` drives both halves |
| `verified_hash` proved the interpreter's ABI and hashed the lockfile's *bytes*, which says what that file claims and nothing about the environment beside it: a distribution upgraded in place, or a `chatterbox-tts` the configured index satisfied at the same version, left every declared input byte-identical and every cache key where it was | The same probe now reports every installed distribution and its PEP 610 `direct_url`, and `check_environment_matches_lock` compares both against `worker/requirements.lock` before a hash is returned; `EnvironmentMismatch` names which of the four faults occurred, and never prints the recorded URL; `t4_e1_an_environment_that_is_not_the_locked_one_is_refused`, `t4_e1_a_lockfile_line_that_is_not_an_exact_pin_is_refused` |
| The evidence record and this document disagreed: 34 walking-skeleton tests recorded against 35 run, `cargo deny` recorded as warning-free while it reports a duplicate `cpufeatures`, and real-model qualification pointed at `.github/workflows/qualification.yml`, which states in its own comments that it invokes no such measurement | Corrected above and in `evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v2.md`, which supersedes v1's controlled-record table and re-pins every digest |
| `ManifestLayout::ALL` was a hand-maintained variant list — the pattern this repository's review standard rejects, because a third variant is silently missing from it | The enum is gone. `parse_stored_manifest` matches the two accepted version strings directly, and `t3_e1_the_published_manifest_schema_names_every_layout_it_describes` now proves the published `const` and the dispatcher's fail-closed arm rather than walking a list |

### What this moves

**Every cache key moves again**, and for one reason only: `worker/study_tts_worker/protocol.py`,
`worker/study_tts_worker/worker.py`, `worker/study_tts_worker/__init__.py`, and
`schemas/worker-protocol-v0.schema.json` are declared bundle inputs, so their
bytes change the worker-bundle hash. `WORKER_BUNDLE_IDENTITY_VERSION` stays at
`e1-s1-v2` because the *derivation* is unchanged — the environment check is a
gate before the hash, not a term in it, and adding it to the digest would make
the identity depend on an environment rather than on the bundle.

`SYNTHESIS_IDENTITY_VERSION` and `VERIFICATION_IDENTITY_VERSION` stay at
`e1-s1-v1`: no input was added to or removed from either key. Typing
`voice_conditioning_hashes` and the three verification profile hashes changes
what those fields *accept*, not what they contribute — the canonical bytes are
the same string they always were.

**Published schema changes.** All seven documents regenerate. Six gain integer
`maximum`s; `job-v0`, `manifest-v0`, and `verification-v1` additionally gain
`BLAKE3_HEX_PATTERN` on the digest fields listed above; `worker-protocol-v0`
gains the two fixed widths and the zero-to-one `progress` range. Every one of
these **narrows** what the schema admits to what its parser already accepted, so
no document this build reads today stops being read. Under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes the schema-only corrections are **compatible patches** because
they publish rules the parsers already applied. Runtime frame narrowings are
absorbed into the breaking `e1.worker.1.0` baseline.

The Python parity repair itself widened acceptance of the then-current extension
version. The later breaking baseline carries that parity forward at
`e1.worker.1.0` and `e1.worker.1.1`.

### What it does not close

- **The environment check cannot run in ordinary CI.** It needs the restored
  `worker/.venv`, which ordinary CI does not have;
  `.github/workflows/qualification.yml` is the automated route that attaches
  one. On a machine without one, `verified_hash` refuses with
  `ToolError::MissingTool` exactly as it did before, and the
  `EnvironmentMismatch` faults are proved against a scripted interpreter
  rather than a real one.
- **Provenance is checked only where the lockfile declares it**, which today is
  `chatterbox-tts` alone. A distribution installed from a local wheelhouse
  writes a `direct_url.json` too, and refusing that would be a stricter rule
  than the lock states.
- **The published `maximum` for a `u64` field is `u64::MAX` exactly.** A
  validator comparing through a double cannot represent it and admits one value
  the Rust parser refuses. That is one value looser rather than the unbounded
  schema it replaces, and narrowing the seed to buy it back would narrow the
  identity ADR-0001 §12.5 hashes.

## What the third audit closed

A third audit raised thirteen findings. Six were already closed when it
reported; the seven below were open, and all are closed here. They are recorded
in this document rather than in a new one for the reason the second audit's were:
none changes the *classification* of E1-S1's interface change, and each closes a
gap between what a boundary claimed and what it enforced.

| Finding | Closed by |
|---|---|
| The governed-source provenance check compared a `code-<commit>` **path component**, and a component proves a directory *name*. A directory install records PEP 610 `dir_info` and no `commit_id` at all, so the tree at `code-<commit>` could hold any bytes and `code-<commit>-backup` beside it is a name an operator really creates | The governed distribution is installed from the tree's git URL at its commit, so `pip` records the `vcs_info.commit_id` it checked out, and that is what is compared. The probe no longer reports the URL at all. A record with no revision is `EnvironmentMismatch::WithoutRecordedRevision`, distinct from `::FromIndex` because the remedy is a different command rather than a different directory; `t4_e1_an_environment_that_is_not_the_locked_one_is_refused` |
| Extra installed distributions were ignored, and an extra distribution can ship a `.pth` — which runs at interpreter startup and joins `sys.path` ahead of the search that resolves `torch`, outside the bundle identity entirely | The probe reports every `.pth` in the interpreter's site directories with the distribution whose `RECORD` lists it, and a hook the lock does not account for is `EnvironmentMismatch::UnownedPathHook` or `::UnlockedPathHook`. The tolerance for extra *distributions* survives, and is what it was always resting on: `setuptools` is pinned and owns the one hook the reference machine has; `t4_e1_a_startup_hook_the_lockfile_does_not_account_for_is_refused` |
| The probe reported distributions as a map keyed by the canonicalized name, and that mapping is many-to-one: two installs collapsed to one entry silently, and the comparison answered for whichever was walked last | The probe reports a list and the collision is `EnvironmentMismatch::AmbiguousDistribution`; `t4_e1_two_installs_canonicalizing_alike_are_refused_rather_than_collapsed` |
| `PlannedSegment::take` was dropped building `SynthesisRequest`, although the worker protocol requires `take` on every `synthesize` frame and the cache key encodes it | `SynthesisRequest::take`, carried from the planned segment; `t1_e1_a_synthesis_request_carries_the_take_its_cache_key_names` edits the plan rather than reading only what the planner writes, because `for_lesson` selects `BASE_TAKE` for every segment and a test reading that would pass for a mapping that hard-coded zero |
| `SelectedTake::segment_id` was unvalidated and unconstrained and the selection list had no count cap, while the lesson boundary refuses the same values | `validate_segment_id`, extracted from `AuthoredLesson::validate` so both boundaries apply one rule, plus the lesson's own segment ceiling on the list; `takes-v1.schema.json` publishes the portable-identifier pattern and `maxItems`; `t1_e1_a_selection_naming_an_identity_no_lesson_can_carry_is_refused`, `t1_e1_takes_selection_ceiling_accepts_the_boundary_and_is_the_lesson_ceiling` |
| `threads` accepted zero at both protocol ends and in the published schema — a value no application could honor — while `worker/launcher.json` says 4 and the session fixture says 1 and nothing reads either | `InitializeParameters::threads` is `NonZeroU32`, `protocol.positive` is the Python end, and `worker-protocol-v0.schema.json` publishes `minimum: 1`. The new `zero-threads` case in `fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` proves both ends refuse it alike. Applying the count is still E1-S3's; refusing a value no application could honor is the parse's, and the launcher and the fixture are free to differ until something reads them |
| `crates/study-tts-testkit/src/json_schema.rs` did not implement `maxItems`, so the takes bound would have been published unchecked | `check_item_count`; the validator's unimplemented-keyword refusal is what surfaced it, which is the behavior that rule exists for |

Two more were found by **running the documented restore end to end** rather than
by reading it, and both are recorded here because each defeated a control that
looked enforced. Neither was reachable from a test before, and neither could be
seen without a restored environment:

| Finding | Closed by |
|---|---|
| `verified_hash` resolved the interpreter through `tools::resolve_executable`, which canonicalizes. A virtualenv's `bin/python` is a symlink chain to the base interpreter and `worker/.venv` is itself a link, so the probe ran `/usr/bin/python3.12` — `sys.prefix` `/usr`, none of the locked distributions present. **The environment check had never once read the environment it exists to read**, and the argument that "which interpreter is asked is not a parameter" was true of a path nobody ran | `tools::executable_in_place`, which checks the file is executable without resolving where it points; `t4_e1_the_interpreter_is_probed_where_it_is_attached_not_where_it_resolves`. Every earlier interpreter in the suite was a regular file, and canonicalizing a regular file returns the same file — which is why the stand-in in that test answers differently depending on the path it is invoked through |
| The probe recorded the first platform tag `packaging` yields. On Linux that is the bare `linux_x86_64`, which is the same string on glibc 2.31 and glibc 2.39 — so two environments loading different compiled wheels would have shared one bundle identity, and it could never match the `manylinux_2_39_x86_64` the manifest declares | The probe skips the bare tag and records the first `manylinux_*` or `musllinux_*` behind it. ADR-0001 §12.5 hashes *platform ABI* identity and [`../operations/REFERENCE-ENVIRONMENT.md`](../operations/REFERENCE-ENVIRONMENT.md) records the reference machine's glibc, so the manifest's declaration was right and the probe was the side that had to move — `worker/bundle-manifest.json` is unchanged and no cache key moves for this reason |

### What this moves

**Every cache key moves again**, for the same one reason as last time and no
other: `worker/study_tts_worker/protocol.py` and
`schemas/worker-protocol-v0.schema.json` are declared bundle inputs, so their
bytes change the worker-bundle hash. `WORKER_BUNDLE_IDENTITY_VERSION` stays at
`e1-s1-v2` — the environment and startup-hook checks are gates before the hash,
not terms in it, and folding an environment into the digest would make the
identity depend on the machine rather than on the bundle.

`SYNTHESIS_IDENTITY_VERSION` and `VERIFICATION_IDENTITY_VERSION` stay at
`e1-s1-v1`, and **no plan hash moves**. `SynthesisRequest` is a Rust value
passed to an executor, not a hashed document; `PlannedSegment::take` was already
a plan field and already a term of `synthesis_digest`.

**The restore procedure breaks, deliberately.** An environment restored by the
previous procedure has `chatterbox-tts` installed from a directory, so
`verified_hash` now refuses it with `WithoutRecordedRevision`. That is the
finding rather than a side effect of it: such an environment never proved which
revision it holds. `docs/operations/WORKER-ENVIRONMENT.md` §Install the governed
Chatterbox source explicitly carries the replacement command, and the reference
machine must reinstall that one distribution before the bundle hash can be read.

**Published schema changes.** `takes-v1` narrows `segment_id` to the portable
identifier pattern and bounds `selections` at the lesson's segment ceiling;
`worker-protocol-v0` narrows `threads` to `minimum: 1`. Both narrow a published
document to what its parser now accepts, and both are narrower than what the
*previous* parser accepted — a takes document with an unusable segment identity,
or a frame with zero threads, parsed before and does not now. Under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes the takes-schema repair is a compatible patch because it
publishes a rule the parser already applied. The zero-thread frame refusal is a
semantic narrowing and is absorbed into the breaking `e1.worker.1.0` baseline.
No takes document and no persisted frame exists to migrate.

### What it does not close

- **The startup-hook rule is scoped to `site.getsitepackages()`.** That is the
  set `site` processes `.pth` files from, and `-I` keeps the user site directory
  out of it, so it is the right set for the interpreter the probe runs — but a
  `PYTHONPATH` entry in the *worker's own* launch environment is a different
  question, answered by the launcher allowlist rather than here.
- **A hook owned by a locked distribution is accepted on the strength of that
  distribution's pin**, not on its contents. `setuptools==78.1.0` fixes what
  `distutils-precedence.pth` contains, which is the same argument the lockfile
  makes for every other file a distribution installs.
- **The environment and startup-hook checks still cannot run in ordinary CI**,
  for the reason §What it does not close already records: they need the restored
  `worker/.venv` that ordinary CI does not have. They have now run against a
  restored environment on a developer machine — see
  [`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v3.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v3.md)
  §Verification run, which records the first worker-bundle hash this project has
  read, and
  [`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v4.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v4.md)
  §The worker-bundle hash for the value it holds now — but that machine is not the reference machine, and the run was by hand
  rather than through the workflow.

## What the fourth audit closed

A worker-tree audit closed three disagreements at existing boundaries. It adds
no backend, dependency, protocol field, or launcher field.

| Finding | Closed by |
|---|---|
| Python accepted a frame containing a carriage return, although the Rust frame parser refuses either line-terminator byte | `read_request` refuses `\r` and `\n` before decoding; `EnvelopeTests.test_a_frame_cannot_carry_a_carriage_return` |
| A future launcher with a new field was reported as malformed under the current layout, hiding the unsupported version that made its shape unknowable | `_load_launcher` checks a string `schema_version` before applying the current layout; `LauncherShapeTests.test_a_future_launcher_is_refused_by_version_before_its_fields` |
| A missing, unreadable, non-UTF-8, or malformed launcher escaped startup as a raw file or JSON exception despite the launcher's documented `SystemExit` boundary | `_load_launcher` translates read and decode failures into one startup error; `LauncherShapeTests.test_an_unreadable_launcher_stops_as_a_startup_error` |

### What this moves

**Every worker-bundle identity moves.** Both changed production modules are
declared inputs in `worker/bundle-manifest.json`; the tests are not. The input
set and derivation are unchanged, so `WORKER_BUNDLE_IDENTITY_VERSION` remains
`e1-s1-v2`. The restored locked environment derived
`e3f81a79b455ab922aa11b452586b7d27ec8922293111cfe38ff8e3c9f532328`
twice on 2026-08-28.

This is a compatible repair, not a wire-version change: Python now enforces the
line framing the Rust end already enforced, and `worker/launcher.json` is local
startup configuration rather than a worker frame. Existing cache entries are
left in place but cannot match the new bundle identity. The accepted v3
qualification report remains the immutable record of the bundle it measured;
it is not amended with this later digest.

## What the fifth audit closed

A published-schema audit closed three disagreements between what this project
publishes and what it enforces. It adds no backend, dependency, protocol field,
or document field.

| Finding | Closed by |
|---|---|
| `takes-v1` and `verification-v1` published `$schema` as `{"type": ["string", "null"]}` — any link accepted — while `TakesDocument::validate` and `VerificationIdentityRecord::validate` both refuse a link naming another schema. Only the lesson published the rule its parser enforced, so an author writing a takes document linked to `lesson-v1.schema.json` had a green editor and a build that refused it | The rule moved to `study_tts_core::schema::schema_link_json_schema`, which all three documents now call, for the reason `accepted_versions_json_schema` beside it already gives: three copies of one rule are three chances to publish a different one. `t3_e1_every_published_schema_link_is_constrained_to_its_own_schema` |
| No published schema declared `$id`, so nothing but the file name connected the URI a document carries to the file holding the schema it names. A tool handed both could not tell they were the same thing | `$id` is published at `PublishedSchema::generate`, beside `publish_integer_bounds` and for the same reason — it belongs to publication, not to seven types remembering an attribute. Declaring a name is not promising to resolve it: `SCHEMA_URI_BASE` stays deliberately unresolvable under RFC 2606. `t3_e1_every_published_schema_claims_the_uri_its_documents_name` |
| `TEST-DATA-MANIFEST.md` typed the four single-frame `e0-s4-worker-*.json` fixtures as `Worker NDJSON`, the same label it gives the two multi-frame `.ndjson` files. One label for two formats | Retyped `Worker frame JSON`. The label now separates a single request or response frame from a session of them |

### What this moves

**Every worker-bundle identity moves again**, for one reason and no other:
`schemas/worker-protocol-v0.schema.json` is a declared input in
`worker/bundle-manifest.json`, and it gained `$id`. No frame shape changed, so
`worker/study_tts_worker/protocol.py` is unmoved and both protocol ends still
decide the committed cases alike. The input set and derivation are unchanged, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v2`. The restored locked
environment derived
`f9a0c8f25e322aa7eeb34382a45dd702be72df7b33e476543c0907a0728e9ec4`
twice on 2026-08-28, against
`e3f81a79b455ab922aa11b452586b7d27ec8922293111cfe38ff8e3c9f532328`
with the change reverted.

`SYNTHESIS_IDENTITY_VERSION` and `VERIFICATION_IDENTITY_VERSION` stay at
`e1-s1-v1`, and **no plan hash moves**. No input was added to or removed from
either key; the bundle hash that is one of them holds a different value.

The two schema narrowings are **compatible patches** under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes: each publishes a constraint the parser already applied, so no
document this build accepts becomes invalid and every version is retained.

## What the sixth audit closed

A full-tree audit found that every published string pattern used `$` as its end
anchor. JSON Schema uses ECMAScript regular expressions, where `$` also matches
immediately before a final line terminator. The schemas therefore accepted
values such as `"lesson\n"`, `"en\n"`, and a 64-digit digest followed by a
newline although their Rust parsers reject each one.

The four pattern bodies now end in `$(?![\s\S])`. The fixed negative lookahead
makes the match position the absolute end under ECMAScript without changing the
accepted parser language. The test validator translates only that suffix to
Rust regex `\z`; every other lookaround remains unsupported and fails closed.
`t1_e1_the_published_patterns_accept_and_refuse` adds the two short trailing-
newline cases, and
`t3_e1_the_published_digest_pattern_accepts_exactly_what_the_parser_does`
retains the digest case through the same JSON Schema boundary.

The same audit closed two fail-open paths in the test-only schema validator. A
subschema that was a number, string, or null was treated like boolean `true`,
and a `type` array silently dropped non-string entries. Both are malformed JSON
Schemas, not permissive ones; both now report a schema error, as do empty type
arrays and unknown type names.

These are compatible patches under §Change classes for the same reason as the
earlier schema narrowings: each rejects only documents the owning Rust parser
already rejected.

### What this moves

Every generated schema containing a pattern moves. The worker-protocol schema
is a declared bundle input, so every worker-bundle identity moves from
`f9a0c8f25e322aa7eeb34382a45dd702be72df7b33e476543c0907a0728e9ec4` to
`8f11f9edc75096688b0d2f17ceed5eec767c32cb42aefa6c4f346ff68df844c5`.
The restored locked environment returned the latter value twice on 2026-08-28.
The input set and derivation did not change, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v2`.

A render plan built with that worker identity receives new synthesis keys and
therefore a new plan hash. The checked-in fake executor carries its own stable
bundle digest, so its golden plan does not move. Existing cache entries remain
valid under the identity that produced them and are not reused under the new
one; none is deleted or re-keyed.

## What the seventh audit closed

The Python environment was version-pinned but not artifact-locked. The same
versions could resolve to different bytes, the `+cpu` PyTorch source depended
on machine-local installer configuration, and the documented governed install
said `--force-reinstall` was mandatory while omitting it from the command.

`worker/requirements.lock` now names PyPI and the official PyTorch CPU index,
defaults to wheels, declares `s3tokenizer` as the sole sdist exception, and
binds every index-supplied pin to the SHA-256 of the artifact selected for the
reference ABI. The governed `chatterbox-tts` pin remains the sole non-artifact
exception and is bound to its adjacent Git commit. The restore first acquires
with `--require-hashes`, then installs offline from that wheelhouse with
`--no-index`, `--no-deps`, `--force-reinstall`, and a pinned, non-isolated
build environment for the one sdist. Its VCS command now includes the required
`--force-reinstall`.

This is enforced before a worker identity is returned. `parse_lockfile`
requires the four exact `REQUIRED_LOCK_DIRECTIVES` and exactly one well-formed
SHA-256 for every index pin; it refuses an unknown or repeated directive, an
unhashed or multiply hashed index pin, and any artifact hash on the governed
pin. `t4_e1_every_index_pin_requires_one_artifact_hash` and
`t4_e1_the_lock_records_its_package_sources_and_artifact_kinds` cover those
failures, and `t4_e1_a_lockfile_fault_no_line_carries_names_no_line` covers the
three it reports against the file rather than a line.

### What this moves

**Every worker-bundle identity moves.** `worker/requirements.lock` is a
declared bundle input, and the validity rule for that input is now part of the
derivation. `WORKER_BUNDLE_IDENTITY_VERSION` therefore moves from `e1-s1-v2`
to `e1-s1-v3`. The restored locked environment returned
`9ef560e8f884f50dc23bd0bc88d41aff88ff58d8077fbe283adb0f297361108e`
twice on 2026-08-28. Existing cache entries remain valid under their original
identities and are not reused under the new one; none is deleted or re-keyed.

The artifact wheelhouse was restored into a fresh CPython 3.12.3 environment
without index access. The runtime imports, including `torch==2.6.0+cpu`,
`torchaudio==2.6.0+cpu`, and the sdist-built `s3tokenizer==0.1.7`, succeeded.
The governed source was not reinstalled into that disposable environment
because its model root is intentionally outside this repository; its existing
qualified install passed the mechanical PEP 610 check when the bundle hash was
read.

## What the eighth audit closed

Lesson `1.1` added `$schema` as an optional field so a `1.0` document without
it remains readable. That compatibility rule also left the schema metadata
writable: `AuthoredLesson` exposed `$schema` and `schema_version` as public
fields, so a caller could serialize a `1.1` document naming another schema, or
none, while every checked-in example still passed.

Both fields are now private, so no caller outside `study-tts-core` can write
them. `AuthoredLesson::new` is the publishing construction path: it supplies
the current version and `schema_uri(LESSON_SCHEMA_STEM, major)` itself, so a
generated document cannot omit or replace either value. Ingestion remains
separate through `ValidatedLesson::from_json`, which still accepts a compatible
earlier-minor document with no link.

This changes no JSON layout and does not tighten ingestion: absent `$schema`
remains the declared default for compatible earlier documents, and
`t1_e1_a_lesson_from_an_earlier_minor_version_is_accepted` still proves a
`1.0` lesson is readable. The Rust construction API is deliberately breaking
before G1: callers that previously wrote schema metadata by hand can no longer
reach those fields. No synthesis input, plan hash, cache key, generated
schema, or worker-bundle identity moves.

## What the ninth audit closed

The four-crate replacement was incomplete in one literal respect: the CLI
crate still identified itself as the E0-S0 placeholder even though the other
three crates implemented the E1-S1 contract baseline. It had no test proving
what the executable reported.

The dependency-free binary now reports the tested E1-S1 contract baseline and
names E1-S5 as the owner of product commands. The process-level
`t4_e1_status_executable_reports_the_contract_baseline` test pins its exit
status, stdout, and empty stderr. This adds no command parser or product
behavior and changes no durable schema or identity.

## What the tenth audit closed

Both ends bounded the correlation identity only by `MAX_WORKER_FRAME_BYTES`,
and a refusal has to repeat the identity it refuses. The parser also quoted the
rejected value at that point, so a frame at the ceiling produced an answer past
it, which the supervisor refuses for length — a sender chose whether its own
mistake was reported or dropped.

`MAX_WORKER_REQUEST_ID_BYTES` and `study_tts_worker.protocol.MAX_REQUEST_ID_BYTES`
are one 256-byte ceiling at both ends, applied at validation rather than on the
way out: `WorkerFrameError::RequestIdTooLong` and the byte-length check in
`request_identity`, published as `maxLength` on every `request_id` in
`worker-protocol-v0.schema.json` so a supervisor reads the bound instead of
discovering it from the first refusal it cannot match.
`MAX_REFUSAL_MESSAGE_CHARS` bounds the diagnostic `message` in `failure`, which
is the one place this worker builds a failure frame and therefore the one place
the bound cannot be forgotten.

The two halves are deliberately unlike each other. Prose truncated is still
prose, and the replacement keeps the original length so a reader can tell a
truncated message from one written that way. An identity truncated is a
*different* identity: it comes back looking like some other request that was
answered, and the supervisor correlates nothing while believing it did. So
`read_request` drops an oversized identity rather than echoing it shortened,
and answers as `unknown`.

`fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` gains
`request-id-at-the-ceiling` and `request-id-past-the-ceiling`, so the ceiling
is decided from one committed file rather than from two suites that would agree
only by coincidence — the same way its `empty-request-id` case already covers
the other identity rule, and the reason the file exists.
`t3_e0_worker_frame_ceiling_and_unknown_fields_fail_closed` covers the boundary
and the byte past it at the Rust end, `RequestIdentityCeilingTests` covers the
same rules at the Python end, and
`test_an_oversized_internal_diagnostic_stays_under_the_ceiling` proves the
failure builder keeps oversized diagnostics inside the frame ceiling.

The same audit closed a governed refusal that routed nowhere. All eleven
`WorkerBundleError` variants named the routing row `Worker bundle input missing
or oversized`, and §Failure routing in
[`../governance/ROUTING-TABLES.md`](../governance/ROUTING-TABLES.md) has no such
row — the one covering this boundary is `Worker protocol or containment
failure`, which every other worker-boundary refusal in the crate already named.
An operator following that advice was sent to a table entry that does not
exist, and nothing compared the string to the document.

Those eleven also shared one action across four different repairs. Restoring a
deleted input, restoring an environment that drifted from the lock,
regenerating the lock, and aligning the manifest with the layout this build
implements are not the same work, and an operator acts on the one they are
handed: a refusal that says an input is missing when the installed environment
drifted sends them to look for a file that is on disk.
`WorkerBundleError::remedy` now selects among the four. The owner and the
routing row stay single because the table gives this boundary one of each, and
`t1_e0_governed_remedy_mappings_are_exhaustive` pins all four by naming every
`WorkerBundleError` variant against the repair it carries.

The same audit closed a published link that did not resolve. §What the eighth
audit closed made `AuthoredLesson`'s schema-metadata fields private and left
`LESSON_SCHEMA_VERSION` documenting itself with an intra-doc link to one of
them, so `cargo doc` warned and the rendered link went nowhere. It names the
`$schema` link in prose instead, because the field it describes is private and
no public item stands in for it.

### What this moves

`worker/study_tts_worker/protocol.py` and
`schemas/worker-protocol-v0.schema.json` are both declared bundle inputs, so
**every worker-bundle identity moves**, from
`9ef560e8f884f50dc23bd0bc88d41aff88ff58d8077fbe283adb0f297361108e` to
`7e1c506f8ab81429f23b5edb7533beaa72dbae1a02d722c2bd289cd416b3be38`. The
restored locked environment returned the latter value twice on 2026-08-28.
Neither the input set nor the derivation changed, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v3`.

A render plan built with that worker identity receives new synthesis keys and
therefore a new plan hash. The checked-in fake executor carries its own stable
bundle digest, so its golden plan does not move. Existing cache entries remain
valid under the identity that produced them and are not reused under the new
one; none is deleted or re-keyed.

`fixtures/contracts/e1-s1-worker-protocol-cases.ndjson` moves with its two new
cases, so its row in
[`../testing/TEST-DATA-MANIFEST.md`](../testing/TEST-DATA-MANIFEST.md) carries a
new SHA-256. Neither file is a bundle input and neither reaches a synthesis,
plan, or cache identity. The `lesson.rs` link repair is a comment: no item,
signature, visibility, or serialized byte moves with it.

The remedy repair moves no identity either. Recovery advice is operator-facing
text produced at the moment of a refusal; it is not serialized, not hashed, and
reaches no lesson, plan, worker, cache, verification, or package document. What
it changes is where an operator is sent, which is the point of it.

The frame contract narrows without changing shape. §Change classes in
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
calls a frame change breaking, and this is a semantic narrowing of two existing
fields rather than the publication of a rule the parsers already applied —
which is what let §What the sixth audit closed classify its pattern anchors as
compatible patches. The later audit therefore moves `worker_frames` to
`e1.worker.1.0`; the bounded practical impact does not change its class.

## What the eleventh audit closed

The next audit found three disagreements in what the E1-S1 baseline called a
complete worker protocol:

| Finding | Closed by |
|---|---|
| The 256-byte request-ID ceiling narrowed accepted frames while the protocol retained its old major | Breaking `e1.worker.1.0` and extension `e1.worker.1.1`; the prior major is a shared refused fixture, and this record supplies impact, migration, and rollback |
| ADR-0001 §10.2 defines six methods while the request enum, executable fake, Python worker, and five-frame session omitted `health` | `WorkerRequestFrame::Health`, `WorkerResponseFrame::Health`, both executables, and a six-frame `t4_e1_fake_worker_passes_shared_protocol_contract` session; the E1-S1 Python worker truthfully reports `ready: false` and `model_loaded: false` |
| Rust and Python counted 256 UTF-8 bytes while JSON Schema `maxLength` counted Unicode characters | Request IDs are ASCII at both runtime parsers and carry the same pattern in `worker-protocol-v1.schema.json`; the shared 200-character `é` fixture is refused by Rust, Python, and the schema |

The new major also absorbs the earlier runtime frame narrowings recorded above:
typed digests, fixed integer widths, and nonzero threads. Schema changes that
only publish a rule the parser already enforced remain compatible patches.

### Migration, rollback, and identity impact

No frame is durable and no compatibility adapter can safely reinterpret an old
frame under new request-ID rules. Upgrade the Rust supervisor, Python worker,
fake, fixtures, and schema together; an `e0.worker.0.1` or `e0.worker.0.2` frame
is refused by version. Rollback reverts those consumers together. Cache entries
remain valid under the worker-bundle identity that produced them and are not
deleted or re-keyed.

The published schema moves from `worker-protocol-v0.schema.json` at `0.1` to
`worker-protocol-v1.schema.json` at `1.0`. That path and the executable protocol
modules are declared worker-bundle inputs, so every worker-bundle hash moves and
`WORKER_BUNDLE_IDENTITY_VERSION` advances from `e1-s1-v3` to `e1-s1-v4`. The
restored locked environment returned
`339e574dd6a06cbc1e5ce08a475e3f5f8de5e30f85e2218dd88b86ddebd24014`
twice on 2026-08-28.

## What the twelfth audit closed

The cancellation correlation identity remained an unrestricted string even
after the eleventh audit bounded every envelope `request_id`. Both workers echo
`active_request_id`, so a cancel request accepted at the 1 MiB frame ceiling
could produce a response beyond that same ceiling. Rust now validates the
field on both `Cancel` and `Cancelled`, Python uses `request_identity` for the
cancel shape, and the generated schema applies the same nonempty ASCII
256-byte rule. The shared decision fixture carries accepted 256-byte and
refused 257-byte cases; the shared subprocess session carries the exact
boundary through both worker executables and asserts the response remains a
readable frame.

E1-S1 task 4 also required generated lessons to carry the stable `$schema`
URI, while the eighth audit had deliberately left no egress path.
`AuthoredLesson::new` now owns current-version construction and always supplies
both metadata fields. `t1_e1_generated_lesson_includes_the_stable_schema_uri`
pins the serialized values. This does not add the E1-S5 CLI command; it gives
that later command the E1-S1 construction boundary it must call.

Finally, every malformed lockfile condition used one
`UnreadableWorkerLockfile` message claiming the named line was not an exact
pin. `WorkerLockfileErrorReason` now distinguishes invalid UTF-8, malformed
pins, artifact hashes, missing and repeated required directives, unsupported
directives, and governed-source provenance. The refusal still withholds line
contents and keeps the same worker/runtime remedy owner, but now names the
invariant the owner must repair.

Three of those invariants are the file's rather than a line's, and the typed
reason alone still attributed them to a line: the refusal carried a `usize`,
so invalid UTF-8 rendered as `line 0` and both the absent required directive
and the missing governed pin rendered one past the last line — a 42-line lock
reporting "line 43 omits a required resolution directive". The `line` field is
now a `WorkerLockfileLocus`, either `Line(n)` or `WholeFile`, and a whole-file
refusal reads "worker lockfile `worker/requirements.lock` as a whole is not
UTF-8". `t4_e1_a_lockfile_fault_no_line_carries_names_no_line` drives all three
through `verified_hash` and asserts the rendered message names no line, which
is the only place the fault was visible.

### Compatibility and identity impact

The active-ID rule completes the same pre-G1 breaking worker baseline the
eleventh audit introduced; it does not create another versioned shape.
Supervisor, workers, fixtures, and `worker-protocol-v1.schema.json` must still
move together, and an old-major frame remains refused rather than translated.

`worker/study_tts_worker/protocol.py` and the generated worker schema are
declared bundle inputs, so every worker-bundle hash moves from
`339e574dd6a06cbc1e5ce08a475e3f5f8de5e30f85e2218dd88b86ddebd24014`
to `8339a5b425781965527e299591a445a1c4452ecdbeea6756fa82fd401b8d508a`.
The declared input set and derivation do not change, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`. Existing cache entries
remain valid under their producing identity and are neither deleted nor
re-keyed.

The lesson constructor, the typed lockfile reason, and the `WorkerLockfileLocus`
that replaces its line number change Rust construction and diagnostic APIs
without changing any durable JSON shape or identity. The two
fixture files are not bundle inputs; their new SHA-256 values are recorded in
`docs/testing/TEST-DATA-MANIFEST.md`.

## What the thirteenth audit closed

The worker documented failure diagnostics as redacted, but its parser embedded
sender-controlled method and protocol-version values unchanged in
`FrameError`, and `_refusal` published that text on the protocol channel. The
same path exposed duplicate and unknown field names, rejected numeric values,
and interpreter exception text. A request using
`/private/voices/owner/reference.wav` as its method therefore returned that
voice path in a failure frame.

Parser refusals now name only the violated invariant, schema-owned JSON path,
and derived resource bound. They do not reproduce the rejected value or an
unknown field name. The subprocess regression sends sentinel lesson text and a
sentinel voice path through the formerly unsafe method and version branches,
then asserts neither stdout nor stderr contains either sentinel and that the
worker continues through shutdown.

### Identity impact

`worker/study_tts_worker/protocol.py` and
`worker/study_tts_worker/worker.py` are declared bundle inputs, so every
worker-bundle hash moves from
`8339a5b425781965527e299591a445a1c4452ecdbeea6756fa82fd401b8d508a` to
`5d77a5a6a520466043cb6a67ae805b148104d74d8c91fe85932b31d782d8b0af`.
The input set and derivation are unchanged, so
`WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`. Existing cache entries
remain valid under their producing identity and are neither deleted nor
re-keyed.

## What the fourteenth audit closed

The environment comparison held a locked distribution to its name, version, and
PEP 610 provenance, and held the site directories to their `.pth` files. None of
the three reads a byte of what is installed, so a Python dependency could change
while the bundle identity stood still. Editing an installed `torch` module in
place, or editing the *contents* of a `.pth` belonging to a locked distribution
— its file name and owning distribution both staying correct — left every
version, every provenance record, and every declared input byte-identical, and
`verified_hash` returned the value it returned before.

`site` also imports `sitecustomize` and `usercustomize` by name as the
interpreter starts, before any declared input is read. Neither is a `.pth`, so
the startup-hook rule never saw them, and the tolerance for unlocked
distributions — which rests on an extra install being inert — did not hold for
either. `-I` settles exactly one of the two: it clears `ENABLE_USER_SITE`, which
`site.main` gates `execusercustomize` on, and nothing suppresses `sitecustomize`.

The runtime probe now verifies each SHA-256-bearing `RECORD` entry beneath a
locked distribution's site-package root, and reports both startup modules with
whether the interpreter would actually import them. Generated wheel scripts
elsewhere in the interpreter environment are not imported by
`python -m study_tts_worker` and are not read. Non-printable paths, absolute
paths, paths outside the interpreter environment, and site-package symlinks
escaping their distribution root are refused before any read. Six refusals
follow:
`EnvironmentMismatch::ModifiedDistributionFile`,
`MissingDistributionFile`, `UnrecordedDistribution`,
`MalformedDistributionRecord`, `UnsafeDistributionRecord`, and
`UnaccountedStartupModule`. Malformed recorded digests are refused as metadata
corruption rather than misreported as content drift. A startup module is
accounted for when a locked distribution's `RECORD` claims its file or when the
manifest declares its digest; one that cannot execute is ignored, because
refusing a file the interpreter will not import would refuse an environment it
does not affect.

This is not a version-only probe. The restored environment holds 1.58 GB across
43,828 site-package files, so the five-second `VERSION_PROBE_POLICY` timed out
before the integrity walk could finish. `WORKER_ENVIRONMENT_PROBE_POLICY` gives
that walk its own two-minute security ceiling, mirrored in
`WALKING-SKELETON.md` §Provisional resource ceilings. The same command completed
in 4.62 and 3.78 seconds on the recorded developer environment; two minutes is
a bound for cold local-Linux storage, not a performance claim.

`t4_e1_the_probe_reads_record_digests_from_a_real_interpreter` runs the probe
script itself against a real interpreter and a real `.dist-info`, rather than
against the shell-script answer the other cases use, because the `RECORD` parse
is the part a canned answer cannot exercise.

### Version and compatibility

`worker/bundle-manifest.json` moves from layout `1.0` to `1.1`, adding
`startup_modules`. The field is optional and defaults to empty, so a `1.0`
manifest remains valid and declares nothing — the same absent-is-the-default
extension the lesson document made at its own `1.1`. A strict `1.0` decoder
rejects the newer field even when it is empty, so the declared version still
determines the accepted shape. Both layouts are accepted;
`SUPPORTED_BUNDLE_MANIFEST_SCHEMA_VERSIONS` is the list, and a layout outside
it is still refused. The change is therefore compatible rather than breaking.

### Identity impact

`worker/bundle-manifest.json` is a declared bundle input, so every worker-bundle
hash moves from
`5d77a5a6a520466043cb6a67ae805b148104d74d8c91fe85932b31d782d8b0af` to
`92bd4e442ed1caf2897660d57be580796d4f88a558ad65d45983f66336db16a3`.
The checked-in manifest declares the observed digest of the developer
environment's `/etc/python3.12/sitecustomize.py`, and `verified_hash` reproduced
that value twice against the restored locked environment. It has **not** been
reproduced on the protected reference machine, and
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md)
§Worker-bundle hash records that reproduction as still pending. The input set
and derivation are unchanged, so `WORKER_BUNDLE_IDENTITY_VERSION` remains
`e1-s1-v4`. Existing cache entries remain valid under their producing identity
and are neither deleted nor re-keyed.

### What it does not close

The comparison reads the distributions the lock names. An unlocked distribution
is still tolerated and still unread, which stays affordable only because the two
ways it could reach interpreter startup — a `.pth`, and a startup module — are
both now refused unless something accounts for them. A dedicated worker-only
environment enforcing an exact installed set would remove the tolerance itself,
and is not in this change.

## What the fifteenth audit closed

The initialization response still admitted a provenance claim no successful
worker could safely make:

| Finding | Closed by |
|---|---|
| `WorkerResponseFrame::Initialized` carried an arbitrary string map, so model and tokenizer revisions could be absent or moving refs, voice profiles could be absent, and unknown identity categories were accepted | Public `WorkerInitializationIdentities` with required checked `Revision`, `WorkerBundleHash`, and nonempty `BTreeMap<String, VoiceProfileHash>` fields; parser tests cover a complete response plus every missing and malformed category, an unknown field, and an empty voice-profile set |
| The E1-S1 product worker returned `initialized` after loading no model, tokenizer, or voice | `initialize` now returns nonrecoverable `initialization_failed`, and a subprocess regression proves later health remains `ready: false` and `model_loaded: false`; `synthesize` retains the same fail-closed result until E1-S3 |
| The deterministic fake reported `ready: true` but `model_loaded: false`, and its initialization and synthesis identities disagreed | The fake represents its loaded synthetic test backend consistently: complete exact initialization identities, `ready: true`, `model_loaded: true`, and the same model, tokenizer, bundle, and voice-profile identities on synthesis; every response is validated against `worker-protocol-v1.schema.json` |

### Compatibility and identity impact

This is a breaking required-response correction folded into the same pre-G1
`e1.worker.1.0` baseline and `1.1` trace extension. Retaining the version
across a **Breaking contract** change is a deviation from
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes, and
the eighteenth audit moved that decision into
[`../adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md`](../adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md),
where it is `Proposed` and therefore authorizes nothing yet. The reasoning
recorded at the time, and carried into that record, is that the still-Proposed
E1-S1 evidence gives no migration promise, no released consumer or durable
frame exists, and preserving the incomplete response under another version
would preserve a false initialization success. Rust supervisor types, the fake,
Python worker, tests, and generated schema move together. Unknown or incomplete
old `initialized` responses are refused rather than translated.

`worker/study_tts_worker/worker.py` and
`schemas/worker-protocol-v1.schema.json` are declared worker-bundle inputs, so
every worker-bundle hash moves. The input set and derivation stay unchanged,
and `WORKER_BUNDLE_IDENTITY_VERSION` remains `e1-s1-v4`. Existing cache entries
remain valid under their producing identity and are neither deleted nor
re-keyed. The deterministic fake and its tests are not production bundle
inputs and move no product synthesis identity.

## What the sixteenth audit closed

The fifteenth-audit contract test supplied the fake's fixed worker-bundle hash,
so it did not expose that initialization echoed any caller-supplied hash while
synthesis reported the fake's real fixed identity. The executable fake now
refuses any other requested bundle with nonrecoverable `initialization_failed`.
`t4_e1_fake_worker_passes_shared_protocol_contract` drives that mismatch and
compares the model, tokenizer or codec, worker-bundle, and voice-profile
identities reported at initialization with those reported by synthesis. Every
response in both paths is still validated against the published schema.

This closes a fake-only implementation and test gap without changing the wire
contract or product worker. Protocol versions remain `e1.worker.1.0` and `1.1`.
The fake, its tests, and these records are not worker-bundle manifest inputs, so
the product worker-bundle identity does not move.

## What the seventeenth audit closed

This audit closed no defect. It answers a review of the E1-S1 baseline against
`rust-review` and `ponytail`, and every item is scope, legibility, or governance
debt rather than behavior. **No wire contract, schema, error variant, refusal
message, or bundle input moved, and the verified bundle hash is unchanged at
`6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`** — which is
the evidence that it changed nothing that matters.

**The environment check is now authorized rather than assumed.** ADR-0001 §12.5
names the bundle-hash inputs exhaustively, and comparing the *installed*
environment against `worker/requirements.lock` is not among them. It is a
precondition on returning an identity, not an input to one, and E1-S1 task 6
asks only for "deterministic worker-bundle hashing".
[`ADR-0001-D004`](../adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md)
records the gap it closes, its measured cost, the alternatives, and a rollback.
The project owner approved it on 2026-08-29, so the check is governed scope
rather than an unrecorded extension of §12.5.

**The module now reads as the two things it does.**
`crates/study-tts-runtime/src/worker_bundle.rs` was 4,119 lines covering both
the §12.5 identity and the D004 precondition. The precondition moved to
`crates/study-tts-runtime/src/worker_environment.rs`, which names D004 in
return, so the boundary between what §12.5 requires and what D004 authorizes is
now visible in the file tree. The split is deliberately line-neutral: it deletes nothing, and
`worker_bundle` reaches the new module through exactly two crate-private
functions, so no probe or lockfile type crosses the boundary.

**The probe script is a Python file.** The ~180-line script was assembled by
`concat!` of Rust string literals; it is now
`crates/study-tts-runtime/src/runtime_probe.py`, loaded with `include_str!`. The
executable code is byte-identical. `t4_e1_the_runtime_probe_script_compiles_as_python`
is deleted rather than moved: `t4_e1_the_probe_reads_record_digests_from_a_real_interpreter`
already runs the script on a real interpreter and strictly subsumes it, and
`.github/workflows/ci.yml` now compiles the file directly.

**Sixteen tests were misfiled by tier.** Every test in `worker_environment`
resolves or spawns an interpreter, which `DELIVERY-PLAN.md` §3.2 puts at T4
rather than T1's "pure deterministic functions". All sixteen are renamed
`t1_e1_*` → `t4_e1_*`. None is named in `DELIVERY-PLAN.md`, so none is a
contract; `t1_e1_worker_bundle_hash_changes_on_owned_runtime_input` and
`t1_e1_worker_bundle_hash_ignores_unrelated_repository_files` are, and keep
their names. The citations in this document moved with them; the superseded
evidence records that also cite them were **not** edited, because they pin what
they measured.

**CI reports tier duration.** `DELIVERY-PLAN.md` §3.3 requires it and nothing
implemented it. Every test carries a `t<tier>_e<epic>_` prefix, so the tier is a
libtest filter over the binaries the suite already built. The report is
visibility, not a gate. It is also why the rename above mattered: before it, T4's
3.5 seconds of subprocess work billed against T1's 30-second budget.

## Impact of the two deliberately incomplete inputs

Two ADR-0001 §12.5 inputs are present in the identity but not yet resolved to
real values. Both are recorded here so the next change to them is expected
rather than surprising:

- **Voice-conditioning artifact hash.** Planning currently supplies an empty
  map, so the input serializes as absent. The voice gate runs after planning and
  its loaded identity is not consumed until the real worker lands. E1-S2 resolves
  voice references and will populate it, changing every cache key again.
- **Generation parameters.** The fake tone executor declares none. E1-S3 supplies
  the pinned Chatterbox parameters, changing every cache key again.

Both are pre-G1 provisional-contract changes under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md),
which owes no migration promise before G1. Each still needs its own record when
it lands.

## Limits this change does not close

Recorded here rather than left for a reader to discover:

- **The worker has no speech backend.** `worker/study_tts_worker/worker.py`
  answers `capabilities`, `health`, `cancel`, and `shutdown`, and refuses both
  `initialize` and `synthesize` with nonrecoverable `initialization_failed`
  naming E1-S3. It does not return a placeholder identity or tone, because the
  cache would publish that under a key claiming a real model produced it.
- **The verification schema describes the identity, not the finding.** The
  transcript, comparison, and scored findings arrive with the ASR stack in
  E4-S1 as a compatible extension at `1.1`.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes. `FakeTtsExecutor`
  and the shared contract scenarios were updated with the descriptor, the
  declared language set, and the reported context, and the walking skeleton and
  provisional-contract suites were rerun against them.
- Migration procedure: update both protocol executables, their fixtures, and
  the published schema together. Old-major frames are refused rather than
  translated. A synthesis key names a requested take, not reproducible bytes,
  so an old cached artifact cannot be re-keyed — the audio it holds was produced
  under inputs the new key does not describe. Old entries are left in place and
  simply not reused; nothing is deleted, because
  [`../governance/ROUTING-TABLES.md`](../governance/ROUTING-TABLES.md) never
  routes a refusal to deletion of a valid artifact.
- Rollback procedure: revert the Rust change and its consumers together. Cache
  entries written under either identity remain on disk and remain valid under
  the identity that produced them, so a rollback loses reuse rather than data.
- Compatibility evidence: the complete workspace suite passes under the new
  identity, including the 35-test walking skeleton against real FFmpeg and
  ffprobe. The pinned golden digests in
  `study-tts-core/src/plan.rs::t1_e0_plan_is_stable_for_identical_inputs` were
  recomputed rather than relaxed, and the test still pins exact values.
- Mapped tests and qualification rerun: per the affected-test mapping in
  [`PROVISIONAL-CONTRACT-BASELINE.md`](PROVISIONAL-CONTRACT-BASELINE.md),
  executor changes map to E1-S1 and E1-S3 — the shared executor scenario, strict
  frame fixtures, malformed/size/version tests, fake-worker process tests, and
  the walking skeleton. All were rerun.
- Walking skeleton result: pass, 35 tests, on Ubuntu 24.04 under WSL2 with real
  `ffmpeg` and `ffprobe` on `PATH`. (Recorded as 34 in the first revision of this
  record; the second audit found the count and the evidence disagreeing, and 35
  is what ran.)
- Not rerun: the real-model qualification measurements. They need the governed
  model, weights, and voice roots that
  [`../governance/RIGHTS-DATA-ARTIFACT-POLICY.md`](../governance/RIGHTS-DATA-ARTIFACT-POLICY.md)
  keeps outside Git and CI, and no synthesis path reaches a model in this
  change. `.github/workflows/qualification.yml` is where an operator runs them.

## Evidence provenance note

`evidence/gates/g0/e0-s4/e0-s4-provisional-contract-baseline-v2.md` pins SHA-256
digests of five documents. Four of them move in this change:
`docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`,
`docs/architecture/WALKING-SKELETON.md`,
`docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, and
`docs/testing/TEST-DATA-MANIFEST.md`. Only
`docs/architecture/E0-S4-INTERFACE-CHANGE-001.md` is unchanged.

`WALKING-SKELETON.md` moves for one sentence: it stated that the lesson fixture
uses `schema_version: 0.1-skeleton`, which this change makes false. An outdated
statement in a governance document is a defect, not a preserved snapshot, so it
is corrected here rather than left to mislead.

That record is **not** amended. `evidence/README.md` makes an accepted report
immutable and superseded only by a new record. The E0-S4 table remains an
accurate snapshot of what was controlled at E0-S4 acceptance, and
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v1.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v1.md)
carries the new values. That record names this one in return, so neither end of
the supersession can be found without the other.

The same rule applies once more, twice. §What the second audit closed moves four
of the six documents that record pins, and three of its statements were wrong when it
was written — so it is superseded rather than corrected, by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v2.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v2.md),
which names this document in return and lists what it corrects. The v1 record
stands as the snapshot of what was controlled when the contract owner adopted
it, errors and all; a record edited after adoption is a record nothing can be
checked against.

§What the third audit closed moves three of those six again — this document,
`WALKING-SKELETON.md`, and `TEST-DATA-MANIFEST.md` — so v2 is superseded in turn
by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v3.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v3.md),
which names this document in return. v2 stands as written.

And once more. §What the fourth audit closed and §What the fifth audit closed
move two of those six between them — this document and, for the fixture types,
`TEST-DATA-MANIFEST.md` — so v3 is superseded by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v4.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v4.md),
which names this document in return and carries the worker-bundle hash it
measured. V3 stands as written, including its own bundle hash: it is the
immutable record of the bundle it read, not a claim about the bundle today.
§What the sixth audit closed moves this document and the bundle hash once more,
so v4 is superseded by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v5.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v5.md).
§What the seventh audit closed moves this document, the worker lock, the
bundle-identity derivation version, and the bundle hash; v5 is therefore
superseded by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v6.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v6.md).
§What the eighth and ninth audits closed move this document, the Rust lesson-
construction boundary, and the CLI status boundary; v6 is therefore superseded by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v7.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v7.md).
§What the tenth audit closed moves this document, both ends of the worker
protocol, the published worker-protocol schema, and the bundle hash; v7 is
therefore superseded by
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md).
§What the eleventh audit closed, §What the twelfth audit closed, §What the
thirteenth audit closed, and §What the fourteenth audit closed are folded into
that still-Proposed v8 record; v8 is updated before approval rather than
superseded as though it were already immutable evidence.

§What the fifteenth audit closed and §What the sixteenth audit closed are folded
into that same still-Proposed v8 record for the same reason. The fifteenth audit
moves this document, the provisional contract, both worker implementations, the
worker protocol type and generated schema, and the worker-bundle hash. The
sixteenth moves only this document, the provisional contract, the fake, and its
contract test; none is a product worker-bundle input. Neither audit amends any
accepted evidence.

§What the seventeenth audit closed is recorded in
[`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v9.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v9.md),
which supersedes v8 rather than amending it. That audit moves this document,
`WORKER-ENVIRONMENT.md`, `worker_bundle.rs`, `lib.rs`, and the two workflow
files, and adds `worker_environment.rs`, `runtime_probe.py`, and `ADR-0001-D004`.
It moves no bundle input and no wire shape. Superseded records that cite the
sixteen renamed tests were left as written: they pin what they measured, which
is what supersession is for.

The fourteenth audit also moves `WALKING-SKELETON.md` to give the integrity
walk its dedicated deadline. Four active older records pin the previous bytes;
[`../../evidence/gates/g1/e1-s1/e1-s1-fourteenth-audit-provenance-reconciliation-v1.md`](../../evidence/gates/g1/e1-s1/e1-s1-fourteenth-audit-provenance-reconciliation-v1.md)
accounts for those exact record/path pairs without amending any accepted
record.

## Approval

- Contract owner decision: identity baseline adopted before the eleventh audit;
  the fifteenth- and sixteenth-audit initialization corrections remain pending
  review. The seventeenth audit changes no contract and asked for no contract
  decision; the owner decision it did ask for,
  [`ADR-0001-D004`](../adr/deviations/ADR-0001-D004-worker-environment-lock-verification.md),
  was approved on 2026-08-29
- Worker-frame classification: breaking at `e1.worker.1.0`; the decision to
  fold the initialization correction into that still-Proposed baseline remains
  pending review
- Engineering owner approval: approved on 2026-08-28 for the baseline through the
  twelfth audit; later amendments are **not** covered by it and remain pending
  in
  [`../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md`](../../evidence/gates/g1/e1-s1/e1-s1-provisional-contract-baseline-v8.md)
  §Review, together with the contract owner, worker/runtime owner, and
  affected-track reviews that record pins there. The approvals above are the
  scope this document claims; v8's table is the authority on what is still open,
  and this record must not be read as approving past it.
- Affected-track approvals: deferred to the G1 fake/real parity review
- Effective version and date: provisional `e1.tts-executor.1.0`, 2026-08-26

## What the eighteenth audit closed

This audit closed no defect in the build. It answers a review of the E1-S1
baseline that found two governance statements standing wider than what supports
them.

| Finding | Closed by |
|---|---|
| The fifteenth audit's decision to retain `e1.worker.1.0` across a required-field change was argued inline in `PROVISIONAL-CONTRACT-BASELINE.md` — a document that describes itself as *mirroring* `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, and that therefore cannot grant an exception to it | [`ADR-0001-D005`](../adr/deviations/ADR-0001-D005-prefreeze-breaking-correction-retains-version.md), `Proposed`, carrying five conditions such a correction must meet and an expiry at G1. The baseline document now names that record instead of reasoning on its own authority, and says plainly that a Proposed record authorizes nothing |
| `PROVISIONAL-CONTRACT-BASELINE.md` §Amendment rules claimed the change classes were "enforced by" `assess_successor` plus `t3_e0_contract_change_requires_version_or_explicit_compatible_extension`. That test reads `fixtures/contracts/e0-s4-contract-*.json` and nothing else, so it could not have observed the fifteenth audit's own change: a required field entered a published schema while its version stood still, and the whole suite stayed green | `t3_e1_published_schema_required_fields_match_the_recorded_surface` and the `PUBLISHED_REQUIRED_SURFACE` table in `crates/study-tts-testkit/tests/schemas.rs`, which hold the required-field surface of all seven published schemas per version. The claim in both documents was also narrowed to what each mechanism actually reaches |

### What the new test does and does not give

Any required field entering or leaving any schema in `schemas/` now fails the
suite until `PUBLISHED_REQUIRED_SURFACE` is edited, and the failure names the
document, the version, the JSON Pointer, and the field. Reverting the fifteenth
audit's `voice_profile_hashes` requirement was run against it and produces
exactly that message.

It does not decide the version. An author who edits a schema and the table in
one commit still passes, and saying otherwise would repeat the overstatement
this audit exists to correct. What it converts is a change that could land
silently into one that is explicit and reviewable where it is made.

### Compatibility and identity impact

No wire contract, schema, error variant, refusal message, or bundle input moved.
`PUBLISHED_REQUIRED_SURFACE` records the surface the seventeenth audit's schemas
already published, so the table is a transcription of the current state rather
than a change to it. The verified worker-bundle hash is unchanged at
`6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`. The new test
and this record are not worker-bundle manifest inputs.

## What the nineteenth audit closed

This remediation closes the approval, deadline, and harness gaps left after
audits 15–18. Statements above that audits 15–16 were pending and
`ADR-0001-D005` was Proposed record the state at those audits; the decisions
below supersede them without rewriting that history.

| Finding | Closed by |
|---|---|
| Audits 15–16 had implemented typed initialization identities, fail-closed product behavior, and fake identity parity without the required owner decisions | The role-specific approvals below and accepted `e1-s1-provisional-contract-baseline-v11` |
| The Delivery Plan fixes the ordinary T4 run at 60 seconds, while CI allowed 120 seconds | `.github/workflows/ci.yml` again applies `timeout --signal=TERM 60s`; the Delivery Plan and walking-skeleton contract are unchanged |
| `t4_e1_fake_worker_passes_shared_protocol_contract` used an unbounded `wait_with_output`, so the fake's existing `hang` behavior could hang the contract driver itself | A test-local standard-library child guard with a two-second session deadline, bounded polling, direct-child kill and reap on timeout, and best-effort `Drop` cleanup; `t4_e1_fake_worker_contract_deadline_kills_and_reaps_a_hung_worker` proves the timeout is bounded and leaves no zombie |

### Compatibility and identity impact

No public Rust API, wire field, schema version, dependency, product-worker
behavior, or audio byte changes. Existing cache entries remain valid only under
the identities that produced them; this audit neither reuses them under a new
identity nor deletes or re-keys them. The worker-bundle identity remains
`6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02`.
Reference-machine reproduction is still required before G1.

### Approval

Ross Todd holds each role below under
`docs/governance/PROJECT-EXECUTION-CHARTER.md`; each row records that role's
separate decision and accepted risk.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept the required typed initialization identities and refusal of incomplete legacy success frames within the unreleased `e1.worker.1.0` baseline and `1.1` extension, including approved `ADR-0001-D005` | 2026-08-29 |
| Engineering owner | Ross Todd | Accept schema, parser, product-worker, fake, and fixture parity, plus the bounded contract harness and restored 60-second CI deadline | 2026-08-29 |
| Project owner | Ross Todd | Accept audits 15–16, `ADR-0001-D005`, and `e1-s1-provisional-contract-baseline-v11`, subject to the remaining G1 limits recorded there | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the fail-closed product worker and current developer-machine bundle hash; require reference-machine reproduction before G1 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that old plan and cache entries remain valid only under their producing identities and are not reused, deleted, or re-keyed by this change | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-29 |

## What the twentieth audit closed

This remediation answers three review findings against the environment
precondition `ADR-0001-D004` authorizes. Each was a control that read as
enforced while a specific action passed it.

| Finding | Closed by |
|---|---|
| The integrity probe ran under `python -I`, which still executes every `.pth` file and imports `sitecustomize` before the script runs. Startup code could therefore edit what the probe reported about it: one `.pth` line replacing `json.dumps` made the probe answer that an environment holding a modified module and an unowned hook was clean, leaving the bundle hash unchanged | The probe is bootstrapped with `-I -S`, makes every observation with the standard library, and imports `packaging` last from a site directory it has checked resolves inside a prefix `site` itself would search. `crates/study-tts-runtime/src/runtime_probe.py` repeats the prefix half of `site.venv`, which `-S` also skips, and nothing else of `site.main`. `t4_e1_interpreter_startup_code_cannot_edit_what_the_probe_reports` drives the real probe against an interpreter carrying exactly that hook, and fails under `-I` alone |
| Installed files were checked against their adjacent, mutable `RECORD`, so editing a module and the `RECORD` line pinning it was one action that left the distribution self-consistent. `docs/operations/WORKER-ENVIRONMENT.md` records that the lock's artifact hashes are explicitly not compared against the installation, so nothing outside the environment stated what it should hold | `worker/bundle-manifest.json` moves to layout `1.2`, adding a required `record_digests` declaring, per locked distribution, a digest over the `RECORD` claims the check rests on. The manifest is a declared bundle input, so changing what the lock may have installed moves every cache key. `check_records_match_their_declarations` compares them, and `t4_e1_an_installed_record_the_manifest_does_not_vouch_for_is_refused` pins both refusals |
| `ln -sfn "${QUALIFIED_WORKER_VENV}" worker/.venv` does not replace an existing real directory. It creates a link *inside* it, exits `0`, and the version check on the next line then runs the stale interpreter that was already there — a success an operator reads as an attached qualified environment | The documented step guards on a non-symlink destination and attaches with `ln -sfnT`, which is what `.github/workflows/qualification.yml` already used. Reproduced in a scratch tree before and after |

### What layout `1.2` costs, and what it does not buy

An omitted `record_digests` is a refusal rather than an exemption, so a layout
`1.0` or `1.1` manifest still loads and then refuses every locked distribution.
That is the field working. It also means the declarations can only be produced
from a restored environment: `docs/operations/WORKER-ENVIRONMENT.md` §Declaring
what the lock installed carries the command that prints them, and the
fifty-six entries now in `worker/bundle-manifest.json` came from it, run against
the reference machine's own restored `worker/.venv`.

The digest is taken over `RECORD` rows rather than over the `RECORD` file, and
excludes the `.dist-info` directory. `INSTALLER`, `REQUESTED`, and
`direct_url.json` are installer bookkeeping that moves with the command that
installed rather than with anything the worker imports; pinning the file itself
would make a correct restore read as tampering, and train an operator to
regenerate on every mismatch, which is the control switched off by habit.

What it does not give is authentication against the locked *artifact*. Nothing
this build can ask the interpreter reports the artifact a distribution came
from, so the declaration is an independently generated manifest rather than a
re-derivation from the wheel.

### Compatibility and identity impact

No public Rust API signature, wire field, published schema, or worker protocol
version changes. Two `EnvironmentMismatch` variants are added —
`UndeclaredDistributionRecord` and `ModifiedDistributionRecord` — and no
existing variant, refusal message, or audio byte moves.

**The worker-bundle identity moves**, from
`6b0a3c1466bd1dc24202b913f8917a49bd0284b39a81807d030216efa8aa8d02` to
`f9711a21f3e046d53c7c617e9308893c9c0240badec0d3656487fe2796c6dc2a`.
`worker/bundle-manifest.json` is a declared input and its bytes changed, which
is the intended shape of this control: what the lock may have installed is now
part of what the identity describes. Existing cache and plan entries remain
valid only under the identities that produced them; nothing is reused under a
new identity, deleted, or re-keyed. `WORKER_BUNDLE_IDENTITY_VERSION` does not
move — the derivation is unchanged and ADR-0001 §12.5's input list gains
nothing; only the bytes of a declared input did.

The cost is unchanged. Five consecutive `verified_hash` runs on the reference
machine took 3.43–3.52 s, inside the 3.43–3.62 s `ADR-0001-D004` records: the
added digest covers a few kilobytes of `RECORD` text per distribution, against
the 1,263 MiB the per-file comparison already reads.

### Approval

Ross Todd holds each role below under
`docs/governance/PROJECT-EXECUTION-CHARTER.md`; each row records that role's
separate decision and accepted risk.

| Role | Name | Decision | Date |
|---|---|---|---|
| Contract owner | Ross Todd for T-CORE | Accept manifest layout `1.2` as a pre-freeze extension of a project-owned format, with `1.0` and `1.1` still readable and refusing rather than exempting | 2026-08-29 |
| Engineering owner | Ross Todd | Accept the `-S` bootstrap, the `site.venv` prefix repetition it requires, the two added `EnvironmentMismatch` variants, and the two added T4 tests | 2026-08-29 |
| Project owner | Ross Todd | Accept that the manifest now carries fifty-six machine-generated declarations, and that regenerating the lock means regenerating them | 2026-08-29 |
| Worker owner | Ross Todd for T-WORKER | Accept the worker-bundle identity moving to `f9711a21…6c6dc2a`, reproduced twice on the reference machine, with hosted-CI and protected qualification reproduction still owed before G1 | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-RUNTIME | Accept that old plan and cache entries remain valid only under their producing identities and are not reused, deleted, or re-keyed by this change | 2026-08-29 |
| Affected-track reviewer | Ross Todd for T-AUDIO | Accept that no audio behavior or bytes changed, so no listening evidence is required | 2026-08-29 |
