//! `manifest.json`: the record of what a build produced and what produced it.
//!
//! Every value written here is derived rather than restated — the artifact
//! names from the constants `pipeline` writes the files at, the release status
//! from the typed value, the digests from the files themselves. A manifest
//! that could disagree with the build it describes is worse than no manifest,
//! because `validate_production_manifest` gates on what it says.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use study_tts_core::{
    AudioDigest, CANONICAL_SAMPLE_RATE, CacheKey, PlanHash, ReleaseStatus, ToolProfileHash,
};

use crate::{
    BuildError, DurableStateError,
    cache::{ValidatedCachedArtifact, hash_file},
    durable::{DurableFileSystem, write_json_atomically},
    export::{ExportProfiles, ToolExecution},
    managed,
    timeline::{TEXT_RENDERER_VERSION, Timeline},
    tools::ToolIdentity,
};

/// The `schema_version` a `manifest.json` this build writes carries.
///
/// Independent of `CACHE_SCHEMA_VERSION` and the lesson schema: each versions a
/// different document and moves separately. `manifest-v1.schema.json` describes
/// this layout and only this one, because that schema is generated from the one
/// stored Rust shape.
///
/// `1.0` rather than `0.3`: E1-S4 makes the timeline, both lossy exports, the
/// text documents, the renderer that wrote them, and every tool execution
/// required fields, which
/// `docs/governance/INTERFACE-FREEZE-AND-CHANGE-CONTROL.md` §Change classes
/// calls a **Breaking contract** and answers with a major increment.
///
/// `-skeleton` rather than a bare `1.0`, for the reason
/// `crate::schemas::JOB_SCHEMA_VERSION` states about its own document: E2-S3
/// adds loudness normalization to this manifest and E2-S4 adds the run report,
/// so a label claiming a frozen `1.0` would claim a stability those stories are
/// going to break. The suffix says the layout is still provisional; the major
/// says the change was breaking. Both are true.
///
/// `docs/architecture/WALKING-SKELETON.md` names both constants in its
/// provisional package-manifest paragraph, and records why reconciliation still
/// reads the legacy layouts and why only the current one is published.
const CURRENT_MANIFEST_LAYOUT_VERSION: &str = "1.0-skeleton";

/// Milliseconds in one second, for checking a declared pause against frames.
const MILLISECONDS_PER_SECOND: u64 = 1_000;

/// The `schema_version` of the E0 walking-skeleton layout.
///
/// Written before tool argument profiles were recorded. Read so an existing
/// package can be reconciled; never written, and never reusable as a matching
/// tool-profile generation.
const LEGACY_MANIFEST_LAYOUT_VERSION: &str = "0.1-skeleton";

/// The `schema_version` of the E1-S1 layout, which recorded argument profiles.
///
/// Read for the same reason as [`LEGACY_MANIFEST_LAYOUT_VERSION`] and never
/// written. It records one FFmpeg encode and one ffprobe validation, so it
/// describes a package with no MP3, transcript, captions, or chapters and can
/// never satisfy this build's reuse: `validate_package` compares the whole
/// profile set, and a package missing three of them cannot match it.
const SKELETON_MANIFEST_LAYOUT_VERSION: &str = "0.2-skeleton";

/// Name of the assembled master inside a preview directory.
///
/// Owned here because the manifest records these paths; `pipeline` writes the
/// files at the same names. Two literals could drift, leaving the manifest
/// pointing at a file that is not there.
///
/// The six artifact names are ADR-0001 §12.1's `output/` tree and §13.5's
/// output-package list, spelled exactly as those sections spell them. Not
/// §7.1, which is the Rust crate workspace and names no artifact.
pub(crate) const MASTER_WAV_NAME: &str = "lesson.wav";

/// Name of the M4A export inside a preview directory.
pub(crate) const M4A_NAME: &str = "lesson.m4a";

/// Name of the MP3 export inside a preview directory.
pub(crate) const MP3_NAME: &str = "lesson.mp3";

/// Name of the readable speaker-labelled transcript.
pub(crate) const TRANSCRIPT_NAME: &str = "transcript.txt";

/// Name of the segment-level WebVTT captions.
pub(crate) const CAPTIONS_NAME: &str = "transcript.vtt";

/// Name of the FFMETADATA chapter source.
pub(crate) const CHAPTERS_NAME: &str = "chapters.ffmetadata";

/// Name of the manifest itself inside a preview directory.
pub(crate) const MANIFEST_NAME: &str = "manifest.json";

/// Every file a complete package holds besides the manifest, in written order.
///
/// One list rather than six call sites: `preview::publish_transaction`
/// synchronizes exactly these before the package becomes durable, and a file
/// added here without being added there would be published unflushed.
pub(crate) const PACKAGE_ARTIFACT_NAMES: [&str; 6] = [
    MASTER_WAV_NAME,
    M4A_NAME,
    MP3_NAME,
    TRANSCRIPT_NAME,
    CAPTIONS_NAME,
    CHAPTERS_NAME,
];

/// The manifest document, borrowed from the build that produced it.
///
/// Borrowed rather than owned throughout: every value already exists in the
/// completed build, and copying them would create a second version that could
/// disagree with it.
#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: &'static str,
    release_status: ReleaseStatus,
    lesson_id: &'a str,
    plan_hash: &'a PlanHash,
    text_renderer_version: &'static str,
    total_frames: u64,
    segments: Vec<ManifestSegment<'a>>,
    artifacts: Artifacts,
    tools: Tools<'a>,
}

/// One segment as the manifest records it: identity, digest, and position.
///
/// `frames` and `pause_after_ms` describe the cache entry this segment came
/// from; `start_frame` and `pause_frames` describe where it was actually
/// written. The pair is deliberate — a declared pause and a written pause
/// disagreeing is exactly the defect the caption boundaries would inherit.
#[derive(Serialize)]
struct ManifestSegment<'a> {
    segment_id: &'a str,
    cache_key: &'a CacheKey,
    audio_blake3: &'a str,
    frames: u32,
    pause_after_ms: u32,
    start_frame: u64,
    pause_frames: u64,
}

/// The six files a build leaves in its preview directory.
#[derive(Serialize)]
struct Artifacts {
    master_wav: Artifact,
    m4a: Artifact,
    mp3: Artifact,
    transcript: Artifact,
    captions: Artifact,
    chapters: Artifact,
}

/// One produced file, named relative to the preview directory and hashed.
#[derive(Serialize)]
struct Artifact {
    path: &'static str,
    blake3: String,
}

