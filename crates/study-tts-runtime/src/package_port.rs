//! Master-first package writing and immutable preview-selection port.
//!
//! The filesystem adapter delegates to the walking skeleton's Rust PCM,
//! FFmpeg, ffprobe, manifest, journal, and atomic-selection implementations.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records its consumers,
//! fake, identity effects, and G1 real-path parity requirement.

use std::{
    collections::{BTreeMap, btree_map::Entry},
    fs,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use study_tts_core::{PlannedSegment, RenderPlan, SelectedPackageIdentity};
use thiserror::Error;

use crate::{
    BuildError, CacheError, ManagedPathError, PackageArtifactMismatch, assembly, audio_edges,
    cache::ValidatedCachedArtifact,
    durable::OsDurableFileSystem,
    durable::write_json_atomically,
    export::{self, EncodedFormat, ExportProfiles, PackagedAudio},
    io_error, managed,
    manifest::{self, RecordedExecution, RecordedTool},
    preview,
    run_report::{JoinFinding, Measured, RUN_REPORT_NAME, RunReport, Unavailable},
    timeline,
    tools::{self, ToolIdentity},
};

/// Mode every file inside a published package carries.
///
/// Not a policy this project invented: `tempfile` creates the master, both
/// exports, and the manifest `0600`, and this is that value written down so the
/// three documents `timeline` renders match rather than inheriting a umask.
/// `t4_e1_every_package_file_is_owner_only` holds all eight to it.
#[cfg(unix)]
const PACKAGE_FILE_MODE: u32 = 0o600;

/// Mirrors the package version in the E0-S4 provisional contract baseline.
///
/// Raised to `3.0` by E2-S4: [`PreparedPackageWriter::write`] returns
/// [`PackageWriteOutcome`] rather than a bare [`PackagePublication`], which
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
/// puts under **Breaking contract**. The record is
/// `docs/architecture/E2-S4-INTERFACE-CHANGE-002.md`, and the migration it
/// defines is empty for this Rust constant: it reaches no cache key, manifest
/// field, published schema, or durable document. The report-bearing manifest's
/// separate package-identity effect is recorded in that change record.
pub const PACKAGE_WRITER_CONTRACT_VERSION: &str = "e0.package-writer.3.0";

#[derive(Clone, Debug)]
struct PackageToolchain {
    ffmpeg: ToolIdentity,
    ffprobe: ToolIdentity,
    profiles: ExportProfiles,
    /// The encoder inventory this build ran, kept so the manifest can record
    /// it.
    ///
    /// Preflight is a real FFmpeg execution and it is what decided the build
    /// could proceed, so a manifest that omitted it would describe a package
    /// produced by a toolchain nobody had checked. It also keeps the recorded
    /// argument profiles complete, which is what `manifest::tools_match`
    /// compares a rebuild against.
    encoder_preflight: export::ToolExecution,
}

impl PackageToolchain {
    /// Resolves both binaries and proves FFmpeg can encode every format.
    ///
    /// The encoder inventory is checked here, inside preflight, and not at the
    /// point of use: `tools::inspect` reports only the first line of
    /// `-version`, which says nothing about which encoders were compiled in, so
    /// without this an FFmpeg lacking `libmp3lame` would be discovered after
    /// the whole lesson had been synthesized.
    fn inspect(ffmpeg_executable: &Path, ffprobe_executable: &Path) -> Result<Self, BuildError> {
        // Both binaries are resolved before either is *run*: `tools::inspect`
        // spawns `-version`, so resolving inside it would have started FFmpeg
        // before discovering that ffprobe is absent. Resolution touches the
        // filesystem only.
        let ffmpeg_path = tools::resolve("FFmpeg", ffmpeg_executable)?;
        let ffprobe_path = tools::resolve("ffprobe", ffprobe_executable)?;
        let ffmpeg = tools::identify("FFmpeg", ffmpeg_path)?;
        let ffprobe = tools::identify("ffprobe", ffprobe_path)?;
        let profiles = export::export_profiles();
        let encoder_preflight =
            export::preflight_encoder(&ffmpeg, &profiles.ffmpeg_encoders, export::MP3_ENCODER)?;
        Ok(Self {
            ffmpeg,
            ffprobe,
            profiles,
            encoder_preflight,
        })
    }
}

/// External executables a package adapter must preflight before durable work.
#[derive(Clone, Copy, Debug)]
pub struct PackagePreflightRequest<'a> {
    /// FFmpeg executable selected by validated configuration.
    pub ffmpeg_executable: &'a Path,
    /// ffprobe executable selected by validated configuration.
    pub ffprobe_executable: &'a Path,
}

