//! Immutable private-preview packages and their atomic selection record.
//!
//! A package is assembled entirely under the provisional job staging root,
//! moved once to `packages/<manifest-blake3>`, and selected only through
//! `current.json`. The journal is intentionally an internal E0 transaction
//! record, not the complete E2 job document or a production schema.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use study_tts_core::{PlanHash, is_blake3_hex};
use tempfile::Builder;

use crate::{
    BuildError, DurableStateError,
    cache::hash_file,
    durable::{
        DurableFileSystem, RenameOutcome, publish_directory_noreplace, sync_directory_transaction,
        write_json_atomically,
    },
    export::ExportProfiles,
    io_error, managed, manifest,
    tools::ToolIdentity,
};

const CURRENT_SCHEMA_VERSION: &str = "0.1-skeleton-current";
const JOURNAL_SCHEMA_VERSION: &str = "0.1-skeleton-publication";
const CURRENT_RECORD_NAME: &str = "current.json";
const JOURNAL_RECORD_NAME: &str = "publication.json";
const PACKAGES_DIRECTORY: &str = "packages";
const STAGING_DIRECTORY: &str = "staging";

/// Managed roots needed by one lesson's package publication.
#[derive(Clone, Debug)]
pub(crate) struct PreviewRoots {
    pub job_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub preview_dir: PathBuf,
    pub packages_dir: PathBuf,
    pub quarantine_root: PathBuf,
}

/// Paths selected by one valid `current.json` record.
#[derive(Clone, Debug)]
pub(crate) struct PublishedPackage {
    pub package_dir: PathBuf,
    pub master_wav: PathBuf,
    pub m4a: PathBuf,
    pub manifest: PathBuf,
    pub publication_record: PathBuf,
}

/// One in-progress package transaction.
#[derive(Clone, Debug)]
pub(crate) struct PackageTransaction {
    pub transaction_id: String,
    pub stage_dir: PathBuf,
    journal_path: PathBuf,
    lesson_id: String,
    plan_hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct CurrentPreview {
    schema_version: String,
    lesson_id: String,
    package_path: String,
    manifest_blake3: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct PublicationJournal {
    schema_version: String,
    lesson_id: String,
    plan_hash: String,
    transaction_id: String,
    transaction: PublicationState,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "state")]
enum PublicationState {
    Staging,
    CompleteStaged { manifest_blake3: String },
    PackageDurable { manifest_blake3: String },
    Selected { manifest_blake3: String },
    Complete { manifest_blake3: String },
    Abandoned,
}

#[derive(Serialize)]
struct TransactionIdentity<'a> {
    identity_version: &'static str,
    lesson_id: &'a str,
    plan_hash: &'a str,
    ffmpeg_executable: String,
    ffmpeg_version: &'a str,
    ffmpeg_profile_blake3: &'a str,
    ffprobe_executable: String,
    ffprobe_version: &'a str,
    ffprobe_profile_blake3: &'a str,
}

/// Creates and contains the managed roots for one provisional lesson job.
///
/// # Errors
///
/// [`crate::ManagedPathError`] when any managed component is unsafe or a
/// symlink, otherwise [`crate::IoError::FileSystem`].
pub(crate) fn roots(workspace: &Path, lesson_id: &str) -> Result<PreviewRoots, BuildError> {
    let jobs = managed::subdirectory(workspace, "jobs")?;
    let job_dir = managed::subdirectory(&jobs, lesson_id)?;
    let staging_dir = managed::subdirectory(&job_dir, STAGING_DIRECTORY)?;
    let previews = managed::subdirectory(workspace, "previews")?;
    let preview_dir = managed::subdirectory(&previews, lesson_id)?;
    let packages_dir = managed::subdirectory(&preview_dir, PACKAGES_DIRECTORY)?;
    let quarantine_root = managed::subdirectory(workspace, "quarantine")?;
    Ok(PreviewRoots {
        job_dir,
        staging_dir,
        preview_dir,
        packages_dir,
        quarantine_root,
    })
}

