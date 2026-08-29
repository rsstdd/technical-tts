//! Canonical bytes for the project's structured identities.
//!
//! ADR-0001 §12.5 requires BLAKE3 over canonical serialization. This module
//! owns that byte form so field order and serializer defaults cannot change an
//! identity. Artifact checksums hash their raw bytes instead.
//!
//! The format is project-local rather than RFC 8785 because no external
//! implementation recomputes identities. Object keys sort by UTF-8 byte order,
//! whitespace is omitted, strings use short JSON escapes where available and
//! lowercase `\u00xx` escapes for other controls, and unsigned integers use
//! their shortest decimal form.
//!
//! Floating-point values are excluded because ADR-0001 §12.5 requires none and
//! their representation would add an unnecessary encoding choice.

use std::collections::BTreeMap;

/// A value supported by the project's canonical identity format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalValue {
    /// The absent value, for an optional input that is genuinely unset.
    Null,
    /// An unsigned integer, which may exceed [`i64::MAX`].
    Unsigned(u64),
    /// A UTF-8 string.
    Text(String),
    /// An ordered sequence, whose order is part of the identity.
    Array(Vec<CanonicalValue>),
    /// A mapping whose keys are unique and sorted by the map itself.
    Object(BTreeMap<String, CanonicalValue>),
}

impl CanonicalValue {
    /// Builds an object from a fixed set of uniquely named entries.
    ///
    /// Takes an array rather than an iterator so an identity is written as a
    /// literal list of named fields at its call site, which is what makes a
    /// missing field visible in review.
    ///
    /// # Panics
    ///
    /// Panics when two entries use the same key, rather than let
    /// `BTreeMap::insert` drop the earlier value and return an identity naming
    /// fewer inputs than the caller wrote — a key wrong in the one direction
    /// that serves audio it does not describe.
    ///
    /// No runtime input reaches it: the keys are a literal array in the call,
    /// so a duplicate is a source edit, and
    /// `t1_e1_a_repeated_identity_field_is_refused_rather_than_collapsed`
    /// fails on one. A `Result` would put a `?` on every identity in the
    /// workspace for a condition only a source edit can produce. Build an
    /// identity whose keys come from data with [`CanonicalValue::Object`],
    /// where the map makes duplicates unrepresentable.
    pub fn object<const N: usize>(entries: [(&str, CanonicalValue); N]) -> Self {
        let mut fields = BTreeMap::new();
        for (key, value) in entries {
            assert!(
                fields.insert(key.to_owned(), value).is_none(),
                "identity field `{key}` is written twice; one of the two values would be \
                 discarded and the identity would then name fewer inputs than it carries"
            );
        }
        Self::Object(fields)
    }

    /// Builds an array whose element order is part of the identity.
    pub fn array(elements: impl IntoIterator<Item = CanonicalValue>) -> Self {
        Self::Array(elements.into_iter().collect())
    }

