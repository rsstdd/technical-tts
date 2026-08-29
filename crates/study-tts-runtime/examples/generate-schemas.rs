//! Regenerates the checked-in JSON Schemas at `schemas/`.
//!
//! Run from anywhere in the workspace:
//!
//! ```text
//! cargo run --package study-tts-runtime --example generate-schemas
//! ```
//!
//! An example rather than a build script: a build script would rewrite tracked
//! files as a side effect of `cargo build`, so a schema change would arrive in
//! somebody's working tree without them asking for it. Regeneration is a
//! decision, and `t3_e1_generated_schemas_match_checked_in_files` is what makes
//! forgetting to take it a failing test rather than a silent divergence.

use std::{fs, path::PathBuf};

use study_tts_runtime::{PUBLISHED_SCHEMAS, SCHEMA_DIRECTORY};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(SCHEMA_DIRECTORY);
    fs::create_dir_all(&directory)?;

    for schema in PUBLISHED_SCHEMAS {
        let path = directory.join(schema.file_name());
        fs::write(&path, schema.to_bytes())?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