/// Reconciles the provisional publication journal and validates `current.json`.
///
/// # Errors
///
/// A specific [`DurableStateError`] when a journal, selection record, package,
/// checksum, or transaction cannot be trusted, otherwise filesystem errors.
pub(crate) fn reconcile(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    lesson_id: &str,
) -> Result<Option<PublishedPackage>, BuildError> {
    let current = read_current(roots, lesson_id)?;
    let journal_path = managed::leaf(&roots.job_dir, JOURNAL_RECORD_NAME)?;
    let journal = read_journal(&journal_path, lesson_id)?;
    reconcile_orphan_stages(
        filesystem,
        roots,
        journal.as_ref().map(|value| value.transaction_id.as_str()),
    )?;

    let Some(mut journal) = journal else {
        return Ok(current);
    };
    loop {
        match &journal.transaction {
            PublicationState::Staging => {
                let stage = transaction_stage(roots, &journal.transaction_id)?;
                if !stage.is_dir() {
                    journal.transaction = PublicationState::Abandoned;
                    write_json_atomically(filesystem, &journal_path, &journal)?;
                    return Ok(current);
                }
                if !stage.join(manifest::MANIFEST_NAME).is_file()
                    || !stage.join(manifest::MASTER_WAV_NAME).is_file()
                    || !stage.join(manifest::M4A_NAME).is_file()
                {
                    quarantine_package_stage(filesystem, roots, &stage)?;
                    journal.transaction = PublicationState::Abandoned;
                    write_json_atomically(filesystem, &journal_path, &journal)?;
                    return Ok(current);
                }
                let plan_matches =
                    manifest::validate_package(&stage, lesson_id, Some(&journal.plan_hash), None)?;
                require_transaction_plan(&stage, plan_matches)?;
                let checksum = hash_file(&stage.join(manifest::MANIFEST_NAME))?;
                journal.transaction = PublicationState::CompleteStaged {
                    manifest_blake3: checksum,
                };
                write_json_atomically(filesystem, &journal_path, &journal)?;
            }
            PublicationState::CompleteStaged { manifest_blake3 } => {
                validate_digest(&journal_path, manifest_blake3)?;
                let stage = transaction_stage(roots, &journal.transaction_id)?;
                let package = package_path(roots, manifest_blake3)?;
                finish_package_move(
                    filesystem,
                    roots,
                    lesson_id,
                    &stage,
                    &package,
                    manifest_blake3,
                )?;
                journal.transaction = PublicationState::PackageDurable {
                    manifest_blake3: manifest_blake3.clone(),
                };
                write_json_atomically(filesystem, &journal_path, &journal)?;
            }
            PublicationState::PackageDurable { manifest_blake3 } => {
                validate_digest(&journal_path, manifest_blake3)?;
                let package = package_path(roots, manifest_blake3)?;
                validate_package_checksum(&package, lesson_id, manifest_blake3)?;
                select_package(filesystem, roots, lesson_id, manifest_blake3)?;
                journal.transaction = PublicationState::Selected {
                    manifest_blake3: manifest_blake3.clone(),
                };
                write_json_atomically(filesystem, &journal_path, &journal)?;
            }
            PublicationState::Selected { manifest_blake3 } => {
                validate_selected_checksum(roots, lesson_id, &journal.plan_hash, manifest_blake3)?;
                journal.transaction = PublicationState::Complete {
                    manifest_blake3: manifest_blake3.clone(),
                };
                write_json_atomically(filesystem, &journal_path, &journal)?;
            }
            PublicationState::Complete { manifest_blake3 } => {
                validate_selected_checksum(roots, lesson_id, &journal.plan_hash, manifest_blake3)?;
                return read_current(roots, lesson_id);
            }
            PublicationState::Abandoned => {
                return read_current(roots, lesson_id);
            }
        }
    }
}

