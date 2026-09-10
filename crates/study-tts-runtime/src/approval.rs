//! Recording a person's decision about an immutable preview package.
//!
//! The package is published first and approved second — E2-S6 task 2 fixes
//! that order, and it is the only order that works: a reviewer cannot listen to
//! a package that does not exist yet. So an approval is written *beside* the
//! package it names, never inside it. A package directory is published by
//! rename and named by the BLAKE3 of its own manifest, so a file added to it
//! afterwards would either falsify that name or mutate a published artifact.
//!
//! `previews/<lesson-id>/approvals/<manifest-blake3>.json` is the resulting
//! layout, beside `current.json`. Keying by manifest digest rather than by
//! lesson means approving one generation never overwrites the record of
//! another: a rejected generation keeps its rejection.

use std::path::{Path, PathBuf};

use study_tts_core::{
    APPROVAL_SCHEMA_VERSION, ApprovalDisposition, ApprovalRecord, ManifestDigest,
    PREVIEW_RELEASE_SCHEMA_VERSION, PREVIEW_REVIEW_CHECKLIST_VERSION, PreviewReleaseRecord,
};

use crate::{
    BuildError, DurableStateError,
    durable::{OsDurableFileSystem, read_bounded_bytes, write_json_atomically},
    managed,
    preview::{self, APPROVALS_DIRECTORY, PreviewRoots},
};

/// Ceiling on a stored approval, enforced before it is decoded.
///
/// An approval is one decision and seven short strings, so this is generous
/// by three orders of magnitude. `rust-production` requires the ceiling before
/// the parse, not after: a document read into memory to discover it was too
/// large has already cost what the ceiling exists to refuse.
const MAX_APPROVAL_JSON_BYTES: usize = 64 * 1024;

/// Name of the record declaring a lesson's current preview released.
///
/// One per lesson, beside `current.json`, replaced when a newer generation is
/// approved and released. The approval documents it names are keyed by
/// manifest digest and accumulate; this is the pointer to the current one.
const PREVIEW_RELEASE_NAME: &str = "release.json";

/// What a reviewer is recording, before it becomes a stored document.
///
/// Borrowed rather than owned because the caller already holds every field and
/// the record is serialized immediately; nothing here outlives the write.
#[derive(Clone, Copy, Debug)]
pub struct ApprovalRequest<'a> {
    /// Lesson whose package was reviewed.
    pub lesson_id: &'a str,
    /// BLAKE3 of the reviewed `manifest.json`, which also names its directory.
    pub manifest_blake3: &'a ManifestDigest,
    /// Who listened.
    pub reviewer: &'a str,
    /// The role they signed in.
    pub reviewer_role: &'a str,
    /// Hardware and room they listened on.
    pub playback_environment: &'a str,
    /// What they decided.
    pub disposition: ApprovalDisposition,
}

/// Records a review decision beside the package it judges.
///
/// The package is not touched. The approval names its manifest digest, the
/// checklist version whose criteria the reviewer answered, and nothing the
/// package could contradict.
///
/// # Errors
///
/// [`crate::DurableStateError::ApprovedPackageMissing`] when no package
/// directory carries the named digest — an approval for a package that is not
/// there records a judgment nobody can reproduce, which is the failure
/// `check_listening_review.py` already refuses in the qualification flow.
/// [`crate::IoError::FileSystem`] while creating the directory or writing.
pub fn approve_preview(
    workspace: &Path,
    request: &ApprovalRequest<'_>,
) -> Result<ApprovalRecord, BuildError> {
    let roots = preview::roots(workspace, request.lesson_id)?;
    let package =
        managed::directory_candidate(&roots.packages_dir, request.manifest_blake3.as_str())?;
    if !package.is_dir() {
        return Err(DurableStateError::ApprovedPackageMissing {
            lesson_id: request.lesson_id.to_owned(),
            manifest_blake3: request.manifest_blake3.as_str().to_owned(),
        }
        .into());
    }

    let record = ApprovalRecord {
        schema_version: APPROVAL_SCHEMA_VERSION,
        lesson_id: request.lesson_id.to_owned(),
        manifest_blake3: request.manifest_blake3.clone(),
        checklist_version: PREVIEW_REVIEW_CHECKLIST_VERSION,
        reviewer: request.reviewer.to_owned(),
        reviewer_role: request.reviewer_role.to_owned(),
        playback_environment: request.playback_environment.to_owned(),
        disposition: request.disposition,
    };

    let destination = approval_path(&roots.preview_dir, request.manifest_blake3)?;
    write_json_atomically(&OsDurableFileSystem, &destination, &record)?;
    Ok(record)
}

