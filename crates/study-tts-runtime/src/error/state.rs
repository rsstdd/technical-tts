//! Durable lock, journal, and selection-record refusals.

use std::path::PathBuf;

use study_tts_core::CacheKey;
use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};
use crate::BuildError;

/// Why durable ownership or preview reconciliation could not proceed safely.
#[derive(Debug, Error)]
pub enum DurableStateError {
    /// Another process still owns the provisional lesson job.
    #[error(
        "job lock `{}` is owned by live process {pid} (Linux start identity {process_start}); \
         wait for that build to finish before retrying",
        path.display()
    )]
    LiveJobLock {
        /// The authoritative lock record.
        path: PathBuf,
        /// Process recorded as owner.
        pid: u32,
        /// Linux process start-time ticks recorded for that process.
        process_start: u64,
    },

    /// A lock record cannot be interpreted without guessing ownership.
    #[error(
        "job lock record `{}` is malformed ({source}); preserve it and route the job to runtime \
         reconciliation",
        path.display()
    )]
    MalformedJobLock {
        /// The malformed lock record.
        path: PathBuf,
        /// What strict JSON parsing reported.
        source: serde_json::Error,
    },

    /// A lock record declares an incompatible version or lesson identity.
    #[error(
        "job lock record `{}` declares schema `{schema_version}` and lesson `{lesson_id}`, not \
         schema `{required_schema_version}` and lesson `{required_lesson_id}`; preserve it and \
         route the job to runtime reconciliation",
        path.display()
    )]
    IncompatibleJobLock {
        /// The incompatible lock record.
        path: PathBuf,
        /// Schema the record declares.
        schema_version: String,
        /// Lesson identity the record declares.
        lesson_id: String,
        /// Schema this build requires.
        required_schema_version: &'static str,
        /// Lesson identity this build requires.
        required_lesson_id: String,
    },

    /// A provisional job snapshot is not valid strict JSON.
    #[error(
        "job snapshot `{}` is malformed ({source}); preserve it and route the job to runtime \
         reconciliation",
        path.display()
    )]
    MalformedJobSnapshot {
        /// The malformed snapshot.
        path: PathBuf,
        /// What strict JSON parsing reported.
        source: serde_json::Error,
    },

    /// A job snapshot names a different job than its managed directory.
    #[error(
        "job snapshot `{}` names job `{recorded}` but its directory requires `{required}`; it \
         will not be overwritten",
        path.display()
    )]
    JobSnapshotIdentityMismatch {
        /// The authoritative job snapshot.
        path: PathBuf,
        /// Job identity the snapshot records.
        recorded: String,
        /// Job identity required by its directory.
        required: String,
    },

    /// A job stage and selected-package field disagree.
    #[error(
        "job snapshot `{}` has an incompatible selected-package value for stage `{stage}`; \
         preserve it for runtime reconciliation",
        path.display()
    )]
    JobSnapshotSelectionMismatch {
        /// The incompatible snapshot.
        path: PathBuf,
        /// Diagnostic stage value that disagrees with selection state.
        stage: String,
    },

    /// A cache-key owner did not release its lock within the bounded wait.
    #[error(
        "cache key `{cache_key}` remained locked at `{}` for {timeout_ms} ms; preserve all \
         attempts and let runtime reconciliation inspect the owner",
        path.display()
    )]
    CacheLockTimeout {
        /// The bounded lock file.
        path: PathBuf,
        /// Cache key whose publication is serialized.
        cache_key: CacheKey,
        /// Maximum time waited.
        timeout_ms: u64,
    },

    /// The provisional publication journal is not valid strict JSON.
    #[error(
        "publication journal `{}` is malformed ({source}); it will not be overwritten and must \
         be reconciled by the runtime owner",
        path.display()
    )]
    MalformedPublicationJournal {
        /// The malformed journal.
        path: PathBuf,
        /// What strict parsing reported.
        source: serde_json::Error,
    },

    /// The current-preview record is not valid strict JSON.
    #[error(
        "current preview record `{}` is malformed ({source}); it will not be overwritten and \
         must be reconciled by the runtime owner",
        path.display()
    )]
    MalformedCurrentPreview {
        /// The malformed publication record.
        path: PathBuf,
        /// What strict parsing reported.
        source: serde_json::Error,
    },

    /// A versioned durable record is not the version this skeleton owns.
    #[error(
        "durable record `{}` declares unsupported schema `{schema_version}`; it will not be \
         overwritten and must be reconciled by the runtime owner",
        path.display()
    )]
    UnsupportedDurableRecord {
        /// The incompatible record.
        path: PathBuf,
        /// Schema it declares.
        schema_version: String,
    },

    /// The current record names a different lesson than its managed directory.
    #[error(
        "current preview record `{}` names lesson `{recorded}` but its directory requires \
         `{required}`; it will not be overwritten",
        path.display()
    )]
    CurrentLessonMismatch {
        /// The corrupt current record.
        path: PathBuf,
        /// Lesson the record names.
        recorded: String,
        /// Lesson the directory requires.
        required: String,
    },

    /// The publication journal names a different lesson than its job directory.
    #[error(
        "publication journal `{}` names lesson `{recorded}` but its job directory requires \
         `{required}`; it will not be overwritten",
        path.display()
    )]
    PublicationJournalLessonMismatch {
        /// The incompatible journal.
        path: PathBuf,
        /// Lesson the journal names.
        recorded: String,
        /// Lesson the job directory requires.
        required: String,
    },

    /// A current record does not contain the one managed package reference
    /// shape.
    #[error(
        "current preview record `{}` contains invalid package reference `{reference}`; it will not \
         be overwritten",
        record.display()
    )]
    InvalidCurrentPackageReference {
        /// The authoritative current record.
        record: PathBuf,
        /// Unsafe or incompatible reference it contains.
        reference: String,
    },

    /// An immutable package directory selected by durable state is absent.
    #[error(
        "immutable package directory `{}` is missing; preserve the selecting records for runtime \
         reconciliation",
        path.display()
    )]
    MissingPackageDirectory {
        /// Package directory durable state requires.
        path: PathBuf,
    },

    /// A package manifest is not valid strict JSON for this build.
    #[error(
        "package manifest `{}` is malformed ({source}); preserve the package for runtime \
         reconciliation",
        path.display()
    )]
    MalformedPackageManifest {
        /// Manifest that strict parsing refused.
        path: PathBuf,
        /// What strict parsing reported.
        source: serde_json::Error,
    },

    /// A package manifest declares an unsupported schema version.
    #[error(
        "package manifest `{}` declares schema `{found}`, not required schema `{required}`; \
         preserve the package for runtime reconciliation",
        path.display()
    )]
    UnsupportedPackageManifest {
        /// Manifest carrying the incompatible schema.
        path: PathBuf,
        /// Schema the manifest declares.
        found: String,
        /// Schema this build requires.
        required: &'static str,
    },

    /// A package manifest is not marked as a private preview.
    #[error(
        "package manifest `{}` declares release status `{found}`, not `private_preview`; preserve \
         the package for runtime reconciliation",
        path.display()
    )]
    PackageReleaseStatusMismatch {
        /// Manifest carrying the unexpected status.
        path: PathBuf,
        /// Status the manifest declares.
        found: String,
    },

    /// A package manifest names a different lesson than its package owner.
    #[error(
        "package manifest `{}` names lesson `{recorded}`, not required lesson `{required}`; \
         preserve the package for runtime reconciliation",
        path.display()
    )]
    PackageLessonMismatch {
        /// Manifest carrying the wrong lesson identity.
        path: PathBuf,
        /// Lesson the manifest records.
        recorded: String,
        /// Lesson the package owner requires.
        required: String,
    },

    /// A package manifest segment has no stable identity.
    #[error(
        "package manifest `{}` contains a segment with an empty identity; preserve the package for \
         runtime reconciliation",
        path.display()
    )]
    EmptyPackageSegmentId {
        /// Manifest carrying the invalid segment.
        path: PathBuf,
    },

    /// A package manifest segment declares no audio frames.
    #[error(
        "package manifest `{}` records zero frames for segment `{segment_id}`; {remedy}",
        path.display(),
        remedy = "preserve the package for runtime reconciliation",
    )]
    EmptyPackageSegmentAudio {
        /// Manifest carrying the empty segment.
        path: PathBuf,
        /// Segment whose frame count is zero.
        segment_id: String,
    },

    /// A package artifact record names an unexpected relative path.
    #[error(
        "package manifest `{}` records artifact path `{recorded}`, not `{required}`; preserve the \
         package for runtime reconciliation",
        manifest.display()
    )]
    UnexpectedPackageArtifactPath {
        /// Manifest carrying the invalid artifact path.
        manifest: PathBuf,
        /// Path the artifact record declares.
        recorded: String,
        /// One relative name this build permits.
        required: &'static str,
    },

    /// A package artifact no longer matches its manifest checksum.
    #[error(
        "package artifact `{}` hashes to `{found}`, not recorded checksum `{expected}`; preserve \
         the package for runtime reconciliation",
        path.display()
    )]
    PackageArtifactChecksumMismatch {
        /// Artifact whose bytes failed integrity.
        path: PathBuf,
        /// Checksum recorded by the manifest.
        expected: String,
        /// Checksum computed from the artifact now.
        found: String,
    },

    /// A package manifest records no executed arguments for one tool.
    #[error(
        "package manifest `{}` records no executed arguments for `{tool}`; preserve the package \
         for runtime reconciliation",
        path.display()
    )]
    MissingPackageToolArguments {
        /// Manifest carrying the empty argument record.
        path: PathBuf,
        /// Tool whose executed argument list is absent.
        tool: &'static str,
    },

    /// A manifest file no longer matches the selecting record's checksum.
    #[error(
        "package manifest `{}` hashes to `{found}`, not selected checksum `{expected}`; preserve \
         the package and selecting record for runtime reconciliation",
        path.display()
    )]
    PackageManifestChecksumMismatch {
        /// Manifest whose bytes failed integrity.
        path: PathBuf,
        /// Checksum the selecting record requires.
        expected: String,
        /// Checksum computed from the manifest now.
        found: String,
    },

    /// A durable record contains a malformed lowercase BLAKE3 identity.
    #[error(
        "durable record `{}` contains malformed BLAKE3 identity `{value}`; preserve it for runtime \
         reconciliation",
        path.display()
    )]
    MalformedDurableDigest {
        /// Record carrying the malformed identity.
        path: PathBuf,
        /// Value found where a digest is required.
        value: String,
    },

    /// A publication journal requires a current record that is absent.
    #[error(
        "publication journal requires current preview record `{}`, but it is missing; preserve the \
         journal and package for runtime reconciliation",
        path.display()
    )]
    MissingCurrentPreview {
        /// Current record the journal requires.
        path: PathBuf,
    },

    /// The journal and current record select different package generations.
    #[error(
        "publication journal selects manifest `{journal_manifest}` but current record `{}` selects \
         `{current_manifest}`; preserve both records for runtime reconciliation",
        record.display()
    )]
    JournalSelectionMismatch {
        /// Current record disagreeing with the journal.
        record: PathBuf,
        /// Manifest identity the journal selects.
        journal_manifest: String,
        /// Manifest identity the current record selects.
        current_manifest: String,
    },

    /// A package plan disagrees with the publication transaction that owns it.
    #[error(
        "package manifest at `{}` disagrees with its publication transaction plan; preserve both \
         for runtime reconciliation",
        path.display()
    )]
    PackagePlanMismatch {
        /// Package or manifest carrying the incompatible plan.
        path: PathBuf,
    },

    /// A managed job directory has no portable lesson-name component.
    #[error(
        "job directory `{}` has no portable lesson name; preserve it for runtime reconciliation",
        path.display()
    )]
    InvalidJobDirectoryName {
        /// Job directory whose final component cannot be used.
        path: PathBuf,
    },

    /// An immutable package name is occupied by different valid content.
    #[error(
        "immutable package publication conflicted at `{}`; preserve both attempts and route the \
         conflict to runtime reconciliation",
        path.display()
    )]
    PublicationConflict {
        /// The destination that already existed.
        path: PathBuf,
    },

    /// Quarantining a failed cache attempt failed as well.
    #[error(
        "cache attempt at `{}` failed ({primary}) and could not be quarantined ({cleanup}); \
         preserve the staging path and both failures for the runtime owner",
        staging_path.display()
    )]
    QuarantineFailed {
        /// Staging directory that could not be retained in quarantine.
        staging_path: PathBuf,
        /// Synthesis or audio failure that required quarantine.
        primary: Box<BuildError>,
        /// Filesystem or durable-state failure while preserving the attempt.
        cleanup: Box<BuildError>,
    },
}