/// Returns the selected package only when it matches this plan and tool stack.
///
/// # Errors
///
/// A specific [`DurableStateError`] when `current.json` or its package is
/// corrupt, otherwise filesystem errors.
pub(crate) fn current_for_build(
    roots: &PreviewRoots,
    lesson_id: &str,
    plan_hash: &PlanHash,
    ffmpeg: &ToolIdentity,
    ffprobe: &ToolIdentity,
    profiles: &ExportProfiles,
) -> Result<Option<PublishedPackage>, BuildError> {
    let Some(package) = read_current(roots, lesson_id)? else {
        return Ok(None);
    };
    if manifest::validate_package(
        &package.package_dir,
        lesson_id,
        Some(plan_hash.as_str()),
        Some(manifest::ToolExpectations {
            ffmpeg,
            ffmpeg_profile: &profiles.ffmpeg,
            ffprobe,
            ffprobe_profile: &profiles.ffprobe,
        }),
    )? {
        return Ok(Some(package));
    }
    Ok(None)
}

/// Starts a durable package transaction under the job staging root.
///
/// # Errors
///
/// Filesystem or JSON persistence errors, or a specific
/// [`DurableStateError`] when an abandoned stage cannot be preserved.
pub(crate) fn start_transaction(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    lesson_id: &str,
    plan_hash: &PlanHash,
    ffmpeg: &ToolIdentity,
    ffprobe: &ToolIdentity,
    profiles: &ExportProfiles,
) -> Result<PackageTransaction, BuildError> {
    let transaction_id = transaction_identity(lesson_id, plan_hash, ffmpeg, ffprobe, profiles);
    let stage_dir = transaction_stage(roots, &transaction_id)?;
    if stage_dir.exists() {
        quarantine_package_stage(filesystem, roots, &stage_dir)?;
    }
    fs::create_dir(&stage_dir).map_err(|error| io_error(&stage_dir, error))?;
    filesystem.sync_directory(&roots.staging_dir)?;
    let journal_path = managed::leaf(&roots.job_dir, JOURNAL_RECORD_NAME)?;
    let journal = PublicationJournal {
        schema_version: JOURNAL_SCHEMA_VERSION.to_owned(),
        lesson_id: lesson_id.to_owned(),
        plan_hash: plan_hash.as_str().to_owned(),
        transaction_id: transaction_id.clone(),
        transaction: PublicationState::Staging,
    };
    write_json_atomically(filesystem, &journal_path, &journal)?;
    Ok(PackageTransaction {
        transaction_id,
        stage_dir,
        journal_path,
        lesson_id: lesson_id.to_owned(),
        plan_hash: plan_hash.as_str().to_owned(),
    })
}

/// Publishes and atomically selects one fully synchronized staged package.
///
/// # Errors
///
/// A specific [`DurableStateError`] when the staged package or an existing
/// immutable winner is inconsistent, otherwise durable-filesystem errors.
pub(crate) fn publish_transaction(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    transaction: &PackageTransaction,
) -> Result<PublishedPackage, BuildError> {
    let plan_matches = manifest::validate_package(
        &transaction.stage_dir,
        &transaction.lesson_id,
        Some(&transaction.plan_hash),
        None,
    )?;
    require_transaction_plan(&transaction.stage_dir, plan_matches)?;
    let manifest_path = transaction.stage_dir.join(manifest::MANIFEST_NAME);
    let manifest_blake3 = hash_file(&manifest_path)?;
    let master_wav = transaction.stage_dir.join(manifest::MASTER_WAV_NAME);
    let m4a = transaction.stage_dir.join(manifest::M4A_NAME);
    sync_directory_transaction(
        filesystem,
        &transaction.stage_dir,
        &[&master_wav, &m4a, &manifest_path],
    )?;
    let mut journal = PublicationJournal {
        schema_version: JOURNAL_SCHEMA_VERSION.to_owned(),
        lesson_id: transaction.lesson_id.clone(),
        plan_hash: transaction.plan_hash.clone(),
        transaction_id: transaction.transaction_id.clone(),
        transaction: PublicationState::CompleteStaged {
            manifest_blake3: manifest_blake3.clone(),
        },
    };
    write_json_atomically(filesystem, &transaction.journal_path, &journal)?;

    let package = package_path(roots, &manifest_blake3)?;
    finish_package_move(
        filesystem,
        roots,
        &transaction.lesson_id,
        &transaction.stage_dir,
        &package,
        &manifest_blake3,
    )?;
    journal.transaction = PublicationState::PackageDurable {
        manifest_blake3: manifest_blake3.clone(),
    };
    write_json_atomically(filesystem, &transaction.journal_path, &journal)?;

    select_package(filesystem, roots, &transaction.lesson_id, &manifest_blake3)?;
    journal.transaction = PublicationState::Selected {
        manifest_blake3: manifest_blake3.clone(),
    };
    write_json_atomically(filesystem, &transaction.journal_path, &journal)?;
    journal.transaction = PublicationState::Complete { manifest_blake3 };
    write_json_atomically(filesystem, &transaction.journal_path, &journal)?;

    read_current(roots, &transaction.lesson_id)?.ok_or_else(|| {
        DurableStateError::MissingCurrentPreview {
            path: roots.preview_dir.join(CURRENT_RECORD_NAME),
        }
        .into()
    })
}

