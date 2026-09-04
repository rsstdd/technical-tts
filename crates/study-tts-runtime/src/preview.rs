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
use study_tts_core::{ManifestDigest, PlanHash, RenderPlan, ToolProfileHash, is_blake3_hex};
use tempfile::Builder;

use crate::{
    BuildError, DurableStateError,
    cache::{self, hash_file},
    durable::{
        DurableFileSystem, RenameOutcome, publish_directory_noreplace, sync_directory_transaction,
        write_json_atomically,
    },
    export::ExportProfiles,
    io_error, managed, manifest, timeline,
    tools::ToolIdentity,
};

const CURRENT_SCHEMA_VERSION: &str = "0.1-skeleton-current";
const JOURNAL_SCHEMA_VERSION: &str = "0.1-skeleton-publication";

/// Version of the transaction-identity document, which is hashed rather than
/// stored.
///
/// Its own constant, and deliberately not [`JOURNAL_SCHEMA_VERSION`]. The two
/// were the same string until E1-S4 added the MP3 profile to the identity: the
/// journal record on disk did not change at all, so bumping the identity
/// through the journal's constant would have made `validate_record_version`
/// refuse every `publication.json` an earlier build wrote. One document changed
/// and one did not, so they now version separately.
///
/// `0.3` is what E1-S4 made it: every argument profile rather than the M4A one,
/// taken from [`ExportProfiles::identities`], plus
/// [`timeline::TEXT_RENDERER_VERSION`], because two builds that would write
/// different captions for one plan must not share a staging directory. Reuse is
/// decided by `manifest::validate_package`, not here — this constant only
/// separates concurrent work.
const TRANSACTION_IDENTITY_VERSION: &str = "0.3-skeleton-transaction";
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
    pub manifest_blake3: ManifestDigest,
    pub package_dir: PathBuf,
    pub master_wav: PathBuf,
    pub m4a: PathBuf,
    pub mp3: PathBuf,
    pub transcript: PathBuf,
    pub captions: PathBuf,
    pub chapters: PathBuf,
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

/// What makes two builds the same package generation.
///
/// Every argument profile is included, not only the M4A one: an MP3 profile
/// change produces different bytes in the package, so a generation keyed
/// without it would let a rebuild reuse a package that no longer matches how
/// this build encodes. `t4_e0_encoding_profile_change_starts_a_new_generation`
/// is what holds that.
///
/// [`ExportProfiles::identities`] supplies the whole set rather than this
/// struct naming each profile: a second hand-written list here could omit a
/// profile that reuse already compares, and the two would disagree about what
/// a generation is.
#[derive(Serialize)]
struct TransactionIdentity<'a> {
    identity_version: &'static str,
    lesson_id: &'a str,
    plan_hash: &'a str,
    ffmpeg_executable: String,
    ffmpeg_version: &'a str,
    ffprobe_executable: String,
    ffprobe_version: &'a str,
    argument_profile_blake3: Vec<&'a str>,
    text_renderer_version: &'a str,
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

