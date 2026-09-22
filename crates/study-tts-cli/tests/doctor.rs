//! Tier 4 tests for E2-S5's `doctor`: real process, real filesystem, real
//! `/proc/mounts`.
//!
//! In this crate for the reason `authoring.rs` records —
//! `CARGO_BIN_EXE_study-tts` is set only for the package that declares the
//! binary.

use std::process::{Command, Output};

use tempfile::TempDir;

fn study_tts(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_study-tts"))
        .args(arguments)
        .output()
        .expect("run the study-tts binary")
}

/// `doctor` reports the filesystem, the tools, the checksums, and the core
/// budget — and says which checks it could not run.
///
/// ADR-0001 §14 lists fourteen things `doctor` verifies. Four of them cannot
/// be answered by this build: ASR identities and ASR threads are E4's, pool
/// size above one is E5-S2's, and the smoke render needs the governed model.
/// They are **named** rather than omitted, because a report that listed only
/// what it could check would let a reader mistake an unrun check for a passing
/// one.
///
/// The wrong implementation this rejects is that omission. It is the natural
/// thing to write — report what you can — and it turns a diagnostic into a
/// false reassurance.
#[test]
fn t4_e2_doctor_reports_drvfs_tools_checksums_and_core_budget() {
    let workspace = TempDir::new().expect("create a workspace");
    let report = study_tts(&[
        "doctor",
        "--workspace",
        &workspace.path().display().to_string(),
    ]);

    assert_eq!(
        report.status.code(),
        Some(0),
        "reporting a bad environment is not itself a failure: {}",
        String::from_utf8_lossy(&report.stderr)
    );
    let said = String::from_utf8(report.stdout).expect("stdout is UTF-8");

    // One per bullet of ADR-0001 §14's list, so a check that is dropped is a
    // failure here rather than an absence nobody notices.
    for subject in [
        "WSL2 and supported Ubuntu version",
        "supported OS and architecture",
        "the workspace is writable",
        "DrvFS",
        "free disk space",
        "gcc",
        "cmake",
        "python3",
        "FFmpeg",
        "ffprobe",
        "worker runtime and locked dependencies",
        // Two labels, not one: a single "checksums" line would let the model
        // verifying read as the voices having been, which is what B4 of the
        // E2-S5 implementation review found `doctor` doing.
        "model checksums",
        "voice-profile checksums",
        "GPU or CPU device",
        "physical-core topology",
        "per-worker threads and oversubscription",
        "offline mode",
    ] {
        assert!(said.contains(subject), "`doctor` reports {subject}: {said}");
    }

    // Every check this build cannot answer names the story that owns it.
    for owner in ["E4", "E5-S2", "E6-S1"] {
        assert!(
            said.contains(owner),
            "a blocked check names {owner} as its owner: {said}"
        );
    }
    assert_eq!(
        said.matches("blocked").count(),
        4,
        "all four unanswerable checks are named, not omitted: {said}"
    );
}

/// A workspace on a filesystem that must not hold durable state is refused.
///
/// `/mnt/c` is what ADR-0001 §14 names, and WSL2 mounts it as `9p` rather than
/// anything called `drvfs` — which is why the check reads `/proc/mounts` for
/// the type rather than matching the path. Skipped where there is no such
/// mount, because the test would otherwise assert about a machine it is not
/// running on.
#[test]
fn t4_e2_doctor_refuses_a_workspace_on_a_windows_mount() {
    let mounts = std::fs::read_to_string("/proc/mounts").unwrap_or_default();
    let windows_mount = mounts.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let (_device, point, kind) = (fields.next()?, fields.next()?, fields.next()?);
        (kind == "9p" && point.starts_with("/mnt/")).then_some(point.to_owned())
    });
    let Some(mount) = windows_mount else {
        return;
    };

    let report = study_tts(&["doctor", "--workspace", &mount]);
    let said = String::from_utf8(report.stdout).expect("stdout is UTF-8");

    // The filesystem line specifically, not merely some refusal: without
    // `--model-root` the checksum check is refused too, so a bare `REFUSED`
    // would pass on a workspace this test says nothing about.
    let filesystem_line = said
        .lines()
        .find(|line| line.contains("DrvFS"))
        .unwrap_or_else(|| panic!("`doctor` reports the filesystem: {said}"));

    assert!(
        filesystem_line.contains("REFUSED"),
        "`{mount}` is a Windows mount and must be refused: {filesystem_line}"
    );
    assert!(
        filesystem_line.contains("9p"),
        "and the refusal names the filesystem it found: {filesystem_line}"
    );
}

/// The voice-profile check reads the profiles, and says so without saying
/// where they are.
///
/// The wrong implementation this rejects is the one the E2-S5 implementation
/// review found: a single "model and voice-profile checksums" line that
/// verified the model and reported `ok` with the voices never read. Two
/// halves: a sound root passes, and the same root with one `reference.wav`
/// overwritten is refused — so the verdict came from hashing the record and
/// not from the directory existing. The path of the tampered file is the raw
/// voice reference, and ADR-0001 §14 keeps it out of the output by default.
#[test]
fn t4_e2_doctor_verifies_voice_profiles_and_names_no_path() {
    let workspace = TempDir::new().expect("create a workspace");
    let voices = workspace.path().join("voices");
    study_tts_testkit::write_voice_profile_root(&voices, &["narrator-v1"]);
    let voices_arg = voices.display().to_string();
    let workspace_arg = workspace.path().display().to_string();

    let line_for = |output: &Output| -> String {
        String::from_utf8(output.stdout.clone())
            .expect("stdout is UTF-8")
            .lines()
            .find(|line| line.contains("voice-profile checksums"))
            .expect("`doctor` reports the voice-profile check")
            .to_owned()
    };

    let sound = study_tts(&[
        "doctor",
        "--workspace",
        &workspace_arg,
        "--voice-root",
        &voices_arg,
    ]);
    assert!(
        line_for(&sound).trim_start().starts_with("ok "),
        "a sound profile root passes: {}",
        line_for(&sound)
    );

    let reference = voices.join("narrator-v1").join("reference.wav");
    std::fs::write(&reference, b"tampered-after-consent").expect("overwrite the reference");
    let tampered = study_tts(&[
        "doctor",
        "--workspace",
        &workspace_arg,
        "--voice-root",
        &voices_arg,
    ]);
    let line = line_for(&tampered);
    assert!(
        line.contains("REFUSED") && line.contains("narrator-v1") && line.contains("reference.wav"),
        "a tampered reference is refused by profile and record: {line}"
    );
    assert!(
        !line.contains(&reference.display().to_string()),
        "the raw voice-reference path is never printed: {line}"
    );
}
