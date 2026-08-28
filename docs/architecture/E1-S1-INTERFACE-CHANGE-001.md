# E1-S1 Interface Change 001 — Complete Synthesis and Verification Identities

## Identification

- Record ID: `E1-S1-INTERFACE-CHANGE-001`
- Contract owner: T-CORE
- Engineering owner: engineering owner
- Affected-track reviewers: T-WORKER, T-AUDIO, and T-RUNTIME consumers
- Accepted ADR, if architectural: not applicable; this implements ADR-0001 §12.5
  as written and changes no authority boundary

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
- Wire or Rust representation changed: Rust representation and durable cache
  keys. No worker frame changed, so `worker_frames` keeps `e0.worker.0.1` and
  its declared `e0.worker.0.2` extension.

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
  check that the list is complete.
- Plan, takes, or package identities affected: every plan hash changes, because
  `PlannedSegment` gains `take` and the plan is hashed canonically. The takes
  document is new at `1.0`; no takes file exists to migrate.
- Consumers and commands affected: `study-tts-runtime`'s preview orchestration
  and `study-tts-testkit`'s fakes. No product command exists at E1-S1; the CLI
  remains the placeholder binary until E1-S5.
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
| Bundle hash trusted the manifest's declared Python runtime and platform ABI | `WorkerBundle::verified_hash` probes the interpreter at `WORKER_INTERPRETER_PATH`; `t1_e1_an_interpreter_disagreeing_with_the_manifest_is_refused` |
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

`WORKER_PROTOCOL_VERSION` therefore stays at `e0.worker.0.1` with its declared
`e0.worker.0.2` extension. **This classification is the contract owner's to
ratify.** The alternative reading — that narrowing a field's accepted values is
a semantic change and therefore breaking under
[`../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`](../governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md)
§Change classes — is defensible, and taking it would move the worker protocol to
`e1.worker.1.0` and its extension to `e1.worker.1.1`, along with the session
fixture, the published schema, the Python `WORKER_PROTOCOL_VERSION`, and
[`PROVISIONAL-CONTRACT-BASELINE.md`](PROVISIONAL-CONTRACT-BASELINE.md). T-CORE
owns that call; it is recorded here rather than taken silently either way.

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
| `verified_hash` proved the interpreter's ABI and hashed the lockfile's *bytes*, which says what that file claims and nothing about the environment beside it: a distribution upgraded in place, or a `chatterbox-tts` the configured index satisfied at the same version, left every declared input byte-identical and every cache key where it was | The same probe now reports every installed distribution and its PEP 610 `direct_url`, and `check_environment_matches_lock` compares both against `worker/requirements.lock` before a hash is returned; `EnvironmentMismatch` names which of the four faults occurred, and never prints the recorded URL; `t1_e1_an_environment_that_is_not_the_locked_one_is_refused`, `t1_e1_a_lockfile_line_that_is_not_an_exact_pin_is_refused` |
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
§Change classes this is the same **compatible patch** classification, and the
same caveat, as the worker-frame typing recorded in §The two the audit left
open: the alternative reading — that narrowing accepted values is semantic and
therefore breaking — is defensible, and **T-CORE owns that call**. It is
recorded here rather than taken silently.