/// Inputs needed to reconcile package state before synthesis begins.
#[derive(Debug)]
pub struct PackagePrepareRequest<'a> {
    /// Canonical managed workspace root.
    pub workspace: &'a Path,
    /// Validated lesson and provisional job identity.
    pub job_id: &'a str,
    /// Deterministic plan whose package may already be selected.
    pub plan: &'a RenderPlan,
}

/// Inputs consumed to create or select one immutable package.
#[derive(Debug)]
pub struct PackageWriteRequest<'a> {
    /// Canonical managed workspace root.
    pub workspace: &'a Path,
    /// Validated lesson and provisional job identity.
    pub job_id: &'a str,
    /// Deterministic render plan.
    pub plan: &'a RenderPlan,
    /// Validated immutable cache artifacts in plan order.
    pub cached_artifacts: &'a [ValidatedCachedArtifact],
    /// What the build measured before the package stage began.
    ///
    /// Passed in rather than assembled here because the manifest can only
    /// checksum a document that already exists, and the manifest is written
    /// inside this call. The writer fills in the two durations it measures and
    /// seals the result.
    pub run_report: &'a RunReport,
    /// Monotonic start of the build or resume operation.
    pub run_started: Instant,
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
    /// MP3 derived independently from the master WAV.
    pub mp3: PathBuf,
    /// Readable speaker-labelled transcript.
    pub transcript: PathBuf,
    /// Segment-level WebVTT captions.
    pub captions: PathBuf,
    /// FFMETADATA chapter source.
    pub chapters: PathBuf,
    /// Manifest recording artifacts and tool provenance.
    pub manifest: PathBuf,
    /// Content identities retained in provisional job state.
    pub identity: SelectedPackageIdentity,
}

/// What producing one package cost, beside the package itself.
///
/// ADR-0001 §14 requires "assembly and encoding durations"; the loudness pass
/// between them is here because it rewrites the master's bytes, and the line
/// this set is drawn on is production rather than validation. Each is
/// [`Measured`] rather than a bare number because a write that reused an
/// existing package performed none of them, and reporting zero would claim the
/// work happened instantly.
///
/// **The three `ffprobe` validations are outside these deliberately.** They
/// prove the outputs rather than produce them, so the three durations account
/// for every byte-producing span of the package write and nothing else.
/// `docs/observability/RUN-REPORT-FIELDS.md` says so where a reader will look.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageTimings {
    /// Time assembling segment audio into the canonical master.
    pub assembly_micros: Measured,
    /// Time normalizing that master's loudness.
    pub normalize_micros: Measured,
    /// Time encoding both lossy outputs from that master.
    pub encode_micros: Measured,
}

impl PackageTimings {
    /// Writes these durations into the report about to be sealed.
    ///
    /// One move rather than a field assignment each, so a duration added here
    /// cannot be measured and then left out of the document.
    pub(crate) fn apply(self, report: &mut RunReport) {
        report.assembly_micros = self.assembly_micros;
        report.normalize_micros = self.normalize_micros;
        report.encode_micros = self.encode_micros;
    }
}

