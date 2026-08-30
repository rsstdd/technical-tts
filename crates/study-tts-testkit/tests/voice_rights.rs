//! T4 enforcement tests for E0-S2 voice consent, approval, checksum, and
//! content-rights gating.
//!
//! Test names are copied character for character from `DELIVERY-PLAN.md`
//! §E0-S2. The rules they enforce are tabulated in
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement.

use std::path::{Path, PathBuf};

use study_tts_core::{LessonError, ReleaseStatus, VoiceError};
use study_tts_runtime::{
    BuildError, BuildRequest, PublicationError, RightsError, ToolError, VoiceProfileError,
    build_preview, validate_production_manifest,
};
use study_tts_testkit::{
    DeterministicToneWorker, FIXTURE_VOICE_PROFILES, VoiceProfileFixtureSpec,
    cache_identity_fixture, walking_skeleton_fixture, write_voice_profile_fixture,
    write_voice_profile_root,
};
use tempfile::TempDir;

/// A request whose FFmpeg cannot exist: if the voice gate ran after tool
/// preflight, these tests would report a missing tool instead of the voice
/// refusal, so asserting the voice error also proves the gate runs before any
/// tool or synthesis work.
fn request_without_ffmpeg(workspace: &Path, voice_profile_root: &Path) -> BuildRequest {
    BuildRequest {
        lesson_path: walking_skeleton_fixture(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "study-tts-missing-ffmpeg".into(),
        ffprobe_executable: "study-tts-missing-ffprobe".into(),
        voice_profile_root: voice_profile_root.to_path_buf(),
    }
}

/// Installs a voice-profile root whose first-resolved profile carries `spec`.
///
/// The walking-skeleton lesson's first speaker names
/// `FIXTURE_VOICE_PROFILES[0]`, so a refusal is attributable to `spec` rather
/// than to whichever profile the gate happened to reach first; the rest are
/// written healthy so they refuse nothing. Returns the profile directory under
/// test, which the record-integrity tests below then damage.
fn root_with(workspace: &Path, spec: &VoiceProfileFixtureSpec) -> (PathBuf, PathBuf) {
    let root = workspace.join("voices");
    let under_test = write_voice_profile_fixture(&root.join(FIXTURE_VOICE_PROFILES[0]), spec);
    write_voice_profile_root(&root, &FIXTURE_VOICE_PROFILES[1..]);
    (root, under_test)
}

fn refused_build(spec: &VoiceProfileFixtureSpec) -> (BuildError, DeterministicToneWorker) {
    let workspace = TempDir::new().expect("create isolated workspace");
    let (root, _under_test) = root_with(workspace.path(), spec);
    let worker = DeterministicToneWorker::default();

    let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
        .expect_err("a refused voice profile must fail the build");
    (error, worker)
}

/// A manifest whose voices are declared and approved, so a test varying
/// `content_rights` is not refused for the section it is not exercising. Tests
/// that exercise `voice_profiles` overwrite the key.
fn production_manifest(content_rights: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0",
        "release_status": "production_release",
        "lesson_id": "e0-s2-rights",
        "content_rights": content_rights,
        "voice_profiles": [{
            "profile_id": "synthetic-test-voice-v1",
            "approval": "approved",
            "rights_record_id": "rights-voice-owner-fallback-v1",
        }],
    })
}

fn validate(manifest: &serde_json::Value) -> Result<(), BuildError> {
    validate_production_manifest(&serde_json::to_vec(manifest).expect("serialize manifest"))
}

