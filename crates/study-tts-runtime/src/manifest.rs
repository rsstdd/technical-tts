//! `manifest.json`: the record of what a build produced and what produced it.
//!
//! Every value written here is derived rather than restated — the artifact
//! names from the constants `pipeline` writes the files at, the release status
//! from the typed value, the digests from the files themselves. A manifest
//! that could disagree with the build it describes is worse than no manifest,
//! because `validate_production_manifest` gates on what it says.

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use study_tts_core::{AudioDigest, CacheKey, PlanHash, ReleaseStatus, ToolProfileHash};

use crate::{
    BuildError, DurableStateError,
    cache::{ValidatedCachedArtifact, hash_file},
    durable::{DurableFileSystem, write_json_atomically},
    export::{ToolExecution, ToolProfile},
    tools::ToolIdentity,
};

/// The `schema_version` a `manifest.json` this build writes carries.
///
/// Independent of `CACHE_SCHEMA_VERSION` and the lesson schema: each versions a
/// different document and moves separately. `manifest-v0.schema.json` describes
/// this layout and only this one, because that schema is generated from the one
/// stored Rust shape.
///
/// `docs/architecture/WALKING-SKELETON.md` names both constants in its
/// provisional package-manifest paragraph, and records why reconciliation still
/// reads the legacy layout and why only the current one is published.
const CURRENT_MANIFEST_LAYOUT_VERSION: &str = "0.2-skeleton";

/// The `schema_version` of the E0 walking-skeleton layout.
///
/// Written before tool argument profiles were recorded. Read so an existing
/// package can be reconciled; never written, and never reusable as a matching
/// tool-profile generation.
const LEGACY_MANIFEST_LAYOUT_VERSION: &str = "0.1-skeleton";

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
    argument_profile_blake3: &'a ToolProfileHash,
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

/// Tool and normalized argument identities required for generation reuse.
pub(crate) struct ToolExpectations<'a> {
    /// FFmpeg binary identity required by this build.
    pub ffmpeg: &'a ToolIdentity,
    /// FFmpeg argument profile required by this build.
    pub ffmpeg_profile: &'a ToolProfile,
    /// ffprobe binary identity required by this build.
    pub ffprobe: &'a ToolIdentity,
    /// ffprobe argument profile required by this build.
    pub ffprobe_profile: &'a ToolProfile,
}

/// Completed build data from which the minimal manifest is derived.
pub(crate) struct ManifestRecords<'a> {
    /// Validated lesson identity.
    pub lesson_id: &'a str,
    /// Deterministic plan identity.
    pub plan_hash: &'a PlanHash,
    /// Selected validated cache segments.
    pub segments: &'a [ValidatedCachedArtifact],
    /// Canonical master inside the staged package.
    pub master_wav: &'a Path,
    /// Encoded output inside the staged package.
    pub m4a: &'a Path,
    /// Tool identities and executed arguments.
    pub tools: ToolRecords<'a>,
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
/// [`crate::IoError::FileSystem`] if either artifact cannot be read for hashing
/// or the manifest cannot be written; [`crate::IoError::WriteJson`] if
/// serialization fails.
pub(crate) fn write(
    filesystem: &dyn DurableFileSystem,
    destination: &Path,
    records: ManifestRecords<'_>,
) -> Result<(), BuildError> {
    let manifest = Manifest {
        schema_version: CURRENT_MANIFEST_LAYOUT_VERSION,
        // The typed value, not a hand-written spelling of it. A literal here
        // would keep whatever it said if `ReleaseStatus` were ever respelled,
        // and this field is what `validate_production_manifest` gates on.
        release_status: ReleaseStatus::PrivatePreview,
        lesson_id: records.lesson_id,
        plan_hash: records.plan_hash,
        segments: records
            .segments
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
                blake3: hash_file(records.master_wav)?,
            },
            m4a: Artifact {
                path: M4A_NAME,
                blake3: hash_file(records.m4a)?,
            },
        },
        tools: Tools {
            ffmpeg: ToolUse {
                resolved_executable: records
                    .tools
                    .ffmpeg
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &records.tools.ffmpeg.version,
                arguments: &records.tools.ffmpeg_execution.arguments,
                argument_profile_blake3: &records.tools.ffmpeg_execution.argument_profile_blake3,
            },
            ffprobe: ToolUse {
                resolved_executable: records
                    .tools
                    .ffprobe
                    .resolved_executable
                    .display()
                    .to_string(),
                version: &records.tools.ffprobe.version,
                arguments: &records.tools.ffprobe_execution.arguments,
                argument_profile_blake3: &records.tools.ffprobe_execution.argument_profile_blake3,
            },
        },
    };
    write_json_atomically(filesystem, destination, &manifest)
}

