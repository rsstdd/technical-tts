//! Master-first package writing and immutable preview-selection port.
//!
//! The filesystem adapter delegates to the walking skeleton's Rust PCM,
//! FFmpeg, ffprobe, manifest, journal, and atomic-selection implementations.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records its consumers,
//! fake, identity effects, and G1 real-path parity requirement.

use std::{fs, path::PathBuf};

use study_tts_core::{RenderPlan, SelectedPackageIdentity};

use crate::{
    BuildError, CacheError, ManagedPathError, PackageArtifactMismatch, assembly,
    cache::{self, ValidatedCachedArtifact},
    durable::OsDurableFileSystem,
    export::{self, ExportProfiles},
    io_error, managed, manifest, preview,
    tools::{self, ToolIdentity},
};

/// Mirrors the package version in the E0-S4 provisional contract baseline.
pub const PACKAGE_WRITER_CONTRACT_VERSION: &str = "e0.package-writer.1.0";

#[derive(Clone, Debug)]
struct PackageToolchain {
    ffmpeg: ToolIdentity,
    ffprobe: ToolIdentity,
    profiles: ExportProfiles,
}

impl PackageToolchain {
    fn inspect(
        ffmpeg_executable: &std::path::Path,
        ffprobe_executable: &std::path::Path,
    ) -> Result<Self, BuildError> {
        Ok(Self {
            ffmpeg: tools::inspect("FFmpeg", ffmpeg_executable)?,
            ffprobe: tools::inspect("ffprobe", ffprobe_executable)?,
            profiles: export::export_profiles(),
        })
    }
}

/// External executables a package adapter must preflight before durable work.
#[derive(Clone, Copy, Debug)]
pub struct PackagePreflightRequest<'a> {
    /// FFmpeg executable selected by validated configuration.
    pub ffmpeg_executable: &'a std::path::Path,
    /// ffprobe executable selected by validated configuration.
    pub ffprobe_executable: &'a std::path::Path,
}

/// Inputs needed to reconcile package state before synthesis begins.
#[derive(Debug)]
pub struct PackagePrepareRequest<'a> {
    /// Canonical managed workspace root.
    pub workspace: &'a std::path::Path,
    /// Validated lesson and provisional job identity.
    pub job_id: &'a str,
    /// Deterministic plan whose package may already be selected.
    pub plan: &'a RenderPlan,
}

/// Inputs consumed to create or select one immutable package.
#[derive(Debug)]
pub struct PackageWriteRequest<'a> {
    /// Canonical managed workspace root.
    pub workspace: &'a std::path::Path,
    /// Validated lesson and provisional job identity.
    pub job_id: &'a str,
    /// Deterministic render plan.
    pub plan: &'a RenderPlan,
    /// Validated immutable cache artifacts in plan order.
    pub cached_artifacts: &'a [ValidatedCachedArtifact],
}

/// Immutable package paths and the identity selected for consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePublication {
    /// Immutable directory holding the selected preview generation.
    pub package_dir: PathBuf,
    /// Atomic record selecting the immutable package.
    pub publication_record: PathBuf,
    /// Canonical lossless master WAV.
    pub master_wav: PathBuf,
    /// M4A derived independently from the master WAV.
    pub m4a: PathBuf,
    /// Manifest recording artifacts and tool provenance.
    pub manifest: PathBuf,
    /// Content identities retained in provisional job state.
    pub identity: SelectedPackageIdentity,
}

