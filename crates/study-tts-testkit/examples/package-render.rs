//! Renders one lesson through the real worker into a complete package.
//!
//! `DELIVERY-PLAN.md` G1 acceptance requires "a reviewed three-segment lesson
//! renders through real Chatterbox, produces a complete private-preview
//! package". The T4 suite proves the package path against a deterministic tone,
//! and `listening-render` produces real speech — but from a *script* of takes,
//! never a lesson, and it stops at the cache. Neither drives a lesson through
//! `build_preview` with a real worker attached, which is the whole of what G1
//! asks for.
//!
//! An example rather than a script, for the reason `listening-render` gives:
//! the render must come through the path production uses, so that what a
//! reviewer hears is what a build produces. A harness that assembled its own
//! package would qualify the harness.
//!
//! # Rights
//!
//! The voice is resolved through the same gate a build passes, at
//! [`VoiceUse::PrivateSynthesis`] — this renders a lesson, not a voice
//! qualification. Governed output: the package is written beneath the
//! `--output-root` the operator names and nothing is copied into the
//! repository, per `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

use std::{collections::BTreeMap, error::Error, path::PathBuf};

use study_tts_core::VoiceUse;
use study_tts_runtime::{
    BuildRequest, TtsExecutor, WorkerConfiguration, WorkerTtsExecutor, build_preview,
    validate_m4a_output,
};

fn main() -> Result<(), Box<dyn Error>> {
    let configuration = Configuration::from_arguments()?;

    // Every profile the worker will deserialize is gated before the child
    // exists, on the terms `listening-render` states.
    let launch = WorkerConfiguration::for_bundle(
        &configuration.bundle_root,
        &configuration.model_root,
        &configuration.voice_root,
        &configuration.workspace,
        VoiceUse::PrivateSynthesis,
    )?;
    let executor = WorkerTtsExecutor::start(&launch)?;
    let bundle = executor.descriptor().worker_bundle_hash.as_str().to_owned();

    let result = build_preview(
        BuildRequest {
            lesson_path: configuration.lesson.clone(),
            workspace: configuration.workspace.clone(),
            ffmpeg_executable: PathBuf::from("ffmpeg"),
            ffprobe_executable: PathBuf::from("ffprobe"),
            voice_profile_root: configuration.voice_root.clone(),
            retakes: configuration.retakes.clone(),
        },
        &executor,
    )?;

    // Probed again from outside the build, so the claim is about the file
    // rather than about the runtime agreeing with itself.
    validate_m4a_output(&PathBuf::from("ffprobe"), &result.m4a)?;

    println!("worker bundle identity: {bundle}");
    println!("package directory:      {}", result.package_dir.display());
    for (label, path) in [
        ("master", &result.master_wav),
        ("m4a", &result.m4a),
        ("mp3", &result.mp3),
        ("transcript", &result.transcript),
        ("captions", &result.captions),
        ("chapters", &result.chapters),
        ("manifest", &result.manifest),
    ] {
        let bytes = std::fs::read(path)?;
        println!(
            "{label:<11} {:>9} bytes  blake3 {}",
            bytes.len(),
            blake3::hash(&bytes).to_hex()
        );
    }
    Ok(())
}

struct Configuration {
    bundle_root: PathBuf,
    model_root: PathBuf,
    voice_root: PathBuf,
    lesson: PathBuf,
    workspace: PathBuf,
    retakes: BTreeMap<String, u32>,
}

impl Configuration {
    /// Reads every root, refusing anything it was not given.
    ///
    /// No defaults, for the reason `listening-render` gives: a default governed
    /// path would put one into a committed file, and a default output root
    /// would let a rerun overwrite the package a review was taken against.
    ///
    /// `--retake <segment-id>=<take>` renders ADR-0001 §11.4's alternate
    /// performance, and is the one case where an *existing* output root is
    /// required rather than refused: a retake is a second build into the
    /// workspace that already holds the take it replaces, which is the only
    /// place §11.4's "retains the prior artifact" and its two joins can be
    /// observed. It overwrites nothing — a package is addressed by its manifest
    /// digest and published no-replace, so the earlier generation stays exactly
    /// where the review that took it will look.
    fn from_arguments() -> Result<Self, Box<dyn Error>> {
        let mut bundle_root = None;
        let mut model_root = None;
        let mut voice_root = None;
        let mut lesson = None;
        let mut output_root = None;
        let mut retakes = BTreeMap::new();

        let mut arguments = std::env::args().skip(1);
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("`{flag}` needs a value"))?;
            match flag.as_str() {
                "--bundle-root" => bundle_root = Some(PathBuf::from(value)),
                "--model-root" => model_root = Some(PathBuf::from(value)),
                "--voice-root" => voice_root = Some(PathBuf::from(value)),
                "--lesson" => lesson = Some(PathBuf::from(value)),
                "--output-root" => output_root = Some(PathBuf::from(value)),
                "--retake" => {
                    let (segment_id, take) = value.split_once('=').ok_or_else(|| {
                        format!("`--retake` takes `<segment-id>=<take>`, got `{value}`")
                    })?;
                    retakes.insert(segment_id.to_owned(), take.parse::<u32>()?);
                }
                unknown => return Err(format!("unknown argument `{unknown}`").into()),
            }
        }

        let output_root: PathBuf = output_root.ok_or("--output-root is required")?;
        if retakes.is_empty() && output_root.exists() {
            return Err("--output-root must not exist; a rerender takes a new root".into());
        }
        if !retakes.is_empty() && !output_root.exists() {
            return Err("--retake continues an existing --output-root; render it first".into());
        }
        // Absolute from here on: every path below is handed to the worker,
        // whose working directory is the bundle's import root rather than this
        // process's.
        let configuration = Self {
            bundle_root: std::path::absolute(bundle_root.ok_or("--bundle-root is required")?)?,
            model_root: std::path::absolute(model_root.ok_or("--model-root is required")?)?,
            voice_root: std::path::absolute(voice_root.ok_or("--voice-root is required")?)?,
            lesson: std::path::absolute(lesson.ok_or("--lesson is required")?)?,
            workspace: std::path::absolute(output_root.join("workspace"))?,
            retakes,
        };
        std::fs::create_dir_all(&configuration.workspace)?;
        Ok(configuration)
    }
}
