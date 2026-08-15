use std::path::Path;

use serde::Serialize;

use crate::{
    BuildError,
    cache::{CachedSegment, hash_file, write_json_atomically},
};

#[derive(Serialize)]
struct Manifest<'a> {
    schema_version: &'static str,
    release_status: &'static str,
    lesson_id: &'a str,
    plan_hash: &'a str,
    segments: Vec<ManifestSegment<'a>>,
    artifacts: Artifacts,
}

#[derive(Serialize)]
struct ManifestSegment<'a> {
    segment_id: &'a str,
    cache_key: &'a str,
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

pub(crate) fn write(
    destination: &Path,
    lesson_id: &str,
    plan_hash: &str,
    segments: &[CachedSegment],
    master_wav: &Path,
    m4a: &Path,
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
    };
    write_json_atomically(destination, &manifest)
}