/// Publishes the one layout `manifest-v0.schema.json` describes.
///
/// [`validate_package`] also reads [`LEGACY_MANIFEST_LAYOUT_VERSION`], and that
/// is deliberately not listed: the legacy layout carries a different `tools`
/// shape, and this schema is generated from the current one. A schema admitting
/// a version whose other fields it describes wrongly is worse than one that
/// admits fewer, and the legacy layout is read to be migrated rather than
/// authored against.
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
    serde_json::Value::from(schemars::schema_for!(
        StoredManifest<StoredTools<CurrentStoredToolUse>>
    ))
}

/// Strict owned shape used when an immutable package is reconciled or reused.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredManifest<T> {
    #[schemars(schema_with = "schema_version_json_schema")]
    schema_version: String,
    release_status: ReleaseStatus,
    lesson_id: String,
    plan_hash: PlanHash,
    segments: Vec<StoredManifestSegment>,
    artifacts: StoredArtifacts,
    tools: T,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredManifestSegment {
    segment_id: String,
    cache_key: CacheKey,
    audio_blake3: AudioDigest,
    frames: u32,
    pause_after_ms: u32,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredArtifacts {
    master_wav: StoredArtifact,
    m4a: StoredArtifact,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredArtifact {
    path: String,
    blake3: AudioDigest,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct StoredTools<T> {
    ffmpeg: T,
    ffprobe: T,
}

#[derive(Debug)]
struct StoredToolUse {
    resolved_executable: String,
    version: String,
    arguments: Vec<String>,
    argument_profile_blake3: Option<ToolProfileHash>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct CurrentStoredToolUse {
    resolved_executable: String,
    version: String,
    arguments: Vec<String>,
    argument_profile_blake3: ToolProfileHash,
}

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

impl<T> StoredManifest<T> {
    fn map_tools<U>(self, map: impl FnOnce(T) -> U) -> StoredManifest<U> {
        StoredManifest {
            schema_version: self.schema_version,
            release_status: self.release_status,
            lesson_id: self.lesson_id,
            plan_hash: self.plan_hash,
            segments: self.segments,
            artifacts: self.artifacts,
            tools: map(self.tools),
        }
    }
}

impl From<StoredTools<CurrentStoredToolUse>> for StoredTools<StoredToolUse> {
    fn from(tools: StoredTools<CurrentStoredToolUse>) -> Self {
        Self {
            ffmpeg: StoredToolUse {
                resolved_executable: tools.ffmpeg.resolved_executable,
                version: tools.ffmpeg.version,
                arguments: tools.ffmpeg.arguments,
                argument_profile_blake3: Some(tools.ffmpeg.argument_profile_blake3),
            },
            ffprobe: StoredToolUse {
                resolved_executable: tools.ffprobe.resolved_executable,
                version: tools.ffprobe.version,
                arguments: tools.ffprobe.arguments,
                argument_profile_blake3: Some(tools.ffprobe.argument_profile_blake3),
            },
        }
    }
}

impl From<StoredTools<LegacyStoredToolUse>> for StoredTools<StoredToolUse> {
    fn from(tools: StoredTools<LegacyStoredToolUse>) -> Self {
        Self {
            ffmpeg: StoredToolUse {
                resolved_executable: tools.ffmpeg.resolved_executable,
                version: tools.ffmpeg.version,
                arguments: tools.ffmpeg.arguments,
                argument_profile_blake3: tools.ffmpeg.argument_profile_blake3,
            },
            ffprobe: StoredToolUse {
                resolved_executable: tools.ffprobe.resolved_executable,
                version: tools.ffprobe.version,
                arguments: tools.ffprobe.arguments,
                argument_profile_blake3: tools.ffprobe.argument_profile_blake3,
            },
        }
    }
}

/// Validates the files and strict manifest inside an immutable package.
///
/// When expected tool identities are supplied, this also decides whether a
/// no-op rebuild may reuse the generation without rerunning FFmpeg.
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
    tools: Option<ToolExpectations<'_>>,
) -> Result<bool, BuildError> {
    let manifest_path = package_dir.join(MANIFEST_NAME);
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
    // artifact's `blake3`, and each tool's `argument_profile_blake3` are value
    // objects, so a malformed one was refused by the parse above and carries
    // that type's own remedy routing. What remains here is what a type cannot
    // say: a field that is well formed and still wrong for this package.
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
        // Read so the compiler agrees the fields are part of the shape this
        // build accepts. `cache_key` and `audio_blake3` are value objects and
        // `pause_after_ms` is bounded by its width, so the parse already said
        // everything there is to say about all three.
        let _ = (
            &segment.cache_key,
            &segment.audio_blake3,
            segment.pause_after_ms,
        );
    }
    validate_artifact(
        package_dir,
        &manifest_path,
        &manifest.artifacts.master_wav,
        MASTER_WAV_NAME,
    )?;
    validate_artifact(
        package_dir,
        &manifest_path,
        &manifest.artifacts.m4a,
        M4A_NAME,
    )?;
    validate_tool_record(&manifest_path, "FFmpeg", &manifest.tools.ffmpeg)?;
    validate_tool_record(&manifest_path, "ffprobe", &manifest.tools.ffprobe)?;

    let plan_matches = plan_hash.is_none_or(|expected| manifest.plan_hash.as_str() == expected);
    let tools_match = tools.is_none_or(|expected| {
        tool_matches(
            &manifest.tools.ffmpeg,
            expected.ffmpeg,
            expected.ffmpeg_profile,
        ) && tool_matches(
            &manifest.tools.ffprobe,
            expected.ffprobe,
            expected.ffprobe_profile,
        )
    });
    Ok(plan_matches && tools_match)
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
) -> Result<StoredManifest<StoredTools<StoredToolUse>>, BuildError> {
    match version {
        LEGACY_MANIFEST_LAYOUT_VERSION => Ok(parse_manifest::<
            StoredManifest<StoredTools<LegacyStoredToolUse>>,
        >(bytes, manifest_path)?
        .map_tools(StoredTools::from)),
        CURRENT_MANIFEST_LAYOUT_VERSION => Ok(parse_manifest::<
            StoredManifest<StoredTools<CurrentStoredToolUse>>,
        >(bytes, manifest_path)?
        .map_tools(StoredTools::from)),
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
    artifact: &StoredArtifact,
    required_name: &'static str,
) -> Result<(), BuildError> {
    if artifact.path != required_name {
        return Err(DurableStateError::UnexpectedPackageArtifactPath {
            manifest: manifest_path.to_path_buf(),
            recorded: artifact.path.clone(),
            required: required_name,
        }
        .into());
    }
    let path = package_dir.join(required_name);
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

fn validate_tool_record(
    path: &Path,
    tool: &'static str,
    recorded: &StoredToolUse,
) -> Result<(), BuildError> {
    if recorded.arguments.is_empty() {
        return Err(DurableStateError::MissingPackageToolArguments {
            path: path.to_path_buf(),
            tool,
        }
        .into());
    }
    Ok(())
}

fn tool_matches(recorded: &StoredToolUse, expected: &ToolIdentity, profile: &ToolProfile) -> bool {
    recorded.resolved_executable == expected.resolved_executable.display().to_string()
        && recorded.version == expected.version
        && recorded.argument_profile_blake3.as_ref() == Some(profile.identity())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;
    use tempfile::TempDir;

    use super::*;
    use crate::{durable::OsDurableFileSystem, export};

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

    fn write_test_package(package: &Path) {
        std::fs::create_dir(package).expect("create test package");
        std::fs::write(package.join(MASTER_WAV_NAME), b"master").expect("write master");
        std::fs::write(package.join(M4A_NAME), b"encoded").expect("write export");
        let segment = cached_segment(blake3::hash(b"segment").to_hex().to_string());
        let plan_hash = PlanHash::from(blake3::hash(b"plan"));
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();
        let ffmpeg_execution = ToolExecution {
            arguments: vec!["encode".to_owned()],
            argument_profile_blake3: profiles.ffmpeg.identity().to_owned(),
        };
        let ffprobe_execution = ToolExecution {
            arguments: vec!["probe".to_owned()],
            argument_profile_blake3: profiles.ffprobe.identity().to_owned(),
        };
        write(
            &OsDurableFileSystem,
            &package.join(MANIFEST_NAME),
            ManifestRecords {
                lesson_id: "lesson",
                plan_hash: &plan_hash,
                segments: std::slice::from_ref(&segment),
                master_wav: &package.join(MASTER_WAV_NAME),
                m4a: &package.join(M4A_NAME),
                tools: ToolRecords {
                    ffmpeg: &ffmpeg,
                    ffmpeg_execution: &ffmpeg_execution,
                    ffprobe: &ffprobe,
                    ffprobe_execution: &ffprobe_execution,
                },
            },
        )
        .expect("write test manifest");
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

    fn remove_tool_profiles(package: &Path, schema_version: &str) {
        rewrite_test_manifest(package, |manifest| {
            manifest["schema_version"] = Value::String(schema_version.to_owned());
            for tool in ["ffmpeg", "ffprobe"] {
                manifest["tools"][tool]
                    .as_object_mut()
                    .expect("tool record is an object")
                    .remove("argument_profile_blake3");
            }
        });
    }

    /// The published schema and the parser agree on which layouts exist.
    ///
    /// The gap this closes is not that the legacy layout is unpublished — that
    /// is deliberate, and `schema_version_json_schema` says why. It is that
    /// nothing held the two facts together: `validate_package` read two
    /// layouts while `manifest-v0.schema.json` described one, and a third
    /// layout could have been added to the parser leaving the schema silently
    /// describing a shrinking fraction of what this build accepts.
    ///
    /// An empty object is enough to tell the two refusals apart, and that is
    /// the whole trick: a known layout reaches a decoder and is refused for its
    /// *shape*, while an unread one is refused for its version before any byte
    /// is decoded. What each decoder then accepts is proved by the legacy and
    /// current package tests above, which read real packages.
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

        let error = parse_stored_manifest(b"{}", path, "0.3-skeleton")
            .expect_err("an unread layout must be refused rather than decoded");

        assert!(
            matches!(
                error,
                BuildError::DurableState(ref error)
                    if matches!(
                        error.as_ref(),
                        DurableStateError::UnsupportedPackageManifest { found, required, .. }
                            if found == "0.3-skeleton"
                                && *required == CURRENT_MANIFEST_LAYOUT_VERSION
                    )
            ),
            "an unread layout must be refused by its version, naming the one this build \
             writes: {error:?}"
        );
    }

    #[test]
    fn t4_e0_legacy_package_manifest_without_tool_profiles_remains_valid() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        remove_tool_profiles(&package, LEGACY_MANIFEST_LAYOUT_VERSION);

        assert!(
            validate_package(&package, "lesson", None, None)
                .expect("legacy package remains structurally valid")
        );

        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();
        assert!(
            !validate_package(
                &package,
                "lesson",
                None,
                Some(ToolExpectations {
                    ffmpeg: &ffmpeg,
                    ffmpeg_profile: &profiles.ffmpeg,
                    ffprobe: &ffprobe,
                    ffprobe_profile: &profiles.ffprobe,
                }),
            )
            .expect("legacy package remains valid but cannot prove its tool profiles")
        );
    }

    #[test]
    fn t4_e0_legacy_package_manifest_with_tool_profiles_remains_reusable() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        rewrite_test_manifest(&package, |manifest| {
            manifest["schema_version"] = Value::String(LEGACY_MANIFEST_LAYOUT_VERSION.to_owned());
        });
        let (ffmpeg, ffprobe) = test_tool_identities();
        let profiles = export::export_profiles();

        assert!(
            validate_package(
                &package,
                "lesson",
                None,
                Some(ToolExpectations {
                    ffmpeg: &ffmpeg,
                    ffmpeg_profile: &profiles.ffmpeg,
                    ffprobe: &ffprobe,
                    ffprobe_profile: &profiles.ffprobe,
                }),
            )
            .expect("legacy package with profiles remains reusable")
        );
    }

    #[test]
    fn t4_e0_current_package_manifest_requires_tool_profiles() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let package = workspace.path().join("package");
        write_test_package(&package);
        remove_tool_profiles(&package, CURRENT_MANIFEST_LAYOUT_VERSION);

        let error = validate_package(&package, "lesson", None, None)
            .expect_err("current package must require tool profiles");

        assert!(matches!(
            error,
            BuildError::DurableState(error)
                if matches!(*error, DurableStateError::MalformedPackageManifest { .. })
        ));
    }

    #[test]
    fn t4_e0_encoding_profile_change_names_a_new_package_generation() {
        let workspace = TempDir::new().expect("create manifest workspace");
        let first = workspace.path().join("first");
        let changed = workspace.path().join("changed");
        std::fs::create_dir(&first).expect("create first package");
        std::fs::create_dir(&changed).expect("create changed package");
        for package in [&first, &changed] {
            std::fs::write(package.join(MASTER_WAV_NAME), b"master").expect("write master");
            std::fs::write(package.join(M4A_NAME), b"encoded").expect("write export");
        }
        let audio_blake3 = blake3::hash(b"segment").to_hex().to_string();
        let segment = cached_segment(audio_blake3);
        let plan_hash = PlanHash::from(blake3::hash(b"plan"));
        let ffmpeg = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffmpeg"),
            version: "ffmpeg version 1".to_owned(),
        };
        let ffprobe = ToolIdentity {
            resolved_executable: PathBuf::from("/tools/ffprobe"),
            version: "ffprobe version 1".to_owned(),
        };
        let profiles = export::export_profiles();
        let changed_ffmpeg = ToolProfile::new(
            "ffmpeg",
            &["-i", "{input_path}", "-c:a", "libopus", "{output_path}"],
        );
        let first_ffmpeg_execution = ToolExecution {
            arguments: vec!["first".to_owned()],
            argument_profile_blake3: profiles.ffmpeg.identity().to_owned(),
        };
        let changed_ffmpeg_execution = ToolExecution {
            arguments: vec!["changed".to_owned()],
            argument_profile_blake3: changed_ffmpeg.identity().to_owned(),
        };
        let ffprobe_execution = ToolExecution {
            arguments: vec!["probe".to_owned()],
            argument_profile_blake3: profiles.ffprobe.identity().to_owned(),
        };

        for (package, ffmpeg_execution) in [
            (&first, &first_ffmpeg_execution),
            (&changed, &changed_ffmpeg_execution),
        ] {
            write(
                &OsDurableFileSystem,
                &package.join(MANIFEST_NAME),
                ManifestRecords {
                    lesson_id: "lesson",
                    plan_hash: &plan_hash,
                    segments: std::slice::from_ref(&segment),
                    master_wav: &package.join(MASTER_WAV_NAME),
                    m4a: &package.join(M4A_NAME),
                    tools: ToolRecords {
                        ffmpeg: &ffmpeg,
                        ffmpeg_execution,
                        ffprobe: &ffprobe,
                        ffprobe_execution: &ffprobe_execution,
                    },
                },
            )
            .expect("write package manifest");
        }

        assert_ne!(
            hash_file(&first.join(MANIFEST_NAME)).expect("hash first manifest"),
            hash_file(&changed.join(MANIFEST_NAME)).expect("hash changed manifest")
        );
        assert!(
            !validate_package(
                &first,
                "lesson",
                Some(plan_hash.as_str()),
                Some(ToolExpectations {
                    ffmpeg: &ffmpeg,
                    ffmpeg_profile: &changed_ffmpeg,
                    ffprobe: &ffprobe,
                    ffprobe_profile: &profiles.ffprobe,
                }),
            )
            .expect("the old package remains structurally valid")
        );
    }
}
