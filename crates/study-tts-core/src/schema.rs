//! Versioning for every document this project defines, and the rule that
//! decides whether this build may read one.
//!
//! `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
//! assigns a semantic meaning to each part of a document version: a compatible
//! extension adds an optional field under a **minor** increment, and a required
//! field, semantic change, or frame change is breaking and takes a **major**
//! increment. That document names this module in return, so the policy and the
//! code that enforces it stay discoverable from either end.
//!
//! [`SchemaVersion::accepted_by`] is the whole of the rule:
//!
//! - A different major is refused. Its required fields are not this build's.
//! - A newer minor is refused. It carries an extension whose default this build
//!   does not know, and every project-owned boundary sets
//!   `deny_unknown_fields`, so reading it would fail later with a worse
//!   message.
//! - An older minor of the same major is accepted, and the fields it omits take
//!   the defaults their extension declared.
//!
//! The direction matters: refusing forward and accepting backward is what lets
//! an author keep a document that predates an extension, while never letting
//! this build guess at one it has not seen.
//!
//! The rule runs *before* a document is parsed, through
//! [`check_declared_version`], and that ordering is load-bearing for the same
//! reason: the documents it exists to refuse are exactly the ones carrying
//! fields this build has never seen, so a strict parse first would report a
//! future version as malformed JSON and send its author looking for a syntax
//! error they did not make.

use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::json_schema_as_string;

/// Identifier prefix every published schema is named by.
///
/// Deliberately not resolvable. ADR-0001 §14 renders offline and denies network
/// egress during the contract test, so a `$schema` a reader might fetch would
/// be a URI that only works where the project promises nothing. `.example` is
/// reserved by RFC 2606 for exactly this: a name that identifies without
/// resolving. The bytes live in the repository at `schemas/`, which
/// [`SchemaVersion`] callers reach through
/// `study_tts_runtime::schemas::SCHEMA_DIRECTORY`.
pub const SCHEMA_URI_BASE: &str = "https://schemas.study-tts.example/";

/// The published URI of one schema file.
///
/// `stem` is the file's name without `-v<major>.schema.json`, and `major` is
/// the document's major version, because ADR-0001 §7.1 names schema files
/// `lesson-v1.schema.json` — one file per major, describing that major's
/// current minor.
///
/// # Examples
///
/// ```rust
/// use study_tts_core::{SchemaVersion, schema_uri};
///
/// let version: SchemaVersion = "1.1".parse()?;
/// assert_eq!(
///     schema_uri("lesson", version.major()),
///     "https://schemas.study-tts.example/lesson-v1.schema.json"
/// );
/// # Ok::<(), study_tts_core::SchemaVersionError>(())
/// ```
pub fn schema_uri(stem: &str, major: u16) -> String {
    format!("{SCHEMA_URI_BASE}{}", schema_file_name(stem, major))
}

/// The repository file name of one schema, matching the ADR-0001 §7.1 tree.
pub fn schema_file_name(stem: &str, major: u16) -> String {
    format!("{stem}-v{major}.schema.json")
}

/// The declared layout version of one document.
///
/// A pair of numbers rather than a free string, because the change-control
/// classes above are defined on the two parts separately: comparing whole
/// strings could only ever answer "same or different", which is not the
/// question a reader has to answer.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct SchemaVersion {
    major: u16,
    minor: u16,
}

impl SchemaVersion {
    /// Declares a version.
    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// The major component, which changes only for a breaking change.
    pub const fn major(self) -> u16 {
        self.major
    }

    /// The minor component, which changes for a compatible extension.
    pub const fn minor(self) -> u16 {
        self.minor
    }

    /// Decides whether a document declaring this version may be read by a build
    /// that publishes `supported`.
    ///
    /// # Errors
    ///
    /// [`SchemaVersionError::UnsupportedMajor`] when the majors differ, and
    /// [`SchemaVersionError::UnsupportedMinor`] when this version is a newer
    /// minor of the same major. Both name the version this build publishes, so
    /// an author is told what to write rather than only that they were wrong.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use study_tts_core::{SchemaCompatibility, SchemaVersion};
    /// use study_tts_core::SchemaVersionError;
    ///
    /// let supported = SchemaVersion::new(1, 1);
    ///
    /// assert_eq!(
    ///     SchemaVersion::new(1, 0).accepted_by(supported)?,
    ///     SchemaCompatibility::CompatibleExtension
    /// );
    /// assert!(matches!(
    ///     SchemaVersion::new(2, 0).accepted_by(supported),
    ///     Err(SchemaVersionError::UnsupportedMajor { .. })
    /// ));
    /// # Ok::<(), SchemaVersionError>(())
    /// ```
    pub fn accepted_by(self, supported: Self) -> Result<SchemaCompatibility, SchemaVersionError> {
        if self.major != supported.major {
            return Err(SchemaVersionError::UnsupportedMajor {
                declared: self,
                supported,
            });
        }
        match self.minor.cmp(&supported.minor) {
            Ordering::Equal => Ok(SchemaCompatibility::Current),
            Ordering::Less => Ok(SchemaCompatibility::CompatibleExtension),
            Ordering::Greater => Err(SchemaVersionError::UnsupportedMinor {
                declared: self,
                supported,
            }),
        }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.major, self.minor)
    }
}

