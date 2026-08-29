//! Shared BLAKE3 digest spelling and value-object implementations.
//!
//! This module validates textual form only. ADR-0001 assigns each digest's
//! meaning to its owning domain module.

/// Length of a BLAKE3 digest rendered as lowercase hexadecimal.
pub(crate) const BLAKE3_HEX_LENGTH: usize = 64;

/// JSON Schema pattern accepting exactly the values [`is_blake3_hex`] accepts.
///
/// The length is spelled out rather than taken from `BLAKE3_HEX_LENGTH`: a
/// `pattern` is a string literal and there is nowhere in one to interpolate a
/// constant. The assertion below is what keeps the two spellings from parting
/// company, which is the whole reason the constant is named at all.
pub const BLAKE3_HEX_PATTERN: &str = r"^[0-9a-f]{64}$(?![\s\S])";

const _: () = assert!(
    BLAKE3_HEX_LENGTH == 64,
    "BLAKE3_HEX_PATTERN spells the digest length out and must be rewritten with it"
);

/// Whether `value` is exactly the form [`blake3::Hash::to_hex`] produces.
///
/// "Well formed" has to mean that exact form, because a recorded digest is
/// compared against that output byte for byte and a cache key is used as a
/// directory name. Uppercase hex is rejected rather than normalized: a value
/// that needs normalizing before it can be compared did not come from this
/// program, and silently accepting it hides that.
pub fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

// Centralizing these conversions keeps every digest parser aligned with
// `is_blake3_hex`; errors stay at call sites because their remedies differ.
macro_rules! blake3_newtype {
    ($name:ident, $error:ident) => {
        impl From<::blake3::Hash> for $name {
            fn from(hash: ::blake3::Hash) -> Self {
                Self(hash.to_hex().to_string())
            }
        }

        impl From<$name> for ::std::string::String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl TryFrom<::std::string::String> for $name {
            type Error = $error;

            fn try_from(value: ::std::string::String) -> Result<Self, Self::Error> {
                if $crate::digest::is_blake3_hex(&value) {
                    return Ok(Self(value));
                }
                Err($error(value))
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from(value.to_owned())
            }
        }
    };
}

pub(crate) use blake3_newtype;

// `schemars` cannot infer the serialized shape through serde's `try_from` and
// `into` conversions. Callers own any pattern because only their domain parser
// can say whether it is exact or deliberately looser.
macro_rules! json_schema_as_string {
    ($type:ty, $name:literal, $description:literal) => {
        impl schemars::JsonSchema for $type {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                $name.into()
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "description": $description,
                })
            }
        }
    };
    ($type:ty, $name:literal, $description:literal, pattern = $pattern:expr $(,)?) => {
        impl schemars::JsonSchema for $type {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                $name.into()
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "description": $description,
                    "pattern": $pattern,
                })
            }
        }
    };
}

pub(crate) use json_schema_as_string;