/// The manifest of every published package under `workspace`, for every
/// lesson.
///
/// A reader of the layout [`roots`] states rather than a second copy of it:
/// the same `previews/<lesson>/packages/<generation>` shape, walked instead of
/// resolved because a retention report is asked about lessons it was not told
/// the names of.
///
/// Nothing is created. A workspace that has published nothing yet reports an
/// empty list, which is not the same answer as a workspace whose previews are
/// unreadable — that is an error, because treating unreadable roots as "no
/// roots" is how live artifacts become prune candidates.
///
/// # Errors
///
/// [`crate::ManagedPathError::ManagedPathEscape`] for a planted link,
/// otherwise [`crate::IoError::FileSystem`].
pub(crate) fn published_manifests(workspace: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let previews = managed::directory_candidate(workspace, "previews")?;
    let mut manifests = Vec::new();
    if !previews.is_dir() {
        return Ok(manifests);
    }

    for lesson_dir in cache::read_directories(&previews)? {
        let packages = managed::directory_candidate(&lesson_dir, PACKAGES_DIRECTORY)?;
        if !packages.is_dir() {
            continue;
        }
        for generation in cache::read_directories(&packages)? {
            let manifest_path = managed::leaf(&generation, manifest::MANIFEST_NAME)?;
            if manifest_path.is_file() {
                manifests.push(manifest_path);
            }
        }
    }
    Ok(manifests)
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

/// Returns the identity selected by a validated `current.json`, when present.
///
/// The selected package must also be complete and carry `plan_hash`; a digest
/// agreement alone cannot prove that `job.json` completed the plan it names.
///
/// # Errors
///
/// The same exact durable-state, containment, and filesystem errors as the
/// selected-record half of [`reconcile`].
pub(crate) fn current_manifest_digest(
    roots: &PreviewRoots,
    lesson_id: &str,
    plan_hash: &PlanHash,
) -> Result<Option<ManifestDigest>, BuildError> {
    let Some(package) = read_current(roots, lesson_id)? else {
        return Ok(None);
    };
    let plan_matches = manifest::validate_package(
        &package.package_dir,
        lesson_id,
        Some(plan_hash.as_str()),
        None,
    )?;
    require_transaction_plan(&package.package_dir, plan_matches)?;
    Ok(Some(package.manifest_blake3))
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
    plan: &RenderPlan,
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
        Some(plan.plan_hash.as_str()),
        Some(manifest::ReuseExpectations {
            ffmpeg,
            ffprobe,
            profiles,
            text_renderer_version: timeline::TEXT_RENDERER_VERSION,
            take_selection_source: plan.take_selection_source,
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
    let transaction_id = transaction_identity(
        lesson_id,
        plan_hash,
        ffmpeg,
        ffprobe,
        profiles,
        timeline::TEXT_RENDERER_VERSION,
    );
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
    // Every artifact plus the manifest: seven files, from the one list
    // `manifest` owns, so a format added there cannot be published unflushed
    // here. `validate_package` above has already confirmed all six exist and
    // hash to what the manifest records.
    let artifacts: Vec<PathBuf> = manifest::PACKAGE_ARTIFACT_NAMES
        .iter()
        .map(|name| transaction.stage_dir.join(name))
        .chain(std::iter::once(manifest_path.clone()))
        .collect();
    let synchronized: Vec<&Path> = artifacts.iter().map(PathBuf::as_path).collect();
    sync_directory_transaction(filesystem, &transaction.stage_dir, &synchronized)?;
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
    // `validate_digest` above established exactly the spelling the value
    // object accepts, so no bytes from `current.json` can reach this expect
    // without first satisfying that invariant.
    let manifest_blake3 = current
        .manifest_blake3
        .parse()
        .expect("the validated current-preview digest parses");
    Ok(Some(published_paths(package, path, manifest_blake3)))
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
    text_renderer_version: &str,
) -> String {
    let identity = TransactionIdentity {
        identity_version: TRANSACTION_IDENTITY_VERSION,
        lesson_id,
        plan_hash: plan_hash.as_str(),
        ffmpeg_executable: ffmpeg.resolved_executable.display().to_string(),
        ffmpeg_version: &ffmpeg.version,
        ffprobe_executable: ffprobe.resolved_executable.display().to_string(),
        ffprobe_version: &ffprobe.version,
        argument_profile_blake3: profiles
            .identities()
            .into_iter()
            .map(ToolProfileHash::as_str)
            .collect(),
        text_renderer_version,
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

fn published_paths(
    package_dir: PathBuf,
    publication_record: PathBuf,
    manifest_blake3: ManifestDigest,
) -> PublishedPackage {
    PublishedPackage {
        manifest_blake3,
        master_wav: package_dir.join(manifest::MASTER_WAV_NAME),
        m4a: package_dir.join(manifest::M4A_NAME),
        mp3: package_dir.join(manifest::MP3_NAME),
        transcript: package_dir.join(manifest::TRANSCRIPT_NAME),
        captions: package_dir.join(manifest::CAPTIONS_NAME),
        chapters: package_dir.join(manifest::CHAPTERS_NAME),
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
        cache::ValidatedCachedArtifact,
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
        let manifest_path = transaction.stage_dir.join(manifest::MANIFEST_NAME);
        let artifacts: Vec<PathBuf> = manifest::PACKAGE_ARTIFACT_NAMES
            .iter()
            .map(|name| {
                let path = transaction.stage_dir.join(name);
                fs::write(&path, name.as_bytes()).expect("write package artifact");
                path
            })
            .collect();
        let segment = ValidatedCachedArtifact {
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
        /// The plan the fixture manifest is written from, matching `segment`.
        fn one_segment_plan(plan_hash: &PlanHash, segment: &ValidatedCachedArtifact) -> RenderPlan {
            RenderPlan {
                schema_version: study_tts_core::PLAN_SCHEMA_VERSION,
                lesson_id: "lesson".to_owned(),
                plan_hash: plan_hash.clone(),
                take_selection_source: study_tts_core::TakeSelectionSource::Implicit,
                segments: vec![study_tts_core::PlannedSegment {
                    id: segment.segment_id.clone(),
                    speaker: "nadia".to_owned(),
                    voice_profile: "nadia-v1".to_owned(),
                    display_text: "Segment.".to_owned(),
                    spoken_text: "Segment.".to_owned(),
                    style: study_tts_core::DeliveryStyle::Calm,
                    pause_after_ms: 0,
                    take: study_tts_core::BASE_TAKE,
                    cache_key: segment.cache_key.clone(),
                    synthesis_base_key: segment.cache_key.clone(),
                    audio_blake3: None,
                }],
            }
        }

        let executions: Vec<(manifest::RecordedTool, ToolExecution)> = profiles
            .identities()
            .into_iter()
            .map(|identity| {
                (
                    manifest::RecordedTool::Ffmpeg,
                    ToolExecution {
                        arguments: vec!["ran".to_owned()],
                        argument_profile_blake3: identity.to_owned(),
                    },
                )
            })
            .collect();
        let recorded: Vec<manifest::RecordedExecution<'_>> = executions
            .iter()
            .map(|(tool, execution)| manifest::RecordedExecution {
                tool: *tool,
                execution,
            })
            .collect();
        manifest::write(
            &OsDurableFileSystem,
            &manifest_path,
            manifest::ManifestRecords {
                lesson_id: "lesson",
                plan: &one_segment_plan(&plan_hash, &segment),
                joins: &[],
                segments: std::slice::from_ref(&segment),
                timeline: &timeline::Timeline {
                    segments: vec![timeline::WrittenSegment {
                        start_frame: 0,
                        audio_frames: 1,
                        pause_frames: 0,
                    }],
                    total_frames: 1,
                },
                package_dir: &transaction.stage_dir,
                tools: manifest::ToolRecords {
                    ffmpeg: &ffmpeg,
                    ffprobe: &ffprobe,
                    executions: &recorded,
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
        // All seven files, not the three E0 published: an artifact renamed into
        // place without its own flush is exactly the loss this ordering exists
        // to prevent, and a test naming only some of them would not see it.
        for required in artifacts
            .iter()
            .chain(std::iter::once(&manifest_path))
            .map(|path| format!("file:{}", path.display()))
            .chain(std::iter::once(format!(
                "directory:{}",
                transaction.stage_dir.display()
            )))
        {
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

    /// A different text renderer is a different generation for staging too.
    ///
    /// Reuse is gated by `manifest::validate_package`, which
    /// `t4_e1_text_renderer_change_names_a_new_package_generation` covers. This
    /// is the other half: two builds that would write different captions for
    /// one plan and one toolchain must not stage into the same directory, and
    /// without this nothing would notice the field leaving the hashed document.
    #[test]
    fn t4_e1_text_renderer_change_starts_a_new_generation() {
        // No workspace: the identity is a pure function of what it is handed,
        // and staging separation follows from the digest differing.
        let plan_hash = PlanHash::from(blake3::hash(b"same plan"));
        let ffmpeg = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffmpeg"),
            version: "ffmpeg version 1".to_owned(),
        };
        let ffprobe = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffprobe"),
            version: "ffprobe version 1".to_owned(),
        };
        let profiles = crate::export::export_profiles();

        let identity = |renderer: &str| {
            transaction_identity("lesson", &plan_hash, &ffmpeg, &ffprobe, &profiles, renderer)
        };

        assert_ne!(
            identity(timeline::TEXT_RENDERER_VERSION),
            identity("0.9-skeleton-text-renderer"),
            "a changed text renderer must name a new generation"
        );
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
        // The MP3 profile, specifically: it is the one E1-S4 added, and a
        // generation keyed only on the M4A profile would have reused a package
        // whose MP3 no longer matched how this build encodes.
        let changed_profiles = ExportProfiles {
            ffmpeg_mp3: ToolProfile::new(
                "ffmpeg",
                &["-i", "{input_path}", "-c:a", "libopus", "{output_path}"],
            ),
            ..first_profiles.clone()
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
