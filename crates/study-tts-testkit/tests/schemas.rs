//! T3: the published schemas, and the version gate that decides which
//! documents this build will read.
//!
//! DELIVERY-PLAN E1-S1 names all four of these tests. They divide the ground
//! two ways, and both halves are needed:
//!
//! - **The files** (`t3_e1_generated_schemas_match_checked_in_files`,
//!   `t3_e1_published_lesson_schema_validates_every_example`) prove that what
//!   is checked in describes what the code actually parses. A schema nobody
//!   regenerated is a schema that lies to an author's editor.
//! - **The gate** (`t3_e1_unknown_major_version_is_rejected`,
//!   `t3_e1_compatible_minor_extension_is_accepted`) proves the rule in
//!   `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
//!   is enforced on real documents, not only in the unit tests that own the
//!   comparison.
//!
//! Two helper tests are not in the plan and carry the other direction, which
//! the four named ones cannot:
//! `t3_e1_the_published_lesson_schema_refuses_the_invalid_fixtures` and
//! `t3_e1_invalid_lesson_fixtures_are_refused_by_their_own_invariant` run the
//! same committed malformed fixtures through the schema and through the
//! parser. Proving the valid examples pass says nothing about a schema that
//! accepts everything, and an author needs the editor and the build to refuse
//! the same document.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;
use study_tts_core::{
    LESSON_SCHEMA_STEM, LESSON_SCHEMA_VERSION, LessonError, SchemaVersionError, ValidatedLesson,
    schema_uri,
};
use study_tts_runtime::{PUBLISHED_SCHEMAS, PublishedSchema, SCHEMA_DIRECTORY};
use study_tts_testkit::validate_against_schema;