impl From<SchemaVersion> for String {
    fn from(version: SchemaVersion) -> Self {
        version.to_string()
    }
}

impl TryFrom<String> for SchemaVersion {
    type Error = SchemaVersionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl FromStr for SchemaVersion {
    type Err = SchemaVersionError;

    /// Accepts exactly `<major>.<minor>` in decimal, with no sign, no leading
    /// zero, and no third component.
    ///
    /// Strict because the version is the one field read before anything else is
    /// trusted. `"1.1-draft"` and `"01.1"` are refused rather than normalized:
    /// a boundary that repairs its own input cannot report that the input was
    /// wrong.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let malformed = || SchemaVersionError::Malformed(value.to_owned());
        let (major, minor) = value.split_once('.').ok_or_else(malformed)?;
        Ok(Self {
            major: parse_version_component(major).ok_or_else(malformed)?,
            minor: parse_version_component(minor).ok_or_else(malformed)?,
        })
    }
}

/// Parses one version component.
///
/// Refuses the spellings `u16::from_str` allows that would give a single
/// version two forms, and — through that same `u16` — anything from 65536 up,
/// which is the range [`SCHEMA_VERSION_PATTERN`] admits and this does not.
fn parse_version_component(value: &str) -> Option<u16> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if value.len() > 1 && value.starts_with('0') {
        return None;
    }
    value.parse().ok()
}

/// Applies [`SchemaVersion::accepted_by`] to the version a document declares,
/// before that document is parsed as the shape this build expects.
///
/// Reads the version out of a *lenient* header, which is the only way to honour
/// the ordering: the strict parse every boundary performs would reject a future
/// major's new field first, and report a version this build cannot read as
/// malformed JSON.
///
/// A document this cannot find a version in is passed through untouched. The
/// parse that follows owns that report and names the missing or mistyped field,
/// which is a better message than anything this function knows enough to give.
///
/// # Errors
///
/// [`SchemaVersionError::Malformed`] when the declared version is not
/// `<major>.<minor>`, and [`SchemaVersionError::UnsupportedMajor`] or
/// [`SchemaVersionError::UnsupportedMinor`] as [`SchemaVersion::accepted_by`]
/// decides them.
pub(crate) fn check_declared_version(
    bytes: &[u8],
    supported: SchemaVersion,
) -> Result<(), SchemaVersionError> {
    /// Just enough of any versioned document to read its version.
    ///
    /// Deliberately without `deny_unknown_fields`: ignoring the rest of the
    /// document is the whole point of reading it separately.
    #[derive(Deserialize)]
    struct VersionHeader {
        schema_version: String,
    }

    let Ok(header) = serde_json::from_slice::<VersionHeader>(bytes) else {
        return Ok(());
    };
    header
        .schema_version
        .parse::<SchemaVersion>()?
        .accepted_by(supported)?;
    Ok(())
}

/// The published JSON Schema `pattern` for a document version.
///
/// The whole of `parse_version_component`'s spelling rule: decimal digits, at
/// least one, and no leading zero on a value that is not itself zero. What it
/// cannot carry is the range — each component parses as a `u16`, and a regular
/// expression has no way to say "below 65536" without becoming unreadable — so
/// `"70000.0"` is a version this pattern admits and the parser refuses. That is
/// the safe direction: an editor accepts a little more than the build, never
/// less. The description says so, because a reader of the published schema
/// cannot see this comment.
pub const SCHEMA_VERSION_PATTERN: &str = r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$(?![\s\S])";

json_schema_as_string!(
    SchemaVersion,
    "SchemaVersion",
    "A document layout version, written as <major>.<minor> in decimal with no \
     leading zeros. Each component is additionally bounded below 65536, which \
     the pattern cannot express and the parser enforces.",
    pattern = SCHEMA_VERSION_PATTERN,
);

