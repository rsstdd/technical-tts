//! Validated digests for external-tool argument profiles.
//!
//! Runtime derives these from path-normalized FFmpeg and ffprobe arguments and
//! records them in manifests. Core owns the parse and schema boundary so a
//! malformed recorded digest is refused before profile comparison.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::{blake3_newtype, json_schema_as_string};

/// BLAKE3 digest of one external tool's path-normalized argument profile.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct ToolProfileHash(String);

impl ToolProfileHash {
    /// The digest as it is written into a manifest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

blake3_newtype!(ToolProfileHash, MalformedToolProfileHash);

/// A recorded tool-profile digest that is not lowercase BLAKE3 hexadecimal.
#[derive(Debug, Error)]
#[error(
    "tool argument profile hash `{0}` is not a BLAKE3 digest in lowercase hexadecimal; it is \
     derived from the normalized argument sequence a build ran the tool with, so rebuild the \
     package rather than editing the recorded value, and preserve the package it was recorded in"
)]
pub struct MalformedToolProfileHash(String);

json_schema_as_string!(
    ToolProfileHash,
    "ToolProfileHash",
    "BLAKE3 over an external tool's path-normalized argument profile, as 64 \
     lowercase hexadecimal characters.",
    pattern = crate::digest::BLAKE3_HEX_PATTERN,
);