impl DurableStateError {
    /// Returns governed advice for the exact ownership or integrity refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::LiveJobLock { .. } => None,
            Self::CacheLockTimeout { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve attempts and inspect the cache-key owner before retrying",
                None,
            )),
            Self::QuarantineFailed { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                "preserve the staging attempt and repair quarantine before retrying",
                None,
            )),
            Self::MalformedJobLock { .. }
            | Self::IncompatibleJobLock { .. }
            | Self::MalformedJobSnapshot { .. }
            | Self::JobSnapshotIdentityMismatch { .. }
            | Self::JobSnapshotSelectionMismatch { .. }
            | Self::MalformedPublicationJournal { .. }
            | Self::MalformedCurrentPreview { .. }
            | Self::UnsupportedDurableRecord { .. }
            | Self::CurrentLessonMismatch { .. }
            | Self::PublicationJournalLessonMismatch { .. }
            | Self::InvalidCurrentPackageReference { .. }
            | Self::MissingPackageDirectory { .. }
            | Self::MalformedPackageManifest { .. }
            | Self::UnsupportedPackageManifest { .. }
            | Self::PackageReleaseStatusMismatch { .. }
            | Self::PackageLessonMismatch { .. }
            | Self::EmptyPackageSegmentId { .. }
            | Self::EmptyPackageSegmentAudio { .. }
            | Self::UnexpectedPackageArtifactPath { .. }
            | Self::PackageArtifactChecksumMismatch { .. }
            | Self::MissingPackageToolArguments { .. }
            | Self::PackageManifestChecksumMismatch { .. }
            | Self::MalformedDurableDigest { .. }
            | Self::MissingCurrentPreview { .. }
            | Self::JournalSelectionMismatch { .. }
            | Self::PackagePlanMismatch { .. }
            | Self::InvalidJobDirectoryName { .. }
            | Self::PublicationConflict { .. } => Some(RemedyAdvice::new(
                RemedyOwner::Runtime,
                concat!(
                    "preserve the artifacts and run runtime reconciliation without overwrite ",
                    "or deletion",
                ),
                Some("State or checksum corruption"),
            )),
        }
    }
}
