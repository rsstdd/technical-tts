use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;
use study_tts_core::{
    Lesson, ReleaseStatus, RenderPlan, RightsDecision, SourceRightsDeclaration, VoiceError,
    VoiceUse, validate_lesson_id,
};

use crate::{
    BuildError, SegmentSynthesizer, assembly, cache, export, io_error, managed, manifest, tools,
    voice_gate,
};

/// Everything one preview build needs, named explicitly rather than read from
/// ambient state.
#[derive(Clone, Debug)]
pub struct BuildRequest {
    /// The lesson document to build.
    pub lesson_path: PathBuf,
    /// Root the build owns; outputs, cache, and staging all resolve beneath it.
    pub workspace: PathBuf,
    /// FFmpeg to encode with, resolved and version-probed before any work
    /// begins.
    pub ffmpeg_executable: PathBuf,
    /// ffprobe to validate the encoded output with, on the same terms.
    pub ffprobe_executable: PathBuf,
    /// Voice profile directory in the ADR-0001 §12.1 layout, gated fail-closed
    /// before any tool or synthesis work. `None` is valid only while the
    /// deterministic skeleton worker is the backend; the real-worker story
    /// (E0-S3/E1) makes a profile mandatory.
    pub voice_profile_dir: Option<PathBuf>,
}

/// What a successful preview build wrote.
#[derive(Clone, Debug)]
pub struct BuildResult {
    /// The assembled canonical-format master.
    pub master_wav: PathBuf,
    /// The encoded distribution copy.
    pub m4a: PathBuf,
    /// The manifest recording segments, checksums, and the tools used.
    pub manifest: PathBuf,
}

/// Builds one lesson into a private preview.
///
/// Every gate runs before any tool or synthesis work, so a refusal names the
/// gate rather than a missing binary. The result is always a private preview;
/// only `publish` can claim more.
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

    // Rights precede work: the profile gate runs before tool preflight and
    // synthesis, so a refused voice performs no observable work. The loaded
    // identity is unused by the skeleton worker; the real-worker story consumes
    // it and records the ADR-0001 §15.3 per-build audit event.
    if let Some(dir) = &request.voice_profile_dir {
        let _profile = voice_gate::load_profile(dir, VoiceUse::PrivateSynthesis)?;
    }

    let ffmpeg = tools::inspect("FFmpeg", &request.ffmpeg_executable)?;
    let ffprobe = tools::inspect("ffprobe", &request.ffprobe_executable)?;

    fs::create_dir_all(&request.workspace).map_err(|error| io_error(&request.workspace, error))?;
    let workspace = fs::canonicalize(&request.workspace)
        .map_err(|error| io_error(&request.workspace, error))?;
    let cache_root = managed::subdirectory(&workspace, "cache")?;
    let previews_root = managed::subdirectory(&workspace, "previews")?;
    let output_root = managed::subdirectory(&previews_root, &lesson.lesson_id)?;

    let cached_segments = plan
        .segments
        .iter()
        .map(|segment| cache::resolve(&cache_root, segment, synthesizer))
        .collect::<Result<Vec<_>, _>>()?;

    // Each output is staged and renamed into place, so a link occupying one of
    // these names would be replaced rather than followed. It is still refused:
    // silently destroying something the operator put there is not this build's
    // decision to make, and the refusal names what it found.
    let master_wav = managed::leaf(&output_root, manifest::MASTER_WAV_NAME)?;
    assembly::assemble(&cached_segments, &master_wav)?;
    let m4a = managed::leaf(&output_root, manifest::M4A_NAME)?;
    let ffmpeg_execution = export::export_m4a(&ffmpeg, &master_wav, &m4a)?;
    let ffprobe_execution = export::probe_m4a(&ffprobe, &m4a)?;
    let manifest_path = managed::leaf(&output_root, manifest::MANIFEST_NAME)?;
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

/// Preflights ffprobe and requires the encoded artifact to be a single mono AAC
/// stream.
///
/// `build_preview` performs this check internally; the entry point exists so
/// the rejection path can be exercised from the integration suite, which is
/// where a test needing a real ffprobe belongs.
pub fn validate_encoded_output(
    ffprobe_executable: &Path,
    encoded: &Path,
) -> Result<(), BuildError> {
    let ffprobe = tools::inspect("ffprobe", ffprobe_executable)?;
    export::probe_m4a(&ffprobe, encoded).map(|_| ())
}

/// Refuses publication for the E0-S0 skeleton.
///
/// The production gates of `docs/governance/RELEASE-PROFILES.md` §3 are not
/// implemented, and a build that cannot evaluate them must refuse rather than
/// publish unevaluated.
pub fn publish(_preview: &BuildResult) -> Result<(), BuildError> {
    Err(BuildError::PublicationRefused {
        reason: "E0-S0 outputs are private previews and production gates are not implemented"
            .to_owned(),
    })
}

/// The one manifest version this build knows how to evaluate.
const PRODUCTION_MANIFEST_VERSION: &str = "1.0";

/// Just enough of any manifest to learn which shape to expect.
///
/// Deliberately not strict: the version is what says which fields are legal, so
/// a document cannot be held to a shape before it has been read.
#[derive(Debug, Deserialize)]
struct ManifestVersion {
    schema_version: Option<String>,
}

