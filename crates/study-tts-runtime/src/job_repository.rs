//! Provisional durable job ownership and snapshot repository port.
//!
//! The filesystem adapter keeps Linux ownership locking and atomic JSON
//! replacement while exposing only the deliberately minimal E0 snapshot.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records why recovery
//! and the complete state machine remain assigned to E2-S1.

use std::{
    fmt::Debug,
    fs,
    path::{Path, PathBuf},
};

use study_tts_core::{
    PROVISIONAL_JOB_SCHEMA_VERSION, ProvisionalJobSnapshot, ProvisionalJobStage, is_blake3_hex,
};

use crate::{
    BuildError, DurableStateError,
    durable::{OsDurableFileSystem, write_json_atomically},
    io_error, locking, managed, preview,
};

/// Mirrors the job-state version in the E0-S4 provisional contract baseline.
pub const JOB_STATE_CONTRACT_VERSION: &str = "e0.job-state.0.1";

const JOB_SNAPSHOT_NAME: &str = "job.json";

/// Keeps exclusive ownership of one provisional job until the guard is dropped.
pub trait JobOwnership: Debug + Send {}

impl JobOwnership for locking::JobLock {}

/// Durable replacement and ownership boundary for minimal E0 job state.
pub trait JobRepository: Send + Sync {
    /// Claims exclusive ownership until the returned guard is dropped.
    ///
    /// # Errors
    ///
    /// [`BuildError::ManagedPath`] when the job path is unsafe,
    /// [`BuildError::DurableState`] when a lock is malformed, incompatible,
    /// or live, or [`BuildError::Io`] when lock storage fails.
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError>;

    /// Loads and validates the current snapshot when one exists.
    ///
    /// # Errors
    ///
    /// [`DurableStateError::MalformedJobSnapshot`],
    /// [`DurableStateError::UnsupportedDurableRecord`],
    /// [`DurableStateError::MalformedDurableDigest`],
    /// [`DurableStateError::JobSnapshotIdentityMismatch`], or
    /// [`DurableStateError::JobSnapshotSelectionMismatch`] when durable state
    /// cannot be trusted; [`BuildError::ManagedPath`] or [`BuildError::Io`]
    /// when the managed snapshot cannot be located or read.
    fn load(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<ProvisionalJobSnapshot>, BuildError>;

    /// Atomically replaces the authoritative snapshot while ownership is held.
    ///
    /// # Errors
    ///
    /// The snapshot-validation variants documented by [`Self::load`],
    /// [`BuildError::ManagedPath`] for an unsafe destination, or
    /// [`BuildError::Io`] when serialization or atomic replacement fails.
    fn replace(
        &self,
        workspace: &Path,
        snapshot: &ProvisionalJobSnapshot,
    ) -> Result<(), BuildError>;
}

/// Linux-filesystem job repository used by the walking skeleton.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemJobRepository;

impl JobRepository for FileSystemJobRepository {
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        let filesystem = OsDurableFileSystem;
        let roots = preview::roots(workspace, job_id)?;
        let lock = locking::acquire_job_lock(&filesystem, &roots.job_dir, job_id)?;
        Ok(Box::new(lock))
    }

    fn load(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<ProvisionalJobSnapshot>, BuildError> {
        let path = job_snapshot_path(workspace, job_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).map_err(|error| io_error(&path, error))?;
        let snapshot: ProvisionalJobSnapshot =
            serde_json::from_slice(&bytes).map_err(|source| {
                DurableStateError::MalformedJobSnapshot {
                    path: path.clone(),
                    source,
                }
            })?;
        validate_snapshot(&path, job_id, &snapshot)?;
        Ok(Some(snapshot))
    }

    fn replace(
        &self,
        workspace: &Path,
        snapshot: &ProvisionalJobSnapshot,
    ) -> Result<(), BuildError> {
        let filesystem = OsDurableFileSystem;
        let path = job_snapshot_path(workspace, &snapshot.job_id)?;
        validate_snapshot(&path, &snapshot.job_id, snapshot)?;
        if path.exists() {
            self.load(workspace, &snapshot.job_id)?;
        }
        write_json_atomically(&filesystem, &path, snapshot)
    }
}

fn job_snapshot_path(workspace: &Path, job_id: &str) -> Result<PathBuf, BuildError> {
    let jobs = managed::subdirectory(workspace, "jobs")?;
    let job_dir = managed::subdirectory(&jobs, job_id)?;
    managed::leaf(&job_dir, JOB_SNAPSHOT_NAME)
}

fn validate_snapshot(
    path: &Path,
    job_id: &str,
    snapshot: &ProvisionalJobSnapshot,
) -> Result<(), BuildError> {
    if snapshot.schema_version != PROVISIONAL_JOB_SCHEMA_VERSION {
        return Err(DurableStateError::UnsupportedDurableRecord {
            path: path.to_path_buf(),
            schema_version: snapshot.schema_version.clone(),
        }
        .into());
    }
    if snapshot.job_id != job_id {
        return Err(DurableStateError::JobSnapshotIdentityMismatch {
            path: path.to_path_buf(),
            recorded: snapshot.job_id.clone(),
            required: job_id.to_owned(),
        }
        .into());
    }
    if !is_blake3_hex(&snapshot.plan_hash) {
        return Err(DurableStateError::MalformedDurableDigest {
            path: path.to_path_buf(),
            value: snapshot.plan_hash.clone(),
        }
        .into());
    }
    let selection_matches = match snapshot.stage {
        ProvisionalJobStage::PackageSelected => snapshot.selected_package.is_some(),
        ProvisionalJobStage::Planned
        | ProvisionalJobStage::Caching
        | ProvisionalJobStage::Packaging => snapshot.selected_package.is_none(),
    };
    if !selection_matches {
        return Err(DurableStateError::JobSnapshotSelectionMismatch {
            path: path.to_path_buf(),
            stage: format!("{:?}", snapshot.stage),
        }
        .into());
    }
    if let Some(package) = &snapshot.selected_package {
        for digest in [&package.package_id, &package.manifest_blake3] {
            if !is_blake3_hex(digest) {
                return Err(DurableStateError::MalformedDurableDigest {
                    path: path.to_path_buf(),
                    value: digest.clone(),
                }
                .into());
            }
        }
    }
    Ok(())
}