#[test]
fn t4_e0_missing_voice_consent_blocks_profile_load() {
    let (error, worker) = refused_build(&VoiceProfileFixtureSpec {
        write_consent: false,
        ..VoiceProfileFixtureSpec::default()
    });

    assert!(
        matches!(
            error,
            BuildError::VoiceProfile(VoiceProfileError::MissingVoiceRecord { record, .. })
                if record == "consent.json"
        ),
        "missing consent produced `{error}`"
    );
    let message = error.to_string();
    assert!(
        message.contains("consent.json") && message.contains("project owner"),
        "refusal must name the missing record and the remedy owner: `{message}`"
    );
    assert!(
        !message.contains("delete"),
        "a consent refusal must not suggest deletion: `{message}`"
    );
    assert_eq!(worker.synthesis_count(), 0);

    for status in ["revoked", "pending"] {
        let (error, worker) = refused_build(&VoiceProfileFixtureSpec {
            consent_status: status.to_owned(),
            ..VoiceProfileFixtureSpec::default()
        });
        assert!(
            matches!(
                error,
                BuildError::Voice(VoiceError::ConsentNotGranted { status: ref reported, .. })
                    if reported == status
            ),
            "consent status `{status}` produced `{error}`"
        );
        assert_eq!(worker.synthesis_count(), 0);
    }
}

#[test]
fn t4_e0_unapproved_voice_profile_cannot_enter_preview_or_production() {
    // Positive control: an approved profile with granted consent passes the
    // gate, and the build then fails on the deliberately absent tool rather
    // than on the voice. This makes the refusals below attributable to approval
    // state alone without needing a real encoder — the full render is already
    // covered by the walking-skeleton suite.
    let (error, worker) = refused_build(&VoiceProfileFixtureSpec::default());
    assert!(
        matches!(error, BuildError::Tool(ToolError::MissingTool { .. })),
        "an approved profile must pass the gate and fail later, but produced `{error}`"
    );
    assert_eq!(worker.synthesis_count(), 0);

    // Preview: every non-approved rights decision refuses the profile before
    // any work.
    for decision in ["restricted", "review_required", "prohibited"] {
        let (error, worker) = refused_build(&VoiceProfileFixtureSpec {
            approval: decision.to_owned(),
            ..VoiceProfileFixtureSpec::default()
        });
        assert!(
            matches!(
                error,
                BuildError::Voice(VoiceError::ProfileNotApproved { decision: ref reported, .. })
                    if reported == decision
            ),
            "rights decision `{decision}` produced `{error}`"
        );
        assert_eq!(worker.synthesis_count(), 0);
    }

    // Production: a manifest declaring a non-approved profile is refused as
    // itself, not as the generic gate refusal.
    let mut manifest = production_manifest(serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "owner_authored",
        "rights_record_id": "rights-qualification-sources-v1",
    }]));
    manifest["voice_profiles"] = serde_json::json!([{
        "profile_id": "synthetic-test-voice-v1",
        "approval": "review_required",
        "rights_record_id": "rights-voice-nadia-v1",
    }]);

    let error = validate(&manifest).expect_err("an unapproved profile must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Voice(VoiceError::ProfileNotApproved { ref decision, .. })
                if decision == "review_required"
        ),
        "unapproved manifest profile produced `{error}`"
    );
}

