//! The published JSON Schemas: what they cover, and where they come from.
//!
//! ADR-0001 §7.1 puts checked-in schemas at `schemas/` and §17 requires every
//! lesson, worker, takes, verification, job, and manifest fixture to be
//! validated against one. This module is the single list of which documents
//! those are and which Rust type defines each.
//!
//! **The schemas are generated, never hand-written.** A hand-written schema is
//! a second definition of a format, and two definitions of one format disagree
//! the moment somebody edits either. Each entry below names the type whose
//! `serde` representation *is* the format, `schemars` derives the schema from
//! it, and `t3_e1_generated_schemas_match_checked_in_files` fails if a
//! checked-in file has drifted from what regeneration would produce. Regenerate
//! with:
//!
//! ```text
//! cargo run --package study-tts-runtime --example generate-schemas
//! ```
//!
//! The catalogue lives in this crate rather than in `study-tts-core` because
//! two of the seven documents — the manifest and the worker protocol — are
//! defined here, and a catalogue that could not see them would be a catalogue
//! with a hole in it.
//!
//! Some documents are described more narrowly than their eventual scope, and
//! each says so on its own type rather than here: the verification record
//! carries the identity and not yet the findings
//! ([`study_tts_core::VerificationIdentityRecord`]), and the job snapshot is
//! the provisional E0 progress record rather than the E2 state machine
//! ([`study_tts_core::ProvisionalJobSnapshot`]).

use serde_json::Value;
use study_tts_core::{
    AuthoredLesson, LESSON_SCHEMA_STEM, LESSON_SCHEMA_VERSION, PLAN_SCHEMA_STEM,
    PLAN_SCHEMA_VERSION, PROVISIONAL_JOB_SCHEMA_VERSION, ProvisionalJobSnapshot, RenderPlan,
    SchemaVersion, TAKES_SCHEMA_STEM, TAKES_SCHEMA_VERSION, TakesDocument,
    VERIFICATION_SCHEMA_STEM, VERIFICATION_SCHEMA_VERSION, VerificationIdentityRecord,
    schema_file_name, schema_uri,
};

use crate::worker_protocol::WorkerFrame;

/// Directory holding the published schemas, relative to the repository root.
pub const SCHEMA_DIRECTORY: &str = "schemas";

/// One published schema: what it is called, what version it describes, and how
/// to produce it.
///
/// Holds a function pointer rather than a generated value so the catalogue is a
/// `const` a reader can take in at once, and so generating seven schemas costs
/// nothing until somebody asks for one.
#[derive(Clone, Copy, Debug)]
pub struct PublishedSchema {
    /// File-name stem, without `-v<major>.schema.json`.
    pub stem: &'static str,
    /// Document version this schema describes.
    pub version: SchemaVersion,
    /// Produces the schema from the Rust type that defines the format.
    generate: fn() -> Value,
}

impl PublishedSchema {
    /// The repository file name of this schema.
    pub fn file_name(&self) -> String {
        schema_file_name(self.stem, self.version.major())
    }

    /// Generates this schema's JSON.
    ///
    /// Pretty-printed with a trailing newline by [`PublishedSchema::to_bytes`],
    /// which is what the checked-in files hold; a compact form would make every
    /// review of a schema change a review of one very long line.
    ///
    /// `publish_integer_bounds` and `$id` are applied over every document on
    /// the way out, so both belong to publication rather than to whichever type
    /// happened to remember an attribute.
    ///
    /// `$id` is the name a document already gives this schema. Three formats
    /// carry a `$schema` link constrained to `schema_uri(stem, major)`, and
    /// without `$id` nothing but the file name connected that URI to the file
    /// holding the schema — a tool handed both could not tell they were the
    /// same thing. Declaring a name is not promising to resolve it:
    /// `SCHEMA_URI_BASE` stays deliberately unresolvable under RFC 2606, and
    /// `$id` says which schema this is rather than where to fetch it.
    pub fn generate(&self) -> Value {
        let mut schema = (self.generate)();
        publish_integer_bounds(&mut schema);
        if let Some(fields) = schema.as_object_mut() {
            fields.insert(
                "$id".to_owned(),
                Value::from(schema_uri(self.stem, self.version.major())),
            );
        }
        schema
    }

    /// The exact bytes this schema's checked-in file must hold.
    ///
    /// # Panics
    ///
    /// Never, from any argument. The value being serialized is a
    /// [`serde_json::Value`] produced by `schemars`, and `Value`'s own
    /// `Serialize` has no failing path: it holds no map with non-string keys
    /// and no nested type with a `Serialize` that can refuse. The arm exists
    /// because `serde_json::to_vec_pretty` returns a `Result`, not because a
    /// caller can reach it.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = serde_json::to_vec_pretty(&self.generate())
            .unwrap_or_else(|error| unreachable!("a serde_json::Value serializes: {error}"));
        bytes.push(b'\n');
        bytes
    }
}

