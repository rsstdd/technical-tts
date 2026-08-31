//! Refusals from the governed model root's integrity gate.
//!
//! Separate from [`crate::error::WorkerBundleError`], which identifies the
//! *worker* this project ships, because these describe a third-party
//! acquisition governed by ADR-0002: the remedies are different people doing
//! different things.
//!
//! No [`crate::error::RemedyAdvice`] is attached.
//! `docs/governance/ROUTING-TABLES.md` §Failure routing establishes no owner
//! for a model-artifact mismatch, and
//! `crate::error`'s own rule is to add governed advice only where that table
//! does. The owner is named in each message instead, from §Decision routing's
//! "Chatterbox/model revision" row.

use std::path::PathBuf;

use thiserror::Error;

/// A governed model artifact that is not the one this build is pinned to.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelArtifactError {
    /// A declared artifact is absent from the revision directory.
    #[error(
        "the governed model root `{root}` does not hold the declared artifact `{artifact}`; the \
         engineering and project owners must restore the acquisition ADR-0002 qualified, because \
         a model this build cannot identify must not render audio a cache key would name"
    )]
    MissingModelArtifact {
        /// The revision directory the artifact was expected in.
        root: PathBuf,
        /// Which declared artifact is absent.
        artifact: &'static str,
    },

    /// A declared artifact's name holds something other than a regular file.
    ///
    /// Refused rather than followed, for the reason `voice_gate` refuses a link
    /// at a voice record: hashing through a link would take both the bytes and
    /// the digest from one file outside the governed root, and the gate would
    /// agree with itself about an acquisition nobody approved.
    #[error(
        "the governed model root `{root}` holds `{artifact}` as something other than a regular \
         file; the engineering and project owners must restore the acquisition ADR-0002 \
         qualified rather than let this build hash whatever the name points at"
    )]
    ModelArtifactNotRegularFile {
        /// The revision directory the artifact was expected in.
        root: PathBuf,
        /// Which declared artifact is not a regular file.
        artifact: &'static str,
    },

    /// A declared artifact is not the size the pinned acquisition records.
    ///
    /// Distinct from a checksum mismatch so a truncated or interrupted download
    /// is reported as what it is, rather than as bytes that disagree.
    #[error(
        "the governed model artifact `{path}` is {found} bytes but the pinned acquisition \
         declares {declared}; the engineering and project owners must re-acquire the revision \
         ADR-0002 qualified, since a partial artifact renders audio no key describes"
    )]
    ModelArtifactSizeMismatch {
        /// The artifact whose size disagrees.
        path: PathBuf,
        /// Size the pinned acquisition declares.
        declared: u64,
        /// Size found on disk.
        found: u64,
    },

    /// A declared artifact's bytes are not the pinned ones.
    ///
    /// The digest is deliberately not quoted. ADR-0001 §12.5 keys cache entries
    /// on the model revision, and an operator who needs the found digest can
    /// take it themselves; printing it invites pasting a governed measurement
    /// into an issue, which `docs/governance/ROUTING-TABLES.md` §Artifact
    /// routing forbids.
    #[error(
        "the governed model artifact `{path}` does not match the checksum the pinned acquisition \
         records; the engineering and project owners must re-acquire the revision ADR-0002 \
         qualified, because changed weights under an unchanged revision would publish audio \
         under a cache key describing bytes that did not produce it"
    )]
    ModelArtifactChecksumMismatch {
        /// The artifact whose bytes disagree.
        path: PathBuf,
    },
}