/// Preflighted package generation and immutable selection boundary.
pub trait PreparedPackageWriter: std::fmt::Debug + Send + Sync {
    /// Reconciles durable package state before cache or worker work begins.
    ///
    /// # Errors
    ///
    /// [`BuildError::DurableState`] when a journal, selected package, manifest,
    /// or checksum cannot be trusted, [`BuildError::ManagedPath`] when a path
    /// leaves its managed root, or [`BuildError::Io`] when durable state cannot
    /// be read.
    fn prepare(&self, request: &PackagePrepareRequest<'_>) -> Result<(), BuildError>;

    /// Reuses the matching selected package or writes and selects a new one.
    ///
    /// # Errors
    ///
    /// [`BuildError::Cache`] for artifact/plan disagreement,
    /// [`BuildError::ManagedPath`] for containment failure,
    /// [`BuildError::Audio`] for PCM assembly failure, [`BuildError::Tool`] for
    /// encoding or probing failure, [`BuildError::DurableState`] for unsafe
    /// publication state, or [`BuildError::Io`] for filesystem failure.
    fn write(&self, request: &PackageWriteRequest<'_>) -> Result<PackagePublication, BuildError>;
}

/// Tool preflight boundary that produces one prepared package writer.
pub trait PackageWriter: Send + Sync {
    /// Preflights adapter dependencies without creating durable build state.
    ///
    /// # Errors
    ///
    /// [`BuildError::Tool`] when a required executable cannot be resolved,
    /// identified, or supervised safely.
    fn preflight(
        &self,
        request: &PackagePreflightRequest<'_>,
    ) -> Result<Box<dyn PreparedPackageWriter>, BuildError>;
}

/// Filesystem package adapter used by the walking skeleton.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemPackageWriter;

impl PackageWriter for FileSystemPackageWriter {
    fn preflight(
        &self,
        request: &PackagePreflightRequest<'_>,
    ) -> Result<Box<dyn PreparedPackageWriter>, BuildError> {
        Ok(Box::new(PreparedFileSystemPackageWriter {
            toolchain: PackageToolchain::inspect(
                request.ffmpeg_executable,
                request.ffprobe_executable,
            )?,
        }))
    }
}

#[derive(Clone, Debug)]
struct PreparedFileSystemPackageWriter {
    toolchain: PackageToolchain,
}

impl PreparedPackageWriter for PreparedFileSystemPackageWriter {
    fn prepare(&self, request: &PackagePrepareRequest<'_>) -> Result<(), BuildError> {
        let filesystem = OsDurableFileSystem;
        let roots = preview::roots(request.workspace, request.job_id)?;
        preview::reconcile(&filesystem, &roots, request.job_id)?;
        Ok(())
    }

    fn write(&self, request: &PackageWriteRequest<'_>) -> Result<PackagePublication, BuildError> {
        validate_cached_artifacts(request)?;
        let filesystem = OsDurableFileSystem;
        let roots = preview::roots(request.workspace, request.job_id)?;
        if let Some(package) = preview::current_for_build(
            &roots,
            request.job_id,
            &request.plan.plan_hash,
            &self.toolchain.ffmpeg,
            &self.toolchain.ffprobe,
            &self.toolchain.profiles,
        )? {
            return publication(package);
        }

        let transaction = preview::start_transaction(
            &filesystem,
            &roots,
            request.job_id,
            &request.plan.plan_hash,
            &self.toolchain.ffmpeg,
            &self.toolchain.ffprobe,
            &self.toolchain.profiles,
        )?;
        let master_wav = managed::leaf(&transaction.stage_dir, manifest::MASTER_WAV_NAME)?;
        assembly::assemble(request.cached_artifacts, &master_wav)?;
        let m4a = managed::leaf(&transaction.stage_dir, manifest::M4A_NAME)?;
        let ffmpeg_execution = export::export_m4a(
            &self.toolchain.ffmpeg,
            &self.toolchain.profiles.ffmpeg,
            &master_wav,
            &m4a,
        )?;
        let ffprobe_execution = export::probe_m4a(
            &self.toolchain.ffprobe,
            &self.toolchain.profiles.ffprobe,
            &m4a,
        )?;
        let manifest_path = managed::leaf(&transaction.stage_dir, manifest::MANIFEST_NAME)?;
        manifest::write(
            &filesystem,
            &manifest_path,
            manifest::ManifestRecords {
                lesson_id: request.job_id,
                plan_hash: &request.plan.plan_hash,
                segments: request.cached_artifacts,
                master_wav: &master_wav,
                m4a: &m4a,
                tools: manifest::ToolRecords {
                    ffmpeg: &self.toolchain.ffmpeg,
                    ffmpeg_execution: &ffmpeg_execution,
                    ffprobe: &self.toolchain.ffprobe,
                    ffprobe_execution: &ffprobe_execution,
                },
            },
        )?;
        publication(preview::publish_transaction(
            &filesystem,
            &roots,
            &transaction,
        )?)
    }
}

fn validate_cached_artifacts(request: &PackageWriteRequest<'_>) -> Result<(), BuildError> {
    if request.cached_artifacts.len() != request.plan.segments.len() {
        return Err(CacheError::PackageArtifactCountMismatch {
            found: request.cached_artifacts.len(),
            required: request.plan.segments.len(),
        }
        .into());
    }

    for (position, (artifact, segment)) in request
        .cached_artifacts
        .iter()
        .zip(&request.plan.segments)
        .enumerate()
    {
        if artifact.segment_id != segment.id
            || artifact.cache_key != segment.cache_key
            || artifact.pause_after_ms != segment.pause_after_ms
        {
            return Err(CacheError::PackageArtifactPlanMismatch {
                mismatch: Box::new(PackageArtifactMismatch {
                    position,
                    recorded_segment_id: artifact.segment_id.clone(),
                    recorded_cache_key: artifact.cache_key.clone(),
                    recorded_pause_after_ms: artifact.pause_after_ms,
                    required_segment_id: segment.id.clone(),
                    required_cache_key: segment.cache_key.clone(),
                    required_pause_after_ms: segment.pause_after_ms,
                }),
            }
            .into());
        }
    }

    let cache_root_path = request.workspace.join("cache");
    let cache_root =
        fs::canonicalize(&cache_root_path).map_err(|error| io_error(&cache_root_path, error))?;
    if !cache_root.starts_with(request.workspace) {
        return Err(ManagedPathError::ManagedPathEscape {
            path: cache_root,
            root: request.workspace.to_path_buf(),
        }
        .into());
    }
    for artifact in request.cached_artifacts {
        let entry_dir = fs::canonicalize(&artifact.entry_dir)
            .map_err(|error| io_error(&artifact.entry_dir, error))?;
        if !entry_dir.starts_with(&cache_root) {
            return Err(ManagedPathError::ManagedPathEscape {
                path: entry_dir,
                root: cache_root,
            }
            .into());
        }
        let audio_path = fs::canonicalize(&artifact.audio_path)
            .map_err(|error| io_error(&artifact.audio_path, error))?;
        if !audio_path.starts_with(&entry_dir) {
            return Err(ManagedPathError::ManagedPathEscape {
                path: audio_path,
                root: entry_dir,
            }
            .into());
        }
    }
    Ok(())
}

fn publication(package: preview::PublishedPackage) -> Result<PackagePublication, BuildError> {
    let manifest_blake3 = cache::hash_file(&package.manifest)?;
    Ok(PackagePublication {
        package_dir: package.package_dir,
        publication_record: package.publication_record,
        master_wav: package.master_wav,
        m4a: package.m4a,
        manifest: package.manifest,
        identity: SelectedPackageIdentity {
            package_id: manifest_blake3.clone(),
            manifest_blake3,
        },
    })
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use study_tts_core::ValidatedLesson;
    use tempfile::TempDir;

    use super::*;

    fn plan() -> RenderPlan {
        let lesson = ValidatedLesson::from_json(include_bytes!(
            "../../../fixtures/lessons/e0-s0-two-segment.json"
        ))
        .expect("validate package test lesson");
        RenderPlan::for_lesson(&lesson, "package-test-executor")
    }

    fn artifact(
        segment: &study_tts_core::PlannedSegment,
        entry_dir: PathBuf,
        audio_path: PathBuf,
    ) -> ValidatedCachedArtifact {
        ValidatedCachedArtifact {
            segment_id: segment.id.clone(),
            cache_key: segment.cache_key.clone(),
            entry_dir,
            audio_path,
            audio_blake3: blake3::hash(b"audio").to_hex().to_string(),
            frames: 1,
            pause_after_ms: segment.pause_after_ms,
        }
    }

    #[test]
    fn t1_e0_package_artifact_count_must_match_the_plan() {
        let workspace = TempDir::new().expect("create package workspace");
        let plan = plan();

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &plan,
            cached_artifacts: &[],
        })
        .expect_err("missing package artifacts must be refused");

        assert!(matches!(
            error,
            BuildError::Cache(CacheError::PackageArtifactCountMismatch {
                found: 0,
                required: 2,
            })
        ));
    }