/// One invalid fixture and the invariant that must be what refuses it.
type InvalidFixture = (&'static str, fn(&LessonError) -> bool);

/// One invalid fixture and the JSON Pointer the published schema must name.
type SchemaRefusal = (&'static str, &'static str);

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn schema_directory() -> PathBuf {
    repository_root().join(SCHEMA_DIRECTORY)
}

/// Every lesson document the repository holds as an example.
///
/// Read from the directory rather than listed here, so a lesson fixture added
/// for some other test is covered by the schema check without anybody
/// remembering to add it.
fn lesson_examples() -> Vec<PathBuf> {
    let mut examples: Vec<PathBuf> = fs::read_dir(repository_root().join("fixtures/lessons"))
        .expect("the lesson fixture directory is readable")
        .map(|entry| entry.expect("a fixture directory entry is readable").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    examples.sort();
    assert!(
        !examples.is_empty(),
        "there must be lesson examples to check"
    );
    examples
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(
        &fs::read(path).unwrap_or_else(|error| panic!("`{}` is readable: {error}", path.display())),
    )
    .unwrap_or_else(|error| panic!("`{}` is JSON: {error}", path.display()))
}

#[test]
fn t3_e1_generated_schemas_match_checked_in_files() {
    let directory = schema_directory();

    // Direction one: every published schema is on disk, byte for byte as
    // regeneration would write it. Comparing bytes rather than parsed values is
    // deliberate — the checked-in file is what a reader and an editor consume,
    // and a file that differs only in formatting still produces a diff nobody
    // asked for on the next regeneration.
    for schema in PUBLISHED_SCHEMAS {
        let path = directory.join(schema.file_name());
        let checked_in = fs::read(&path).unwrap_or_else(|error| {
            panic!(
                "`{}` is missing ({error}); run `cargo run --package study-tts-runtime \
                 --example generate-schemas`",
                path.display()
            )
        });

        assert_eq!(
            String::from_utf8(checked_in).expect("a checked-in schema is UTF-8"),
            String::from_utf8(schema.to_bytes()).expect("a generated schema is UTF-8"),
            "`{}` has drifted from the Rust type that defines it; run `cargo run --package \
             study-tts-runtime --example generate-schemas`",
            path.display()
        );
    }

    // Direction two: nothing else is in `schemas/`. A file with no entry in the
    // catalogue is a schema no test regenerates and no type defines, which is
    // the state the whole arrangement exists to prevent.
    let published: BTreeSet<String> = PUBLISHED_SCHEMAS
        .iter()
        .map(PublishedSchema::file_name)
        .collect();
    let on_disk: BTreeSet<String> = fs::read_dir(&directory)
        .expect("the schema directory is readable")
        .map(|entry| {
            entry
                .expect("a schema directory entry is readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();

    assert_eq!(
        on_disk,
        published,
        "`{}` must hold exactly the published schemas",
        directory.display()
    );
}

#[test]
fn t3_e1_published_lesson_schema_validates_every_example() {
    let lesson_schema = read_json(&schema_directory().join(format!(
        "{LESSON_SCHEMA_STEM}-v{}.schema.json",
        LESSON_SCHEMA_VERSION.major()
    )));

    for example in lesson_examples() {
        let document = read_json(&example);
        let declared = document
            .get("schema_version")
            .and_then(Value::as_str)
            .unwrap_or_default();

        // The schema describes one major version, so only the examples of that
        // major are its to validate. An example of another major is checked by
        // the gate test below instead, which is where it belongs.
        let Ok(version) = declared.parse::<study_tts_core::SchemaVersion>() else {
            continue;
        };
        if version.major() != LESSON_SCHEMA_VERSION.major() {
            continue;
        }

        if let Err(violations) = validate_against_schema(&lesson_schema, &document) {
            panic!(
                "`{}` does not satisfy the published lesson schema:\n  {}",
                example.display(),
                violations.join("\n  ")
            );
        }

        // The schema and the parser must agree about the same bytes. A schema
        // that accepted what the parser refuses would send an author away with
        // a green editor and a failing build.
        ValidatedLesson::from_json(&fs::read(&example).expect("the example is readable"))
            .unwrap_or_else(|error| {
                panic!(
                    "`{}` satisfies the published schema but the parser refuses it: {error}",
                    example.display()
                )
            });
    }
}

#[test]
fn t3_e1_unknown_major_version_is_rejected() {
    // A real document, not a mutated string: `e1-s1-unknown-major.json` is a
    // complete and otherwise valid lesson that declares a major this build does
    // not implement, so nothing but the version can be what refuses it.
    let example = repository_root().join("fixtures/lessons/e1-s1-unknown-major.json");
    let bytes = fs::read(&example).expect("the unknown-major example is readable");

    let error = ValidatedLesson::from_json(&bytes)
        .expect_err("a lesson of an unknown major version must be refused");

    assert!(
        matches!(
            error,
            LessonError::UnsupportedSchema(SchemaVersionError::UnsupportedMajor { .. })
        ),
        "expected a major-version refusal, got {error}"
    );

    // The refusal must name what this build publishes, because the author's
    // next action is to migrate the document to that version.
    assert!(
        error
            .to_string()
            .contains(&LESSON_SCHEMA_VERSION.to_string()),
        "the refusal must name the supported version, got `{error}`"
    );

    // The same document with only its version corrected is accepted, which is
    // what proves the version is the only thing wrong with it.
    let mut corrected = read_json(&example);
    corrected["schema_version"] = Value::String(LESSON_SCHEMA_VERSION.to_string());
    ValidatedLesson::from_json(
        &serde_json::to_vec(&corrected).expect("the corrected example serializes"),
    )
    .expect("only the version may be what refuses the unknown-major example");
}

#[test]
fn t3_e1_compatible_minor_extension_is_accepted() {
    // `$schema` arrived with lesson `1.1` as a compatible extension under
    // `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md`, whose declared
    // default is absent. `e1-s1-prior-minor.json` is a document written before
    // it, and it must still be readable — otherwise the increment was breaking
    // and was published under the wrong number.
    let example = repository_root().join("fixtures/lessons/e1-s1-prior-minor.json");
    let document = read_json(&example);
    assert_eq!(
        document.get("schema_version").and_then(Value::as_str),
        Some("1.0"),
        "the compatible-extension example must predate the current minor"
    );
    assert!(
        document.get("$schema").is_none(),
        "the compatible-extension example must omit the field the extension added"
    );

    let lesson = ValidatedLesson::from_json(&fs::read(&example).expect("the example is readable"))
        .expect("an earlier minor version of the same major must be accepted");

    // The document keeps the version it declared. Silently upgrading it would
    // make the next refusal report a version nobody wrote.
    assert_eq!(
        lesson.schema_version(),
        "1.0".parse().expect("`1.0` parses")
    );

    // A newer minor is refused in the other direction: this build does not know
    // the default that extension declared, so reading it would be a guess.
    let mut newer = document.clone();
    newer["schema_version"] = Value::String(format!(
        "{}.{}",
        LESSON_SCHEMA_VERSION.major(),
        LESSON_SCHEMA_VERSION.minor() + 1
    ));
    let error = ValidatedLesson::from_json(
        &serde_json::to_vec(&newer).expect("the newer-minor example serializes"),
    )
    .expect_err("a newer minor version must be refused");
    assert!(
        matches!(
            error,
            LessonError::UnsupportedSchema(SchemaVersionError::UnsupportedMinor { .. })
        ),
        "expected a minor-version refusal, got {error}"
    );

    // The extension itself is still checked when it is present: an optional
    // field is optional, not unvalidated.
    let mut mislinked =
        read_json(&repository_root().join("fixtures/lessons/e0-s0-two-segment.json"));
    mislinked["$schema"] = Value::String(schema_uri("takes", 1));
    assert!(
        matches!(
            ValidatedLesson::from_json(
                &serde_json::to_vec(&mislinked).expect("the mislinked example serializes")
            ),
            Err(LessonError::UnexpectedSchemaLink { .. })
        ),
        "a link naming another schema must be refused"
    );
}

#[test]
fn t3_e1_the_published_lesson_schema_refuses_the_invalid_fixtures() {
    // The other direction of
    // `t3_e1_published_lesson_schema_validates_every_example`, and the half
    // that makes it mean something: proving the valid examples pass says
    // nothing about a schema that accepts everything.
    //
    // These are the same committed fixtures
    // `t3_e1_invalid_lesson_fixtures_are_refused_by_their_own_invariant` runs
    // through the parser, so the pair asserts what an author actually needs —
    // that the editor and the build refuse the same document. A fixture the
    // parser refuses and the schema accepts sends its author away with a green
    // editor and a failing build.
    //
    // The expected pointer is the field the author mistyped, not merely "some
    // violation": a schema that refused the whole document for an unrelated
    // reason would otherwise pass here.
    let lesson_schema = read_json(&schema_directory().join(format!(
        "{LESSON_SCHEMA_STEM}-v{}.schema.json",
        LESSON_SCHEMA_VERSION.major()
    )));
    let cases: [SchemaRefusal; 3] = [
        ("e1-s1-lesson-unknown-field.json", "difficulty"),
        ("e1-s1-lesson-malformed-language.json", "/language"),
        ("e1-s1-lesson-mislinked-schema.json", "/$schema"),
    ];

    for (fixture, expected) in cases {
        let path = repository_root().join("fixtures/contracts").join(fixture);
        let document = read_json(&path);

        let violations = validate_against_schema(&lesson_schema, &document).expect_err(&format!(
            "`{fixture}` must not satisfy the published schema"
        ));

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains(expected)),
            "`{fixture}` was refused, but not for `{expected}`:\n  {}",
            violations.join("\n  ")
        );
    }
}

#[test]
fn t3_e1_invalid_lesson_fixtures_are_refused_by_their_own_invariant() {
    // Committed alongside the valid examples so the two stay in step: each of
    // these differs from `e0-s0-two-segment.json` in exactly one way, and each
    // must be refused for that one reason rather than for whichever check
    // happens to run first.
    //
    // The expectation is a table read against `lesson.rs`, not a `matches!`
    // copied out of it: a table that recomputed the rule would pass for any
    // rule, including a wrong one.
    let cases: [InvalidFixture; 3] = [
        ("e1-s1-lesson-unknown-field.json", |error| {
            matches!(error, LessonError::InvalidJson(_))
        }),
        ("e1-s1-lesson-malformed-language.json", |error| {
            matches!(error, LessonError::MalformedLanguage(_))
        }),
        ("e1-s1-lesson-mislinked-schema.json", |error| {
            matches!(error, LessonError::UnexpectedSchemaLink { .. })
        }),
    ];

    for (fixture, expected) in cases {
        let path = repository_root().join("fixtures/contracts").join(fixture);
        let bytes = fs::read(&path).expect("the invalid fixture is readable");

        let error = ValidatedLesson::from_json(&bytes)
            .err()
            .unwrap_or_else(|| panic!("`{fixture}` must be refused"));

        assert!(
            expected(&error),
            "`{fixture}` was refused by the wrong invariant: {error}"
        );
    }
}

/// One published format's parser, reduced to accept-or-say-why.
///
/// A function pointer rather than a closure so the table below is a `const` a
/// reviewer reads in one place, and so a format's entry names the boundary that
/// actually reads that document rather than a generic `serde_json` call.
type ParserCheck = fn(&[u8]) -> Result<(), String>;

/// One published format, a document that must satisfy it, and the parser that
/// must agree.
type ValidExample = (&'static str, &'static str, ParserCheck);

/// One published format, a document that must not satisfy it, and every JSON
/// Pointer the schema's refusal has to name.
///
/// A list rather than one pointer because a format's digest fields all
/// reference the same constrained definition, and what has to be proved is that
/// *each field* references it. One document spoiling every such field proves
/// exactly that, and does it without one near-identical fixture per field.
type InvalidExample = (&'static str, &'static str, &'static [&'static str]);

fn accepts_lesson(bytes: &[u8]) -> Result<(), String> {
    ValidatedLesson::from_json(bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn accepts_takes(bytes: &[u8]) -> Result<(), String> {
    study_tts_core::ValidatedTakes::from_json(bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn accepts_verification(bytes: &[u8]) -> Result<(), String> {
    let record: study_tts_core::VerificationIdentityRecord =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    record.validate().map_err(|error| error.to_string())
}

fn accepts_job(bytes: &[u8]) -> Result<(), String> {
    serde_json::from_slice::<study_tts_core::ProvisionalJobSnapshot>(bytes)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn accepts_worker_frame(bytes: &[u8]) -> Result<(), String> {
    // One frame is one line, and `parse_worker_request` refuses a trailing
    // newline because on the wire that byte separates two frames. A fixture
    // file ends with one like every other text file in the repository, so the
    // fixture is the frame plus its terminator.
    let frame = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    study_tts_runtime::parse_worker_request(frame)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// The formats whose agreement is proved somewhere other than the table below,
/// and where.
///
/// Neither has a reader this crate can call, and both are checked against a
/// document a *writer* produced rather than one committed beside them:
///
/// - `plan` is serialized and never read back (ADR-0001 §12.2 puts the loader
///   at E2), so
///   `t3_e1_the_published_plan_schema_describes_what_the_planner_writes` plans
///   a real lesson and validates the result. A committed valid plan would
///   drift from the planner the first time planning changed, and nothing would
///   read it back to notice.
/// - `manifest` is written and read inside `study-tts-runtime` through private
///   functions, so
///   `t4_e1_the_published_manifest_schema_describes_what_a_package_writes`
///   validates the manifest a real package build wrote. It lives in the T4
///   suite because producing one needs real FFmpeg.
const CHECKED_ELSEWHERE: [&str; 2] = ["plan", "manifest"];

const VALID_EXAMPLES: [ValidExample; 5] = [
    (
        LESSON_SCHEMA_STEM,
        "fixtures/lessons/e0-s0-two-segment.json",
        accepts_lesson,
    ),
    (
        study_tts_core::TAKES_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-takes-valid.json",
        accepts_takes,
    ),
    (
        study_tts_core::VERIFICATION_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-verification-valid.json",
        accepts_verification,
    ),
    (
        "job",
        "fixtures/contracts/e1-s1-job-valid.json",
        accepts_job,
    ),
    (
        "worker-protocol",
        "fixtures/contracts/e0-s4-worker-valid.json",
        accepts_worker_frame,
    ),
];

const INVALID_EXAMPLES: [InvalidExample; 10] = [
    (
        study_tts_core::TAKES_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-takes-uppercase-digest.json",
        &["/selections/0/audio_blake3"],
    ),
    (
        study_tts_core::TAKES_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-takes-unusable-lesson-id.json",
        &["/lesson_id"],
    ),
    (
        study_tts_core::VERIFICATION_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-verification-truncated-key.json",
        &["/verification_key"],
    ),
    (
        study_tts_core::VERIFICATION_SCHEMA_STEM,
        "fixtures/contracts/e1-s1-verification-malformed-profile-hashes.json",
        &[
            "/context/expected_pattern_profile_hash",
            "/context/comparison_normalizer_hash",
            "/context/threshold_profile_hash",
        ],
    ),
    (
        "job",
        "fixtures/contracts/e1-s1-job-unknown-version.json",
        &["/schema_version"],
    ),
    (
        "job",
        "fixtures/contracts/e1-s1-job-malformed-digests.json",
        &[
            "/plan_hash",
            "/selected_package/package_id",
            "/selected_package/manifest_blake3",
        ],
    ),
    (
        "plan",
        "fixtures/contracts/e1-s1-plan-uppercase-cache-key.json",
        &["/segments/0/cache_key"],
    ),
    (
        "manifest",
        "fixtures/contracts/e1-s1-manifest-uppercase-cache-key.json",
        &["/segments/0/cache_key"],
    ),
    (
        "manifest",
        "fixtures/contracts/e1-s1-manifest-malformed-digests.json",
        &[
            "/plan_hash",
            "/segments/0/audio_blake3",
            "/artifacts/master_wav/blake3",
            "/artifacts/m4a/blake3",
            "/tools/ffmpeg/argument_profile_blake3",
            "/tools/ffprobe/argument_profile_blake3",
        ],
    ),
    (
        "worker-protocol",
        "fixtures/contracts/e0-s4-worker-incompatible-version.json",
        &["/protocol_version"],
    ),
];

/// A synthesis context to plan against.
///
/// Written out here rather than taken from a helper because the values are not
/// what this test is about: what a plan's schema has to describe is its
/// *shape*, and every field below reaches the plan only through a cache key.
fn planning_context() -> study_tts_core::SynthesisContext {
    study_tts_core::SynthesisContext {
        worker_bundle_hash: blake3::hash(b"schema-test worker bundle").into(),
        model_repository: "study-tts/deterministic-tone".to_owned(),
        model_revision: "none".parse().expect("`none` is a revision"),
        tokenizer_revision: "none".parse().expect("`none` is a revision"),
        language: "en".parse().expect("`en` is a language tag"),
        determinism_class: study_tts_core::DeterminismClass::Reproducible,
        seed: 0,
        generation_parameters: BTreeMap::new(),
        voice_conditioning_hashes: BTreeMap::new(),
    }
}

/// The published schema for one stem, read from `schemas/`.
fn published_schema(stem: &str) -> Value {
    let schema = PUBLISHED_SCHEMAS
        .iter()
        .find(|published| published.stem == stem)
        .unwrap_or_else(|| panic!("`{stem}` is a published schema"));
    read_json(&schema_directory().join(schema.file_name()))
}

#[test]
fn t3_e1_every_published_format_has_an_example_its_schema_and_parser_both_accept() {
    // The half `t3_e1_published_lesson_schema_validates_every_example` proves
    // for one format, proved for all seven. A published schema is a promise to
    // an author's editor, and a format with no example behind it is a promise
    // nobody checked: the editor and the build can disagree about every
    // document of that kind and no test would say so.
    for (stem, fixture, parser) in VALID_EXAMPLES {
        let path = repository_root().join(fixture);
        let document = read_json(&path);

        if let Err(violations) = validate_against_schema(&published_schema(stem), &document) {
            panic!(
                "`{fixture}` does not satisfy the published `{stem}` schema:\n  {}",
                violations.join("\n  ")
            );
        }

        let bytes = fs::read(&path).expect("the valid example is readable");
        parser(&bytes).unwrap_or_else(|error| {
            panic!("`{fixture}` satisfies the `{stem}` schema but its parser refuses it: {error}")
        });
    }

    // Nothing published is left without one. A schema added to the catalogue
    // with no example fails here rather than the first time somebody authors a
    // document against it.
    let covered: BTreeSet<&str> = VALID_EXAMPLES
        .iter()
        .map(|(stem, ..)| *stem)
        .chain(CHECKED_ELSEWHERE)
        .collect();
    let published: BTreeSet<&str> = PUBLISHED_SCHEMAS.iter().map(|schema| schema.stem).collect();

    assert_eq!(
        covered, published,
        "every published format needs a document proving its schema and its parser agree"
    );
}

#[test]
fn t3_e1_every_published_format_refuses_a_document_at_the_field_that_is_wrong() {
    // The direction that makes the test above mean something: proving the
    // valid examples pass says nothing about a schema that accepts everything.
    //
    // Most of these differ from their valid counterpart in exactly one
    // character class — a digest written in uppercase, one digit short, or
    // outside the hexadecimal alphabet — which is what the parsers have always
    // refused and what the published schemas said nothing about until they
    // carried `BLAKE3_HEX_PATTERN`. Every expected pointer is a field that is
    // wrong, not merely "some violation", so a schema refusing the whole
    // document for an unrelated reason still fails here, and a digest field
    // left pointing at a bare `"type": "string"` fails at its own pointer while
    // its neighbours pass.
    for (stem, fixture, pointers) in INVALID_EXAMPLES {
        let document = read_json(&repository_root().join(fixture));

        let violations = validate_against_schema(&published_schema(stem), &document)
            .expect_err(&format!("`{fixture}` must not satisfy the `{stem}` schema"));

        for pointer in pointers {
            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains(pointer)),
                "`{fixture}` was refused, but not at `{pointer}`:\n  {}",
                violations.join("\n  ")
            );
        }
    }

    let covered: BTreeSet<&str> = INVALID_EXAMPLES.iter().map(|(stem, ..)| *stem).collect();
    let published: BTreeSet<&str> = PUBLISHED_SCHEMAS.iter().map(|schema| schema.stem).collect();

    assert_eq!(
        covered,
        published
            .into_iter()
            .filter(|stem| *stem != LESSON_SCHEMA_STEM)
            .collect(),
        "every published format except the lesson, which has its own refusal test, needs a \
         document its schema must refuse"
    );
}

#[test]
fn t3_e1_the_published_plan_schema_describes_what_the_planner_writes() {
    // A plan has no reader, so there is no committed example and no parser to
    // agree with: what the schema has to describe is whatever
    // `RenderPlan::for_lesson` serializes. Planning a real lesson and
    // validating the result is the whole of that check, and it cannot drift —
    // the document is produced by the code under test rather than transcribed
    // beside it.
    let lesson = ValidatedLesson::from_json(
        &fs::read(repository_root().join("fixtures/lessons/e0-s0-two-segment.json"))
            .expect("the lesson fixture is readable"),
    )
    .expect("the lesson fixture validates");
    let plan = study_tts_core::RenderPlan::for_lesson(&lesson, &planning_context());
    let document = serde_json::to_value(&plan).expect("a plan serializes");

    assert_eq!(
        document.get("schema_version").and_then(Value::as_str),
        Some(study_tts_core::PLAN_SCHEMA_VERSION.to_string().as_str()),
        "a persisted plan must say which layout it is, or the E2 loader has nothing to refuse"
    );
    if let Err(violations) = validate_against_schema(&published_schema("plan"), &document) {
        panic!(
            "the planner writes a document its own published schema refuses:\n  {}",
            violations.join("\n  ")
        );
    }
}

#[test]
fn t3_e1_the_published_digest_pattern_accepts_exactly_what_the_parser_does() {
    // `BLAKE3_HEX_PATTERN` is published as `pattern` on every digest in every
    // schema, which is only safe while it is `is_blake3_hex` rather than an
    // approximation of it. Compared over the cases that separate the two rules:
    // length either side of the boundary, uppercase, non-hex letters, and the
    // anchoring that decides whether a digest may be part of a longer string.
    let hex = "0123456789abcdef".repeat(4);
    let cases = [
        hex.clone(),
        "a".repeat(64),
        "f".repeat(64),
        "a".repeat(63),
        "a".repeat(65),
        "A".repeat(64),
        "g".repeat(64),
        String::new(),
        format!(" {hex}"),
        format!("{hex} "),
        format!("{hex}\n"),
        format!("x{}", "a".repeat(63)),
    ];

    let schema = serde_json::json!({"pattern": study_tts_core::BLAKE3_HEX_PATTERN});

    for value in cases {
        assert_eq!(
            validate_against_schema(&schema, &Value::String(value.clone())).is_ok(),
            study_tts_core::is_blake3_hex(&value),
            "the published pattern and the parser disagree about `{value}`"
        );
    }
}

#[test]
fn t3_e1_every_published_schema_link_is_constrained_to_its_own_schema() {
    // Each document carrying `$schema` refuses a link naming another schema,
    // and the published half of that rule is a `schemars` attribute somebody
    // has to remember. Deriving `Option<String>` instead yields
    // `{"type": ["string", "null"]}` — a schema that accepts any link its own
    // parser would refuse, which leaves an author green in the editor and
    // refused by the build. Takes and verification shipped exactly that until
    // the rule moved into `study_tts_core::schema`, so this is the check that
    // notices the next document type to forget it.
    let mut checked = 0;
    for schema in PUBLISHED_SCHEMAS {
        let generated = schema.generate();
        let Some(link) = generated.pointer("/properties/$schema") else {
            continue;
        };
        let expected = schema_uri(schema.stem, schema.version.major());

        assert_eq!(
            link.pointer("/oneOf/0/const").and_then(Value::as_str),
            Some(expected.as_str()),
            "`{}` publishes `$schema` as {link} rather than a constant naming its own schema",
            schema.stem
        );
        checked += 1;
    }

    assert!(
        checked >= 3,
        "the lesson, takes, and verification documents each carry `$schema`, found {checked}"
    );
}

#[test]
fn t3_e1_every_published_schema_claims_the_uri_its_documents_name() {
    // Before `$id`, nothing but the file name connected
    // `https://schemas.study-tts.example/takes-v1.schema.json` — the URI a
    // takes document is now required to carry — to the file that actually
    // holds that schema. A tool handed both could not tell they were the same
    // thing. `$id` declares the name; it does not promise to resolve it, and
    // `SCHEMA_URI_BASE` stays deliberately unresolvable under RFC 2606.
    for schema in PUBLISHED_SCHEMAS {
        let declared = schema
            .generate()
            .pointer("/$id")
            .and_then(Value::as_str)
            .map(str::to_owned);

        assert_eq!(
            declared.as_deref(),
            Some(schema_uri(schema.stem, schema.version.major()).as_str()),
            "`{}` must claim the URI its documents name",
            schema.stem
        );
        // The identity and the file on disk are two spellings of one name, so
        // a schema moved without its URI, or renamed without its file, fails
        // here rather than at whichever tool tried to match them.
        assert!(
            declared
                .as_deref()
                .is_some_and(|id| id.ends_with(&schema.file_name())),
            "`{}` claims `{declared:?}`, which does not end in its file name `{}`",
            schema.stem,
            schema.file_name()
        );
    }
}

/// Every required-field location the published schemas declare today.
///
/// Keyed `"<stem> <major>.<minor>"`, then by the JSON Pointer of the object
/// carrying the `required` array, so a row reads as "this version of this
/// document requires these fields here".
///
/// The controlling rule is
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes,
/// whose **Breaking contract** row names a required field first among the
/// changes that need a major version, migration, impact report, and owner
/// approval. That document names this table in return.
///
/// Written out rather than derived. A table computed from the schemas would
/// agree with any schema it was handed, including one that grew a required
/// field nobody meant to add — which is the change this table exists to make
/// impossible to land quietly.
const PUBLISHED_REQUIRED_SURFACE: [(&str, &str, &[&str]); 38] = [
    (
        "job 0.1",
        "/",
        &["job_id", "plan_hash", "schema_version", "stage"],
    ),
    (
        "job 0.1",
        "/$defs/SelectedPackageIdentity",
        &["manifest_blake3", "package_id"],
    ),
    (
        "lesson 1.1",
        "/",
        &[
            "language",
            "lesson_id",
            "schema_version",
            "segments",
            "title",
        ],
    ),
    (
        "lesson 1.1",
        "/$defs/LessonSegment",
        &[
            "display_text",
            "id",
            "pause_after_ms",
            "review_status",
            "role",
            "source_refs",
            "speaker",
            "spoken_text",
            "style",
        ],
    ),
    (
        "manifest 0.2",
        "/",
        &[
            "artifacts",
            "lesson_id",
            "plan_hash",
            "release_status",
            "schema_version",
            "segments",
            "tools",
        ],
    ),
    (
        "manifest 0.2",
        "/$defs/CurrentStoredToolUse",
        &[
            "argument_profile_blake3",
            "arguments",
            "resolved_executable",
            "version",
        ],
    ),
    ("manifest 0.2", "/$defs/StoredArtifact", &["blake3", "path"]),
    (
        "manifest 0.2",
        "/$defs/StoredArtifacts",
        &["m4a", "master_wav"],
    ),
    (
        "manifest 0.2",
        "/$defs/StoredManifestSegment",
        &[
            "audio_blake3",
            "cache_key",
            "frames",
            "pause_after_ms",
            "segment_id",
        ],
    ),
    ("manifest 0.2", "/$defs/StoredTools", &["ffmpeg", "ffprobe"]),
    (
        "plan 1.0",
        "/",
        &["lesson_id", "plan_hash", "schema_version", "segments"],
    ),
    (
        "plan 1.0",
        "/$defs/PlannedSegment",
        &[
            "cache_key",
            "id",
            "pause_after_ms",
            "speaker",
            "spoken_text",
            "style",
            "take",
        ],
    ),
    (
        "takes 1.0",
        "/",
        &["lesson_id", "schema_version", "selections"],
    ),
    (
        "takes 1.0",
        "/$defs/SelectedTake",
        &[
            "audio_blake3",
            "segment_id",
            "selected_cache_key",
            "selected_take",
            "synthesis_base_key",
        ],
    ),
    (
        "verification 1.0",
        "/",
        &["context", "schema_version", "subject", "verification_key"],
    ),
    (
        "verification 1.0",
        "/$defs/AsrConversionIdentity",
        &["arguments", "ffmpeg_version"],
    ),
    (
        "verification 1.0",
        "/$defs/AsrStackIdentity",
        &[
            "compilation_features",
            "execution_device",
            "model_identity",
            "whisper_cpp_revision",
            "whisper_rs_sys_version",
            "whisper_rs_version",
        ],
    ),
    (
        "verification 1.0",
        "/$defs/VerificationContext",
        &[
            "comparison_normalizer_hash",
            "conversion",
            "decoder_parameters",
            "expected_pattern_profile_hash",
            "stack",
            "thread_count",
            "threshold_profile_hash",
        ],
    ),
    (
        "verification 1.0",
        "/$defs/VerificationSubject",
        &["audio_blake3", "spoken_text"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/InitializeParameters",
        &["threads", "worker_bundle_hash"],
    ),
    ("worker-protocol 1.0", "/$defs/TraceContext", &["trace_id"]),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerCapabilities",
        &[
            "channels",
            "deterministic_seed",
            "device",
            "languages",
            "max_text_bytes",
            "sample_format",
            "sample_rate",
            "styles",
            "voices",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerInitializationIdentities",
        &[
            "model_revision",
            "tokenizer_revision",
            "voice_profile_hashes",
            "worker_bundle_hash",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/0",
        &["method", "parameters", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/1",
        &["method", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/2",
        &["method", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/3",
        &["method", "parameters", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/4",
        &[
            "active_request_id",
            "method",
            "protocol_version",
            "request_id",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerRequestFrame/oneOf/5",
        &["method", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/0",
        &["event", "identities", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/1",
        &["capabilities", "event", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/2",
        &[
            "event",
            "model_loaded",
            "protocol_version",
            "ready",
            "request_id",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/3",
        &["event", "progress", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/4",
        &[
            "channels",
            "codec_revision",
            "event",
            "frames",
            "model_revision",
            "protocol_version",
            "request_id",
            "sample_rate",
            "voice_profile_hash",
            "worker_bundle_hash",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/5",
        &[
            "active_request_id",
            "event",
            "protocol_version",
            "request_id",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/6",
        &["event", "protocol_version", "request_id"],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerResponseFrame/oneOf/7",
        &[
            "code",
            "event",
            "message",
            "protocol_version",
            "recoverable",
            "request_id",
        ],
    ),
    (
        "worker-protocol 1.0",
        "/$defs/WorkerSynthesisParameters",
        &["output", "seed", "style", "take", "text", "voice"],
    ),
];

/// The required-field surface of one generated schema, by JSON Pointer.
///
/// Walks the whole document rather than its top level: `$defs` is where every
/// frame and nested record declares what it requires, and a check that read
/// only the root would have watched the one object that changes least.
fn required_surface(schema: &Value) -> BTreeMap<String, Vec<String>> {
    fn walk(node: &Value, pointer: &str, found: &mut BTreeMap<String, Vec<String>>) {
        match node {
            Value::Object(fields) => {
                if let Some(required) = fields.get("required").and_then(Value::as_array) {
                    // Sorted, because `required` is a set: reordering it
                    // changes no contract, and an alphabetical list is what
                    // lets a reviewer see at a glance which name appeared.
                    // Reordering still moves the published bytes, and
                    // `t3_e1_generated_schemas_match_checked_in_files` is what
                    // catches that.
                    let mut names: Vec<String> = required
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect();
                    names.sort();
                    // A property *named* `required` holding something other
                    // than a list of strings is a subschema, not a constraint.
                    if names.len() == required.len() {
                        let location = if pointer.is_empty() { "/" } else { pointer };
                        found.insert(location.to_owned(), names);
                    }
                }
                for (key, value) in fields {
                    walk(value, &format!("{pointer}/{key}"), found);
                }
            }
            Value::Array(items) => {
                for (index, item) in items.iter().enumerate() {
                    walk(item, &format!("{pointer}/{index}"), found);
                }
            }
            _ => {}
        }
    }

    let mut found = BTreeMap::new();
    walk(schema, "", &mut found);
    found
}

/// A required field cannot enter or leave a published schema unremarked.
///
/// Not in `DELIVERY-PLAN.md`; it carries a direction the four named E1-S1
/// schema tests do not. They prove the checked-in files match the types and
/// that the version gate refuses the right documents. None of them looks at
/// *what changed* between one version of a document and the next, so a
/// required field could appear in a published schema while its version stood
/// still, and every one of them would still pass.
///
/// What this test gives is exact, and worth stating as narrowly as it holds:
/// any movement in the required-field surface fails the suite until somebody
/// edits [`PUBLISHED_REQUIRED_SURFACE`], and the diff then names the document,
/// the version, the pointer, and the field. It does not decide the version —
/// an author who edits the schema and this table together still passes. The
/// guarantee is that the change is explicit and reviewable at the point it is
/// made, not that a machine chose the version number.
#[test]
fn t3_e1_published_schema_required_fields_match_the_recorded_surface() {
    let mut recorded: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for (version, pointer, fields) in PUBLISHED_REQUIRED_SURFACE {
        let previous = recorded.entry(version.to_owned()).or_default().insert(
            pointer.to_owned(),
            fields.iter().map(|f| (*f).to_owned()).collect(),
        );
        assert!(previous.is_none(), "`{version}` records `{pointer}` twice");
    }

    for schema in PUBLISHED_SCHEMAS {
        let version = format!("{} {}", schema.stem, schema.version);
        let expected = recorded.remove(&version).unwrap_or_else(|| {
            panic!(
                "no required-field surface is recorded for `{version}`. A published document at a \
                 version this table does not know has either just been added or just moved; \
                 record it here together with the interface-change record that classifies it."
            )
        });

        // Reported location by location rather than as two whole maps. The
        // worker protocol declares nineteen of them, and a reviewer handed
        // both copies in full has to find the one that moved by eye.
        let published = required_surface(&schema.generate());
        let moved: Vec<String> = published
            .keys()
            .chain(expected.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|pointer| published.get(*pointer) != expected.get(*pointer))
            .map(|pointer| {
                let recorded = expected.get(pointer).map(Vec::as_slice).unwrap_or_default();
                let now = published
                    .get(pointer)
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                format!("{pointer}: recorded {recorded:?}, published {now:?}")
            })
            .collect();

        assert!(
            moved.is_empty(),
            "the required-field surface of `{version}` is not what this table records. \
             `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes puts a \
             required-field change under **Breaking contract**: move the major version and file \
             the interface-change record, then record the new surface here. What moved: {moved:#?}"
        );
    }

    assert!(
        recorded.is_empty(),
        "this table records surfaces no published document claims: {:?}. A retired version is \
         removed by the change that retired it.",
        recorded.keys().collect::<Vec<_>>()
    );
}
