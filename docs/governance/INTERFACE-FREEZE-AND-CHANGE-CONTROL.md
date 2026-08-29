# Interface Freeze and Change Control

## Provisional baseline

E0-S4 publishes versioned fakes and shared contract suites for:

- lesson load and validation;
- render planning;
- `TtsExecutor` and worker frames;
- synthesis-cache publication;
- PCM assembly and package writing;
- job state and recovery.

These seams permit independent work, but they remain provisional until the real Chatterbox worker and real package path pass the same contracts at G1.

The concrete IDs, versions, public representations, consumers, fakes,
fixtures, identity effects, stabilization stories, and affected-test mapping
are recorded in
[`../architecture/PROVISIONAL-CONTRACT-BASELINE.md`](../architecture/PROVISIONAL-CONTRACT-BASELINE.md).
The change classes below are mechanized in three places, each naming this
document in return. What each one reaches is stated exactly, because the gap
between them is where a class change escapes:

- `crates/study-tts-core/src/contract.rs::ContractDescriptor::assess_successor`
  classifies **a pair of declared descriptors** against the class each claims.
  It reads no schema and no code: it answers whether a stated version move
  matches a stated change, never whether either describes what was built.
- `crates/study-tts-core/src/schema.rs::SchemaVersion::accepted_by` applies them
  to a *document* on disk: a different major is refused, a newer minor is
  refused, and an older minor of the same major is read with the defaults its
  compatible extensions declared. It governs which documents this build will
  **read**, not whether a published schema was entitled to change.
- `PUBLISHED_REQUIRED_SURFACE` and
  `t3_e1_published_schema_required_fields_match_the_recorded_surface` in
  `crates/study-tts-testkit/tests/schemas.rs` hold the required-field surface of
  every published schema, per version. Any required field entering or leaving
  any schema in `schemas/` fails the suite until that table is edited, and the
  failure names the document, the version, the JSON Pointer, and the field.

The third exists because the first two, together, still let a **Breaking
contract** land silently: a required field could appear in a published schema
while its version stood still, and no test in the suite would notice. What it
guarantees is that such a change is explicit and reviewable where it is made.
It does not choose the version — an author who edits a schema and that table in
one commit still passes, and the class table below remains a rule people apply,
not one a machine decides.

## G1 freeze

The G1 freeze record must contain:

| Field | Requirement |
|---|---|
| Contract | Name and semantic version |
| Owner | One accountable module owner |
| Consumers | Crates, worker, schemas, fixtures, and commands |
| Canonical representation | Checked-in schema or Rust API path |
| Compatibility rule | Accepted extension and rejection behavior |
| Contract tests | Fake and real implementation results |
| Identity effect | Whether a change invalidates synthesis, verification, plan, takes, or package |
| Migration | Upgrade and rollback procedure |
| Approval | Engineering owner and affected-track review |

## Change classes

| Class | Example | Required action |
|---|---|---|
| Compatible patch | Diagnostic clarification with no durable-byte or behavioral change | Retain version; tests and documentation |
| Compatible extension | Optional field with defined default and unknown-field policy | Minor version, fixtures, compatibility tests |
| Breaking contract | Required field, semantic change, frame change | Major version, migration, impact report, owner approval |
| Architectural | Backend collection, database, remote rendering, changed authority boundary | New or amended ADR before implementation |
| Emergency containment | Disable unsafe publish/profile | Incident record, test, follow-up decision; no silent permanent change |

## Change procedure

1. Create an interface-change record from
   [`../templates/INTERFACE-CHANGE-TEMPLATE.md`](../templates/INTERFACE-CHANGE-TEMPLATE.md).
2. Name affected contracts, identities, stories, fixtures, tests, and cached artifacts.
3. Prove compatibility or define migration and rollback.
4. Update fakes and shared contract tests first.
5. Update each implementation and consumer.
6. Run the walking skeleton and affected qualification tests.
7. Record approval and effective version.

No merged code may cause an old take, verification result, or package to appear valid under a changed identity.

Before G1, every amendment updates its fake, fixtures, and shared suite before
its consumers. Executor and worker changes map to E1-S1/E1-S3 and the
worker/security suites; cache changes map to E1-S3/E2-S1/E2-S2/E4; package
changes map to E1-S4/E2-S3; job changes map to E2-S1/E4-S4/E5 recovery. Every
class reruns the walking skeleton.