/// The provisional production-manifest shape, pending the E1-S1 versioned JSON
/// Schemas.
///
/// `deny_unknown_fields` because a top-level field this build does not know
/// is a field it cannot gate on, and publication must refuse what it cannot
/// evaluate rather than ignore it.
///
/// The rights sections stay as `Value` and are deserialized one at a time by
/// `declare_section`, so a malformed entry names the section it is in.
/// `serde_json` errors carry no field path, so typing them here would tell an
/// operator that something failed to parse without saying where.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductionManifest {
    schema_version: String,
    /// Typed, so a status this build does not know is a parse error here rather
    /// than a string carried past every gate that would have consulted it.
    release_status: ReleaseStatus,
    lesson_id: String,
    content_rights: Option<Value>,
    voice_profiles: Option<Value>,
}

/// A voice profile a production manifest declares it used.
///
/// Provisional shape pending the E1-S1 versioned JSON Schemas, like
/// `content_rights` below.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredVoiceProfile {
    profile_id: String,
    approval: RightsDecision,
    rights_record_id: String,
}

/// Rejects an identifier that parses but names nothing.
fn require_identifier(
    section: &'static str,
    field: &'static str,
    value: &str,
) -> Result<(), BuildError> {
    if value.trim().is_empty() {
        return Err(BuildError::EmptyManifestIdentifier { section, field });
    }
    Ok(())
}

/// Parses one rights section of a production manifest, naming the section when
/// it does not parse.
///
/// Borrows the subtree rather than cloning it: `&Value` is itself a
/// deserializer.
fn declare_section<'de, T: Deserialize<'de>>(
    section: &'static str,
    value: &'de Value,
) -> Result<Vec<T>, BuildError> {
    Vec::<T>::deserialize(value)
        .map_err(|source| BuildError::InvalidRightsDeclaration { section, source })
}

/// Always refuses publication until the production manifest and release gates
/// exist.
///
/// Every precondition this build can check runs before that refusal, so each is
/// reported as itself rather than as the generic gate refusal. They run
/// outward in: what the document claims to be, then what it claims about its
/// sources. The rights checks enforce
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Unresolved external
/// distribution blocks publish") over provisional `content_rights` and
/// `voice_profiles` manifest sections that the E1-S1 schema story will version.
pub fn validate_production_manifest(bytes: &[u8]) -> Result<(), BuildError> {
    // Two stages, because the version is what says which shape is legal: a
    // document of an unknown version must be reported as an unknown version,
    // not as a violation of a shape it never claimed.
    let declared_version: ManifestVersion = serde_json::from_slice(bytes)
        .map_err(|source| BuildError::MalformedProductionManifest { source })?;
    let version = declared_version
        .schema_version
        .unwrap_or_else(|| "missing".to_owned());
    if version != PRODUCTION_MANIFEST_VERSION {
        return Err(BuildError::UnsupportedProductionManifest { version });
    }

    let manifest: ProductionManifest = serde_json::from_slice(bytes)
        .map_err(|source| BuildError::MalformedProductionManifest { source })?;
    debug_assert_eq!(manifest.schema_version, PRODUCTION_MANIFEST_VERSION);

    // What the document claims to be, before what it claims about its sources.
    // Adjudicating the rights of a manifest that never asked to be published
    // would hand its author corrections for a release they did not request.
    if manifest.release_status != ReleaseStatus::ProductionRelease {
        return Err(BuildError::ManifestNotProductionRelease {
            declared: manifest.release_status,
        });
    }

    // Through the lesson rule rather than a blank check: this identifier names
    // the same output directory a lesson's does, so a manifest must not name
    // what a lesson could not.
    validate_lesson_id(&manifest.lesson_id)?;

    // An absent section and an empty one are the same claim: nothing was
    // classified. A production lesson always has at least one source.
    let declared = manifest
        .content_rights
        .as_ref()
        .map(|section| declare_section::<SourceRightsDeclaration>("content_rights", section))
        .transpose()?
        .unwrap_or_default();
    if declared.is_empty() {
        return Err(BuildError::MissingContentRightsDeclaration);
    }
    for source in &declared {
        require_identifier("content_rights", "source_id", &source.source_id)?;
        require_identifier(
            "content_rights",
            "rights_record_id",
            &source.rights_record_id,
        )?;
        if !source.classification.permits_production_release() {
            return Err(BuildError::UnresolvedContentRights {
                source_id: source.source_id.clone(),
                classification: source.classification.as_str().to_owned(),
            });
        }
    }

    if let Some(section) = &manifest.voice_profiles {
        for profile in declare_section::<DeclaredVoiceProfile>("voice_profiles", section)? {
            require_identifier("voice_profiles", "profile_id", &profile.profile_id)?;
            require_identifier(
                "voice_profiles",
                "rights_record_id",
                &profile.rights_record_id,
            )?;
            if profile.approval != RightsDecision::Approved {
                return Err(BuildError::Voice(VoiceError::ProfileNotApproved {
                    profile_id: profile.profile_id,
                    decision: profile.approval.as_str().to_owned(),
                }));
            }
        }
    }

    Err(BuildError::ProductionGatesUnavailable)
}