/// Every schema this project publishes.
///
/// The list ADR-0001 §7.1 and DELIVERY-PLAN E1-S1 task 3 require: lesson, plan,
/// job, takes, verification, manifest, and worker protocol.
///
/// A missing entry cannot hide, because
/// `t3_e1_generated_schemas_match_checked_in_files` compares this list against
/// the contents of `schemas/` in both directions: a file with no entry and an
/// entry with no file each fail it.
pub const PUBLISHED_SCHEMAS: [PublishedSchema; 7] = [
    PublishedSchema {
        stem: LESSON_SCHEMA_STEM,
        version: LESSON_SCHEMA_VERSION,
        generate: || schema_of::<AuthoredLesson>(),
    },
    PublishedSchema {
        stem: PLAN_SCHEMA_STEM,
        version: PLAN_SCHEMA_VERSION,
        generate: || schema_of::<RenderPlan>(),
    },
    PublishedSchema {
        stem: "job",
        version: JOB_SCHEMA_VERSION,
        generate: || schema_of::<ProvisionalJobSnapshot>(),
    },
    PublishedSchema {
        stem: TAKES_SCHEMA_STEM,
        version: TAKES_SCHEMA_VERSION,
        generate: || schema_of::<TakesDocument>(),
    },
    PublishedSchema {
        stem: VERIFICATION_SCHEMA_STEM,
        version: VERIFICATION_SCHEMA_VERSION,
        generate: || schema_of::<VerificationIdentityRecord>(),
    },
    PublishedSchema {
        stem: "manifest",
        version: MANIFEST_SCHEMA_VERSION,
        generate: crate::manifest::current_manifest_schema,
    },
    PublishedSchema {
        stem: "worker-protocol",
        version: WORKER_PROTOCOL_SCHEMA_VERSION,
        generate: || schema_of::<WorkerFrame>(),
    },
];

/// Version of the published job-state schema.
///
/// `0.1` rather than `1.0`, and deliberately: the record is the provisional E0
/// snapshot that `PROVISIONAL_JOB_SCHEMA_VERSION` labels
/// `e0.job-state.0.1`, and ADR-0001 §12.4 assigns the real state machine to
/// E2-S1. Publishing it as `1.0` would claim a stability E2-S1 is going to
/// break.
pub const JOB_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(0, 1);

/// Version of the published manifest schema.
///
/// `0.2` because `manifest.json` already exists on disk under the
/// `0.2-skeleton` label that `crate::manifest` writes, and a published schema
/// that renumbered it would describe a document nothing produces.
pub const MANIFEST_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(0, 2);

/// Version of the published worker-protocol schema.
///
/// `0.1`, matching the `e0.worker.0.1` baseline frame version in
/// `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`. The frames stay
/// provisional until the G1 freeze, and the schema says so by sharing their
/// number.
pub const WORKER_PROTOCOL_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(0, 1);

// The job snapshot's two version spellings must not drift: one labels the
// record on disk, the other names its published schema.
const _: () = assert!(
    matches!(
        PROVISIONAL_JOB_SCHEMA_VERSION.as_bytes(),
        b"e0.job-state.0.1"
    ),
    "the published job schema version must follow PROVISIONAL_JOB_SCHEMA_VERSION"
);

/// Generates the schema for one document type.
fn schema_of<T: schemars::JsonSchema>() -> Value {
    Value::from(schemars::schema_for!(T))
}

/// The largest value each fixed-width integer format can hold.
///
/// The whole table rather than the widths this project happens to use today:
/// the point is that a new field of any width is bounded the moment it is
/// published, and a table with holes in it would bound some and not others.
const INTEGER_FORMAT_MAXIMUMS: [(&str, u64); 4] = [
    ("uint8", u8::MAX as u64),
    ("uint16", u16::MAX as u64),
    ("uint32", u32::MAX as u64),
    ("uint64", u64::MAX),
];

