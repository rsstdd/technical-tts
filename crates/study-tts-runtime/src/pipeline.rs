use std::{fs, path::PathBuf};

use serde_json::Value;
use study_tts_core::{Lesson, RenderPlan};

use crate::{BuildError, SegmentSynthesizer, assembly, cache, export, io_error, manifest, tools};

#[derive(Clone, Debug)]
pub struct BuildRequest {
    pub lesson_path: PathBuf,
    pub workspace: PathBuf,
    pub ffmpeg_executable: PathBuf,
    pub ffprobe_executable: PathBuf,
}

#[derive(Clone, Debug)]
pub struct BuildResult {
    pub master_wav: PathBuf,
    pub m4a: PathBuf,
    pub manifest: PathBuf,
}

pub fn build_preview(
    request: BuildRequest,
    synthesizer: &dyn SegmentSynthesizer,
) -> Result<BuildResult, BuildError> {
    let ffmpeg = tools::inspect("FFmpeg", &request.ffmpeg_executable)?;
    let ffprobe = tools::inspect("ffprobe", &request.ffprobe_executable)?;

    let lesson_bytes = fs::read(&request.lesson_path).map_err(|source| BuildError::ReadFile {
        path: request.lesson_path.clone(),
        source,
    })?;
    let lesson = Lesson::from_json(&lesson_bytes)?;
    let plan = RenderPlan::for_lesson(&lesson, synthesizer.identity());

    let cache_root = request.workspace.join("cache");
    let output_root = request.workspace.join("previews").join(&lesson.lesson_id);
    fs::create_dir_all(&cache_root).map_err(|error| io_error(&cache_root, error))?;
    fs::create_dir_all(&output_root).map_err(|error| io_error(&output_root, error))?;

    let cached_segments = plan
        .segments
        .iter()
        .map(|segment| cache::resolve(&cache_root, segment, synthesizer))
        .collect::<Result<Vec<_>, _>>()?;

    let master_wav = output_root.join("lesson.wav");
    assembly::assemble(&cached_segments, &master_wav)?;
    let m4a = output_root.join("lesson.m4a");
    let ffmpeg_execution = export::export_m4a(&ffmpeg, &master_wav, &m4a)?;
    let ffprobe_execution = export::probe_m4a(&ffprobe, &m4a)?;
    let manifest_path = output_root.join("manifest.json");
    manifest::write(
        &manifest_path,
        &lesson.lesson_id,
        &plan.plan_hash,
        &cached_segments,
        &master_wav,
        &m4a,
        manifest::ToolRecords {
            ffmpeg: &ffmpeg,
            ffmpeg_execution: &ffmpeg_execution,
            ffprobe: &ffprobe,
            ffprobe_execution: &ffprobe_execution,
        },
    )?;

    Ok(BuildResult {
        master_wav,
        m4a,
        manifest: manifest_path,
    })
}

pub fn publish(_preview: &BuildResult) -> Result<(), BuildError> {
    Err(BuildError::PublicationRefused {
        reason: "E0-S0 outputs are private previews and production gates are not implemented"
            .to_owned(),
    })
}

pub fn validate_production_manifest(bytes: &[u8]) -> Result<(), BuildError> {
    let manifest: Value = serde_json::from_slice(bytes)?;
    let version = manifest["schema_version"]
        .as_str()
        .unwrap_or("missing")
        .to_owned();
    if version != "1.0" {
        return Err(BuildError::UnsupportedProductionManifest { version });
    }

    Err(BuildError::PublicationRefused {
        reason: "production manifest acceptance is unavailable before the production gates"
            .to_owned(),
    })
}
