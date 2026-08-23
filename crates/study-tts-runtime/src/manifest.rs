use std::path::Path;

use serde::Serialize;
use study_tts_core::{CacheKey, PlanHash};

use crate::{
    BuildError,
    cache::{CachedSegment, hash_file, write_json_atomically},
    export::ToolExecution,
    tools::ToolIdentity,
};

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: &'static str,
    release_status: &'static str,
    lesson_id: &'a str,
    plan_hash: &'a PlanHash,
    segments: Vec<ManifestSegment<'a>>,
    artifacts: Artifacts,
    tools: Tools<'a>,
}

#[derive(Serialize)]
struct ManifestSegment<'a> {
    segment_id: &'a str,
    cache_key: &'a CacheKey,
    audio_blake3: &'a str,
    frames: u32,
    pause_after_ms: u32,
}

#[derive(Serialize)]
struct Artifacts {
    master_wav: Artifact,
    m4a: Artifact,
}

#[derive(Serialize)]
struct Artifact {
    path: &'static str,
    blake3: String,
}

#[derive(Serialize)]
struct Tools<'a> {
    ffmpeg: ToolUse<'a>,
    ffprobe: ToolUse<'a>,
}

#[derive(Serialize)]
struct ToolUse<'a> {
    resolved_executable: String,
    version: &'a str,
    arguments: &'a [String],
}

pub(crate) struct ToolRecords<'a> {
    pub ffmpeg: &'a ToolIdentity,
    pub ffmpeg_execution: &'a ToolExecution,
    pub ffprobe: &'a ToolIdentity,
    pub ffprobe_execution: &'a ToolExecution,
}

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
        schema_version: "0.1-skeleton",
        release_status: "private_preview",
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
                path: "lesson.wav",
                blake3: hash_file(master_wav)?,
            },
            m4a: Artifact {
                path: "lesson.m4a",
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