    #[test]
    fn t1_e0_package_artifact_identity_must_match_its_plan_position() {
        let workspace = TempDir::new().expect("create package workspace");
        let plan = plan();
        let artifact = artifact(
            &plan.segments[1],
            workspace.path().join("unused-entry"),
            workspace.path().join("unused.wav"),
        );

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &RenderPlan {
                lesson_id: plan.lesson_id.clone(),
                plan_hash: plan.plan_hash.clone(),
                segments: vec![plan.segments[0].clone()],
            },
            cached_artifacts: &[artifact],
        })
        .expect_err("an artifact from another plan position must be refused");

        assert!(matches!(
            error,
            BuildError::Cache(CacheError::PackageArtifactPlanMismatch { mismatch })
                if mismatch.position == 0
        ));
    }

    #[test]
    fn t4_e0_package_artifact_paths_must_remain_under_the_workspace_cache() {
        let workspace = TempDir::new().expect("create package workspace");
        fs::create_dir(workspace.path().join("cache")).expect("create cache root");
        let outside = TempDir::new().expect("create outside artifact root");
        let audio_path = outside.path().join("audio.wav");
        fs::write(&audio_path, b"audio").expect("write outside artifact");
        let plan = plan();
        let artifact = artifact(&plan.segments[0], outside.path().to_path_buf(), audio_path);
        let one_segment_plan = RenderPlan {
            lesson_id: plan.lesson_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            segments: vec![plan.segments[0].clone()],
        };

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &one_segment_plan,
            cached_artifacts: &[artifact],
        })
        .expect_err("an artifact outside the managed cache must be refused");

        assert!(matches!(
            error,
            BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn t4_e0_package_cache_root_cannot_resolve_outside_the_workspace() {
        let workspace = TempDir::new().expect("create package workspace");
        let outside = TempDir::new().expect("create outside cache root");
        symlink(outside.path(), workspace.path().join("cache")).expect("link cache root outside");
        let audio_path = outside.path().join("audio.wav");
        fs::write(&audio_path, b"audio").expect("write outside artifact");
        let plan = plan();
        let artifact = artifact(&plan.segments[0], outside.path().to_path_buf(), audio_path);
        let one_segment_plan = RenderPlan {
            lesson_id: plan.lesson_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            segments: vec![plan.segments[0].clone()],
        };

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &one_segment_plan,
            cached_artifacts: &[artifact],
        })
        .expect_err("a cache root outside the workspace must be refused");

        assert!(matches!(
            error,
            BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
        ));
    }
}
