//! T4 enforcement tests for E0-S2 voice consent, approval, checksum, and
//! content-rights gating.
//!
//! Test names are copied character for character from `DELIVERY-PLAN.md`
//! §E0-S2. The rules they enforce are tabulated in
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Enforcement.

use std::path::Path;

use study_tts_core::{LessonError, ReleaseStatus, VoiceError};
use study_tts_runtime::{BuildError, BuildRequest, build_preview, validate_production_manifest};
use study_tts_testkit::{
    DeterministicToneWorker, VoiceProfileFixtureSpec, walking_skeleton_fixture,
    write_voice_profile_fixture,
};
use tempfile::TempDir;

/// A request whose FFmpeg cannot exist: if the voice gate ran after tool
/// preflight, these tests would report a missing tool instead of the voice
/// refusal, so asserting the voice error also proves the gate runs before any
/// tool or synthesis work.
fn request_without_ffmpeg(workspace: &Path, voice_dir: &Path) -> BuildRequest {
    BuildRequest {
        lesson_path: walking_skeleton_fixture(),
        workspace: workspace.to_path_buf(),
        ffmpeg_executable: "study-tts-missing-ffmpeg".into(),
        ffprobe_executable: "study-tts-missing-ffprobe".into(),
        voice_profile_dir: Some(voice_dir.to_path_buf()),
    }
}

fn refused_build(spec: &VoiceProfileFixtureSpec) -> (BuildError, DeterministicToneWorker) {
    let workspace = TempDir::new().expect("create isolated workspace");
    let voice_dir = write_voice_profile_fixture(&workspace.path().join("voice"), spec);
    let worker = DeterministicToneWorker::default();

    let error = build_preview(
        request_without_ffmpeg(workspace.path(), &voice_dir),
        &worker,
    )
    .expect_err("a refused voice profile must fail the build");
    (error, worker)
}

fn production_manifest(content_rights: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "1.0",
        "release_status": "production_release",
        "lesson_id": "e0-s2-rights",
        "content_rights": content_rights,
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
            BuildError::MissingVoiceRecord { record, .. } if record == "consent.json"
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
        matches!(error, BuildError::MissingTool { .. }),
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

#[test]
fn t4_e0_voice_checksum_mismatch_blocks_use() {
    for tampered in ["reference.wav", "conditionals.pt"] {
        let workspace = TempDir::new().expect("create isolated workspace");
        let voice_dir = write_voice_profile_fixture(
            &workspace.path().join("voice"),
            &VoiceProfileFixtureSpec::default(),
        );
        let tampered_path = voice_dir.join(tampered);
        std::fs::write(&tampered_path, b"tampered-after-consent")
            .expect("overwrite fixture file to break its checksum");

        let worker = DeterministicToneWorker::default();
        let error = build_preview(
            request_without_ffmpeg(workspace.path(), &voice_dir),
            &worker,
        )
        .expect_err("a checksum mismatch must refuse the profile");

        assert!(
            matches!(
                error,
                BuildError::VoiceChecksumMismatch { ref path, .. } if path == &tampered_path
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
                BuildError::UnresolvedContentRights { ref source_id, ref classification }
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
        Err(BuildError::ProductionGatesUnavailable)
    ));

    // Declaring nothing and declaring something unresolved are different claims
    // with different remedies, so an absent section and an empty one are both
    // refused as their own error rather than as the gate refusal.
    let mut undeclared = production_manifest(serde_json::json!([]));
    assert!(
        matches!(
            validate(&undeclared),
            Err(BuildError::MissingContentRightsDeclaration)
        ),
        "an empty classification list must be refused as undeclared"
    );
    undeclared
        .as_object_mut()
        .expect("manifest is an object")
        .remove("content_rights");
    let error = validate(&undeclared).expect_err("an undeclared manifest must refuse production");
    assert!(
        matches!(error, BuildError::MissingContentRightsDeclaration),
        "undeclared rights produced `{error}`"
    );

    // An unknown classification value is a parse error, never silently
    // accepted, and it is reported as an invalid rights declaration rather than
    // as the generic JSON catch-all.
    let unknown = production_manifest(serde_json::json!([{
        "source_id": "lesson-source-1",
        "classification": "unclassified",
        "rights_record_id": "rights-qualification-sources-v1",
    }]));
    let error = validate(&unknown).expect_err("an unknown classification must refuse production");
    assert!(
        matches!(
            error,
            BuildError::InvalidRightsDeclaration { section, .. } if section == "content_rights"
        ),
        "unknown classification produced `{error}`"
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

    // Bytes that are not JSON at all are reported as a manifest failure, not as
    // the generic JSON catch-all, which names no subsystem.
    let error = validate_production_manifest(b"{ not json")
        .expect_err("malformed bytes must refuse production");
    assert!(
        matches!(error, BuildError::MalformedProductionManifest { .. }),
        "malformed JSON produced `{error}`"
    );

    // A field this build cannot evaluate must not be published past. The
    // version is read first, so this is refused as a shape violation of version
    // 1.0 rather than as an unknown version.
    let mut extra = production_manifest(resolved.clone());
    extra["unexpected_field"] = serde_json::json!(true);
    let error = validate(&extra).expect_err("an unknown top-level field must refuse production");
    assert!(
        matches!(error, BuildError::MalformedProductionManifest { .. }),
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
        matches!(error, BuildError::MalformedProductionManifest { .. }),
        "unknown release status produced `{error}`"
    );

    let mut preview = production_manifest(resolved.clone());
    preview["release_status"] = serde_json::json!("private_preview");
    let error = validate(&preview).expect_err("a private preview must refuse production");
    assert!(
        matches!(
            error,
            BuildError::ManifestNotProductionRelease {
                declared: ReleaseStatus::PrivatePreview
            }
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
            matches!(error, BuildError::Lesson(LessonError::MissingLessonId)),
            "blank lesson_id `{absent}` produced `{error}`"
        );
    }
    for malformed in ["../escape", ".hidden", "lesson id"] {
        let error = refuse_lesson_id(malformed);
        assert!(
            matches!(
                error,
                BuildError::Lesson(LessonError::InvalidLessonId(ref reported))
                    if reported == malformed
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
                BuildError::EmptyManifestIdentifier {
                    section: reported_section,
                    field: reported_field,
                } if reported_section == section && reported_field == field
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
        let voice_dir = write_voice_profile_fixture(
            &workspace.path().join("voice"),
            &VoiceProfileFixtureSpec::default(),
        );
        std::fs::remove_file(voice_dir.join(record)).expect("remove a required record");
        let worker = DeterministicToneWorker::default();

        let error = build_preview(
            request_without_ffmpeg(workspace.path(), &voice_dir),
            &worker,
        )
        .expect_err("an absent record must refuse the profile");

        assert!(
            matches!(
                error,
                BuildError::MissingVoiceRecord { record: reported, .. } if reported == record
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
