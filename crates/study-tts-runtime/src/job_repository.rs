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

use study_tts_core::{PROVISIONAL_JOB_SCHEMA_VERSION, ProvisionalJobSnapshot, ProvisionalJobStage};

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
    /// [`DurableStateError::MalformedJobSnapshot`], including when `plan_hash`
    /// or a selected-package identity is not a digest and its value object
    /// refuses it during parsing,
    /// [`DurableStateError::UnsupportedDurableRecord`],
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
    // No digest is checked here. `plan_hash` and the selected package's two
    // identities are value objects, so a recorded value that is not a digest
    // was refused by the parse above and named by that type's own routed
    // message. What is left is the state machine, which no type can express.
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
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::Value;
    use study_tts_core::{
        PROVISIONAL_JOB_SCHEMA_VERSION, ProvisionalJobSnapshot, ProvisionalJobStage,
        SelectedPackageIdentity,
    };

    use super::validate_snapshot;
    use crate::{BuildError, DurableStateError};

    const JOB_ID: &str = "job-1";
    const SNAPSHOT_PATH: &str = "jobs/job-1/job.json";

    fn snapshot(stage: ProvisionalJobStage) -> ProvisionalJobSnapshot {
        ProvisionalJobSnapshot {
            schema_version: PROVISIONAL_JOB_SCHEMA_VERSION.to_owned(),
            job_id: JOB_ID.to_owned(),
            plan_hash: "a".repeat(64).parse().expect("a digest of a parses"),
            stage,
            selected_package: None,
        }
    }

    fn selected_package() -> SelectedPackageIdentity {
        SelectedPackageIdentity {
            package_id: "b".repeat(64).parse().expect("a digest of b parses"),
            manifest_blake3: "c".repeat(64).parse().expect("a digest of c parses"),
        }
    }

    fn validation_error(snapshot: &ProvisionalJobSnapshot, job_id: &str) -> BuildError {
        validate_snapshot(Path::new(SNAPSHOT_PATH), job_id, snapshot)
            .expect_err("the malformed snapshot must be refused")
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_unsupported_schema_version() {
        let mut snapshot = snapshot(ProvisionalJobStage::Planned);
        snapshot.schema_version = "e0.job-state.9.0".to_owned();

        let error = validation_error(&snapshot, JOB_ID);

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::UnsupportedDurableRecord {
                        path,
                        schema_version,
                    } if path == &PathBuf::from(SNAPSHOT_PATH)
                        && schema_version == "e0.job-state.9.0"
                )
        ));
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_job_identity_mismatch() {
        let snapshot = snapshot(ProvisionalJobStage::Planned);

        let error = validation_error(&snapshot, "job-2");

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(
                    error.as_ref(),
                    DurableStateError::JobSnapshotIdentityMismatch {
                        path,
                        recorded,
                        required,
                    } if path == &PathBuf::from(SNAPSHOT_PATH)
                        && recorded == JOB_ID
                        && required == "job-2"
                )
        ));
    }

    #[test]
    fn t1_e0_job_snapshot_refuses_a_recorded_digest_that_is_not_one() {
        // Refused at the parse rather than by a check after it, because
        // `plan_hash`, `package_id`, and `manifest_blake3` are value objects
        // now. That a snapshot carrying a malformed digest can no longer be
        // *constructed* is the point, so the case has to be expressed as the
        // JSON a repository actually reads back.
        //
        // The offending value is asserted, not just the failure: a mistyped
        // field name here would also fail to deserialize, and a variant-only
        // check would pass without the digest ever having been exercised.
        let mut selected = snapshot(ProvisionalJobStage::PackageSelected);
        selected.selected_package = Some(selected_package());

        for (field, malformed) in [
            ("plan_hash", "not-a-plan-hash"),
            ("package_id", "not-a-package-id"),
            ("manifest_blake3", "not-a-manifest-digest"),
        ] {
            let mut recorded = serde_json::to_value(&selected).expect("a job snapshot serializes");
            let holder = if recorded.get(field).is_some() {
                &mut recorded
            } else {
                &mut recorded["selected_package"]
            };
            holder[field] = Value::String(malformed.to_owned());

            let error = serde_json::from_value::<ProvisionalJobSnapshot>(recorded)
                .expect_err("a recorded digest that is not one must not deserialize");

            assert!(
                error.to_string().contains(malformed),
                "refusing `{field}` must name the offending value: {error}"
            );
        }
    }
}