/// The versions of one document this build reads, as a published `enum`.
///
/// Every document whose `schema_version` is authored text calls this with the
/// version it publishes, so the schema lists exactly what
/// [`SchemaVersion::accepted_by`] accepts: this major, at this minor and every
/// earlier one. Written once because three documents need it and three copies
/// of a version rule are three chances to publish a different one.
pub(crate) fn accepted_versions_json_schema(current: SchemaVersion) -> schemars::Schema {
    let accepted: Vec<String> = (0..=current.minor())
        .map(|minor| SchemaVersion::new(current.major(), minor).to_string())
        .collect();
    schemars::json_schema!({
        "type": "string",
        "enum": accepted,
    })
}

/// The one schema link a document of this major may carry, as a published
/// `oneOf`.
///
/// Written once for the reason [`accepted_versions_json_schema`] is: three
/// documents carry a `$schema` field and each refuses a link naming another
/// schema, so three copies of the rule are three chances to publish a
/// different one — or, as happened, to publish it for one document and leave
/// the other two accepting any string their parser would refuse.
///
/// `Option<String>` accepts either the URI or explicit `null`; omission is
/// represented by the field not being required.
pub(crate) fn schema_link_json_schema(stem: &str, version: SchemaVersion) -> schemars::Schema {
    schemars::json_schema!({
        "oneOf": [
            {"const": schema_uri(stem, version.major())},
            {"type": "null"},
        ],
    })
}

/// How a declared document version relates to the one this build publishes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaCompatibility {
    /// The document declares exactly the version this build publishes.
    Current,
    /// The document predates one or more compatible extensions, whose declared
    /// defaults apply to the fields it omits.
    CompatibleExtension,
}