The worker protocol's own version is untouched by that classification either
way: no frame this build accepted before is refused now, and one it refused
before — a `capabilities` frame declaring `e0.worker.0.2` — is accepted by the
Python end for the first time, which is a widening rather than a narrowing.

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
| The governed-source provenance check compared a `code-<commit>` **path component**, and a component proves a directory *name*. A directory install records PEP 610 `dir_info` and no `commit_id` at all, so the tree at `code-<commit>` could hold any bytes and `code-<commit>-backup` beside it is a name an operator really creates | The governed distribution is installed from the tree's git URL at its commit, so `pip` records the `vcs_info.commit_id` it checked out, and that is what is compared. The probe no longer reports the URL at all. A record with no revision is `EnvironmentMismatch::WithoutRecordedRevision`, distinct from `::FromIndex` because the remedy is a different command rather than a different directory; `t1_e1_an_environment_that_is_not_the_locked_one_is_refused` |
| Extra installed distributions were ignored, and an extra distribution can ship a `.pth` — which runs at interpreter startup and joins `sys.path` ahead of the search that resolves `torch`, outside the bundle identity entirely | The probe reports every `.pth` in the interpreter's site directories with the distribution whose `RECORD` lists it, and a hook the lock does not account for is `EnvironmentMismatch::UnownedPathHook` or `::UnlockedPathHook`. The tolerance for extra *distributions* survives, and is what it was always resting on: `setuptools` is pinned and owns the one hook the reference machine has; `t1_e1_a_startup_hook_the_lockfile_does_not_account_for_is_refused` |
| The probe reported distributions as a map keyed by the canonicalized name, and that mapping is many-to-one: two installs collapsed to one entry silently, and the comparison answered for whichever was walked last | The probe reports a list and the collision is `EnvironmentMismatch::AmbiguousDistribution`; `t1_e1_two_installs_canonicalizing_alike_are_refused_rather_than_collapsed` |
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
| `verified_hash` resolved the interpreter through `tools::resolve_executable`, which canonicalizes. A virtualenv's `bin/python` is a symlink chain to the base interpreter and `worker/.venv` is itself a link, so the probe ran `/usr/bin/python3.12` — `sys.prefix` `/usr`, none of the locked distributions present. **The environment check had never once read the environment it exists to read**, and the argument that "which interpreter is asked is not a parameter" was true of a path nobody ran | `tools::executable_in_place`, which checks the file is executable without resolving where it points; `t1_e1_the_interpreter_is_probed_where_it_is_attached_not_where_it_resolves`. Every earlier interpreter in the suite was a regular file, and canonicalizing a regular file returns the same file — which is why the stand-in in that test answers differently depending on the path it is invoked through |
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
§Change classes that is the same **compatible patch** classification and the same
caveat as the narrowings in §What the second audit closed, and it is **T-CORE's
to ratify** for the same reason: no document or frame that could ever have been
*published* is refused — a takes file naming a segment no lesson can carry could
not have been applied, and a worker given no threads could not have rendered —
but the alternative reading, that narrowing accepted values is semantic and
therefore breaking, is defensible. No takes document and no persisted frame
exists to migrate either way.

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
document this build accepts becomes invalid and every version is retained. That
classification is **T-CORE's to ratify**, as the earlier narrowings are.

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
already rejected. T-CORE still owns ratification of that classification.

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
  answers `initialize`, `capabilities`, `cancel`, and `shutdown`, and refuses
  `synthesize` with `initialization_failed` naming E1-S3. It does not return a
  placeholder tone, because the cache would publish that under a key claiming a
  real model produced it.
- **The Python lock pins versions, not artifact hashes.**
  `pip install --require-hashes` cannot be used against `worker/requirements.lock`
  yet. [`../operations/WORKER-ENVIRONMENT.md`](../operations/WORKER-ENVIRONMENT.md)
  §Adding artifact hashes records the procedure, what it changes, and the fact
  that taking it moves every cache key and needs its own record.
- **The verification schema describes the identity, not the finding.** The
  transcript, comparison, and scored findings arrive with the ASR stack in
  E4-S1 as a compatible extension at `1.1`.

## Delivery and recovery

- Fake and shared-suite update completed before consumers: yes. `FakeTtsExecutor`
  and the shared contract scenarios were updated with the descriptor, the
  declared language set, and the reported context, and the walking skeleton and
  provisional-contract suites were rerun against them.
- Migration procedure: none is owed or possible. A synthesis key names a
  requested take, not reproducible bytes, so an old cached artifact cannot be
  re-keyed — the audio it holds was produced under inputs the new key does not
  describe. Old entries are left in place and simply not reused; nothing is
  deleted, because [`../governance/ROUTING-TABLES.md`](../governance/ROUTING-TABLES.md)
  never routes a refusal to deletion of a valid artifact.
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

## Approval

- Contract owner decision: adopted as the E1-S1 identity baseline
- Engineering owner approval: pending review of this change
- Affected-track approvals: deferred to the G1 fake/real parity review
- Effective version and date: provisional `e1.tts-executor.1.0`, 2026-08-26
