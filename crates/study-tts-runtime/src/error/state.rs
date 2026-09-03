//! Durable lock, journal, and selection-record refusals.

use std::path::PathBuf;

use study_tts_core::{CacheKey, JobState};
use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};
use crate::BuildError;

/// Why durable ownership or preview reconciliation could not proceed safely.
#[derive(Debug, Error)]
pub enum DurableStateError {
    /// Another live process owns the lesson job.
    #[error(
        "job lock `{}` is owned by {}; wait for that build to finish before retrying",
        path.display(),
        describe_lock_owner(*pid, *process_start)
    )]
    LiveJobLock {
        /// The authoritative lock record.
        path: PathBuf,
        /// Process recorded as owner, absent while the holder is mid-release
        /// and has already cleared its record.
        pid: Option<u32>,
        /// Linux process start-time ticks recorded for that process.
        process_start: Option<u64>,
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

    /// A job document is not valid strict JSON.
    #[error(
        "job snapshot `{}` is malformed ({source}); preserve it and route the job to runtime \
         reconciliation",
        path.display()
    )]
    MalformedJobSnapshot {
        /// The malformed document.
        path: PathBuf,
        /// What strict JSON parsing reported.
        source: serde_json::Error,
    },

    /// A job event log line is malformed, foreign, or partial.
    #[error(
        "job event log `{}` contains a malformed, foreign, or partial line; preserve it and route \
         the job to runtime reconciliation",
        path.display()
    )]
    MalformedJobEventLog {
        /// The event log that cannot be trusted.
        path: PathBuf,
    },

    /// A durable JSON record exceeds the memory ceiling for its boundary.
    #[error(
        "durable record `{}` exceeds the {max_bytes}-byte limit; preserve it and route the job \
         to runtime reconciliation",
        path.display()
    )]
    DurableRecordTooLarge {
        /// Record refused before it was decoded.
        path: PathBuf,
        /// Maximum bytes this boundary reads.
        max_bytes: usize,
    },

    /// One event line exceeds the message ceiling for the internal log.
    #[error(
        "job event log `{}` contains a line exceeding the {max_bytes}-byte limit; preserve it \
         and route the job to runtime reconciliation",
        path.display()
    )]
    JobEventLineTooLarge {
        /// Event log containing the oversized line.
        path: PathBuf,
        /// Maximum bytes one event line may carry.
        max_bytes: usize,
    },

    /// A job document names a different job than its managed directory.
    #[error(
        "job snapshot `{}` names job `{recorded}` but its directory requires `{required}`; it \
         will not be overwritten",
        path.display()
    )]
    JobSnapshotIdentityMismatch {
        /// The authoritative job document.
        path: PathBuf,
        /// Job identity the snapshot records.
        recorded: String,
        /// Job identity required by its directory.
        required: String,
    },

    /// A job document contains more segment records than a lesson can own.
    #[error(
        "job document `{}` contains {found} segments, exceeding the limit of {max}; preserve it \
         for runtime reconciliation",
        path.display()
    )]
    JobSnapshotSegmentCountExceeded {
        /// Oversized job document.
        path: PathBuf,
        /// Segment records found.
        found: usize,
        /// Maximum segment records permitted.
        max: usize,
    },

    /// The current and abandoned build-attempt identities are not consecutive.
    #[error(
        "job document `{}` records build attempt {build_attempt} with abandoned attempt \
         {abandoned_attempt:?}; the first attempt must have none and every later attempt must \
         name its immediate predecessor; preserve it for runtime reconciliation",
        path.display()
    )]
    JobSnapshotAttemptMismatch {
        /// The incoherent document.
        path: PathBuf,
        /// Current build-attempt identity.
        build_attempt: u32,
        /// Claimed predecessor, when present.
        abandoned_attempt: Option<u32>,
    },

    /// A replacement does not continue the attempt currently on disk.
    #[error(
        "job document `{}` would replace attempt {current_attempt} in state `{current_state:?}` \
         with attempt {replacement_attempt} whose predecessor is {abandoned_attempt:?} in state \
         {abandoned_state:?}; preserve the current document for runtime reconciliation",
        path.display()
    )]
    JobReplacementPredecessorMismatch {
        /// The authoritative job document.
        path: PathBuf,
        /// Build attempt currently on disk.
        current_attempt: u32,
        /// State currently on disk.
        current_state: JobState,
        /// Build attempt proposed as its replacement.
        replacement_attempt: u32,
        /// Attempt the replacement claims to supersede.
        abandoned_attempt: Option<u32>,
        /// State the replacement claims its predecessor reached.
        abandoned_state: Option<JobState>,
    },

    /// A job document's current and last-successful states cannot coexist.
    #[error(
        "job document `{}` records current state `{state}` with last successful state \
         `{last_successful_state}`; preserve it for runtime reconciliation",
        path.display()
    )]
    JobSnapshotLastSuccessfulStateMismatch {
        /// The incoherent document.
        path: PathBuf,
        /// Current state the document records.
        state: String,
        /// Last successful state the document records.
        last_successful_state: String,
    },

    /// A job document records a preview package before rendering completed.
    #[error(
        "job document `{}` records a preview package while in state `{state}`, before rendering \
         completed; preserve it for runtime reconciliation",
        path.display()
    )]
    JobSnapshotSelectionMismatch {
        /// The incoherent document.
        path: PathBuf,
        /// The state the document records.
        state: String,
    },

    /// A selected package's directory and manifest identities disagree.
    #[error(
        "job document `{}` records package identity `{package_id}` but manifest identity \
         `{manifest_blake3}`; preserve it for runtime reconciliation",
        path.display()
    )]
    JobSnapshotPackageIdentityMismatch {
        /// The incoherent document.
        path: PathBuf,
        /// Digest naming the immutable package directory.
        package_id: String,
        /// Digest recorded for the manifest inside that directory.
        manifest_blake3: String,
    },

    /// The job document and selected preview name different packages.
    #[error(
        "job `{job_id}` records preview package `{recorded}` but the selected output is \
         `{selected}`; preserve both for runtime reconciliation",
        selected = selected.as_deref().unwrap_or("<none>")
    )]
    JobPreviewSelectionMismatch {
        /// Job whose authoritative records disagree.
        job_id: String,
        /// Package identity recorded by `job.json`.
        recorded: String,
        /// Package identity selected by `current.json`, if one exists.
        selected: Option<String>,
    },

    /// A build asked its job document for a move ADR-0001 §6.4 does not have.
    #[error(
        "job `{job_id}` cannot move from `{from:?}` to `{to:?}`: ADR-0001 §6.4 has no such \
         transition; preserve the job document for runtime reconciliation"
    )]
    IllegalJobTransition {
        /// The job whose document was asked to move.
        job_id: String,
        /// The state the document records.
        from: JobState,
        /// The state that was requested.
        to: JobState,
    },

    /// A job cannot open another uniquely identified build attempt.
    #[error(
        "job `{job_id}` exhausted its build-attempt identity space; preserve its state for runtime \
         reconciliation rather than reusing an attempt number"
    )]
    JobAttemptOverflow {
        /// Job whose next attempt cannot be represented.
        job_id: String,
    },

    /// The retained lesson is not the one the job document was built from.
    #[error(
        "retained lesson `{}` has BLAKE3 `{actual}` but the job document records `{recorded}`; \
         preserve both for runtime reconciliation rather than re-planning from the changed copy",
        path.display()
    )]
    RetainedLessonMismatch {
        /// The retained lesson document.
        path: PathBuf,
        /// The digest the job document records.
        recorded: String,
        /// The digest of the bytes found there now.
        actual: String,
    },

    /// The retained lesson names a different job than the lock protects.
    #[error(
        "retained lesson `{}` names job `{actual}` but the claimed job is `{required}`; preserve \
         the job directory for runtime reconciliation",
        path.display()
    )]
    RetainedLessonIdentityMismatch {
        /// The retained lesson document.
        path: PathBuf,
        /// Job identity protected by the held lock.
        required: String,
        /// Lesson identity parsed from the retained document.
        actual: String,
    },

    /// The retained render plan is not valid strict JSON.
    #[error(
        "retained plan `{}` is malformed ({source}); preserve it and route the job to runtime \
         reconciliation",
        path.display()
    )]
    MalformedRetainedPlan {
        /// The malformed retained plan.
        path: PathBuf,
        /// What strict JSON parsing reported.
        source: serde_json::Error,
    },

    /// The retained plan names a different job than the lock protects.
    #[error(
        "retained plan `{}` names job `{actual}` but the claimed job is `{required}`; preserve the \
         job directory for runtime reconciliation",
        path.display()
    )]
    RetainedPlanIdentityMismatch {
        /// The retained plan document.
        path: PathBuf,
        /// Job identity protected by the held lock.
        required: String,
        /// Lesson identity parsed from the retained plan.
        actual: String,
    },

    /// The retained plan's recorded and derived identities disagree.
    #[error(
        "retained plan `{}` records hash `{recorded}` but its segments derive `{actual}`; preserve \
         it for runtime reconciliation",
        path.display()
    )]
    RetainedPlanHashMismatch {
        /// The retained plan document.
        path: PathBuf,
        /// Plan identity recorded inside `plan.json`.
        recorded: String,
        /// Identity derived from the plan's segments.
        actual: String,
    },

    /// A retained plan contains more segments than a lesson can own.
    #[error(
        "retained plan `{}` contains {found} segments, exceeding the limit of {max}; preserve it \
         for runtime reconciliation",
        path.display()
    )]
    RetainedPlanSegmentCountExceeded {
        /// Oversized retained plan.
        path: PathBuf,
        /// Planned segments found.
        found: usize,
        /// Maximum planned segments permitted.
        max: usize,
    },

    /// The job document and retained plan name different plan identities.
    #[error(
        "retained plan `{}` records hash `{plan_recorded}` but the job document records \
         `{job_recorded}`; preserve both for runtime reconciliation",
        path.display()
    )]
    JobPlanHashMismatch {
        /// The retained plan document.
        path: PathBuf,
        /// Plan identity recorded by `job.json`.
        job_recorded: String,
        /// Plan identity recorded inside `plan.json`.
        plan_recorded: String,
    },

    /// A resume named a job that has no document to resume from.
    #[error(
        "job directory `{}` holds no job document to resume; build the lesson first",
        path.display()
    )]
    NoJobToResume {
        /// The job directory that was asked to resume.
        path: PathBuf,
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

    /// A package manifest's written timeline does not describe one master.
    #[error(
        "package manifest `{}` records a timeline that cannot describe one master: {detail}; \
         preserve the package for runtime reconciliation",
        path.display()
    )]
    IncoherentPackageTimeline {
        /// Manifest carrying the contradictory timeline.
        path: PathBuf,
        /// Which agreement failed, and the two values that disagree.
        detail: String,
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

fn describe_lock_owner(pid: Option<u32>, process_start: Option<u64>) -> String {
    match (pid, process_start) {
        (Some(pid), Some(process_start)) => {
            format!("live process {pid} (Linux start identity {process_start})")
        }
        _ => "a live process that is releasing it".to_owned(),
    }
}

impl DurableStateError {
    /// Returns governed advice for the exact ownership or integrity refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::LiveJobLock { .. } | Self::NoJobToResume { .. } => None,
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
            | Self::MalformedJobEventLog { .. }
            | Self::DurableRecordTooLarge { .. }
            | Self::JobEventLineTooLarge { .. }
            | Self::JobSnapshotIdentityMismatch { .. }
            | Self::JobSnapshotSegmentCountExceeded { .. }
            | Self::JobSnapshotAttemptMismatch { .. }
            | Self::JobReplacementPredecessorMismatch { .. }
            | Self::JobSnapshotLastSuccessfulStateMismatch { .. }
            | Self::JobSnapshotSelectionMismatch { .. }
            | Self::JobSnapshotPackageIdentityMismatch { .. }
            | Self::JobPreviewSelectionMismatch { .. }
            | Self::IllegalJobTransition { .. }
            | Self::JobAttemptOverflow { .. }
            | Self::RetainedLessonMismatch { .. }
            | Self::RetainedLessonIdentityMismatch { .. }
            | Self::MalformedRetainedPlan { .. }
            | Self::RetainedPlanIdentityMismatch { .. }
            | Self::RetainedPlanHashMismatch { .. }
            | Self::RetainedPlanSegmentCountExceeded { .. }
            | Self::JobPlanHashMismatch { .. }
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
            | Self::IncoherentPackageTimeline { .. }
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