impl PackageTimings {
    /// What a write has measured before any package stage runs.
    #[must_use]
    pub const fn not_reached() -> Self {
        let absent = Measured::Unavailable {
            reason: Unavailable::StageNotReached,
        };
        Self {
            assembly_micros: absent,
            normalize_micros: absent,
            encode_micros: absent,
        }
    }

    /// What a write that selected an existing package spent: nothing.
    ///
    /// An earlier write produced the package, so this call assembled and
    /// encoded none of it. That is a different statement from a duration of
    /// zero, which is why all three fields are [`Measured`] rather than
    /// numbers.
    /// Public because the fake writer in `study-tts-testkit` reports the same
    /// thing, and one definition of it is what keeps the two agreeing.
    #[must_use]
    pub const fn reused() -> Self {
        let reused = Measured::Unavailable {
            reason: Unavailable::ReusedFromCache,
        };
        Self {
            assembly_micros: reused,
            normalize_micros: reused,
            encode_micros: reused,
        }
    }
}

/// Whether a package write produced new bytes or selected existing bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageDisposition {
    /// This call produced and atomically published a new package.
    Published,
    /// This call selected an already-published matching package.
    Reused,
}

/// A selected package, the current report, and how the package was obtained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageWriteOutcome {
    /// The immutable package selected by this call.
    pub publication: PackagePublication,
    /// The report describing this call.
    pub run_report: RunReport,
    /// Whether this call published or reused the package.
    pub disposition: PackageDisposition,
}

/// A package failure and the stage timings observed before it returned.
#[derive(Debug, Error)]
#[error("{source}")]
pub struct PackageWriteFailure {
    /// Original build failure, preserved without category collapse.
    #[source]
    pub source: Box<BuildError>,
    /// Package-stage measurements accumulated through the failing operation.
    pub timings: PackageTimings,
}

impl From<BuildError> for PackageWriteFailure {
    fn from(source: BuildError) -> Self {
        Self {
            source: Box::new(source),
            timings: PackageTimings::not_reached(),
        }
    }
}

/// `elapsed` in microseconds, saturated rather than wrapped.
///
/// A package write long enough to overflow would run for some five hundred
/// thousand years, so the saturation guards a nonsense clock rather than a
/// reachable case.
fn observed(elapsed: Duration) -> u64 {
    duration_micros(elapsed)
}

fn duration_micros(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX)
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
    fn write(
        &self,
        request: &PackageWriteRequest<'_>,
    ) -> Result<PackageWriteOutcome, PackageWriteFailure>;
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

    fn write(
        &self,
        request: &PackageWriteRequest<'_>,
    ) -> Result<PackageWriteOutcome, PackageWriteFailure> {
        let mut timings = PackageTimings::not_reached();
        self.write_with_timings(request, &mut timings)
            .map_err(|source| PackageWriteFailure {
                source: Box::new(source),
                timings,
            })
    }
}

