//! Prints the identity of the checked-in worker bundle.
//!
//! ```text
//! cargo run --package study-tts-runtime --example worker-bundle-hash
//! ```
//!
//! ADR-0001 §12.5 makes this hash a synthesis-key input and §22 requires it to
//! be derived mechanically rather than read from a marker somebody maintains.
//! This is that derivation, exposed so an operator recording qualification
//! evidence can name the bundle a measurement was taken with — without
//! transcribing a value by hand, which is the failure mode §22 is about.
//!
//! It refuses rather than prints in two cases, and both are the point. A
//! manifest that disagrees with the tree — an undeclared imported module, a
//! missing input — describes a bundle that is not this one. And an interpreter
//! that disagrees with the manifest's declared runtime ABI means the identity
//! would name a bundle this machine cannot run, so it is refused rather than
//! recorded into evidence. Restore `worker/.venv` per
//! `docs/operations/WORKER-ENVIRONMENT.md` before running this.

use std::path::PathBuf;

use study_tts_runtime::WorkerBundle;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let bundle = WorkerBundle::load(&root)?;

    println!("{}", bundle.verified_hash()?);
    Ok(())
}