/// The three ways a declared profile fails to resolve, each reported as
/// itself.
///
/// `Path::is_dir` would have collapsed all of these into one `false`, and with
/// it a permission failure into "the profile is not installed". Each row names
/// the entry the root holds and the refusal it must produce.
#[test]
fn t4_e1_a_declared_profile_that_does_not_resolve_is_refused_as_itself() {
    let declared = FIXTURE_VOICE_PROFILES[0];

    // Absent: the root holds nothing of that name.
    let workspace = TempDir::new().expect("create absent-profile workspace");
    let root = workspace.path().join("voices");
    write_voice_profile_root(&root, &FIXTURE_VOICE_PROFILES[1..]);
    let worker = DeterministicToneWorker::default();
    let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
        .expect_err("a profile the root does not hold must be refused");
    assert!(
        matches!(
            error,
            BuildError::VoiceProfile(VoiceProfileError::MissingVoiceProfileDirectory {
                ref profile_id,
                ..
            }) if profile_id == declared
        ),
        "an absent profile produced `{error}`"
    );
    assert_eq!(worker.synthesis_count(), 0);

    // Present but not a directory: installing it is not the remedy, so this
    // must not report the profile as missing.
    let workspace = TempDir::new().expect("create not-a-directory workspace");
    let root = workspace.path().join("voices");
    write_voice_profile_root(&root, &FIXTURE_VOICE_PROFILES[1..]);
    std::fs::write(root.join(declared), b"not a profile directory")
        .expect("write a file where a profile directory belongs");
    let worker = DeterministicToneWorker::default();
    let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
        .expect_err("an entry that is not a directory must be refused");
    assert!(
        matches!(
            error,
            BuildError::VoiceProfile(VoiceProfileError::VoiceProfileNotDirectory {
                ref profile_id,
                ..
            }) if profile_id == declared
        ),
        "a non-directory profile entry produced `{error}`"
    );
    assert_eq!(worker.synthesis_count(), 0);

    // Present and complete, but the record inside calls itself something else.
    // Accepting it would attribute one voice's consent record to another
    // voice's audio.
    let workspace = TempDir::new().expect("create mismatched-identity workspace");
    let root = workspace.path().join("voices");
    write_voice_profile_root(&root, &FIXTURE_VOICE_PROFILES[1..]);
    write_voice_profile_fixture(
        &root.join(declared),
        &VoiceProfileFixtureSpec {
            profile_id: "some-other-voice-v1".to_owned(),
            ..VoiceProfileFixtureSpec::default()
        },
    );
    let worker = DeterministicToneWorker::default();
    let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
        .expect_err("a record naming another identity must be refused");
    assert!(
        matches!(
            error,
            BuildError::VoiceProfile(VoiceProfileError::VoiceProfileIdMismatch {
                declared: ref reported,
                ref recorded,
            }) if reported == declared && recorded == "some-other-voice-v1"
        ),
        "a mismatched profile identity produced `{error}`"
    );
    assert_eq!(worker.synthesis_count(), 0);
}

/// Two speakers may name one voice profile, and the build resolves it.
///
/// Names what this can observe from outside the crate. That the profile is
/// read *once* is structural — `resolve_speakers` keys its work by profile
/// identity, not by speaker — and no seam here can count reads, so this test
/// does not claim it. The reason the distinction matters is recorded on that
/// function: two reads could return two digests if the profile changed between
/// them, keying two segments of one build on two versions of one voice.
#[test]
fn t4_e1_two_speakers_may_share_one_voice_profile() {
    let workspace = TempDir::new().expect("create shared-profile workspace");
    let root = workspace.path().join("voices");
    write_voice_profile_root(&root, &FIXTURE_VOICE_PROFILES);

    // The cache-identity fixture maps both its speakers to one profile, and
    // its `seg-a`/`seg-f` pair differs only by speaker.
    let lesson = cache_identity_fixture();
    let mut request = request_without_ffmpeg(workspace.path(), &root);
    request.lesson_path = lesson;
    let worker = DeterministicToneWorker::default();

    let error = build_preview(request, &worker)
        .expect_err("the absent encoder must be what fails, not the voices");

    assert!(
        matches!(error, BuildError::Tool(ToolError::MissingTool { .. })),
        "two speakers sharing one profile must resolve, but produced `{error}`"
    );
    assert_eq!(worker.synthesis_count(), 0);
}

#[test]
fn t4_e0_voice_checksum_mismatch_blocks_use() {
    for tampered in ["reference.wav", "conditionals.pt"] {
        let workspace = TempDir::new().expect("create isolated workspace");
        let (root, voice_dir) = root_with(workspace.path(), &VoiceProfileFixtureSpec::default());
        let tampered_path = voice_dir.join(tampered);
        std::fs::write(&tampered_path, b"tampered-after-consent")
            .expect("overwrite fixture file to break its checksum");

        let worker = DeterministicToneWorker::default();
        let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
            .expect_err("a checksum mismatch must refuse the profile");

        assert!(
            matches!(
                error,
                BuildError::VoiceProfile(VoiceProfileError::VoiceChecksumMismatch {
                    ref path,
                    ..
                }) if path == &tampered_path
            ),
            "tampering `{tampered}` produced `{error}`"
        );
        let message = error.to_string();
        assert!(
            message.contains(tampered) && message.contains("rights record"),
            "refusal must name the mismatched file and the remedy: `{message}`"
        );
        assert_eq!(worker.synthesis_count(), 0);
    }
}