impl PreparedFileSystemPackageWriter {
    fn write_with_timings(
        &self,
        request: &PackageWriteRequest<'_>,
        timings: &mut PackageTimings,
    ) -> Result<PackageWriteOutcome, BuildError> {
        validate_cached_artifacts(request)?;
        let filesystem = OsDurableFileSystem;
        let roots = preview::roots(request.workspace, request.job_id)?;
        if let Some(package) = preview::current_for_build(
            &roots,
            request.job_id,
            request.plan,
            &self.toolchain.ffmpeg,
            &self.toolchain.ffprobe,
            &self.toolchain.profiles,
        )? {
            // Nothing was assembled and nothing encoded: this call selected
            // a package an earlier one produced, and the report sealed beside
            // it already describes the build that made those bytes.
            *timings = PackageTimings::reused();
            let mut run_report = request.run_report.clone();
            timings.apply(&mut run_report);
            run_report.join_findings = advisory_join_findings(&assess_replacement_joins(request)?);
            run_report.wall_micros = duration_micros(request.run_started.elapsed());
            return Ok(PackageWriteOutcome {
                publication: publication(package)?,
                run_report,
                disposition: PackageDisposition::Reused,
            });
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
        let stage = &transaction.stage_dir;
        let master_wav = managed::leaf(stage, manifest::MASTER_WAV_NAME)?;
        // Rust PCM assembly only. The three timeline documents written below
        // depend on `assembled` but touch no audio, and the loudness pass and
        // the `ffprobe` validations are FFmpeg work this figure excludes —
        // `docs/observability/RUN-REPORT-FIELDS.md` names what is left out.
        let assembled = measure(&mut timings.assembly_micros, || {
            assembly::assemble(request.cached_artifacts, &master_wav)
        })?;

        // Normalized before the master is probed and before either encode, so
        // the bytes ffprobe validates and the bytes both lossy outputs derive
        // from are the published ones. ADR-0001 §13.5 has M4A and MP3 derive
        // independently from the master, which is what makes normalizing the
        // master once the whole loudness decision rather than three.
        //
        // The references are provisional under `ADR-0001-D012`.
        // Two FFmpeg passes, measure then apply, both unconditional. This is
        // plausibly the largest single span in the package path, which is why
        // it is measured rather than folded into the encode figure it precedes.
        let normalization = measure(&mut timings.normalize_micros, || {
            export::normalize_master(
                &self.toolchain.ffmpeg,
                &self.toolchain.profiles,
                &master_wav,
                assembled.total_frames,
            )
        })?;

        // Written before the encodes, so a package that reaches the encoder has
        // its whole text surface already staged. All three are ordinary files
        // inside the staged directory: the transaction is the atomicity unit,
        // and nothing here becomes authoritative until
        // `preview::publish_transaction` synchronizes and renames it.
        let plan_segments = &request.plan.segments;
        for (name, document) in [
            (
                manifest::TRANSCRIPT_NAME,
                timeline::transcript(plan_segments),
            ),
            (
                manifest::CAPTIONS_NAME,
                timeline::captions(plan_segments, &assembled),
            ),
            (
                manifest::CHAPTERS_NAME,
                timeline::chapters(plan_segments, &assembled),
            ),
        ] {
            let path = managed::leaf(stage, name)?;
            write_package_document(&path, &document)?;
        }

        // Both exports are derived from `master_wav` and never from each other,
        // which is ADR-0001 §13.5's rule that a lossy output is never the
        // source of another export.
        let mut performed = Vec::with_capacity(8);
        performed.push((
            RecordedTool::Ffmpeg,
            self.toolchain.encoder_preflight.clone(),
        ));
        performed.extend(
            normalization
                .into_iter()
                .map(|execution| (RecordedTool::Ffmpeg, execution)),
        );
        performed.push((
            RecordedTool::Ffprobe,
            export::probe(
                &self.toolchain.ffprobe,
                &self.toolchain.profiles.ffprobe,
                PackagedAudio::MasterWav,
                &master_wav,
            )?,
        ));
        // Summed over both formats and covering the encodes only: each
        // format's `ffprobe` validation below is what proves the output, not
        // what produced it.
        for (format, name) in [
            (EncodedFormat::M4a, manifest::M4A_NAME),
            (EncodedFormat::Mp3, manifest::MP3_NAME),
        ] {
            let destination = managed::leaf(stage, name)?;
            performed.push((
                RecordedTool::Ffmpeg,
                measure(&mut timings.encode_micros, || {
                    export::encode(
                        &self.toolchain.ffmpeg,
                        &self.toolchain.profiles,
                        format,
                        &master_wav,
                        &destination,
                    )
                })?,
            ));
            performed.push((
                RecordedTool::Ffprobe,
                export::probe(
                    &self.toolchain.ffprobe,
                    &self.toolchain.profiles.ffprobe,
                    format.packaged(),
                    &destination,
                )?,
            ));
        }
        let executions: Vec<RecordedExecution<'_>> = performed
            .iter()
            .map(|(tool, execution)| RecordedExecution {
                tool: *tool,
                execution,
            })
            .collect();

        let joins = assess_replacement_joins(request)?;
        let mut sealed = request.run_report.clone();
        timings.apply(&mut sealed);
        sealed.join_findings = advisory_join_findings(&joins);
        sealed.wall_micros = duration_micros(request.run_started.elapsed());
        let report_path = managed::leaf(stage, RUN_REPORT_NAME)?;
        write_json_atomically(&filesystem, &report_path, &sealed)?;

        let manifest_path = managed::leaf(stage, manifest::MANIFEST_NAME)?;
        manifest::write(
            &filesystem,
            &manifest_path,
            manifest::ManifestRecords {
                lesson_id: request.job_id,
                build_attempt: request.run_report.build_attempt,
                plan: request.plan,
                joins: &joins,
                segments: request.cached_artifacts,
                timeline: &assembled,
                package_dir: stage,
                tools: manifest::ToolRecords {
                    ffmpeg: &self.toolchain.ffmpeg,
                    ffprobe: &self.toolchain.ffprobe,
                    executions: &executions,
                },
            },
        )?;
        Ok(PackageWriteOutcome {
            publication: publication(preview::publish_transaction(
                &filesystem,
                &roots,
                &transaction,
            )?)?,
            run_report: sealed,
            disposition: PackageDisposition::Published,
        })
    }
}

fn measure<T>(
    measurement: &mut Measured,
    operation: impl FnOnce() -> Result<T, BuildError>,
) -> Result<T, BuildError> {
    let started = Instant::now();
    let result = operation();
    let elapsed = observed(started.elapsed());
    let prior = match *measurement {
        Measured::Observed { value } => value,
        Measured::Unavailable { .. } => 0,
    };
    *measurement = Measured::Observed {
        value: prior.saturating_add(elapsed),
    };
    result
}

fn advisory_join_findings(joins: &[manifest::RecordedJoin<'_>]) -> Vec<JoinFinding> {
    joins
        .iter()
        .filter(|join| {
            join.continuity.provisional_tolerance() == audio_edges::JoinTolerance::Outside
        })
        .map(|join| JoinFinding {
            earlier_segment_id: join.earlier_segment_id.to_owned(),
            later_segment_id: join.later_segment_id.to_owned(),
        })
        .collect()
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

/// Writes one text document into a package at the mode every other package
/// file already carries.
///
/// `fs::write` would create it `0666 & ~umask`, which is 644 under the common
/// default: the transcript and the captions carry the whole authored lesson in
/// plaintext, and they would be the only world-readable files in a package
/// whose `release_status` is `private_preview`. It would also make the mode
/// vary with the operator's umask, in a package whose other four files are
/// created `0600` by `tempfile` on every machine.
///
/// # Errors
///
/// [`crate::IoError::FileSystem`] when the document cannot be created or
/// written.
#[cfg(unix)]
fn write_package_document(path: &Path, document: &str) -> Result<(), BuildError> {
    use std::os::unix::fs::OpenOptionsExt as _;

    // `create_new`, not `create`: a mode is applied only when the file is made,
    // so reusing an existing file would silently keep whatever mode it had.
    // `preview::start_transaction` quarantines any stage that already exists
    // and creates a fresh directory, so nothing here should ever be present.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(PACKAGE_FILE_MODE)
        .open(path)
        .map_err(|error| io_error(path, error))?;
    file.write_all(document.as_bytes())
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn write_package_document(path: &Path, document: &str) -> Result<(), BuildError> {
    fs::write(path, document).map_err(|error| io_error(path, error))
}

fn publication(package: preview::PublishedPackage) -> Result<PackagePublication, BuildError> {
    let manifest_blake3 = package.manifest_blake3;
    Ok(PackagePublication {
        package_dir: package.package_dir,
        publication_record: package.publication_record,
        master_wav: package.master_wav,
        m4a: package.m4a,
        mp3: package.mp3,
        transcript: package.transcript,
        captions: package.captions,
        chapters: package.chapters,
        manifest: package.manifest,
        identity: SelectedPackageIdentity {
            package_id: manifest_blake3.clone(),
            manifest_blake3,
        },
    })
}

/// Measures both joins of every segment this build rendered at a later take.
///
/// ADR-0001 §11.4: "After a mid-lesson retake, automated loudness and
/// speaking-rate comparisons plus a listening check evaluate both joins." A
/// segment at the start or end of a lesson has one join rather than two, which
/// is a fact about the lesson and recorded as such rather than refused. Two
/// adjacent replacements share the join between them, and it is measured once.
///
/// The measurement is recorded and never thresholded. ADR-0003 owns the
/// join-discontinuity threshold, it is `Proposed`, and its calibration table
/// records the value as `Pending`.
///
/// Planned segments are zipped with cached artifacts rather than indexed by
/// position: `validate_cached_artifacts` has already refused a list that is not
/// this plan's, artifact for artifact, and zipping keeps that precondition from
/// becoming a panic if it is ever weakened.
///
/// # Errors
///
/// [`crate::IoError::AudioAt`] when a cache entry's audio cannot be opened or
/// decoded. The entries have already been validated by
/// `validate_cached_artifacts` and re-hashed by `assembly::assemble`, so a
/// failure here is a filesystem fault rather than a rejected entry.
fn assess_replacement_joins<'a>(
    request: &'a PackageWriteRequest<'a>,
) -> Result<Vec<manifest::RecordedJoin<'a>>, BuildError> {
    let paired: Vec<(&PlannedSegment, &ValidatedCachedArtifact)> = request
        .plan
        .segments
        .iter()
        .zip(request.cached_artifacts)
        .collect();
    if paired
        .iter()
        .all(|(segment, _)| segment.take == study_tts_core::BASE_TAKE)
    {
        return Ok(Vec::new());
    }

    // Read once per segment rather than once per join, because the segment
    // between two replacements is a side of both. Each entry is bounded by
    // `audio_edges::MAX_SEGMENT_AUDIO_MS`, which cache acceptance has already
    // held it to.
    let mut samples: BTreeMap<usize, Vec<f32>> = BTreeMap::new();
    for (index, window) in paired.windows(2).enumerate() {
        if let [(earlier, _), (later, _)] = window
            && (earlier.take != study_tts_core::BASE_TAKE
                || later.take != study_tts_core::BASE_TAKE)
        {
            for position in [index, index + 1] {
                if let Entry::Vacant(slot) = samples.entry(position)
                    && let Some((_, artifact)) = paired.get(position)
                {
                    slot.insert(read_segment_samples(artifact)?);
                }
            }
        }
    }

    let side = |position: usize, segment: &PlannedSegment| {
        samples.get(&position).map(|samples| audio_edges::JoinSide {
            samples,
            sample_rate: study_tts_core::CANONICAL_SAMPLE_RATE,
            characters: segment.spoken_text.chars().count(),
        })
    };
    let mut recorded = Vec::new();
    for (index, window) in paired.windows(2).enumerate() {
        if let [(earlier, _), (later, _)] = window
            && let (Some(earlier_side), Some(later_side)) =
                (side(index, earlier), side(index + 1, later))
        {
            recorded.push(manifest::RecordedJoin {
                earlier_segment_id: &earlier.id,
                later_segment_id: &later.id,
                continuity: audio_edges::assess_join(&earlier_side, &later_side),
            });
        }
    }
    Ok(recorded)
}

/// Reads one validated cache entry's canonical samples.
///
/// # Errors
///
/// [`crate::IoError::AudioAt`] when the entry cannot be opened or decoded.
fn read_segment_samples(segment: &ValidatedCachedArtifact) -> Result<Vec<f32>, BuildError> {
    let mut reader = hound::WavReader::open(segment.audio_path())
        .map_err(|error| crate::audio_error(segment.audio_path(), error))?;
    reader
        .samples::<f32>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| crate::audio_error(segment.audio_path(), error))
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use study_tts_core::ValidatedLesson;
    use tempfile::TempDir;

    use super::*;

    fn plan() -> RenderPlan {
        let lesson = ValidatedLesson::from_json(
            "fixtures/lessons/e0-s0-two-segment.json",
            include_bytes!("../../../fixtures/lessons/e0-s0-two-segment.json"),
        )
        .expect("validate package test lesson");
        // Every speaker the fixture declares, because planning refuses a
        // lesson whose voices were not resolved.
        let conditioning = lesson
            .speakers()
            .keys()
            .map(|speaker| {
                (
                    speaker.clone(),
                    blake3::hash(b"package-port-conditioning").into(),
                )
            })
            .collect();
        RenderPlan::for_lesson(
            &lesson,
            &crate::synthesis::sample_descriptor()
                .synthesis_context(lesson.language().clone(), conditioning),
        )
        .expect("the package test context resolves every speaker")
    }

    fn artifact(
        segment: &PlannedSegment,
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
    fn t1_e2_a_failing_package_stage_keeps_its_elapsed_observation() {
        let mut measurement = Measured::Unavailable {
            reason: Unavailable::StageNotReached,
        };
        let error = measure::<()>(&mut measurement, || {
            Err(io_error(
                Path::new("lesson.m4a"),
                std::io::Error::other("injected stage failure"),
            ))
        })
        .expect_err("the injected stage failure must surface");

        assert!(matches!(error, BuildError::Io(_)));
        assert!(matches!(measurement, Measured::Observed { .. }));
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
            run_report: &RunReport::unmeasured(
                "job",
                "job",
                &"0".repeat(64),
                1,
                crate::ReportCompletion::Complete,
            ),
            run_started: Instant::now(),
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
                schema_version: study_tts_core::PLAN_SCHEMA_VERSION,
                lesson_id: plan.lesson_id.clone(),
                plan_hash: plan.plan_hash.clone(),
                take_selection_source: plan.take_selection_source,
                segments: vec![plan.segments[0].clone()],
            },
            cached_artifacts: &[artifact],
            run_report: &RunReport::unmeasured(
                "job",
                "job",
                &"0".repeat(64),
                1,
                crate::ReportCompletion::Complete,
            ),
            run_started: Instant::now(),
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
            schema_version: study_tts_core::PLAN_SCHEMA_VERSION,
            lesson_id: plan.lesson_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            take_selection_source: plan.take_selection_source,
            segments: vec![plan.segments[0].clone()],
        };

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &one_segment_plan,
            cached_artifacts: &[artifact],
            run_report: &RunReport::unmeasured(
                "job",
                "job",
                &"0".repeat(64),
                1,
                crate::ReportCompletion::Complete,
            ),
            run_started: Instant::now(),
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
            schema_version: study_tts_core::PLAN_SCHEMA_VERSION,
            lesson_id: plan.lesson_id.clone(),
            plan_hash: plan.plan_hash.clone(),
            take_selection_source: plan.take_selection_source,
            segments: vec![plan.segments[0].clone()],
        };

        let error = validate_cached_artifacts(&PackageWriteRequest {
            workspace: workspace.path(),
            job_id: "job",
            plan: &one_segment_plan,
            cached_artifacts: &[artifact],
            run_report: &RunReport::unmeasured(
                "job",
                "job",
                &"0".repeat(64),
                1,
                crate::ReportCompletion::Complete,
            ),
            run_started: Instant::now(),
        })
        .expect_err("a cache root outside the workspace must be refused");

        assert!(matches!(
            error,
            BuildError::ManagedPath(ManagedPathError::ManagedPathEscape { .. })
        ));
    }
}