/// The external tools the build shelled out to, and everything they ran.
///
/// Identity and execution are separated because one binary performs several
/// operations: recording a resolved executable once per tool and then listing
/// what each was told to do is what lets a reader see all six invocations —
/// the encoder preflight, the master probe, and an encode and a probe for each
/// export — without six copies of the same version string.
#[derive(Serialize)]
struct Tools<'a> {
    ffmpeg: ToolIdentityRecord<'a>,
    ffprobe: ToolIdentityRecord<'a>,
    executions: Vec<ExecutionRecord<'a>>,
}

/// Which binary ran, as the manifest records it.
#[derive(Serialize)]
struct ToolIdentityRecord<'a> {
    resolved_executable: String,
    version: &'a str,
}

/// Which of the two binaries an execution record belongs to.
///
/// A closed vocabulary rather than a free string, so a manifest naming a third
/// tool is a parse error at the boundary rather than a record nothing reads.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordedTool {
    /// The encoder.
    Ffmpeg,
    /// The prober.
    Ffprobe,
}

impl RecordedTool {
    /// The spelling this project's diagnostics use for this tool.
    fn label(self) -> &'static str {
        match self {
            Self::Ffmpeg => "FFmpeg",
            Self::Ffprobe => "ffprobe",
        }
    }
}

/// One invocation: which tool, and the exact arguments it was given.
///
/// No artifact label: the executed arguments already name the file, and a
/// second derived copy of that could disagree with the list beside it. Which
/// operation ran is read from the arguments and the profile digest, both of
/// which are what actually determined the output.
#[derive(Serialize)]
struct ExecutionRecord<'a> {
    tool: RecordedTool,
    arguments: &'a [String],
    argument_profile_blake3: &'a ToolProfileHash,
}

/// One external-tool invocation a completed build performed.
pub(crate) struct RecordedExecution<'a> {
    /// Which binary ran.
    pub tool: RecordedTool,
    /// What it was told to do, and under which argument profile.
    pub execution: &'a ToolExecution,
}

/// The external tools a build used, as the manifest must record them.
pub(crate) struct ToolRecords<'a> {
    /// Which FFmpeg binary ran.
    pub ffmpeg: &'a ToolIdentity,
    /// Which ffprobe binary ran.
    pub ffprobe: &'a ToolIdentity,
    /// Every invocation, in the order the build performed it.
    pub executions: &'a [RecordedExecution<'a>],
}

/// Everything besides the plan a package must match to be reused.
///
/// Not named for tools, because the renderer is not one: the three text
/// documents are produced by this crate and FFmpeg never sees them.
pub(crate) struct ReuseExpectations<'a> {
    /// FFmpeg binary identity required by this build.
    pub ffmpeg: &'a ToolIdentity,
    /// ffprobe binary identity required by this build.
    pub ffprobe: &'a ToolIdentity,
    /// Every argument profile this build would record.
    pub profiles: &'a ExportProfiles,
    /// Identity of the rules this build would render the text documents by.
    pub text_renderer_version: &'a str,
}

/// Completed build data from which the minimal manifest is derived.
pub(crate) struct ManifestRecords<'a> {
    /// Validated lesson identity.
    pub lesson_id: &'a str,
    /// Deterministic plan identity.
    pub plan_hash: &'a PlanHash,
    /// Selected validated cache segments.
    pub segments: &'a [ValidatedCachedArtifact],
    /// Where each segment was written, in exact frames.
    pub timeline: &'a Timeline,
    /// The staged package directory holding every artifact.
    pub package_dir: &'a Path,
    /// Tool identities and executed arguments.
    pub tools: ToolRecords<'a>,
}

/// Writes `manifest.json` for a completed build.
///
/// Hashes every artifact as it goes, so the recorded digests describe the bytes
/// on disk rather than what the build believed it wrote. Written atomically: a
/// half-written manifest would describe a build that does not exist.
///
/// # Panics
///
/// Never, from any argument. `records.timeline` was produced by
/// `assembly::assemble` over `records.segments`, so the two have the same
/// length and the zip below consumes both in full.
///
/// # Errors
///
/// [`crate::IoError::FileSystem`] if any artifact cannot be read for hashing
/// or the manifest cannot be written; [`crate::IoError::WriteJson`] if
/// serialization fails.
pub(crate) fn write(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    records: ManifestRecords<'_>,
) -> Result<(), BuildError> {
    let artifact = |name: &'static str| -> Result<Artifact, BuildError> {
        Ok(Artifact {
            path: name,
            blake3: hash_file(&managed::leaf(records.package_dir, name)?)?,
        })
    };
    let manifest = Manifest {
        schema_version: CURRENT_MANIFEST_LAYOUT_VERSION,
        // The typed value, not a hand-written spelling of it. A literal here
        // would keep whatever it said if `ReleaseStatus` were ever respelled,
        // and this field is what `validate_production_manifest` gates on.
        release_status: ReleaseStatus::PrivatePreview,
        lesson_id: records.lesson_id,
        plan_hash: records.plan_hash,
        // The constant `timeline` renders from, never a literal: a second
        // spelling here could claim a generation that never produced the
        // documents beside it.
        text_renderer_version: TEXT_RENDERER_VERSION,
        total_frames: records.timeline.total_frames,
        segments: records
            .segments
            .iter()
            .zip(&records.timeline.segments)
            .map(|(segment, written)| ManifestSegment {
                segment_id: &segment.segment_id,
                cache_key: &segment.cache_key,
                audio_blake3: &segment.audio_blake3,
                frames: segment.frames,
                pause_after_ms: segment.pause_after_ms,
                start_frame: written.start_frame,
                pause_frames: written.pause_frames,
            })
            .collect(),
        artifacts: Artifacts {
            master_wav: artifact(MASTER_WAV_NAME)?,
            m4a: artifact(M4A_NAME)?,
            mp3: artifact(MP3_NAME)?,
            transcript: artifact(TRANSCRIPT_NAME)?,
            captions: artifact(CAPTIONS_NAME)?,
            chapters: artifact(CHAPTERS_NAME)?,
        },
        tools: Tools {
            ffmpeg: ToolIdentityRecord {
                resolved_executable: records
                    .tools
                    .ffmpeg
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &records.tools.ffmpeg.version,
            },
            ffprobe: ToolIdentityRecord {
                resolved_executable: records
                    .tools
                    .ffprobe
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &records.tools.ffprobe.version,
            },
            executions: records
                .tools
                .executions
                .iter()
                .map(|recorded| ExecutionRecord {
                    tool: recorded.tool,
                    arguments: &recorded.execution.arguments,
                    argument_profile_blake3: &recorded.execution.argument_profile_blake3,
                })
                .collect(),
        },
    };
    write_json_atomically(filesystem, destination, &manifest)
}