#[test]
fn t4_e0_production_release_rejects_unresolved_content_rights_classification() {
    for unresolved in ["rights_review_required", "evaluation_only", "prohibited"] {
        let manifest = production_manifest(serde_json::json!([
            {
                "source_id": "lesson-source-1",
                "classification": "owner_authored",
                "rights_record_id": "rights-qualification-sources-v1",
            },
            {
                "source_id": "external-source-1",
                "classification": unresolved,
                "rights_record_id": "rights-qualification-sources-v1",
            },
        ]));

        let error =
            validate(&manifest).expect_err("an unresolved classification must refuse production");
        assert!(
            matches!(
                error,
                BuildError::Rights(RightsError::UnresolvedContentRights {
                    ref source_id,
                    ref classification,
                })
                    if source_id == "external-source-1" && classification == unresolved
            ),
            "classification `{unresolved}` produced `{error}`"
        );
    }

    // A fully resolved declaration still meets the gate refusal: the rights
    // check is a precondition ahead of the unimplemented production gates, not
    // a bypass of them.
    let resolved = production_manifest(serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "owner_authored",
        "rights_record_id": "rights-qualification-sources-v1",
    }]));
    assert!(matches!(
        validate(&resolved),
        Err(BuildError::Publication(
            PublicationError::ProductionGatesUnavailable
        ))
    ));

    // Declaring nothing and declaring something unresolved are different claims
    // with different remedies, so an absent section and an empty one are both
    // refused as their own error rather than as the gate refusal.
    let mut undeclared = production_manifest(serde_json::json!([]));
    assert!(
        matches!(
            validate(&undeclared),
            Err(BuildError::Rights(
                RightsError::MissingContentRightsDeclaration
            ))
        ),
        "an empty classification list must be refused as undeclared"
    );
    undeclared
        .as_object_mut()
        .expect("manifest is an object")
        .remove("content_rights");
    let error = validate(&undeclared).expect_err("an undeclared manifest must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Rights(RightsError::MissingContentRightsDeclaration)
        ),
        "undeclared rights produced `{error}`"
    );

    // An unknown classification value is a parse error, never silently
    // accepted, and it names the rights section rather than reporting that some
    // JSON somewhere failed to parse.
    let unknown = production_manifest(serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "unclassified",
        "rights_record_id": "rights-qualification-sources-v1",
    }]));
    let error = validate(&unknown).expect_err("an unknown classification must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Rights(RightsError::InvalidRightsDeclaration { section, .. })
                if section == "content_rights"
        ),
        "unknown classification produced `{error}`"
    );
}

/// The two rights sections are held to one rule. `content_rights` already
/// refuses an absent section and an empty one as the same claim; this pins
/// `voice_profiles` to the same refusal, so a manifest cannot omit the voices
/// it was rendered with while being held to name its sources.
///
/// Not a `DELIVERY-PLAN.md` §E0-S2 name: it guards the symmetry the two
/// declaration sections share rather than one planned behavior.
#[test]
fn t4_e0_production_release_rejects_an_undeclared_voice_profile() {
    let mut undeclared = production_manifest(serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "owner_authored",
        "rights_record_id": "rights-qualification-sources-v1",
    }]));

    undeclared["voice_profiles"] = serde_json::json!([]);
    assert!(
        matches!(
            validate(&undeclared),
            Err(BuildError::Rights(
                RightsError::MissingVoiceProfileDeclaration
            ))
        ),
        "an empty voice_profiles list must be refused as undeclared"
    );

    undeclared
        .as_object_mut()
        .expect("manifest is an object")
        .remove("voice_profiles");
    let error =
        validate(&undeclared).expect_err("an undeclared voice profile must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Rights(RightsError::MissingVoiceProfileDeclaration)
        ),
        "an absent voice_profiles section produced `{error}`"
    );
    let message = error.to_string();
    assert!(
        message.contains("voice_profiles") && message.contains("project owner"),
        "refusal must name the section and the remedy owner: `{message}`"
    );
}