/// The approval covering the package a preview consumer would receive today.
///
/// **This is where a content change invalidates an approval, and it needs no
/// comparison to do it.** An approval is stored under the digest of the
/// manifest it judged, and a package directory is named by that same digest,
/// so a rebuilt package is a different name with no approval beside it. The
/// equality check below is belt to that braces: it catches a record whose file
/// was renamed, which is the only way the two could disagree.
///
/// Returns `None` when nothing is published, or when what is published has not
/// been approved. A rejection is *not* an approval and returns `None` too —
/// the record survives on disk as the evidence that a generation was judged.
///
/// # Errors
///
/// [`crate::DurableStateError::ApprovalManifestMismatch`] when a stored
/// approval names a manifest other than the one its own filename claims.
/// [`crate::DurableStateError::MalformedApproval`] when the document does not
/// parse as this layout, [`crate::DurableStateError::UnsupportedApproval`]
/// when it declares a layout this build cannot read, and
/// [`crate::IoError::FileSystem`] while reading.
pub fn approved_package(
    workspace: &Path,
    lesson_id: &str,
) -> Result<Option<ApprovalRecord>, BuildError> {
    let roots = preview::roots(workspace, lesson_id)?;
    Ok(read_approved(&roots, lesson_id)?.map(|(record, _)| record))
}

/// The accepting approval for the selected package, with the bytes it was read
/// from.
///
/// The bytes travel with the record because
/// [`publish_preview_release`] checksums them: reading the file a second time
/// to hash it would let the release name bytes that were never validated
/// against the record this returned.
fn read_approved(
    roots: &PreviewRoots,
    lesson_id: &str,
) -> Result<Option<(ApprovalRecord, Vec<u8>)>, BuildError> {
    let Some(published) = preview::read_current(roots, lesson_id)? else {
        return Ok(None);
    };

    let path = approval_path(&roots.preview_dir, &published.manifest_blake3)?;
    if !path.is_file() {
        return Ok(None);
    }

    let bytes = read_bounded_bytes(&path, MAX_APPROVAL_JSON_BYTES)?;
    let record: ApprovalRecord =
        serde_json::from_slice(&bytes).map_err(|source| DurableStateError::MalformedApproval {
            path: path.clone(),
            source,
        })?;

    // The version is gated on read, as every other stored document here gates
    // its own: a future build's major would otherwise parse into this shape and
    // be treated as an approval whose fields this build cannot have understood.
    // `SchemaVersion::accepted_by` is the whole rule — a higher major refused,
    // an older minor accepted — and no second comparison is hand-rolled beside
    // it.
    record
        .schema_version
        .accepted_by(APPROVAL_SCHEMA_VERSION)
        .map_err(|source| DurableStateError::UnsupportedApproval {
            path: path.clone(),
            source,
        })?;

    if record.manifest_blake3 != published.manifest_blake3 {
        return Err(DurableStateError::ApprovalManifestMismatch {
            path,
            recorded: record.manifest_blake3.as_str().to_owned(),
            selected: published.manifest_blake3.as_str().to_owned(),
        }
        .into());
    }

    Ok(match record.disposition {
        ApprovalDisposition::Accepted => Some((record, bytes)),
        ApprovalDisposition::Rejected => None,
    })
}

/// Declares the reviewed generation released as a private preview.
///
/// **This is E2-S6 task 7's gate, and it is the only place it can live.**
/// Approval cannot gate *writing* a package — a reviewer needs a package to
/// listen to, so that reading is a cycle in which nothing is ever built. What
/// it gates is the claim that a preview is finished, which is exactly this
/// document.
///
/// The record names the manifest and the approval by digest. Neither names it
/// back, and neither has a field that could.
///
/// # Errors
///
/// [`crate::DurableStateError::PreviewNotApproved`] when the selected package
/// has no accepting approval — the refusal E2-S6 task 7 exists to produce, and
/// the project owner's to resolve by reviewing the generation.
/// [`crate::DurableStateError::ApprovalManifestMismatch`] and
/// [`crate::DurableStateError::MalformedApproval`] propagate from
/// [`approved_package`], and [`crate::IoError::FileSystem`] from the write.
pub fn publish_preview_release(
    workspace: &Path,
    lesson_id: &str,
) -> Result<PreviewReleaseRecord, BuildError> {
    let roots = preview::roots(workspace, lesson_id)?;
    let Some((approval, bytes)) = read_approved(&roots, lesson_id)? else {
        return Err(DurableStateError::PreviewNotApproved {
            lesson_id: lesson_id.to_owned(),
        }
        .into());
    };

    let record = PreviewReleaseRecord {
        schema_version: PREVIEW_RELEASE_SCHEMA_VERSION,
        lesson_id: lesson_id.to_owned(),
        manifest_blake3: approval.manifest_blake3.clone(),
        // Hashed from the bytes that were read and validated, not from the
        // value in hand: this is an artifact checksum, and `rust-production`
        // separates those from identities. The digest therefore names the
        // document a reader will open, not a re-serialization that may differ.
        approval_blake3: blake3::hash(&bytes).into(),
    };

    let destination = managed::leaf(&roots.preview_dir, PREVIEW_RELEASE_NAME)?;
    write_json_atomically(&OsDurableFileSystem, &destination, &record)?;
    Ok(record)
}

/// Where one package's approval is stored.
fn approval_path(preview_dir: &Path, manifest: &ManifestDigest) -> Result<PathBuf, BuildError> {
    let approvals = managed::subdirectory(preview_dir, APPROVALS_DIRECTORY)?;
    managed::leaf(&approvals, &format!("{}.json", manifest.as_str()))
}