fn read_journal(path: &Path, lesson_id: &str) -> Result<Option<PublicationJournal>, BuildError> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|error| io_error(path, error))?;
    let journal: PublicationJournal = serde_json::from_slice(&bytes).map_err(|source| {
        DurableStateError::MalformedPublicationJournal {
            path: path.to_path_buf(),
            source,
        }
    })?;
    validate_record_version(path, &journal.schema_version, JOURNAL_SCHEMA_VERSION)?;
    if journal.lesson_id != lesson_id {
        return Err(DurableStateError::PublicationJournalLessonMismatch {
            path: path.to_path_buf(),
            recorded: journal.lesson_id,
            required: lesson_id.to_owned(),
        }
        .into());
    }
    validate_digest(path, &journal.plan_hash)?;
    validate_digest(path, &journal.transaction_id)?;
    match &journal.transaction {
        PublicationState::CompleteStaged { manifest_blake3 }
        | PublicationState::PackageDurable { manifest_blake3 }
        | PublicationState::Selected { manifest_blake3 }
        | PublicationState::Complete { manifest_blake3 } => {
            validate_digest(path, manifest_blake3)?;
        }
        PublicationState::Staging | PublicationState::Abandoned => {}
    }
    Ok(Some(journal))
}

fn read_current(
    roots: &PreviewRoots,
    lesson_id: &str,
) -> Result<Option<PublishedPackage>, BuildError> {
    let path = managed::leaf(&roots.preview_dir, CURRENT_RECORD_NAME)?;
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).map_err(|error| io_error(&path, error))?;
    let current: CurrentPreview = serde_json::from_slice(&bytes).map_err(|source| {
        DurableStateError::MalformedCurrentPreview {
            path: path.clone(),
            source,
        }
    })?;
    validate_record_version(&path, &current.schema_version, CURRENT_SCHEMA_VERSION)?;
    if current.lesson_id != lesson_id {
        return Err(DurableStateError::CurrentLessonMismatch {
            path,
            recorded: current.lesson_id,
            required: lesson_id.to_owned(),
        }
        .into());
    }
    validate_digest(&path, &current.manifest_blake3)?;
    let package = parse_package_reference(roots, &path, &current)?;
    validate_package_checksum(&package, lesson_id, &current.manifest_blake3)?;
    Ok(Some(published_paths(package, path)))
}

fn parse_package_reference(
    roots: &PreviewRoots,
    record_path: &Path,
    current: &CurrentPreview,
) -> Result<PathBuf, BuildError> {
    let relative = Path::new(&current.package_path);
    let components = relative.components().collect::<Vec<_>>();
    if components.len() != 2
        || components[0] != Component::Normal(PACKAGES_DIRECTORY.as_ref())
        || components[1] != Component::Normal(current.manifest_blake3.as_ref())
    {
        return Err(DurableStateError::InvalidCurrentPackageReference {
            record: record_path.to_path_buf(),
            reference: current.package_path.clone(),
        }
        .into());
    }
    managed::directory_candidate(&roots.packages_dir, &current.manifest_blake3)
}