/// The manifest is an external contract, so its whole shape is refused when it
/// is wrong — not only the parts a rights check happens to read.
#[test]
fn t3_e0_production_manifest_is_a_strict_typed_boundary() {
    let resolved = serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "owner_authored",
        "rights_record_id": "rights-qualification-sources-v1",
    }]);

    // Bytes that are not JSON at all are still reported as a manifest failure,
    // so the refusal names the artifact the operator has to correct.
    let error = validate_production_manifest(b"{ not json")
        .expect_err("malformed bytes must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Publication(PublicationError::MalformedProductionManifest { .. })
        ),
        "malformed JSON produced `{error}`"
    );

    // A field this build cannot evaluate must not be published past. The
    // version is read first, so this is refused as a shape violation of version
    // 1.0 rather than as an unknown version.
    let mut extra = production_manifest(resolved.clone());
    extra["unexpected_field"] = serde_json::json!(true);
    let error = validate(&extra).expect_err("an unknown top-level field must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Publication(PublicationError::MalformedProductionManifest { .. })
        ),
        "unknown top-level field produced `{error}`"
    );

    // The typed fields are gated, not merely parsed. A status this build does
    // not know never becomes a value a later gate could consult, and a manifest
    // that does not claim production release is refused as the preview it says
    // it is rather than reaching the missing-gates refusal.
    let mut unknown_status = production_manifest(resolved.clone());
    unknown_status["release_status"] = serde_json::json!("released");
    let error =
        validate(&unknown_status).expect_err("an unknown release status must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Publication(PublicationError::MalformedProductionManifest { .. })
        ),
        "unknown release status produced `{error}`"
    );

    let mut preview = production_manifest(resolved.clone());
    preview["release_status"] = serde_json::json!("private_preview");
    let error = validate(&preview).expect_err("a private preview must refuse production");
    assert!(
        matches!(
            error,
            BuildError::Publication(PublicationError::ManifestNotProductionRelease {
                declared: ReleaseStatus::PrivatePreview
            })
        ),
        "a private-preview manifest produced `{error}`"
    );
    let message = error.to_string();
    assert!(
        message.contains("private_preview") && message.contains("project owner"),
        "refusal must quote the declared status and name the remedy owner: `{message}`"
    );

    // The lesson identifier names an output directory, so the manifest holds it
    // to the rule lessons are held to rather than accepting anything non-empty.
    let refuse_lesson_id = |lesson_id: &str| {
        let mut named = production_manifest(resolved.clone());
        named["lesson_id"] = serde_json::json!(lesson_id);
        validate(&named).expect_err("an unusable lesson_id must refuse production")
    };

    // Absent and malformed stay different authoring mistakes here, exactly as
    // they are for a lesson.
    for absent in ["", "   "] {
        let error = refuse_lesson_id(absent);
        assert!(
            matches!(
                error,
                BuildError::Lesson(ref diagnostic)
                    if matches!(diagnostic.error(), LessonError::MissingLessonId)
            ),
            "blank lesson_id `{absent}` produced `{error}`"
        );
    }
    for malformed in ["../escape", ".hidden", "lesson id"] {
        let error = refuse_lesson_id(malformed);
        assert!(
            matches!(
                error,
                BuildError::Lesson(ref diagnostic)
                    if matches!(
                        diagnostic.error(),
                        LessonError::InvalidLessonId(reported) if reported == malformed
                    )
            ),
            "lesson_id `{malformed}` produced `{error}`"
        );
    }

    // A blank identifier parses and then traces to no record, so it would
    // satisfy the classification check while naming nothing.
    for (section, field, manifest) in [
        ("content_rights", "source_id", {
            let mut blank = resolved.clone();
            blank[0]["source_id"] = serde_json::json!("   ");
            production_manifest(blank)
        }),
        ("content_rights", "rights_record_id", {
            let mut blank = resolved.clone();
            blank[0]["rights_record_id"] = serde_json::json!("");
            production_manifest(blank)
        }),
        ("voice_profiles", "profile_id", {
            let mut with_voice = production_manifest(resolved.clone());
            with_voice["voice_profiles"] = serde_json::json!([{
                "profile_id": "",
                "approval": "approved",
                "rights_record_id": "rights-voice-owner-fallback-v1",
            }]);
            with_voice
        }),
        ("voice_profiles", "rights_record_id", {
            let mut with_voice = production_manifest(resolved.clone());
            with_voice["voice_profiles"] = serde_json::json!([{
                "profile_id": "synthetic-test-voice-v1",
                "approval": "approved",
                "rights_record_id": "  ",
            }]);
            with_voice
        }),
    ] {
        let error = validate(&manifest).expect_err("a blank identifier must refuse production");
        assert!(
            matches!(
                error,
                BuildError::Rights(RightsError::EmptyManifestIdentifier {
                    section: reported_section,
                    field: reported_field,
                }) if reported_section == section && reported_field == field
            ),
            "blank `{section}.{field}` produced `{error}`"
        );
    }
}

