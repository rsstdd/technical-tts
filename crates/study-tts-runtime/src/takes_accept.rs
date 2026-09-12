//! Turning a reviewed package's takes into a document a later build
//! must honour.
//!
//! Until this existed nothing wrote a [`TakesDocument`]. `ValidatedTakes`
//! could read one and `pipeline` would apply one found beside the lesson, but
//! the only way to produce one was by hand — so `take_selection_source` was
//! `implicit` on every package this project has ever built, and both M2
//! records name that as what puts a production claim out of reach.
//!
//! # Why the manifest is the source
//!
//! A selection needs four things together: the segment, the take-zero identity
//! it was made against, the entry actually assembled, and the digest of the
//! audio approved. The manifest is the only document that holds all four,
//! because it records what a build *published* rather than what it planned.
//!
//! # Why it is written durably
//!
//! A takes document decides what a later build synthesizes. A half-written one
//! that still parses would approve some segments and silently drop others, so
//! it is published the way every other durable record here is — staged,
//! flushed, renamed, and the parent flushed — which `durable` owns and
//! ADR-0001 §12.3 requires.

use std::path::Path;

use study_tts_core::{TAKES_SCHEMA_VERSION, TakesDocument, ValidatedTakes};

use crate::{
    BuildError,
    durable::{OsDurableFileSystem, write_json_atomically},
    manifest, preview,
};

/// Writes the current package's take selection to `destination`.
///
/// The document is validated before it is written, not after: a selection this
/// build would refuse to read is one it must refuse to produce, and writing it
/// first would leave the operator a file they must delete.
///
/// `Ok(None)` when the lesson has no current package, which is the honest
/// answer to "accept what I reviewed" when nothing has been published.
///
/// # Errors
///
/// [`crate::DurableStateError`] when the current record or its package is
/// unreadable, [`BuildError::Takes`] when the selections do not form a
/// document this build would accept, and [`crate::IoError`] for the write.
pub fn accept_current_takes(
    workspace: &Path,
    lesson_id: &str,
    destination: &Path,
) -> Result<Option<TakesDocument>, BuildError> {
    let Some(package) = preview::current_package_manifest(workspace, lesson_id)? else {
        return Ok(None);
    };
    let selections = manifest::recorded_selections(&package)?;

    let document = TakesDocument {
        schema: None,
        schema_version: TAKES_SCHEMA_VERSION.to_string(),
        lesson_id: lesson_id.to_owned(),
        selections,
    };

    // Validated here rather than trusted. `recorded_selections` reads a
    // manifest this build already published, so the values are sound — but the
    // *document* is a new artifact, and the invariant that matters is one
    // selection per segment naming identities a lesson can carry, which only
    // `ValidatedTakes` checks.
    ValidatedTakes::from_json(&serde_json::to_vec(&document).map_err(|source| {
        crate::IoError::WriteJson {
            path: destination.to_path_buf(),
            source,
        }
    })?)
    .map_err(|error| BuildError::Takes(Box::new(error)))?;

    write_json_atomically(&OsDurableFileSystem, destination, &document)?;
    Ok(Some(document))
}