fn validate_package_checksum(
    package: &Path,
    lesson_id: &str,
    expected: &str,
) -> Result<(), BuildError> {
    if !package.is_dir() {
        return Err(DurableStateError::MissingPackageDirectory {
            path: package.to_path_buf(),
        }
        .into());
    }
    manifest::validate_package(package, lesson_id, None, None)?;
    let manifest_path = package.join(manifest::MANIFEST_NAME);
    let found = hash_file(&manifest_path)?;
    if found != expected {
        return Err(DurableStateError::PackageManifestChecksumMismatch {
            path: manifest_path,
            expected: expected.to_owned(),
            found,
        }
        .into());
    }
    Ok(())
}

fn finish_package_move(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    lesson_id: &str,
    stage: &Path,
    package: &Path,
    manifest_blake3: &str,
) -> Result<(), BuildError> {
    if stage.is_dir() {
        validate_package_checksum(stage, lesson_id, manifest_blake3)?;
        match publish_directory_noreplace(filesystem, stage, package)? {
            RenameOutcome::Published => return Ok(()),
            RenameOutcome::DestinationExists => {
                validate_package_checksum(package, lesson_id, manifest_blake3)?;
                quarantine_package_stage(filesystem, roots, stage)?;
                return Ok(());
            }
        }
    }
    validate_package_checksum(package, lesson_id, manifest_blake3)
}

fn select_package(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    lesson_id: &str,
    manifest_blake3: &str,
) -> Result<(), BuildError> {
    if roots.preview_dir.join(CURRENT_RECORD_NAME).exists() {
        let _ = read_current(roots, lesson_id)?;
    }
    let record = CurrentPreview {
        schema_version: CURRENT_SCHEMA_VERSION.to_owned(),
        lesson_id: lesson_id.to_owned(),
        package_path: format!("{PACKAGES_DIRECTORY}/{manifest_blake3}"),
        manifest_blake3: manifest_blake3.to_owned(),
    };
    write_json_atomically(
        filesystem,
        &managed::leaf(&roots.preview_dir, CURRENT_RECORD_NAME)?,
        &record,
    )
}

fn validate_selected_checksum(
    roots: &PreviewRoots,
    lesson_id: &str,
    plan_hash: &str,
    manifest_blake3: &str,
) -> Result<(), BuildError> {
    let current = read_current(roots, lesson_id)?.ok_or_else(|| {
        DurableStateError::MissingCurrentPreview {
            path: roots.preview_dir.join(CURRENT_RECORD_NAME),
        }
    })?;
    let current_manifest = current
        .package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if current_manifest != manifest_blake3 {
        return Err(DurableStateError::JournalSelectionMismatch {
            record: current.publication_record,
            journal_manifest: manifest_blake3.to_owned(),
            current_manifest: current_manifest.to_owned(),
        }
        .into());
    }
    let plan_matches =
        manifest::validate_package(&current.package_dir, lesson_id, Some(plan_hash), None)?;
    require_transaction_plan(&current.package_dir, plan_matches)?;
    Ok(())
}

fn reconcile_orphan_stages(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    active: Option<&str>,
) -> Result<(), BuildError> {
    for entry in
        fs::read_dir(&roots.staging_dir).map_err(|error| io_error(&roots.staging_dir, error))?
    {
        let entry = entry.map_err(|error| io_error(&roots.staging_dir, error))?;
        if entry
            .file_type()
            .map_err(|error| io_error(entry.path(), error))?
            .is_dir()
            && entry.file_name().to_str() != active
        {
            quarantine_package_stage(filesystem, roots, &entry.path())?;
        }
    }
    Ok(())
}

fn quarantine_package_stage(
    filesystem: &dyn DurableFileSystem,
    roots: &PreviewRoots,
    stage: &Path,
) -> Result<(), BuildError> {
    if !stage.exists() {
        return Ok(());
    }
    let lesson = managed::subdirectory(
        &roots.quarantine_root,
        roots
            .job_dir
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| DurableStateError::InvalidJobDirectoryName {
                path: roots.job_dir.clone(),
            })?,
    )?;
    let publication = managed::subdirectory(&lesson, "publication")?;
    let attempt = Builder::new()
        .prefix("attempt-")
        .tempdir_in(&publication)
        .map_err(|error| io_error(&publication, error))?
        .keep();
    let destination = attempt.join("package-attempt");
    if publish_directory_noreplace(filesystem, stage, &destination)? != RenameOutcome::Published {
        return Err(DurableStateError::PublicationConflict { path: destination }.into());
    }
    filesystem.sync_directory(&publication)
}

