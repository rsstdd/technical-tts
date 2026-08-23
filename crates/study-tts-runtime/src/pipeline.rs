use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use study_tts_core::{
    Lesson, RenderPlan, RightsDecision, SourceRightsDeclaration, VoiceError, VoiceUse,
};

use crate::{
    BuildError, SegmentSynthesizer, assembly, cache, export, io_error, manifest, tools, voice_gate,
};

#[derive(Clone, Debug)]
pub struct BuildRequest {
    pub lesson_path: PathBuf,
    pub workspace: PathBuf,
    pub ffmpeg_executable: PathBuf,
    pub ffprobe_executable: PathBuf,
    /// Voice profile directory in the ADR-0001 §12.1 layout, gated fail-closed before any tool
    /// or synthesis work. `None` is valid only while the deterministic skeleton worker is the
    /// backend; the real-worker story (E0-S3/E1) makes a profile mandatory.
    pub voice_profile_dir: Option<PathBuf>,
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
    let lesson_bytes = fs::read(&request.lesson_path).map_err(|source| BuildError::ReadFile {
        path: request.lesson_path.clone(),
        source,
    })?;
    let lesson = Lesson::from_json(&lesson_bytes)?;
    let plan = RenderPlan::for_lesson(&lesson, synthesizer.identity());

    // Rights precede work: the profile gate runs before tool preflight and synthesis, so a
    // refused voice performs no observable work. The loaded identity is unused by the skeleton
    // worker; the real-worker story consumes it and records the ADR-0001 §15.3 per-build audit
    // event.
    if let Some(dir) = &request.voice_profile_dir {
        let _profile = voice_gate::load_profile(dir, VoiceUse::PrivateSynthesis)?;
    }

    let ffmpeg = tools::inspect("FFmpeg", &request.ffmpeg_executable)?;
    let ffprobe = tools::inspect("ffprobe", &request.ffprobe_executable)?;

    fs::create_dir_all(&request.workspace).map_err(|error| io_error(&request.workspace, error))?;
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let cache_root = managed_subdirectory(&workspace, "cache")?;
    let previews_root = managed_subdirectory(&workspace, "previews")?;
    let output_root = managed_subdirectory(&previews_root, &lesson.lesson_id)?;

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

/// Preflights ffprobe and requires the encoded artifact to be a single mono AAC stream.
///
/// `build_preview` performs this check internally; the entry point exists so the rejection path
/// can be exercised from the integration suite, which is where a test needing a real ffprobe
/// belongs.
pub fn validate_encoded_output(
    ffprobe_executable: &Path,
    encoded: &Path,
) -> Result<(), BuildError> {
    let ffprobe = tools::inspect("ffprobe", ffprobe_executable)?;
    export::probe_m4a(&ffprobe, encoded).map(|_| ())
}

/// Creates `root/component` and proves it stays beneath `root`.
///
/// `root` is always canonical: the workspace is canonicalized by the caller, and each returned
/// path is canonical and becomes the `root` of the next call. Only the final component is
/// therefore unresolved, and it is inspected before anything is created, because
/// `create_dir_all` follows a symlinked leaf and would create the target outside the workspace
/// even though the containment check afterwards rejects the result.
///
/// A window remains between the inspection and the creation. Closing it requires
/// directory-relative `openat` operations and a new dependency, which belongs to the E5-S4
/// containment story. For a single-user local tool the attacker would already need write access
/// to the workspace, so the check-then-verify pair is proportionate here.
fn managed_subdirectory(root: &Path, component: &str) -> Result<PathBuf, BuildError> {
    // Reject anything that is not a single ordinary path element. `is_portable_id` already
    // rejects separators in `lesson_id`, but this helper is generic over its component and the
    // two checks fail independently.
    let mut parts = Path::new(component).components();
    if !matches!(parts.next(), Some(Component::Normal(_))) || parts.next().is_some() {
        return Err(BuildError::ManagedPathEscape {
            path: root.join(component),
            root: root.to_path_buf(),
        });
    }

    let candidate = root.join(component);

    match fs::symlink_metadata(&candidate) {
        // `symlink_metadata` reports the link's own type, so `is_symlink` catches a leaf that
        // would otherwise be followed. The `is_dir` clause rejects a regular file occupying the
        // managed name; that is an obstruction rather than an escape, and it shares this variant
        // only until E5-S4 introduces a dedicated one.
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(BuildError::ManagedPathEscape {
                path: candidate,
                root: root.to_path_buf(),
            });
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&candidate, error)),
    }

    fs::create_dir_all(&candidate).map_err(|error| io_error(&candidate, error))?;

    // Defence in depth: catches a link planted between the inspection and the creation.
    let resolved = fs::canonicalize(&candidate).map_err(|error| io_error(&candidate, error))?;
    if !resolved.starts_with(root) {
        return Err(BuildError::ManagedPathEscape {
            path: resolved,
            root: root.to_path_buf(),
        });
    }
    Ok(resolved)
}