/// Publishes the one layout `manifest-v1.schema.json` describes.
///
/// [`validate_package`] also reads [`LEGACY_MANIFEST_LAYOUT_VERSION`] and
/// [`SKELETON_MANIFEST_LAYOUT_VERSION`], and neither is listed: both carry a
/// different `artifacts` and `tools` shape, and this schema is generated from
/// the current one. A schema admitting a version whose other fields it
/// describes wrongly is worse than one that admits fewer, and the older
/// layouts are read to be reconciled rather than authored against.
///
/// `t3_e1_the_published_manifest_schema_names_every_layout_it_describes` holds
/// this function and [`parse_stored_manifest`] together — so a layout added to
/// the parser cannot leave this schema quietly describing another one.
fn schema_version_json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "string",
        "const": CURRENT_MANIFEST_LAYOUT_VERSION,
    })
}

/// The published schema of the manifest this build writes and reads back.
///
/// Derived from the *stored* shape rather than the borrowed writing shape,
/// because the stored shape is the parse boundary: it is what
/// `deny_unknown_fields` guards, and a schema that described the writer would
/// describe what this build happens to emit rather than what it will accept.
pub(crate) fn current_manifest_schema() -> serde_json::Value {
    serde_json::Value::from(schemars::schema_for!(StoredManifest))
}