fn transaction_stage(roots: &PreviewRoots, transaction_id: &str) -> Result<PathBuf, BuildError> {
    validate_digest(&roots.staging_dir, transaction_id)?;
    managed::directory_candidate(&roots.staging_dir, transaction_id)
}

fn package_path(roots: &PreviewRoots, manifest_blake3: &str) -> Result<PathBuf, BuildError> {
    validate_digest(&roots.packages_dir, manifest_blake3)?;
    managed::directory_candidate(&roots.packages_dir, manifest_blake3)
}

fn transaction_identity(
    lesson_id: &str,
    plan_hash: &PlanHash,
    ffmpeg: &ToolIdentity,
    ffprobe: &ToolIdentity,
    profiles: &ExportProfiles,
) -> String {
    let identity = TransactionIdentity {
        identity_version: JOURNAL_SCHEMA_VERSION,
        lesson_id,
        plan_hash: plan_hash.as_str(),
        ffmpeg_executable: ffmpeg.resolved_executable.display().to_string(),
        ffmpeg_version: &ffmpeg.version,
        ffmpeg_profile_blake3: profiles.ffmpeg.identity(),
        ffprobe_executable: ffprobe.resolved_executable.display().to_string(),
        ffprobe_version: &ffprobe.version,
        ffprobe_profile_blake3: profiles.ffprobe.identity(),
    };
    let bytes = serde_json::to_vec(&identity)
        .expect("transaction identity contains only infallibly serializable values");
    blake3::hash(&bytes).to_hex().to_string()
}

fn validate_record_version(path: &Path, found: &str, required: &str) -> Result<(), BuildError> {
    if found == required {
        return Ok(());
    }
    Err(DurableStateError::UnsupportedDurableRecord {
        path: path.to_path_buf(),
        schema_version: found.to_owned(),
    }
    .into())
}

fn validate_digest(path: &Path, digest: &str) -> Result<(), BuildError> {
    if is_blake3_hex(digest) {
        return Ok(());
    }
    Err(DurableStateError::MalformedDurableDigest {
        path: path.to_path_buf(),
        value: digest.to_owned(),
    }
    .into())
}

fn require_transaction_plan(path: &Path, matches: bool) -> Result<(), BuildError> {
    if matches {
        return Ok(());
    }
    Err(DurableStateError::PackagePlanMismatch {
        path: path.to_path_buf(),
    }
    .into())
}