/// Writes the `maximum` each fixed-width integer format already implies.
///
/// `schemars` describes a `u32` as `"format": "uint32"` and, above `u16`, stops
/// there. `format` is an annotation a JSON Schema validator may ignore, so the
/// published documents admitted values the Rust parsers refuse — a worker frame
/// carrying `take: 4294967296` validated against `worker-protocol-v0` and was
/// then dropped by `parse_worker_request`. An author whose editor is green and
/// whose build fails has been told the wrong thing by the schema this project
/// publishes for exactly that purpose.
///
/// Applied here rather than as a `#[schemars(range(...))]` on each field
/// because a per-field attribute is a list somebody has to remember to extend:
/// the next integer field would be published unbounded and nothing would say
/// so. What a field still owns is a bound narrower than its width —
/// `WorkerResponseFrame::Progress`'s zero-to-one fraction is a domain rule and
/// carries its own attribute.
///
/// An existing `maximum` is never overwritten, so the narrower bound wins.
///
/// The worker protocol's ceilings have a second reader: `UNSIGNED_32_MAXIMUM`
/// and `UNSIGNED_64_MAXIMUM` in `worker/study_tts_worker/protocol.py` bound the
/// same fields at the other end of the wire, and name this function in return.
/// Python integers have no width, so without them a frame this build refuses
/// was answered there.
fn publish_integer_bounds(node: &mut Value) {
    match node {
        Value::Object(fields) => {
            if let Some(maximum) = fields
                .get("format")
                .and_then(Value::as_str)
                .and_then(format_maximum)
                && !fields.contains_key("maximum")
            {
                fields.insert("maximum".to_owned(), Value::from(maximum));
            }
            for value in fields.values_mut() {
                publish_integer_bounds(value);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(publish_integer_bounds),
        _ => {}
    }
}

/// The maximum one `format` annotation implies, if this build knows the format.
fn format_maximum(format: &str) -> Option<u64> {
    INTEGER_FORMAT_MAXIMUMS
        .iter()
        .find(|(name, _)| *name == format)
        .map(|(_, maximum)| *maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_e1_every_published_schema_has_a_distinct_file_name() {
        // Two documents sharing a file name would silently publish one and
        // discard the other, and the checked-in comparison would still pass
        // because the surviving file matches its own generator.
        let mut names: Vec<String> = PUBLISHED_SCHEMAS
            .iter()
            .map(PublishedSchema::file_name)
            .collect();
        let published = names.len();
        names.sort();
        names.dedup();

        assert_eq!(
            names.len(),
            published,
            "published schema names must be distinct"
        );
    }

    #[test]
    fn t3_e1_every_published_numeric_field_declares_the_range_it_accepts() {
        // The gap this closes: `format` is an annotation a validator may
        // ignore, so an unbounded `"format": "uint32"` published a document
        // that admits `4294967296` and a parser that refuses it. Presence is
        // what is asserted rather than the value — a test that recomputed the
        // bound would pass for any bound, including a wrong one.
        for schema in PUBLISHED_SCHEMAS {
            let mut unbounded = Vec::new();
            collect_unbounded_numbers(&schema.generate(), &mut Vec::new(), &mut unbounded);

            assert!(
                unbounded.is_empty(),
                "`{}` publishes numeric fields with no `maximum`: {unbounded:?}",
                schema.file_name()
            );
        }
    }

    /// Records the JSON path of every numeric node carrying no `maximum`.
    fn collect_unbounded_numbers(node: &Value, trail: &mut Vec<String>, found: &mut Vec<String>) {
        match node {
            Value::Object(fields) => {
                let numeric = matches!(
                    fields.get("type").and_then(Value::as_str),
                    Some("integer" | "number")
                );
                if numeric && !fields.contains_key("maximum") {
                    found.push(trail.join("."));
                }
                for (name, value) in fields {
                    trail.push(name.clone());
                    collect_unbounded_numbers(value, trail, found);
                    trail.pop();
                }
            }
            Value::Array(items) => {
                for (index, value) in items.iter().enumerate() {
                    trail.push(index.to_string());
                    collect_unbounded_numbers(value, trail, found);
                    trail.pop();
                }
            }
            _ => {}
        }
    }

    #[test]
    fn t1_e1_every_published_schema_generates_an_object_schema() {
        // Each entry must actually produce a schema for a document; a type that
        // generated a bare `true` would validate anything at all.
        for schema in PUBLISHED_SCHEMAS {
            let generated = schema.generate();

            assert_eq!(
                generated.get("$schema").and_then(Value::as_str),
                Some("https://json-schema.org/draft/2020-12/schema"),
                "`{}` must declare the JSON Schema dialect it is written in",
                schema.file_name()
            );
            assert!(
                ["properties", "oneOf", "anyOf"]
                    .iter()
                    .any(|keyword| generated.get(keyword).is_some()),
                "`{}` must describe a document rather than accept anything",
                schema.file_name()
            );
        }
    }
}