    /// Builds the value for an input that may legitimately be unset.
    ///
    /// An absent input serializes differently from an empty string, because a
    /// voice with no conditioning artifact and a voice with an empty one are
    /// different synthesis inputs and must not share a cache key.
    pub fn optional(value: Option<impl Into<CanonicalValue>>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

impl From<&str> for CanonicalValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for CanonicalValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<u16> for CanonicalValue {
    fn from(value: u16) -> Self {
        Self::Unsigned(u64::from(value))
    }
}

impl From<u32> for CanonicalValue {
    fn from(value: u32) -> Self {
        Self::Unsigned(u64::from(value))
    }
}

impl From<u64> for CanonicalValue {
    fn from(value: u64) -> Self {
        Self::Unsigned(value)
    }
}

/// Serializes a value into the canonical bytes this project hashes.
pub fn canonical_bytes(value: &CanonicalValue) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(value, &mut bytes);
    bytes
}

/// Derives the BLAKE3 digest ADR-0001 §12.5 requires for structured identities.
pub fn canonical_digest(value: &CanonicalValue) -> blake3::Hash {
    blake3::hash(&canonical_bytes(value))
}

fn write_value(value: &CanonicalValue, bytes: &mut Vec<u8>) {
    match value {
        CanonicalValue::Null => bytes.extend_from_slice(b"null"),
        CanonicalValue::Unsigned(number) => {
            bytes.extend_from_slice(number.to_string().as_bytes());
        }
        CanonicalValue::Text(text) => write_json_string(text, bytes),
        CanonicalValue::Array(elements) => {
            bytes.push(b'[');
            for (position, element) in elements.iter().enumerate() {
                if position > 0 {
                    bytes.push(b',');
                }
                write_value(element, bytes);
            }
            bytes.push(b']');
        }
        CanonicalValue::Object(entries) => {
            bytes.push(b'{');
            for (position, (key, entry)) in entries.iter().enumerate() {
                if position > 0 {
                    bytes.push(b',');
                }
                write_json_string(key, bytes);
                bytes.push(b':');
                write_value(entry, bytes);
            }
            bytes.push(b'}');
        }
    }
}

fn write_json_string(text: &str, bytes: &mut Vec<u8>) {
    bytes.push(b'"');
    for character in text.chars() {
        match character {
            '"' => bytes.extend_from_slice(br#"\""#),
            '\\' => bytes.extend_from_slice(br"\\"),
            '\u{8}' => bytes.extend_from_slice(br"\b"),
            '\u{c}' => bytes.extend_from_slice(br"\f"),
            '\n' => bytes.extend_from_slice(br"\n"),
            '\r' => bytes.extend_from_slice(br"\r"),
            '\t' => bytes.extend_from_slice(br"\t"),
            control if control < '\u{20}' => {
                bytes.extend_from_slice(format!("\\u{:04x}", control as u32).as_bytes());
            }
            other => {
                let mut buffer = [0_u8; 4];
                bytes.extend_from_slice(other.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    bytes.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t2_e1_canonical_serialization_is_byte_stable() {
        let expected = concat!(
            r#"{"absent":null,"depth":{"inner":[1,2]},"#,
            r#""escapes":"quote \" backslash \\ backspace \b form feed \f "#,
            r#"newline \n carriage \r tab \t nul \u0000 unit \u001f","#,
            r#""seed":18446744073709551615,"unicode":"café 日","zz":"last","#,
            r#""é":"e acute","日":"ideograph"}"#,
        );

        let value = CanonicalValue::object([
            ("zz", CanonicalValue::from("last")),
            ("seed", CanonicalValue::from(u64::MAX)),
            ("unicode", CanonicalValue::from("café 日")),
            (
                "escapes",
                CanonicalValue::from(
                    "quote \" backslash \\ backspace \u{8} form feed \u{c} newline \n \
                     carriage \r tab \t nul \u{0} unit \u{1f}",
                ),
            ),
            ("é", CanonicalValue::from("e acute")),
            ("日", CanonicalValue::from("ideograph")),
            ("absent", CanonicalValue::optional(None::<&str>)),
            (
                "depth",
                CanonicalValue::object([(
                    "inner",
                    CanonicalValue::array([
                        CanonicalValue::from(1_u32),
                        CanonicalValue::from(2_u32),
                    ]),
                )]),
            ),
        ]);

        assert_eq!(canonical_bytes(&value), expected.as_bytes());
    }

    #[test]
    fn t1_e1_an_absent_input_is_distinct_from_an_empty_one() {
        let absent = CanonicalValue::object([("voice", CanonicalValue::optional(None::<&str>))]);
        let empty = CanonicalValue::object([("voice", CanonicalValue::from(""))]);

        assert_ne!(canonical_digest(&absent), canonical_digest(&empty));
    }

    #[test]
    #[should_panic(expected = "identity field `take` is written twice")]
    fn t1_e1_a_repeated_identity_field_is_refused_rather_than_collapsed() {
        let _ = CanonicalValue::object([
            ("take", CanonicalValue::from(0_u32)),
            ("take", CanonicalValue::from(1_u32)),
        ]);
    }
}