/// Why a declared document version is not one this build can read.
///
/// Remedy routing: every variant names the version this build publishes, and
/// the owner is the document's author. `docs/governance/ROUTING-TABLES.md`
/// routes an authoring refusal to the author rather than to runtime, because
/// nothing on disk is damaged — the document simply describes a layout this
/// build does not implement.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SchemaVersionError {
    /// The value is not `<major>.<minor>` in decimal.
    #[error(
        "schema version `{0}` is not `<major>.<minor>` in decimal; the document's author must \
         write the version the build publishes rather than a label"
    )]
    Malformed(String),
    /// The document belongs to a different major version of its schema.
    #[error(
        "schema version `{declared}` is a different major version from `{supported}`, which this \
         build publishes; the document's author must migrate the document rather than relabel it, \
         because a major version changes required fields"
    )]
    UnsupportedMajor {
        /// Version the document declares.
        declared: SchemaVersion,
        /// Version this build publishes.
        supported: SchemaVersion,
    },
    /// The document declares a newer minor than this build knows.
    #[error(
        "schema version `{declared}` is newer than `{supported}`, which this build publishes; the \
         document's author must upgrade the build rather than downgrade the document, because \
         this build does not know the defaults that extension declared"
    )]
    UnsupportedMinor {
        /// Version the document declares.
        declared: SchemaVersion,
        /// Version this build publishes.
        supported: SchemaVersion,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_e1_schema_versions_round_trip_through_their_written_form() {
        for (text, version) in [
            ("0.1", SchemaVersion::new(0, 1)),
            ("1.0", SchemaVersion::new(1, 0)),
            ("1.11", SchemaVersion::new(1, 11)),
            ("65535.65535", SchemaVersion::new(u16::MAX, u16::MAX)),
        ] {
            assert_eq!(text.parse::<SchemaVersion>(), Ok(version), "parsing {text}");
            assert_eq!(version.to_string(), text, "writing {version:?}");
        }
    }

    #[test]
    fn t1_e1_schema_versions_with_a_second_spelling_are_refused() {
        // Each of these would otherwise give one version two written forms, and
        // the version is the field read before anything else is trusted.
        for malformed in [
            "",
            "1",
            "1.",
            ".1",
            "01.1",
            "1.01",
            "1.0.0",
            "1.1-draft",
            "v1.1",
            "+1.1",
            "-1.1",
            "1 . 1",
            "1.65536",
            "0.1-skeleton",
        ] {
            assert_eq!(
                malformed.parse::<SchemaVersion>(),
                Err(SchemaVersionError::Malformed(malformed.to_owned())),
                "`{malformed}` must not parse as a schema version"
            );
        }
    }

    #[test]
    fn t1_e1_version_acceptance_follows_the_change_class_table() {
        // Read off `INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
        // rather than re-derived from `accepted_by`: a table that recomputed
        // the rule would agree with any rule, including a wrong one.
        let supported = SchemaVersion::new(1, 2);
        let expected: [(u16, u16, Result<SchemaCompatibility, ()>); 8] = [
            (1, 2, Ok(SchemaCompatibility::Current)),
            (1, 1, Ok(SchemaCompatibility::CompatibleExtension)),
            (1, 0, Ok(SchemaCompatibility::CompatibleExtension)),
            (1, 3, Err(())),
            (0, 2, Err(())),
            (0, 9, Err(())),
            (2, 0, Err(())),
            (2, 2, Err(())),
        ];

        for (major, minor, outcome) in expected {
            let declared = SchemaVersion::new(major, minor);

            assert_eq!(
                declared.accepted_by(supported).map_err(|_| ()),
                outcome,
                "`{declared}` against a build publishing `{supported}`"
            );
        }
    }

    #[test]
    fn t1_e1_a_refused_version_names_which_part_disagreed() {
        // The two refusals have different remedies — migrate the document, or
        // upgrade the build — so a caller must be able to tell them apart.
        let supported = SchemaVersion::new(1, 2);

        assert_eq!(
            SchemaVersion::new(2, 0).accepted_by(supported),
            Err(SchemaVersionError::UnsupportedMajor {
                declared: SchemaVersion::new(2, 0),
                supported,
            })
        );
        assert_eq!(
            SchemaVersion::new(1, 3).accepted_by(supported),
            Err(SchemaVersionError::UnsupportedMinor {
                declared: SchemaVersion::new(1, 3),
                supported,
            })
        );
    }

    #[test]
    fn t1_e1_a_declared_version_is_gated_before_the_document_is_parsed() {
        // The whole reason the gate reads a lenient header of its own: every
        // document this rule exists to refuse carries a field this build has
        // never seen, and every boundary denies unknown fields. Parsed first,
        // each of these is malformed JSON naming a field, and its author goes
        // looking for a syntax error they did not make.
        let supported = SchemaVersion::new(1, 1);

        assert_eq!(
            check_declared_version(br#"{"schema_version":"2.0","added_by_two":1}"#, supported),
            Err(SchemaVersionError::UnsupportedMajor {
                declared: SchemaVersion::new(2, 0),
                supported,
            })
        );
        assert_eq!(
            check_declared_version(
                br#"{"schema_version":"1.2","added_by_one_two":1}"#,
                supported
            ),
            Err(SchemaVersionError::UnsupportedMinor {
                declared: SchemaVersion::new(1, 2),
                supported,
            })
        );
        assert_eq!(
            check_declared_version(br#"{"schema_version":"0.1-skeleton"}"#, supported),
            Err(SchemaVersionError::Malformed("0.1-skeleton".to_owned()))
        );

        // Both accepted directions, so the gate cannot pass by refusing
        // everything it is handed.
        let accepted: [&[u8]; 2] = [
            br#"{"schema_version":"1.1","unread_by_this_check":[]}"#,
            br#"{"schema_version":"1.0"}"#,
        ];
        for document in accepted {
            assert_eq!(
                check_declared_version(document, supported),
                Ok(()),
                "a version this build publishes, or an earlier minor, must pass the gate"
            );
        }
    }

    #[test]
    fn t1_e1_a_document_with_no_readable_version_is_left_to_its_own_parser() {
        // The arm most likely to be "fixed" into a refusal, which would be a
        // regression: this function knows only that it found no version, while
        // the strict parse that follows can name the field that is missing or
        // mistyped. Refusing here would replace that message with a worse one.
        let supported = SchemaVersion::new(1, 1);
        let unreadable: [&[u8]; 5] = [
            b"{}",
            br#"{"lesson_id":"e0-s0-walking-skeleton"}"#,
            br#"{"schema_version":11}"#,
            br#"{"schema_version":null}"#,
            b"not json at all",
        ];

        for document in unreadable {
            assert_eq!(
                check_declared_version(document, supported),
                Ok(()),
                "a document with no readable version must reach the parse that reports it"
            );
        }
    }

    #[test]
    fn t1_e1_schema_uris_name_the_major_version_only() {
        // One file per major, describing that major's current minor: a minor
        // extension must not orphan the URI already written into documents.
        assert_eq!(
            schema_uri("lesson", 1),
            "https://schemas.study-tts.example/lesson-v1.schema.json"
        );
        assert_eq!(
            schema_file_name("worker-protocol", 2),
            "worker-protocol-v2.schema.json"
        );
    }
}