fn published_paths(package_dir: PathBuf, publication_record: PathBuf) -> PublishedPackage {
    PublishedPackage {
        master_wav: package_dir.join(manifest::MASTER_WAV_NAME),
        m4a: package_dir.join(manifest::M4A_NAME),
        manifest: package_dir.join(manifest::MANIFEST_NAME),
        package_dir,
        publication_record,
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::{
        cache::CachedSegment,
        durable::{OsDurableFileSystem, TracingFileSystem},
        export::{ToolExecution, ToolProfile},
    };

    #[test]
    fn t4_e0_package_publication_flushes_files_before_the_directory_rename() {
        let workspace = TempDir::new().expect("create preview workspace");
        let roots = roots(workspace.path(), "lesson").expect("create preview roots");
        let plan_hash = PlanHash::from(blake3::hash(b"plan"));
        let ffmpeg = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffmpeg"),
            version: "ffmpeg version 1".to_owned(),
        };
        let ffprobe = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffprobe"),
            version: "ffprobe version 1".to_owned(),
        };
        let profiles = crate::export::export_profiles();
        let transaction = start_transaction(
            &OsDurableFileSystem,
            &roots,
            "lesson",
            &plan_hash,
            &ffmpeg,
            &ffprobe,
            &profiles,
        )
        .expect("start package transaction");
        let master = transaction.stage_dir.join(manifest::MASTER_WAV_NAME);
        let m4a = transaction.stage_dir.join(manifest::M4A_NAME);
        let manifest_path = transaction.stage_dir.join(manifest::MANIFEST_NAME);
        fs::write(&master, b"master").expect("write master");
        fs::write(&m4a, b"encoded").expect("write encoded output");
        let segment = CachedSegment {
            segment_id: "segment".to_owned(),
            cache_key: "a"
                .repeat(study_tts_core::CacheKey::LENGTH)
                .parse()
                .expect("valid cache key"),
            entry_dir: PathBuf::from("cache-entry"),
            audio_path: PathBuf::from("cache-entry/audio.wav"),
            audio_blake3: blake3::hash(b"segment").to_hex().to_string(),
            frames: 1,
            pause_after_ms: 0,
        };
        let ffmpeg_execution = ToolExecution {
            arguments: vec!["encode".to_owned()],
            argument_profile_blake3: profiles.ffmpeg.identity().to_owned(),
        };
        let ffprobe_execution = ToolExecution {
            arguments: vec!["probe".to_owned()],
            argument_profile_blake3: profiles.ffprobe.identity().to_owned(),
        };
        manifest::write(
            &OsDurableFileSystem,
            &manifest_path,
            manifest::ManifestRecords {
                lesson_id: "lesson",
                plan_hash: &plan_hash,
                segments: std::slice::from_ref(&segment),
                master_wav: &master,
                m4a: &m4a,
                tools: manifest::ToolRecords {
                    ffmpeg: &ffmpeg,
                    ffmpeg_execution: &ffmpeg_execution,
                    ffprobe: &ffprobe,
                    ffprobe_execution: &ffprobe_execution,
                },
            },
        )
        .expect("write package manifest");
        let filesystem = TracingFileSystem::default();

        publish_transaction(&filesystem, &roots, &transaction).expect("publish package");

        let events = filesystem.events.lock().expect("trace lock");
        let rename_index = events
            .iter()
            .position(|event| event.starts_with("rename:"))
            .expect("package rename event");
        for required in [
            format!("file:{}", master.display()),
            format!("file:{}", m4a.display()),
            format!("file:{}", manifest_path.display()),
            format!("directory:{}", transaction.stage_dir.display()),
        ] {
            let sync_index = events
                .iter()
                .position(|event| event == &required)
                .unwrap_or_else(|| panic!("missing durability event `{required}`"));
            assert!(
                sync_index < rename_index,
                "`{required}` followed package rename"
            );
        }
    }

    #[test]
    fn t4_e0_encoding_profile_change_starts_a_new_generation() {
        let workspace = TempDir::new().expect("create preview workspace");
        let roots = roots(workspace.path(), "lesson").expect("create preview roots");
        let plan_hash = PlanHash::from(blake3::hash(b"same plan"));
        let ffmpeg = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffmpeg"),
            version: "ffmpeg version 1".to_owned(),
        };
        let ffprobe = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffprobe"),
            version: "ffprobe version 1".to_owned(),
        };
        let first_profiles = crate::export::export_profiles();
        let changed_profiles = ExportProfiles {
            ffmpeg: ToolProfile::new(
                "ffmpeg",
                &["-i", "{input_path}", "-c:a", "libopus", "{output_path}"],
            ),
            ffprobe: first_profiles.ffprobe.clone(),
        };

        let first = start_transaction(
            &OsDurableFileSystem,
            &roots,
            "lesson",
            &plan_hash,
            &ffmpeg,
            &ffprobe,
            &first_profiles,
        )
        .expect("start first generation");
        let changed = start_transaction(
            &OsDurableFileSystem,
            &roots,
            "lesson",
            &plan_hash,
            &ffmpeg,
            &ffprobe,
            &changed_profiles,
        )
        .expect("start changed-profile generation");

        assert_ne!(first.transaction_id, changed.transaction_id);
        assert_ne!(first.stage_dir, changed.stage_dir);
        assert!(first.stage_dir.is_dir());
        assert!(changed.stage_dir.is_dir());
    }
}