/// Every record the ADR-0001 §12.1 layout requires refuses the same way when it
/// is absent, so no one of them degrades into a bare filesystem error.
#[test]
fn t4_e0_every_absent_voice_record_names_its_remedy_owner() {
    for record in [
        "profile.json",
        "consent.json",
        "reference.wav",
        "conditionals.pt",
    ] {
        let workspace = TempDir::new().expect("create isolated workspace");
        let (root, voice_dir) = root_with(workspace.path(), &VoiceProfileFixtureSpec::default());
        std::fs::remove_file(voice_dir.join(record)).expect("remove a required record");
        let worker = DeterministicToneWorker::default();

        let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
            .expect_err("an absent record must refuse the profile");

        assert!(
            matches!(
                error,
                BuildError::VoiceProfile(VoiceProfileError::MissingVoiceRecord {
                    record: reported,
                    ..
                }) if reported == record
            ),
            "removing `{record}` produced `{error}`"
        );
        let message = error.to_string();
        assert!(
            message.contains(record) && message.contains("project owner"),
            "removing `{record}` did not name the record and the remedy owner: `{message}`"
        );
        assert_eq!(worker.synthesis_count(), 0);
    }
}

/// A record that is present but is not a regular file is refused before its
/// bytes are read.
///
/// `reference.wav` is the load-bearing case, and the link here points at an
/// untouched copy of the fixture so its digest still matches what the profile
/// records. Without this refusal the gate would read the bytes and compute the
/// digest through the same link and agree with itself, admitting audio from
/// outside the profile directory that the consent record never covered.
#[cfg(unix)]
#[test]
fn t4_e0_voice_records_that_are_not_regular_files_are_refused() {
    use std::os::unix::fs::symlink;

    for record in [
        "profile.json",
        "consent.json",
        "reference.wav",
        "conditionals.pt",
    ] {
        let workspace = TempDir::new().expect("create isolated workspace");
        let (root, voice_dir) = root_with(workspace.path(), &VoiceProfileFixtureSpec::default());
        let outside = workspace.path().join("outside");
        std::fs::create_dir(&outside).expect("create the directory the link points into");
        let target = outside.join(record);
        let planted = voice_dir.join(record);
        std::fs::rename(&planted, &target).expect("move a required record out of the profile");
        symlink(&target, &planted).expect("plant a voice record symlink");
        let worker = DeterministicToneWorker::default();

        let error = build_preview(request_without_ffmpeg(workspace.path(), &root), &worker)
            .expect_err("a planted record must refuse the profile");

        assert!(
            matches!(
                error,
                BuildError::VoiceProfile(VoiceProfileError::VoiceRecordNotRegularFile {
                    record: reported,
                    ..
                })
                    if reported == record
            ),
            "a symlinked `{record}` produced `{error}`"
        );
        let message = error.to_string();
        assert!(
            message.contains(record) && message.contains("project owner"),
            "a symlinked `{record}` did not name the record and the remedy owner: `{message}`"
        );
        assert_eq!(worker.synthesis_count(), 0);
    }
}