/// Strict owned shape used when an immutable package is reconciled or reused.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredManifest {
    #[schemars(schema_with = "schema_version_json_schema")]
    schema_version: String,
    release_status: ReleaseStatus,
    lesson_id: String,
    plan_hash: PlanHash,
    /// Identity of the rules that produced the transcript, captions, and
    /// chapters. A package whose value differs from this build's is rebuilt
    /// rather than reused.
    text_renderer_version: String,
    total_frames: u64,
    segments: Vec<StoredManifestSegment>,
    artifacts: StoredArtifacts,
    tools: StoredTools,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredManifestSegment {
    segment_id: String,
    cache_key: CacheKey,
    audio_blake3: AudioDigest,
    frames: u32,
    pause_after_ms: u32,
    start_frame: u64,
    pause_frames: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredArtifacts {
    master_wav: StoredArtifact,
    m4a: StoredArtifact,
    mp3: StoredArtifact,
    transcript: StoredArtifact,
    captions: StoredArtifact,
    chapters: StoredArtifact,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredArtifact {
    path: String,
    blake3: AudioDigest,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredTools {
    ffmpeg: StoredToolIdentity,
    ffprobe: StoredToolIdentity,
    executions: Vec<StoredExecution>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredToolIdentity {
    resolved_executable: String,
    version: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredExecution {
    tool: RecordedTool,
    arguments: Vec<String>,
    argument_profile_blake3: ToolProfileHash,
}

/// The E0 layout: two artifacts, one encode, one probe, no argument profiles.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct LegacyStoredManifest<T> {
    schema_version: String,
    release_status: ReleaseStatus,
    lesson_id: String,
    plan_hash: PlanHash,
    segments: Vec<LegacyStoredSegment>,
    artifacts: LegacyStoredArtifacts,
    tools: LegacyStoredTools<T>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct LegacyStoredSegment {
    segment_id: String,
    cache_key: CacheKey,
    audio_blake3: AudioDigest,
    frames: u32,
    pause_after_ms: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct LegacyStoredArtifacts {
    master_wav: StoredArtifact,
    m4a: StoredArtifact,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct LegacyStoredTools<T> {
    ffmpeg: T,
    ffprobe: T,
}

/// The E1-S1 layout's tool record, which requires an argument profile.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct SkeletonStoredToolUse {
    resolved_executable: String,
    version: String,
    arguments: Vec<String>,
    argument_profile_blake3: ToolProfileHash,
}

/// The E0 layout's tool record, which predates argument profiles.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct LegacyStoredToolUse {
    resolved_executable: String,
    version: String,
    arguments: Vec<String>,
    argument_profile_blake3: Option<ToolProfileHash>,
}

#[derive(Debug, Deserialize)]
struct StoredManifestVersion {
    schema_version: String,
    // This pass only selects the strict decoder. The selected decoder reparses
    // the same bytes with unknown-field rejection.
    #[serde(flatten)]
    _remaining: BTreeMap<String, serde::de::IgnoredAny>,
}

/// What every layout reduces to once decoded under its own strict shape.
///
/// `validate_package` works from this rather than from three parsers, so a
/// layout is described in exactly one place and checked in exactly one place.
#[derive(Debug)]
struct PackageRecord {
    release_status: ReleaseStatus,
    lesson_id: String,
    plan_hash: PlanHash,
    /// Frames in the master, for a layout that records one.
    total_frames: Option<u64>,
    /// `None` for a layout written before the renderer was versioned, which is
    /// therefore never a matching generation.
    text_renderer_version: Option<String>,
    segments: Vec<RecordedSegment>,
    artifacts: Vec<RecordedArtifact>,
    ffmpeg: StoredToolIdentity,
    ffprobe: StoredToolIdentity,
    executions: Vec<RecordedToolUse>,
}

#[derive(Debug)]
struct RecordedSegment {
    segment_id: String,
    frames: u32,
    /// The written positions, for a layout that carries them.
    ///
    /// `None` for the two historical layouts, which recorded a declared
    /// duration and no written boundary at all. A layout that says nothing
    /// about where a segment landed cannot be checked for saying it wrongly.
    written: Option<WrittenPosition>,
}

/// One segment's written boundary and the pause the plan declared for it.
#[derive(Debug)]
struct WrittenPosition {
    start_frame: u64,
    pause_after_ms: u32,
    pause_frames: u64,
}

#[derive(Debug)]
struct RecordedArtifact {
    required_name: &'static str,
    path: String,
    blake3: AudioDigest,
}

#[derive(Debug)]
struct RecordedToolUse {
    tool: RecordedTool,
    arguments: Vec<String>,
    argument_profile_blake3: Option<ToolProfileHash>,
}

impl From<StoredManifest> for PackageRecord {
    fn from(manifest: StoredManifest) -> Self {
        // Read so the compiler agrees these are part of the shape this build
        // accepts. `schema_version` is pinned to one string by the published
        // schema and was already read by `StoredManifestVersion`, and
        // `total_frames`, `cache_key`, `audio_blake3`, `start_frame`, and
        // `pause_frames` are value objects or width-bounded integers, so the
        // strict parse above already said everything there is to say about
        // each of them.
        let _ = &manifest.schema_version;
        let artifacts = manifest.artifacts;
        Self {
            release_status: manifest.release_status,
            lesson_id: manifest.lesson_id,
            plan_hash: manifest.plan_hash,
            text_renderer_version: Some(manifest.text_renderer_version),
            total_frames: Some(manifest.total_frames),
            segments: manifest
                .segments
                .into_iter()
                .map(|segment| {
                    let _ = (&segment.cache_key, &segment.audio_blake3);
                    RecordedSegment {
                        segment_id: segment.segment_id,
                        frames: segment.frames,
                        written: Some(WrittenPosition {
                            start_frame: segment.start_frame,
                            pause_after_ms: segment.pause_after_ms,
                            pause_frames: segment.pause_frames,
                        }),
                    }
                })
                .collect(),
            artifacts: PACKAGE_ARTIFACT_NAMES
                .into_iter()
                .zip([
                    artifacts.master_wav,
                    artifacts.m4a,
                    artifacts.mp3,
                    artifacts.transcript,
                    artifacts.captions,
                    artifacts.chapters,
                ])
                .map(|(required_name, artifact)| RecordedArtifact {
                    required_name,
                    path: artifact.path,
                    blake3: artifact.blake3,
                })
                .collect(),
            ffmpeg: manifest.tools.ffmpeg,
            ffprobe: manifest.tools.ffprobe,
            executions: manifest
                .tools
                .executions
                .into_iter()
                .map(|execution| RecordedToolUse {
                    tool: execution.tool,
                    arguments: execution.arguments,
                    argument_profile_blake3: Some(execution.argument_profile_blake3),
                })
                .collect(),
        }
    }
}

/// Reduces one older two-artifact layout to the common record.
///
/// The two older layouts differ only in whether a tool record carries an
/// argument profile, so `profile` is what separates them and everything else is
/// shared. Their two executions are the M4A encode and the M4A validation, in
/// that order, because that is the only pair either layout could describe.
fn legacy_record<T>(
    manifest: LegacyStoredManifest<T>,
    read: impl Fn(RecordedTool, T) -> RecordedToolUse,
    identity: impl Fn(&T) -> StoredToolIdentity,
) -> PackageRecord {
    // Read on the same terms as the current layout's: the version selected this
    // decoder before any field was decoded.
    let _ = &manifest.schema_version;
    let LegacyStoredTools { ffmpeg, ffprobe } = manifest.tools;
    let ffmpeg_identity = identity(&ffmpeg);
    let ffprobe_identity = identity(&ffprobe);
    PackageRecord {
        release_status: manifest.release_status,
        lesson_id: manifest.lesson_id,
        plan_hash: manifest.plan_hash,
        // Predates the versioned renderer, and predates the text documents
        // themselves, so it can never be a matching generation.
        text_renderer_version: None,
        // The historical layouts record no written boundary and no master
        // length, so there is nothing here to check for self-agreement.
        total_frames: None,
        segments: manifest
            .segments
            .into_iter()
            .map(|segment| {
                let _ = (
                    &segment.cache_key,
                    &segment.audio_blake3,
                    segment.pause_after_ms,
                );
                RecordedSegment {
                    segment_id: segment.segment_id,
                    frames: segment.frames,
                    written: None,
                }
            })
            .collect(),
        artifacts: vec![
            RecordedArtifact {
                required_name: MASTER_WAV_NAME,
                path: manifest.artifacts.master_wav.path,
                blake3: manifest.artifacts.master_wav.blake3,
            },
            RecordedArtifact {
                required_name: M4A_NAME,
                path: manifest.artifacts.m4a.path,
                blake3: manifest.artifacts.m4a.blake3,
            },
        ],
        ffmpeg: ffmpeg_identity,
        ffprobe: ffprobe_identity,
        executions: vec![
            read(RecordedTool::Ffmpeg, ffmpeg),
            read(RecordedTool::Ffprobe, ffprobe),
        ],
    }
}

/// Validates the files and strict manifest inside an immutable package.
///
/// When `reuse` is supplied, this also decides whether a no-op rebuild may
/// reuse the generation rather than reassembling and re-encoding it.
///
/// # Errors
///
/// A distinct [`DurableStateError`] naming the malformed or mismatched package
/// invariant; otherwise [`crate::IoError::FileSystem`] while reading or hashing
/// package files.
pub(crate) fn validate_package(
    package_dir: &Path,
    lesson_id: &str,
    plan_hash: Option<&str>,
    reuse: Option<ReuseExpectations<'_>>,
) -> Result<bool, BuildError> {
    // Through the resolver, not `join`: a published package is read back from
    // a directory this build did not just write, and a symlink planted at
    // `manifest.json` would otherwise be followed straight out of the package.
    // `rust-production` states the rule; `managed` owns it.
    let manifest_path = managed::leaf(package_dir, MANIFEST_NAME)?;
    let bytes =
        std::fs::read(&manifest_path).map_err(|error| crate::io_error(&manifest_path, error))?;
    let version: StoredManifestVersion = parse_manifest(&bytes, &manifest_path)?;
    let manifest = parse_stored_manifest(&bytes, &manifest_path, &version.schema_version)?;
    if manifest.release_status != ReleaseStatus::PrivatePreview {
        return Err(DurableStateError::PackageReleaseStatusMismatch {
            path: manifest_path,
            found: manifest.release_status.as_str().to_owned(),
        }
        .into());
    }
    if manifest.lesson_id != lesson_id {
        return Err(DurableStateError::PackageLessonMismatch {
            path: manifest_path,
            recorded: manifest.lesson_id,
            required: lesson_id.to_owned(),
        }
        .into());
    }
    // No digest is checked by hand below. `plan_hash`, `audio_blake3`, each
    // artifact's `blake3`, and each execution's `argument_profile_blake3` are
    // value objects, so a malformed one was refused by the parse above and
    // carries that type's own remedy routing. What remains here is what a type
    // cannot say: a field that is well formed and still wrong for this package.
    for segment in &manifest.segments {
        if segment.segment_id.is_empty() {
            return Err(DurableStateError::EmptyPackageSegmentId {
                path: manifest_path,
            }
            .into());
        }
        if segment.frames == 0 {
            return Err(DurableStateError::EmptyPackageSegmentAudio {
                path: manifest_path,
                segment_id: segment.segment_id.clone(),
            }
            .into());
        }
    }
    validate_written_timeline(&manifest, &manifest_path)?;
    for artifact in &manifest.artifacts {
        validate_artifact(package_dir, &manifest_path, artifact)?;
    }
    for recorded in &manifest.executions {
        if recorded.arguments.is_empty() {
            return Err(DurableStateError::MissingPackageToolArguments {
                path: manifest_path,
                tool: recorded.tool.label(),
            }
            .into());
        }
    }

    let plan_matches = plan_hash.is_none_or(|expected| manifest.plan_hash.as_str() == expected);
    let reusable = reuse.is_none_or(|expected| {
        // The renderer is checked beside the tools rather than after them
        // because it is the same question: was this package produced by what
        // this build would produce it with.
        tools_match(&manifest, &expected)
            && manifest.text_renderer_version.as_deref() == Some(expected.text_renderer_version)
    });
    Ok(plan_matches && reusable)
}

/// Decodes a stored manifest under the layout its `schema_version` names.
///
/// Fail-closed for every other string, including a future layout: a manifest
/// this build cannot describe is refused rather than read under the nearest
/// shape it happens to know.
fn parse_stored_manifest(
    bytes: &[u8],
    manifest_path: &Path,
    version: &str,
) -> Result<PackageRecord, BuildError> {
    match version {
        LEGACY_MANIFEST_LAYOUT_VERSION => Ok(legacy_record(
            parse_manifest::<LegacyStoredManifest<LegacyStoredToolUse>>(bytes, manifest_path)?,
            |which, tool| RecordedToolUse {
                tool: which,
                arguments: tool.arguments,
                argument_profile_blake3: tool.argument_profile_blake3,
            },
            |tool| StoredToolIdentity {
                resolved_executable: tool.resolved_executable.clone(),
                version: tool.version.clone(),
            },
        )),
        SKELETON_MANIFEST_LAYOUT_VERSION => Ok(legacy_record(
            parse_manifest::<LegacyStoredManifest<SkeletonStoredToolUse>>(bytes, manifest_path)?,
            |which, tool| RecordedToolUse {
                tool: which,
                arguments: tool.arguments,
                argument_profile_blake3: Some(tool.argument_profile_blake3),
            },
            |tool| StoredToolIdentity {
                resolved_executable: tool.resolved_executable.clone(),
                version: tool.version.clone(),
            },
        )),
        CURRENT_MANIFEST_LAYOUT_VERSION => Ok(PackageRecord::from(
            parse_manifest::<StoredManifest>(bytes, manifest_path)?,
        )),
        found => Err(DurableStateError::UnsupportedPackageManifest {
            path: manifest_path.to_path_buf(),
            found: found.to_owned(),
            required: CURRENT_MANIFEST_LAYOUT_VERSION,
        }
        .into()),
    }
}

fn parse_manifest<T: DeserializeOwned>(
    bytes: &[u8],
    manifest_path: &Path,
) -> Result<T, BuildError> {
    serde_json::from_slice(bytes).map_err(|source| {
        DurableStateError::MalformedPackageManifest {
            path: manifest_path.to_path_buf(),
            source,
        }
        .into()
    })
}

fn validate_artifact(
    package_dir: &Path,
    manifest_path: &Path,
    artifact: &RecordedArtifact,
) -> Result<(), BuildError> {
    if artifact.path != artifact.required_name {
        return Err(DurableStateError::UnexpectedPackageArtifactPath {
            manifest: manifest_path.to_path_buf(),
            recorded: artifact.path.clone(),
            required: artifact.required_name,
        }
        .into());
    }
    let path = managed::leaf(package_dir, artifact.required_name)?;
    let found = hash_file(&path)?;
    if found != artifact.blake3.as_str() {
        return Err(DurableStateError::PackageArtifactChecksumMismatch {
            path,
            expected: artifact.blake3.as_str().to_owned(),
            found,
        }
        .into());
    }
    Ok(())
}

/// The exact sequence of invocations a complete build performs.
///
/// `package_port::PreparedFileSystemPackageWriter::write` performs precisely
/// this, in this order, and names this function in return. Six entries: the
/// encoder preflight, the master probe, then an encode and a probe for each
/// export.
///
/// A **sequence**, not a set. A set cannot see that ffprobe never ran over the
/// MP3, that the M4A was encoded twice, or that an execution was recorded
/// against the wrong binary — a package missing half its verification would
/// have compared equal and been reused as complete.
fn expected_executions(profiles: &ExportProfiles) -> Vec<(RecordedTool, &ToolProfileHash)> {
    let probe = profiles.ffprobe.identity();
    vec![
        (RecordedTool::Ffmpeg, profiles.ffmpeg_encoders.identity()),
        (RecordedTool::Ffprobe, probe),
        (RecordedTool::Ffmpeg, profiles.ffmpeg_m4a.identity()),
        (RecordedTool::Ffprobe, probe),
        (RecordedTool::Ffmpeg, profiles.ffmpeg_mp3.identity()),
        (RecordedTool::Ffprobe, probe),
    ]
}

/// Refuses a package whose recorded timeline cannot describe one master.
///
/// The manifest records both what the plan *declared* and what the write loop
/// *wrote*, and `assembly` holds the two together while building. Nothing held
/// them together on the way back in: a manifest could claim segments that
/// overlap, a pause that is not its declared duration in frames, or a master
/// longer than its own segments, and reuse would have accepted it and shipped
/// the captions and chapters derived from those numbers.
///
/// Checked only for a layout that records them. The two historical layouts
/// carry no written boundary, and inventing one to check would be checking this
/// function's arithmetic rather than the manifest's.
fn validate_written_timeline(
    manifest: &PackageRecord,
    manifest_path: &Path,
) -> Result<(), BuildError> {
    let Some(total_frames) = manifest.total_frames else {
        return Ok(());
    };
    let incoherent = |detail: String| DurableStateError::IncoherentPackageTimeline {
        path: manifest_path.to_path_buf(),
        detail,
    };

    let mut boundary = 0_u64;
    for segment in &manifest.segments {
        let Some(written) = &segment.written else {
            continue;
        };
        let id = &segment.segment_id;
        if written.start_frame != boundary {
            return Err(incoherent(format!(
                "segment `{id}` starts at frame {} where the previous segment ended at {boundary}",
                written.start_frame
            ))
            .into());
        }
        // The same conversion `assembly::pause_frames` performs. A declared
        // pause and a written pause that disagree is exactly the defect every
        // caption and chapter boundary after it inherits.
        let declared = u64::from(written.pause_after_ms)
            .checked_mul(u64::from(CANONICAL_SAMPLE_RATE))
            .map(|frames| frames / MILLISECONDS_PER_SECOND);
        if declared != Some(written.pause_frames) {
            return Err(incoherent(format!(
                "segment `{id}` declares a {} ms pause but records {} frames of silence",
                written.pause_after_ms, written.pause_frames
            ))
            .into());
        }
        boundary = boundary
            .checked_add(u64::from(segment.frames))
            .and_then(|running| running.checked_add(written.pause_frames))
            .ok_or_else(|| incoherent(format!("the boundaries past segment `{id}` overflow")))?;
    }

    if boundary != total_frames {
        return Err(incoherent(format!(
            "the segments span {boundary} frames while the master records {total_frames}"
        ))
        .into());
    }
    Ok(())
}

/// Whether a recorded package was produced by the toolchain this build would
/// use, running what this build would run.
fn tools_match(manifest: &PackageRecord, expected: &ReuseExpectations<'_>) -> bool {
    let identity_matches = |recorded: &StoredToolIdentity, tool: &ToolIdentity| {
        recorded.resolved_executable == tool.resolved_executable.display().to_string()
            && recorded.version == tool.version
    };
    if !identity_matches(&manifest.ffmpeg, expected.ffmpeg)
        || !identity_matches(&manifest.ffprobe, expected.ffprobe)
    {
        return false;
    }

    let required = expected_executions(expected.profiles);
    if manifest.executions.len() != required.len() {
        return false;
    }
    manifest
        .executions
        .iter()
        .zip(required)
        .all(|(recorded, (tool, profile))| {
            // A layout that predates argument profiles cannot prove what
            // produced it, so it is never a matching generation.
            recorded.tool == tool && recorded.argument_profile_blake3.as_ref() == Some(profile)
        })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::{
        durable::OsDurableFileSystem,
        export::{self, ToolProfile},
        timeline::WrittenSegment,
    };

    fn cached_segment(audio_blake3: String) -> ValidatedCachedArtifact {
        ValidatedCachedArtifact {
            segment_id: "segment".to_owned(),
            cache_key: "a"
                .repeat(CacheKey::LENGTH)
                .parse()
                .expect("valid cache key"),
            entry_dir: PathBuf::from("cache-entry"),
            audio_path: PathBuf::from("cache-entry/audio.wav"),
            audio_blake3,
            frames: 1,
            pause_after_ms: 0,
        }
    }

    fn one_segment_timeline() -> Timeline {
        Timeline {
            segments: vec![WrittenSegment {
                start_frame: 0,
                audio_frames: 1,
                pause_frames: 0,
            }],
            total_frames: 1,
        }
    }

    fn test_tool_identities() -> (ToolIdentity, ToolIdentity) {
        (
            ToolIdentity {
                resolved_executable: PathBuf::from("/tools/ffmpeg"),
                version: "ffmpeg version 1".to_owned(),
            },
            ToolIdentity {
                resolved_executable: PathBuf::from("/tools/ffprobe"),
                version: "ffprobe version 1".to_owned(),
            },
        )
    }

    /// Exactly what a complete build performs, in order.
    ///
    /// Built from [`expected_executions`] rather than listed again: a fixture
    /// that named its own shorter sequence would be a package the production
    /// path could never write, and every reuse test would then pass against
    /// something that does not exist.
    fn test_executions(profiles: &ExportProfiles) -> Vec<(RecordedTool, ToolExecution)> {
        expected_executions(profiles)
            .into_iter()
            .map(|(tool, profile)| {
                (
                    tool,
                    ToolExecution {
                        arguments: vec![format!("{}-{}", tool.label(), profile.as_str())],
                        argument_profile_blake3: profile.to_owned(),
                    },
                )
            })
            .collect()
    }

    fn borrowed(executions: &[(RecordedTool, ToolExecution)]) -> Vec<RecordedExecution<'_>> {
        executions
            .iter()
            .map(|(tool, execution)| RecordedExecution {
                tool: *tool,
                execution,
            })
            .collect()
    }

    fn write_test_package(package: &Path) {
        std::fs::create_dir(package).expect("create test package");
        for name in PACKAGE_ARTIFACT_NAMES {
            std::fs::write(package.join(name), name.as_bytes()).expect("write package artifact");
        }
        let segment = cached_segment(blake3::hash(b"segment").to_hex().to_string());
        let plan_hash = PlanHash::from(blake3::hash(b"plan"));
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();
        let executions = test_executions(&profiles);
        write(
            &OsDurableFileSystem,
            &package.join(MANIFEST_NAME),
            ManifestRecords {
                lesson_id: "lesson",
                plan_hash: &plan_hash,
                segments: std::slice::from_ref(&segment),
                timeline: &one_segment_timeline(),
                package_dir: package,
                tools: ToolRecords {
                    ffmpeg: &ffmpeg,
                    ffprobe: &ffprobe,
                    executions: &borrowed(&executions),
                },
            },
        )
        .expect("write test manifest");
    }

    /// Writes one of the two historical two-artifact packages.
    ///
    /// Hand-built rather than produced by [`write`], which can only emit the
    /// current layout. That is the point of the test: these bytes are what an
    /// earlier build left on disk, and this build must still read them.
    fn write_historical_package(package: &Path, schema_version: &str, profiles: bool) {
        std::fs::create_dir(package).expect("create historical package");
        let mut tool = json!({
            "resolved_executable": "/tools/ffmpeg",
            "version": "ffmpeg version 1",
            "arguments": ["encode"],
        });
        let mut prober = json!({
            "resolved_executable": "/tools/ffprobe",
            "version": "ffprobe version 1",
            "arguments": ["probe"],
        });
        if profiles {
            let published = export::export_profiles();
            tool["argument_profile_blake3"] = json!(published.ffmpeg_m4a.identity().as_str());
            prober["argument_profile_blake3"] = json!(published.ffprobe.identity().as_str());
        }
        let mut artifacts = serde_json::Map::new();
        for (field, name) in [("master_wav", MASTER_WAV_NAME), ("m4a", M4A_NAME)] {
            std::fs::write(package.join(name), name.as_bytes()).expect("write historical artifact");
            let blake3 = blake3::hash(name.as_bytes()).to_hex().to_string();
            artifacts.insert(field.to_owned(), json!({ "path": name, "blake3": blake3 }));
        }
        let manifest = json!({
            "schema_version": schema_version,
            "release_status": "private_preview",
            "lesson_id": "lesson",
            "plan_hash": PlanHash::from(blake3::hash(b"plan")).as_str(),
            "segments": [{
                "segment_id": "segment",
                "cache_key": "a".repeat(CacheKey::LENGTH),
                "audio_blake3": blake3::hash(b"segment").to_hex().to_string(),
                "frames": 1,
                "pause_after_ms": 0,
            }],
            "artifacts": Value::Object(artifacts),
            "tools": { "ffmpeg": tool, "ffprobe": prober },
        });
        std::fs::write(
            package.join(MANIFEST_NAME),
            serde_json::to_vec_pretty(&manifest).expect("serialize historical manifest"),
        )
        .expect("write historical manifest");
    }

    fn rewrite_test_manifest(package: &Path, update: impl FnOnce(&mut Value)) {
        let manifest_path = package.join(MANIFEST_NAME);
        let mut manifest: Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).expect("read test manifest"))
                .expect("parse test manifest");
        update(&mut manifest);
        std::fs::write(
            manifest_path,
            serde_json::to_vec_pretty(&manifest).expect("serialize test manifest"),
        )
        .expect("write changed test manifest");
    }

    /// One named way to corrupt a written manifest, for a table of them.
    type ManifestMutation<T> = (&'static str, fn(&mut T));

    fn expectations<'a>(
        ffmpeg: &'a ToolIdentity,
        ffprobe: &'a ToolIdentity,
        profiles: &'a ExportProfiles,
    ) -> ReuseExpectations<'a> {
        ReuseExpectations {
            ffmpeg,
            ffprobe,
            profiles,
            text_renderer_version: TEXT_RENDERER_VERSION,
        }
    }

    /// The published schema and the parser agree on which layouts exist.
    ///
    /// The gap this closes is not that the historical layouts are unpublished —
    /// that is deliberate, and `schema_version_json_schema` says why. It is
    /// that
    /// nothing held the two facts together: `validate_package` read several
    /// layouts while the published schema described one, and another layout
    /// could have been added to the parser leaving the schema silently
    /// describing a shrinking fraction of what this build accepts.
    ///
    /// An empty object is enough to tell the two refusals apart, and that is
    /// the whole trick: a known layout reaches a decoder and is refused for its
    /// *shape*, while an unread one is refused for its version before any byte
    /// is decoded. What each decoder then accepts is proved by the historical
    /// and current package tests below, which read real packages.
    #[test]
    fn t3_e1_the_published_manifest_schema_names_every_layout_it_describes() {
        let schema = current_manifest_schema();
        let published = schema["properties"]["schema_version"]["const"]
            .as_str()
            .expect("the published manifest schema pins `schema_version` to one string");

        assert_eq!(
            published, CURRENT_MANIFEST_LAYOUT_VERSION,
            "the schema must publish the layout this build writes"
        );

        let path = Path::new("manifest.json");
        for known in [
            LEGACY_MANIFEST_LAYOUT_VERSION,
            SKELETON_MANIFEST_LAYOUT_VERSION,
            CURRENT_MANIFEST_LAYOUT_VERSION,
        ] {
            let error = parse_stored_manifest(b"{}", path, known)
                .expect_err("an empty object is not a manifest of any layout");

            assert!(
                matches!(
                    error,
                    BuildError::DurableState(ref error)
                        if matches!(
                            error.as_ref(),
                            DurableStateError::MalformedPackageManifest { .. }
                        )
                ),
                "`{known}` must reach a decoder rather than be refused as an unread layout: \
                 {error:?}"
            );
        }

        let error = parse_stored_manifest(b"{}", path, "9.0-skeleton")
            .expect_err("an unread layout must be refused rather than decoded");

        assert!(
            matches!(
                error,
                BuildError::DurableState(ref error)
                    if matches!(
                        error.as_ref(),
                        DurableStateError::UnsupportedPackageManifest { found, required, .. }
                            if found == "9.0-skeleton"
                                && *required == CURRENT_MANIFEST_LAYOUT_VERSION
                    )
            ),
            "an unread layout must be refused by its version, naming the one this build \
             writes: {error:?}"
        );
    }

    /// Both historical layouts stay readable, and neither can be reused.
    ///
    /// Preservation and reuse are different questions and this build answers
    /// them differently: an operator's existing preview must keep validating,
    /// while a package holding two artifacts can never stand in for one holding
    /// six. Table-driven because the only difference between the two layouts is
    /// whether a tool record carried an argument profile, and that difference
    /// must not change either answer.
    #[test]
    fn t4_e0_historical_packages_remain_valid_but_cannot_satisfy_current_reuse() {
        const CASES: [(&str, bool); 3] = [
            (LEGACY_MANIFEST_LAYOUT_VERSION, false),
            (LEGACY_MANIFEST_LAYOUT_VERSION, true),
            (SKELETON_MANIFEST_LAYOUT_VERSION, true),
        ];
        let workspace = TempDir::new().expect("create manifest workspace");
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();

        for (index, (schema_version, has_profiles)) in CASES.into_iter().enumerate() {
            let package = workspace.path().join(format!("package-{index}"));
            write_historical_package(&package, schema_version, has_profiles);

            assert!(
                validate_package(&package, "lesson", None, None)
                    .expect("a historical package remains structurally valid"),
                "`{schema_version}` (profiles: {has_profiles}) must stay readable"
            );
            assert!(
                !validate_package(
                    &package,
                    "lesson",
                    None,
                    Some(expectations(&ffmpeg, &ffprobe, &profiles)),
                )
                .expect("a historical package remains valid without being reusable"),
                "`{schema_version}` (profiles: {has_profiles}) must not satisfy current reuse"
            );
        }
    }

    #[test]
    fn t4_e0_current_package_manifest_requires_tool_profiles() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        rewrite_test_manifest(&package, |manifest| {
            manifest["tools"]["executions"][0]
                .as_object_mut()
                .expect("execution record is an object")
                .remove("argument_profile_blake3");
        });

        let error = validate_package(&package, "lesson", None, None)
            .expect_err("current package must require tool profiles");

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(*error, DurableStateError::MalformedPackageManifest { .. })
        ));
    }

    /// A complete package validates and is reusable by the build that wrote it.
    #[test]
    fn t4_e1_a_complete_package_validates_and_is_reusable() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();

        assert!(
            validate_package(
                &package,
                "lesson",
                None,
                Some(expectations(&ffmpeg, &ffprobe, &profiles)),
            )
            .expect("a package this build wrote must validate")
        );
    }

    /// Every one of the six artifacts is checksummed, not only the audio.
    ///
    /// The gap this closes: the transcript, captions, and chapters are text a
    /// reader trusts as a record of the audio, and a manifest that hashed only
    /// the media would let any of the three be edited inside a published
    /// package without the checksum noticing.
    #[test]
    fn t4_e1_every_package_artifact_is_checksummed() {
        let workspace = TempDir::new().expect("create manifest workspace");

        for (index, name) in PACKAGE_ARTIFACT_NAMES.into_iter().enumerate() {
            let package = workspace.path().join(format!("package-{index}"));
            write_test_package(&package);
            std::fs::write(package.join(name), b"tampered").expect("tamper with the artifact");

            let error = validate_package(&package, "lesson", None, None)
                .expect_err("a tampered artifact must be refused");

            assert!(
                matches!(
                    error,
                    BuildError::DurableState(ref error)
                        if matches!(
                            error.as_ref(),
                            DurableStateError::PackageArtifactChecksumMismatch { .. }
                        )
                ),
                "editing `{name}` must be refused by its checksum: {error:?}"
            );
        }
    }

    #[test]
    fn t4_e0_encoding_profile_change_names_a_new_package_generation() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        let (ffmpeg, ffprobe) = test_tool_identities();
        let mut profiles = export::export_profiles();
        profiles.ffmpeg_mp3 = ToolProfile::new(
            "ffmpeg",
            &["-i", "{input_path}", "-c:a", "libopus", "{output_path}"],
        );

        assert!(
            !validate_package(
                &package,
                "lesson",
                None,
                Some(expectations(&ffmpeg, &ffprobe, &profiles)),
            )
            .expect("the old package remains structurally valid"),
            "a changed MP3 profile must name a new generation"
        );
    }

    /// A package missing, repeating, or reassigning an invocation is not
    /// reusable.
    ///
    /// The gap this closes: reuse compared a deduplicated *set* of profile
    /// hashes, which cannot see that ffprobe never validated the MP3, that one
    /// encode ran twice, or that an execution was recorded against the other
    /// binary. Each case below is a package whose profile set is identical to a
    /// complete one.
    #[test]
    fn t4_e1_an_incomplete_tool_sequence_is_not_reusable() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();

        // Each mutation keeps the recorded profile *set* whole, so a set
        // comparison passes every one of them.
        let mutations: [ManifestMutation<Vec<Value>>; 4] = [
            ("dropped the final probe", |executions| {
                executions.pop();
            }),
            ("repeated the M4A encode", |executions| {
                executions[3] = executions[2].clone();
            }),
            ("reassigned an encode to ffprobe", |executions| {
                executions[2]["tool"] = json!("ffprobe");
            }),
            ("reordered the encodes", |executions| {
                executions.swap(2, 4);
            }),
        ];

        for (index, (case, mutate)) in mutations.into_iter().enumerate() {
            let package = workspace.path().join(format!("package-{index}"));
            write_test_package(&package);
            rewrite_test_manifest(&package, |manifest| {
                let executions = manifest["tools"]["executions"]
                    .as_array_mut()
                    .expect("the manifest records an execution list");
                mutate(executions);
            });

            assert!(
                !validate_package(
                    &package,
                    "lesson",
                    None,
                    Some(expectations(&ffmpeg, &ffprobe, &profiles)),
                )
                .expect("the package remains structurally valid"),
                "a package that {case} must not be reusable"
            );
        }
    }

    /// A manifest whose recorded timeline contradicts itself is refused.
    ///
    /// The gap this closes: `start_frame`, `pause_frames`, and `total_frames`
    /// were parsed and thrown away, so a package could record segments that
    /// overlap, a pause that is not its declared duration, or a master longer
    /// than its own segments — and be reused, shipping the captions and
    /// chapters those numbers produced.
    #[test]
    fn t4_e1_a_self_contradictory_timeline_is_refused() {
        let workspace = TempDir::new().expect("create manifest workspace");

        let mutations: [ManifestMutation<Value>; 3] = [
            ("a segment starting off the previous boundary", |manifest| {
                manifest["segments"][0]["start_frame"] = json!(1);
            }),
            ("a pause that is not its declared duration", |manifest| {
                manifest["segments"][0]["pause_frames"] = json!(7);
            }),
            ("a master longer than its own segments", |manifest| {
                let recorded = manifest["total_frames"].as_u64().expect("a frame total");
                manifest["total_frames"] = json!(recorded + 1);
            }),
        ];

        for (index, (case, mutate)) in mutations.into_iter().enumerate() {
            let package = workspace.path().join(format!("package-{index}"));
            write_test_package(&package);
            rewrite_test_manifest(&package, mutate);

            let error = validate_package(&package, "lesson", None, None)
                .expect_err("a contradictory timeline must be refused");

            assert!(
                matches!(
                    error,
                    BuildError::DurableState(ref error)
                        if matches!(
                            error.as_ref(),
                            DurableStateError::IncoherentPackageTimeline { .. }
                        )
                ),
                "{case} produced `{error}`"
            );
        }
    }

    /// A symlink standing in for a package file is refused, not followed.
    ///
    /// A published package is read back from a directory this build did not
    /// just write, so `join` would follow a planted link straight out of the
    /// package — reading, hashing, and reusing bytes from wherever it pointed.
    #[test]
    #[cfg(unix)]
    fn t4_e1_a_symlinked_package_file_is_refused() {
        for name in [MANIFEST_NAME, MASTER_WAV_NAME] {
            let workspace = TempDir::new().expect("create manifest workspace");
            let package = workspace.path().join("package");
            write_test_package(&package);

            // The bytes the link points at live outside the package.
            let outside = workspace.path().join("outside");
            std::fs::write(&outside, b"outside the package").expect("write the escape target");
            let planted = package.join(name);
            std::fs::remove_file(&planted).expect("remove the real file");
            std::os::unix::fs::symlink(&outside, &planted).expect("plant the symlink");

            let error = validate_package(&package, "lesson", None, None)
                .expect_err("a symlinked package file must be refused");

            assert!(
                matches!(
                    error,
                    BuildError::ManagedPath(crate::ManagedPathError::ManagedPathEscape { .. })
                ),
                "`{name}` as a symlink produced `{error}`"
            );
        }
    }

    /// The renderer that wrote the text documents is recorded, and reuse is
    /// gated on it.
    ///
    /// The gap this closes: FFmpeg never sees the transcript, the captions, or
    /// the chapters, so nothing in the plan hash or the tool profiles moves
    /// when the rules that produce them change. Replacing `timeline::timestamp`
    /// as `ADR-0001-D010`'s rollback describes would rewrite every cue in
    /// `transcript.vtt` while both stood still, and the selected package would
    /// be reused with the captions of the projection that was replaced.
    ///
    /// Both halves are asserted together because the second is vacuous without
    /// the first: a manifest recording some other string would still refuse a
    /// package recording a third.
    #[test]
    fn t4_e1_text_renderer_change_names_a_new_package_generation() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();

        let recorded: Value = serde_json::from_slice(
            &std::fs::read(package.join(MANIFEST_NAME)).expect("read the written manifest"),
        )
        .expect("the written manifest is JSON");
        assert_eq!(
            recorded["text_renderer_version"],
            json!(TEXT_RENDERER_VERSION),
            "the manifest must record the renderer that wrote the documents"
        );

        // The shape an earlier renderer left behind: same plan, same tools,
        // different rules for the three documents.
        rewrite_test_manifest(&package, |manifest| {
            manifest["text_renderer_version"] = json!("0.9-skeleton-text-renderer");
        });

        assert!(
            !validate_package(
                &package,
                "lesson",
                None,
                Some(expectations(&ffmpeg, &ffprobe, &profiles)),
            )
            .expect("the old package remains structurally valid"),
            "a changed text renderer must name a new generation"
        );
    }
}
