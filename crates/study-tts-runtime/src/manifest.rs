//! `manifest.json`: the record of what a build produced and what produced it.
//!
//! Every value written here is derived rather than restated — the artifact
//! names from the constants `pipeline` writes the files at, the release status
//! from the typed value, the digests from the files themselves. A manifest
//! that could disagree with the build it describes is worse than no manifest,
//! because `validate_production_manifest` gates on what it says.

use std::path::Path;

use serde::Serialize;
use study_tts_core::{CacheKey, PlanHash, ReleaseStatus};

use crate::{
    BuildError,
    cache::{CachedSegment, hash_file, write_json_atomically},
    export::ToolExecution,
    tools::ToolIdentity,
};

/// Layout version of `manifest.json`.
///
/// Independent of `CACHE_SCHEMA_VERSION` and the lesson schema despite sharing
/// a value today: each versions a different document and moves separately.
/// E1-S1 replaces all three with versioned JSON Schemas.
const MANIFEST_SCHEMA_VERSION: &str = "0.1-skeleton";

/// Name of the assembled master inside a preview directory.
///
/// Owned here because the manifest records these paths; `pipeline` writes the
/// files at the same names. Two literals could drift, leaving the manifest
/// pointing at a file that is not there.
pub(crate) const MASTER_WAV_NAME: &str = "lesson.wav";

/// Name of the encoded export inside a preview directory.
pub(crate) const M4A_NAME: &str = "lesson.m4a";

/// Name of the manifest itself inside a preview directory.
pub(crate) const MANIFEST_NAME: &str = "manifest.json";

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
    segments: Vec<ManifestSegment<'a>>,
    artifacts: Artifacts,
    tools: Tools<'a>,
}

/// One segment as the manifest records it: identity, digest, and length.
#[derive(Serialize)]
struct ManifestSegment<'a> {
    segment_id: &'a str,
    cache_key: &'a CacheKey,
    audio_blake3: &'a str,
    frames: u32,
    pause_after_ms: u32,
}

/// The two files a build leaves in its preview directory.
#[derive(Serialize)]
struct Artifacts {
    master_wav: Artifact,
    m4a: Artifact,
}

/// One produced file, named relative to the preview directory and hashed.
#[derive(Serialize)]
struct Artifact {
    path: &'static str,
    blake3: String,
}

/// The external tools the build shelled out to.
#[derive(Serialize)]
struct Tools<'a> {
    ffmpeg: ToolUse<'a>,
    ffprobe: ToolUse<'a>,
}

/// One tool as the manifest records it: which binary, which version, and the
/// arguments it was actually given.
#[derive(Serialize)]
struct ToolUse<'a> {
    resolved_executable: String,
    version: &'a str,
    arguments: &'a [String],
}

/// The two external tools a build used, as the manifest must record them.
///
/// Identity and execution are carried separately because they answer different
/// questions: which binary ran, and what it was told to do.
pub(crate) struct ToolRecords<'a> {
    /// Which FFmpeg binary ran.
    pub ffmpeg: &'a ToolIdentity,
    /// What that FFmpeg was told to do.
    pub ffmpeg_execution: &'a ToolExecution,
    /// Which ffprobe binary ran.
    pub ffprobe: &'a ToolIdentity,
    /// What that ffprobe was told to do.
    pub ffprobe_execution: &'a ToolExecution,
}

/// Writes `manifest.json` for a completed build.
///
/// Hashes the master and the export as it goes, so the recorded digests
/// describe the bytes on disk rather than what the build believed it wrote.
/// Written atomically: a half-written manifest would describe a build that
/// does not exist.
///
/// # Errors
///
/// [`BuildError::FileSystem`] if either artifact cannot be read for hashing or
/// the manifest cannot be written; [`BuildError::WriteJson`] if serialization
/// fails.
pub(crate) fn write(
    destination: &Path,
    lesson_id: &str,
    plan_hash: &PlanHash,
    segments: &[CachedSegment],
    master_wav: &Path,
    m4a: &Path,
    tool_records: ToolRecords<'_>,
) -> Result<(), BuildError> {
    let manifest = Manifest {
        schema_version: MANIFEST_SCHEMA_VERSION,
        // The typed value, not a hand-written spelling of it. A literal here
        // would keep whatever it said if `ReleaseStatus` were ever respelled,
        // and this field is what `validate_production_manifest` gates on.
        release_status: ReleaseStatus::PrivatePreview,
        lesson_id,
        plan_hash,
        segments: segments
            .iter()
            .map(|segment| ManifestSegment {
                segment_id: &segment.segment_id,
                cache_key: &segment.cache_key,
                audio_blake3: &segment.audio_blake3,
                frames: segment.frames,
                pause_after_ms: segment.pause_after_ms,
            })
            .collect(),
        artifacts: Artifacts {
            master_wav: Artifact {
                path: MASTER_WAV_NAME,
                blake3: hash_file(master_wav)?,
            },
            m4a: Artifact {
                path: M4A_NAME,
                blake3: hash_file(m4a)?,
            },
        },
        tools: Tools {
            ffmpeg: ToolUse {
                resolved_executable: tool_records
                    .ffmpeg
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &tool_records.ffmpeg.version,
                arguments: &tool_records.ffmpeg_execution.arguments,
            },
            ffprobe: ToolUse {
                resolved_executable: tool_records
                    .ffprobe
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &tool_records.ffprobe.version,
                arguments: &tool_records.ffprobe_execution.arguments,
            },
        },
    };
    write_json_atomically(destination, &manifest)
}