pub fn publish(_preview: &BuildResult) -> Result<(), BuildError> {
    Err(BuildError::PublicationRefused {
        reason: "E0-S0 outputs are private previews and production gates are not implemented"
            .to_owned(),
    })
}

/// A voice profile a production manifest declares it used.
///
/// Provisional shape pending the E1-S1 versioned JSON Schemas, like `content_rights` below.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredVoiceProfile {
    profile_id: String,
    approval: RightsDecision,
    #[expect(
        dead_code,
        reason = "read for shape validation; consumed by release evidence"
    )]
    rights_record_id: String,
}

/// Parses one rights section of a production manifest, naming the section when it does not parse.
///
/// Borrows the subtree rather than cloning it: `&Value` is itself a deserializer.
fn declare_section<'de, T: Deserialize<'de>>(
    section: &'static str,
    value: &'de Value,
) -> Result<Vec<T>, BuildError> {
    Vec::<T>::deserialize(value)
        .map_err(|source| BuildError::InvalidRightsDeclaration { section, source })
}

/// Always refuses publication until the production manifest and release gates exist.
///
/// The rights preconditions run first, so an unresolved content classification or an unapproved
/// voice profile is reported as itself rather than as the generic gate refusal. They enforce
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Unresolved external distribution blocks
/// publish") over provisional `content_rights` and `voice_profiles` manifest sections that the
/// E1-S1 schema story will version.
pub fn validate_production_manifest(bytes: &[u8]) -> Result<(), BuildError> {
    let manifest: Value = serde_json::from_slice(bytes)?;
    let version = manifest["schema_version"]
        .as_str()
        .unwrap_or("missing")
        .to_owned();
    if version != "1.0" {
        return Err(BuildError::UnsupportedProductionManifest { version });
    }

    let Some(declared) = manifest.get("content_rights") else {
        return Err(BuildError::PublicationRefused {
            reason: "production manifest declares no content_rights classification for its \
                     sources"
                .to_owned(),
        });
    };
    let declared = declare_section::<SourceRightsDeclaration>("content_rights", declared)?;
    for source in &declared {
        if !source.classification.permits_production_release() {
            return Err(BuildError::UnresolvedContentRights {
                source_id: source.source_id.clone(),
                classification: source.classification.as_str().to_owned(),
            });
        }
    }

    if let Some(profiles) = manifest.get("voice_profiles") {
        for profile in declare_section::<DeclaredVoiceProfile>("voice_profiles", profiles)? {
            if profile.approval != RightsDecision::Approved {
                return Err(BuildError::Voice(VoiceError::ProfileNotApproved {
                    profile_id: profile.profile_id,
                    decision: profile.approval.as_str().to_owned(),
                }));
            }
        }
    }

    Err(BuildError::PublicationRefused {
        reason: "production manifest acceptance is unavailable before the production gates"
            .to_owned(),
    })
}
