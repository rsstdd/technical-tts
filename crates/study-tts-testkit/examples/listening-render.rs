//! Renders the E1-S3 listening set and blinds it for review.
//!
//! ADR-0001 §17.5 makes a human listening review a gate condition, and E1-S3 is
//! where this project produces speech for the first time. The four `t5_e1_`
//! criteria measure session behavior — one model load per lifetime,
//! protocol-only stdout, staging containment, a stable bundle identity — and
//! none of them listens to anything.
//!
//! # Why this exists as a committed instrument
//!
//! The first E1-S3 listening set was rendered by piping a hand-built NDJSON
//! session straight into the worker. That set could not be reproduced: its
//! session predated the required `staging_root`, so it could not even be
//! replayed against the worker that exists now, and nothing recorded how the
//! takes had been produced. A review whose material cannot be re-rendered is a
//! review that cannot be repeated when the audio changes — and edge
//! conditioning under `ADR-0001-D007` changes it.
//!
//! An example rather than a script, for the reason
//! `scripts/qualification/README.md` gives about the qualification instrument:
//! the takes must come through [`WorkerTtsExecutor`], the path production uses.
//! A Python harness would re-implement the protocol client and then review the
//! re-implementation's output.
//!
//! The same argument reaches past the executor, and an earlier version of this
//! stopped short of it. Every take is **published through the cache** before it
//! is blinded, because edge conditioning under `ADR-0001-D007` runs inside
//! publication: an instrument that wrote the worker's WAV straight to a file
//! would hand the reviewer audio the conditioner had never seen, which is the
//! audio this review exists to judge. And the voice is resolved through
//! [`resolve_voice_conditioning`], the gate a build passes, so the material is
//! not reviewed under a consent the record does not give.
//!
//! # Blinding
//!
//! Copies are shuffled into `sample-NN.wav` and the mapping is written to a
//! separate file, so a reviewer forms a judgment without knowing which line
//! produced which audio. The sheet records every judgment against a take's
//! SHA-256 rather than its filename, so a completed review is bound to bytes
//! and `scripts/qualification/check_listening_review.py` can prove it still is.
//!
//! **The key file is readable; the discipline is procedural.** Nothing here can
//! stop an operator opening `randomization-key.json` early, and pretending
//! otherwise would be theatre. What is mechanical is the other half: the
//! checker refuses a sheet that is incomplete or whose digests no longer match
//! the audio, and it is the sanctioned way to reveal the mapping.
//!
//! ```text
//! cargo run --package study-tts-testkit --example listening-render -- \
//!     --bundle-root . \
//!     --model-root <governed model root> \
//!     --voice-root <governed voice root> \
//!     --output-root <fresh directory>
//! ```

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, DeliveryStyle,
    PlannedSegment, VoiceConditioningHash, VoiceUse,
};
use study_tts_runtime::{
    CachePublisher, CacheResolveRequest, FileSystemCachePublisher, SynthesisRequest, TtsExecutor,
    WorkerConfiguration, WorkerTtsExecutor, resolve_voice_conditioning,
};
use study_tts_testkit::run_tts_executor_contract_scenario;

/// The committed text this review is taken against.
///
/// Relative to the repository root, which is what `--bundle-root` names.
const LISTENING_SCRIPT: &str = "fixtures/listening/e1-s3-listening-script.json";

/// Layout version of the sheet a reviewer fills in.
const REVIEW_SHEET_SCHEMA: &str = "1.0-e1-s3-listening-review";

/// Layout version of the mapping the sheet is blind to.
const RANDOMIZATION_KEY_SCHEMA: &str = "1.0-e1-s3-randomization-key";

/// The language the committed script is written in.
const LISTENING_LANGUAGE: &str = "en";

/// The job this instrument's cache entries and quarantine are filed under.
const LISTENING_JOB_ID: &str = "e1-s3-listening";

/// The criteria a reviewer answers for every sample.
///
/// The same five the E1-S3 story record's §Review result tabulates, in the same
/// order and spelling, so a completed sheet transcribes into that table without
/// anyone deciding what a renamed criterion used to mean.
const REVIEW_CRITERIA: [&str; 5] = [
    "omissions_or_additions",
    "pronunciation",
    "voice_consistency",
    "pacing",
    "noise_or_artifacts",
];

fn main() -> Result<(), Box<dyn Error>> {
    let configuration = Configuration::from_arguments()?;
    let script = Script::read(&configuration.bundle_root.join(LISTENING_SCRIPT))?;

    let launch = WorkerConfiguration::for_bundle(
        &configuration.bundle_root,
        &configuration.model_root,
        &configuration.voice_root,
        &configuration.workspace,
        // Gates every profile the worker will deserialize, before the child
        // exists — not only the one `governed_voice` selects below, which is
        // why that call can no longer be the first governed read. Same scope:
        // a run that qualifies a worker and never reaches a lesson.
        VoiceUse::VoiceQualification,
    )?;
    let executor = WorkerTtsExecutor::start(&launch)?;
    let bundle = executor.descriptor().worker_bundle_hash.as_str().to_owned();
    let voice = governed_voice(&configuration.voice_root)?;

    let takes = render_takes(
        &executor,
        &FileSystemCachePublisher,
        &configuration.workspace,
        &script,
        &voice,
    )?;
    // Read before the shutdown joins the reader threads, then shut down through
    // the protocol rather than by dropping the executor.
    let diagnostics = executor.diagnostics().len();
    executor.shutdown()?;

    let blinded = blind(&configuration.listening_root, &takes)?;
    let sheet = configuration.listening_root.join("review-sheet.json");
    let key = configuration.listening_root.join("randomization-key.json");
    fs::write(
        &sheet,
        render_review_sheet(&bundle, &voice.profile_id, &blinded)?,
    )?;
    fs::write(&key, render_randomization_key(&blinded)?)?;

    println!("worker bundle identity: {bundle}");
    println!("voice profile: {}", voice.profile_id);
    println!(
        "takes: {} ({} bytes of diagnostics)",
        takes.len(),
        diagnostics
    );
    println!(
        "review sheet: {} (SHA-256 {})",
        sheet.display(),
        digest_of(&sheet)?
    );
    println!("randomization key: {}", key.display());
    println!(
        "\nReview every sample in {} before opening the key.\nWhen the sheet is complete, run:\n  \
         python3 scripts/qualification/check_listening_review.py {}",
        configuration.listening_root.display(),
        configuration.listening_root.display()
    );
    Ok(())
}

/// Renders every script line and returns the takes, newest first line first.
///
/// Separate from `main` so the composition can be driven offline by a fake
/// executor: this is the part that decides *what audio a reviewer hears*, and
/// an instrument whose output a gate record cites should not have that part
/// reachable only through a governed root and a real model.
fn render_takes(
    executor: &dyn TtsExecutor,
    cache: &dyn CachePublisher,
    workspace: &Path,
    script: &Script,
    voice: &GovernedVoice,
) -> Result<Vec<Take>, Box<dyn Error>> {
    fs::create_dir_all(workspace)?;
    let workspace = fs::canonicalize(workspace)?;

    // The composition `pipeline.rs` uses for a real build: the backend
    // contributes its own identity, the lesson its language, and the voice gate
    // the conditioning hash. Assembling it here rather than taking the worker's
    // word is what keeps the cache's identity gate a real comparison — the
    // planned key is derived from a record read off disk, the reported one from
    // what the worker says it loaded.
    let context = executor.descriptor().synthesis_context(
        LISTENING_LANGUAGE.parse()?,
        BTreeMap::from([(voice.profile_id.clone(), voice.conditioning.clone())]),
    );

    let mut takes = Vec::with_capacity(script.lines.len());
    for (index, line) in script.lines.iter().enumerate() {
        let mut segment = planned_segment(index, line, &script.style, voice)?;
        segment.cache_key = context.key_for(&segment);
        let synthesis = request(voice, index, &line.text, &script.style, &segment)?;

        // Through the cache, not around it. Edge conditioning under
        // `ADR-0001-D007` runs inside publication, so an instrument that
        // rendered straight to a file would hand the reviewer audio the
        // conditioner had never seen — which is precisely what the D007 retake
        // exists to listen to.
        let mut producer = |destination: &Path| {
            run_tts_executor_contract_scenario(executor, synthesis.clone(), destination)
        };
        let published = cache.resolve(
            &CacheResolveRequest {
                workspace: workspace.clone(),
                job_id: LISTENING_JOB_ID.to_owned(),
                segment: segment.clone(),
            },
            &mut producer,
        )?;

        takes.push(Take {
            line_id: line.id.clone(),
            digest: digest_of(published.audio_path())?,
            path: published.audio_path().to_path_buf(),
        });
    }
    Ok(takes)
}

/// One planned segment for one script line.
///
/// A real [`PlannedSegment`], because that is what the cache resolves against:
/// its `cache_key` is filled in by the caller from the synthesis context, so
/// the entry is filed under the identity a build would file it under.
fn planned_segment(
    index: usize,
    line: &ScriptLine,
    style: &str,
    voice: &GovernedVoice,
) -> Result<PlannedSegment, Box<dyn Error>> {
    Ok(PlannedSegment {
        id: format!("listening-{index}"),
        speaker: voice.profile_id.clone(),
        voice_profile: voice.profile_id.clone(),
        display_text: line.text.clone(),
        spoken_text: line.text.clone(),
        // Through the closed vocabulary rather than as a string, so a script
        // naming a fifth style is refused here rather than rendered.
        style: serde_json::from_value::<DeliveryStyle>(serde_json::Value::String(
            style.to_owned(),
        ))?,
        pause_after_ms: 0,
        // Every line is its own segment, so nothing here is a retake.
        take: 0,
        // Replaced by the caller. `CacheKey` has no empty value, and a
        // placeholder that reached the cache would be refused by the identity
        // gate rather than published.
        cache_key: "0".repeat(64).parse()?,
    })
}

/// The voice this run renders with, resolved through the rights gate.
#[derive(Debug)]
struct GovernedVoice {
    profile_id: String,
    conditioning: VoiceConditioningHash,
}

/// One rendered take, before it is blinded.
#[derive(Debug)]
struct Take {
    line_id: String,
    digest: String,
    path: PathBuf,
}

/// One blinded sample, and the take it came from.
struct Blinded {
    blind_id: String,
    line_id: String,
    digest: String,
}

/// The committed script this review is taken against.
struct Script {
    style: String,
    lines: Vec<ScriptLine>,
}

/// One line of it.
struct ScriptLine {
    id: String,
    text: String,
}

impl Script {
    /// Reads the committed script, refusing anything it cannot render.
    fn read(path: &Path) -> Result<Self, Box<dyn Error>> {
        let document: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
        let style = document
            .get("style")
            .and_then(serde_json::Value::as_str)
            .ok_or("the listening script states no style")?
            .to_owned();
        let lines = document
            .get("lines")
            .and_then(serde_json::Value::as_array)
            .ok_or("the listening script states no lines")?
            .iter()
            .map(|line| {
                Ok(ScriptLine {
                    id: line
                        .get("id")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("a listening script line states no id")?
                        .to_owned(),
                    text: line
                        .get("text")
                        .and_then(serde_json::Value::as_str)
                        .ok_or("a listening script line states no text")?
                        .to_owned(),
                })
            })
            .collect::<Result<Vec<ScriptLine>, Box<dyn Error>>>()?;
        if lines.is_empty() {
            return Err("the listening script names no lines to render".into());
        }
        Ok(Self { style, lines })
    }
}

/// Governed locations this run was pointed at.
struct Configuration {
    bundle_root: PathBuf,
    model_root: PathBuf,
    voice_root: PathBuf,
    /// The one directory the worker is told it may write inside, and the
    /// managed root the cache publishes beneath. One directory, because the
    /// cache assigns its own staging destination inside it and the worker
    /// refuses to write outside the root it was declared at `initialize`.
    workspace: PathBuf,
    /// Where the blinded copies and both records go.
    listening_root: PathBuf,
}

impl Configuration {
    /// Reads the four roots, refusing anything it was not given.
    ///
    /// No defaults, for the reason the qualification instrument gives: a
    /// default governed path would put one into a committed file, and a default
    /// output root would let a rerun overwrite the audio a completed review was
    /// taken against.
    fn from_arguments() -> Result<Self, Box<dyn Error>> {
        let mut bundle_root = None;
        let mut model_root = None;
        let mut voice_root = None;
        let mut output_root = None;

        let mut arguments = std::env::args().skip(1);
        while let Some(flag) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("`{flag}` needs a value"))?;
            match flag.as_str() {
                "--bundle-root" => bundle_root = Some(PathBuf::from(value)),
                "--model-root" => model_root = Some(PathBuf::from(value)),
                "--voice-root" => voice_root = Some(PathBuf::from(value)),
                "--output-root" => output_root = Some(PathBuf::from(value)),
                unknown => return Err(format!("unknown argument `{unknown}`").into()),
            }
        }

        let output_root: PathBuf = output_root.ok_or("--output-root is required")?;
        if output_root.exists() {
            return Err("--output-root must not exist; a retake takes a new root".into());
        }
        // Absolute from here on. Every path below is handed to the worker,
        // whose working directory is the bundle's import root rather than this
        // process's: a relative `--output-root` becomes a directory the worker
        // cannot find, and it refuses the render rather than writing somewhere
        // nobody meant.
        let configuration = Self {
            bundle_root: std::path::absolute(bundle_root.ok_or("--bundle-root is required")?)?,
            model_root: std::path::absolute(model_root.ok_or("--model-root is required")?)?,
            voice_root: std::path::absolute(voice_root.ok_or("--voice-root is required")?)?,
            workspace: std::path::absolute(output_root.join("workspace"))?,
            listening_root: std::path::absolute(output_root.join("listening"))?,
        };
        fs::create_dir_all(&configuration.workspace)?;
        fs::create_dir_all(&configuration.listening_root)?;
        Ok(configuration)
    }
}

/// Copies each take to a shuffled `sample-NN.wav` and reports the mapping.
///
/// The copy is re-hashed rather than assumed: a blinded sample whose bytes
/// differ from the take it names would make every judgment in the sheet
/// describe audio nobody rendered.
fn blind(listening_root: &Path, takes: &[Take]) -> Result<Vec<Blinded>, Box<dyn Error>> {
    let mut order: Vec<usize> = (0..takes.len()).collect();
    shuffle(&mut order)?;

    let mut blinded = Vec::with_capacity(takes.len());
    for (position, take_index) in order.into_iter().enumerate() {
        let take = &takes[take_index];
        let blind_id = format!("sample-{:02}", position + 1);
        let destination = listening_root.join(format!("{blind_id}.wav"));
        fs::copy(&take.path, &destination)?;
        let copied = digest_of(&destination)?;
        if copied != take.digest {
            return Err("a blinded listening copy changed bytes".into());
        }
        blinded.push(Blinded {
            blind_id,
            line_id: take.line_id.clone(),
            digest: copied,
        });
    }
    Ok(blinded)
}

/// Fisher-Yates over indices drawn from the operating system.
///
/// `/dev/urandom` rather than a crate: this needs an ordering a reviewer cannot
/// predict, which the OS CSPRNG already provides, and `AGENTS.md` admits a
/// dependency only when it removes more risk than it adds. Unix-only, like the
/// rest of this instrument.
fn shuffle(order: &mut [usize]) -> Result<(), Box<dyn Error>> {
    if order.len() < 2 {
        return Ok(());
    }
    let mut source = fs::File::open("/dev/urandom")?;
    for index in (1..order.len()).rev() {
        let mut bytes = [0_u8; 8];
        source.read_exact(&mut bytes)?;
        // Modulo bias across a range this small is far below what a reviewer
        // could exploit, and the alternative — rejection sampling — buys
        // nothing here: the property needed is unpredictability, not a uniform
        // distribution over orderings.
        let choice = usize::try_from(u64::from_le_bytes(bytes) % (index as u64 + 1))?;
        order.swap(index, choice);
    }
    Ok(())
}

/// The sheet a reviewer fills in, with every judgment left unanswered.
///
/// Written pending, and that is the point: an instrument that recorded a
/// verdict would be answering the one question it exists to ask a human.
fn render_review_sheet(
    bundle: &str,
    voice_profile: &str,
    blinded: &[Blinded],
) -> Result<String, serde_json::Error> {
    let samples: Vec<serde_json::Value> = blinded
        .iter()
        .map(|sample| {
            let findings: serde_json::Map<String, serde_json::Value> = REVIEW_CRITERIA
                .iter()
                .map(|criterion| ((*criterion).to_owned(), serde_json::Value::Null))
                .collect();
            serde_json::json!({
                "blind_id": sample.blind_id,
                "wav": format!("{}.wav", sample.blind_id),
                "sha256": sample.digest,
                "findings": findings,
                "disposition": serde_json::Value::Null,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": REVIEW_SHEET_SCHEMA,
        "status": "pending_human_review",
        "instructions": "Answer every criterion for every sample before opening \
            randomization-key.json. Record `none` where nothing was heard, and a description \
            otherwise. Set each disposition to `accept` or `reject`.",
        "worker_bundle_hash": bundle,
        "voice_profile": voice_profile,
        "reviewer": serde_json::Value::Null,
        "playback_environment": serde_json::Value::Null,
        "reviewed_at": serde_json::Value::Null,
        "samples": samples,
        "overall_finding": serde_json::Value::Null,
    }))
}

/// The mapping the sheet is blind to.
fn render_randomization_key(blinded: &[Blinded]) -> Result<String, serde_json::Error> {
    let mapping: Vec<serde_json::Value> = blinded
        .iter()
        .map(|sample| {
            serde_json::json!({
                "blind_id": sample.blind_id,
                "line_id": sample.line_id,
                "sha256": sample.digest,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": RANDOMIZATION_KEY_SCHEMA,
        "mapping": mapping,
    }))
}

/// SHA-256 of a file, in the spelling this project writes every digest in.
///
/// SHA-256 rather than BLAKE3 because these records are cited by evidence, and
/// `scripts/check-evidence-provenance.py` verifies a citation with SHA-256.
fn digest_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(Sha256::digest(fs::read(path)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// One synthesis request for one line of the script.
fn request(
    voice: &GovernedVoice,
    index: usize,
    text: &str,
    style: &str,
    segment: &PlannedSegment,
) -> Result<SynthesisRequest, Box<dyn Error>> {
    Ok(SynthesisRequest {
        request_id: format!("e1-s3-listening-{index}"),
        segment_id: segment.id.clone(),
        spoken_text: text.to_owned(),
        voice: voice.profile_id.clone(),
        voice_profile: voice.profile_id.clone(),
        voice_conditioning_hash: voice.conditioning.clone(),
        style: style.to_owned(),
        language: LISTENING_LANGUAGE.parse()?,
        take: segment.take,
        // The key the cache filed this segment under, so what the worker is
        // asked for and what the entry is named by cannot come apart.
        cache_key: segment.cache_key.clone(),
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
    })
}

/// The voice this run renders with, resolved through the rights gate.
///
/// Discovery is by directory name; the *load* goes through
/// [`resolve_voice_conditioning`], which is the gate a build passes before any
/// synthesis: consent status, rights decision, permitted-use scope, and the
/// bytes of both `reference.wav` and `conditionals.pt` against the digests the
/// record states. An earlier version of this read `profile.json` by hand and
/// took `conditionals_blake3` on trust, so the review could be taken against a
/// voice whose consent had been withdrawn and whose artifact had been swapped.
///
/// [`VoiceUse::VoiceQualification`] rather than
/// [`VoiceUse::PrivateSynthesis`]: this renders a committed script to discharge
/// ADR-0001 §17.5's gate condition, and never reaches a lesson, which is what
/// that variant is for.
///
/// Read from disk rather than asked of the worker, deliberately. The cache's
/// identity gate compares the conditioning artifact the worker reports against
/// the one the planned key was derived from, and a key derived from the
/// worker's own answer would make that comparison pass by construction.
fn governed_voice(voice_root: &Path) -> Result<GovernedVoice, Box<dyn Error>> {
    let mut profiles: Vec<PathBuf> = fs::read_dir(voice_root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("profile.json").is_file())
        .collect();
    profiles.sort();
    let profile_id = profiles
        .first()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .ok_or("the governed voice root holds no profile record")?
        .to_owned();

    let conditioning =
        resolve_voice_conditioning(voice_root, &profile_id, VoiceUse::VoiceQualification)?;
    Ok(GovernedVoice {
        profile_id,
        conditioning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use study_tts_testkit::{
        FakeTtsExecutor, VoiceProfileFixtureSpec, write_voice_profile_fixture,
    };
    use tempfile::TempDir;

    fn blinded() -> Vec<Blinded> {
        vec![
            Blinded {
                blind_id: "sample-01".to_owned(),
                line_id: "line-03".to_owned(),
                digest: "a".repeat(64),
            },
            Blinded {
                blind_id: "sample-02".to_owned(),
                line_id: "line-01".to_owned(),
                digest: "b".repeat(64),
            },
        ]
    }

    #[test]
    fn t1_e1_the_written_review_sheet_is_the_shape_its_checker_reads() {
        // `scripts/qualification/check_listening_review.py` is the other end
        // of this shape.
        let mut samples = blinded();
        samples[0].blind_id = "sample-\"\\01".to_owned();
        let rendered = render_review_sheet("bundle-\"\\digest", "owner-\"\\voice", &samples)
            .expect("serialize the review sheet");
        let sheet: serde_json::Value =
            serde_json::from_str(&rendered).expect("the review sheet is JSON");

        assert_eq!(sheet["schema_version"], REVIEW_SHEET_SCHEMA);
        assert_eq!(sheet["status"], "pending_human_review");
        assert_eq!(sheet["worker_bundle_hash"], "bundle-\"\\digest");
        assert_eq!(sheet["voice_profile"], "owner-\"\\voice");
        let samples = sheet["samples"].as_array().expect("samples is an array");
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0]["blind_id"], "sample-\"\\01");
        assert_eq!(samples[0]["wav"], "sample-\"\\01.wav");
        assert_eq!(samples[0]["sha256"], "a".repeat(64));
        for criterion in REVIEW_CRITERIA {
            assert!(
                samples[0]["findings"][criterion].is_null(),
                "`{criterion}` must be written unanswered, not decided here"
            );
        }
        assert!(samples[0]["disposition"].is_null());
    }

    #[test]
    fn t1_e1_the_written_key_is_the_shape_its_checker_reads() {
        let mut samples = blinded();
        samples[0].line_id = "line-\"\\03".to_owned();
        let rendered = render_randomization_key(&samples).expect("serialize the key");
        let key: serde_json::Value =
            serde_json::from_str(&rendered).expect("the randomization key is JSON");

        assert_eq!(key["schema_version"], RANDOMIZATION_KEY_SCHEMA);
        let mapping = key["mapping"].as_array().expect("mapping is an array");
        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping[0]["blind_id"], "sample-01");
        assert_eq!(mapping[0]["line_id"], "line-\"\\03");
    }

    #[test]
    fn t1_e1_the_sheet_never_names_the_line_a_sample_came_from() {
        // The blinding itself. A sheet that carried the line id would let a
        // reviewer infer the ordering without opening the key at all.
        let rendered =
            render_review_sheet("c".repeat(64).as_str(), "owner-fallback-v1", &blinded())
                .expect("serialize the review sheet");

        assert!(
            !rendered.contains("line-01") && !rendered.contains("line-03"),
            "the review sheet must not name the lines it is blind to: {rendered}"
        );
    }

    #[test]
    fn t1_e1_the_committed_script_renders_every_line_it_declares() {
        // The script is a committed fixture, so a malformed one is a defect
        // this repository can catch without a model or a governed root.
        let script = Script::read(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(LISTENING_SCRIPT),
        )
        .expect("the committed listening script is readable");

        assert_eq!(script.style, "calm_explanatory");
        assert!(
            script.lines.len() >= 5,
            "a listening set needs enough material to judge pacing across takes"
        );
        for line in &script.lines {
            assert!(!line.id.is_empty(), "every line carries an id");
            assert!(!line.text.trim().is_empty(), "every line carries text");
        }
    }

    /// A fake executor, a real publisher, and a synthetic governed voice root.
    ///
    /// Returns the workspace guard alongside, because dropping it removes the
    /// published entry the caller is about to read.
    /// The scope this instrument's material is rendered under.
    ///
    /// `voice_qualification`, because the listening set never reaches a lesson.
    /// A governed root whose `consent.json` omits it refuses the render, which
    /// is the check working rather than a fixture detail.
    fn qualification_spec(consent_status: &str) -> VoiceProfileFixtureSpec {
        VoiceProfileFixtureSpec {
            consent_status: consent_status.to_owned(),
            permitted_use: vec!["voice_qualification".to_owned()],
            ..VoiceProfileFixtureSpec::default()
        }
    }

    fn rendered_offline(
        spec: &VoiceProfileFixtureSpec,
    ) -> (TempDir, FakeTtsExecutor, Result<Vec<Take>, Box<dyn Error>>) {
        let workspace = TempDir::new().expect("create a listening workspace");
        let voice_root = workspace.path().join("voices");
        write_voice_profile_fixture(&voice_root.join(&spec.profile_id), spec);

        let executor = FakeTtsExecutor::default();
        let script = Script {
            style: "calm_explanatory".to_owned(),
            lines: vec![ScriptLine {
                id: "line-01".to_owned(),
                text: "A checksum proves that bytes did not change.".to_owned(),
            }],
        };
        let takes = governed_voice(&voice_root).and_then(|voice| {
            render_takes(
                &executor,
                &FileSystemCachePublisher,
                &workspace.path().join("run"),
                &script,
                &voice,
            )
        });
        (workspace, executor, takes)
    }

    /// Samples at the start and end of a WAV that are exactly zero.
    fn zero_edges(path: &Path) -> (usize, usize) {
        let mut reader = hound::WavReader::open(path).expect("the take is readable as WAV");
        let samples: Vec<f32> = reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .expect("the take holds float samples");
        let leading = samples.iter().take_while(|sample| **sample == 0.0).count();
        let trailing = samples.iter().rev().take_while(|s| **s == 0.0).count();
        (leading, trailing)
    }

    #[test]
    fn t1_e1_the_reviewed_audio_is_the_conditioned_audio_the_cache_publishes() {
        // The defect this exists for: the instrument rendered through the
        // executor and blinded the worker's raw WAV, so the D007 retake — whose
        // whole purpose is to review conditioned audio — reviewed audio the
        // conditioner had never seen.
        //
        // ADR-0001 §13.4 requires at least 10 ms of silence at each exposed
        // edge, which is 240 samples at the canonical rate. The fake's tone
        // carries one leading zero, because `sin(0)` is zero, and no trailing
        // zeros at all.
        let (_workspace, _executor, takes) = rendered_offline(&qualification_spec("granted"));
        let takes = takes.expect("a rights-clean voice renders");

        let (leading, trailing) = zero_edges(&takes[0].path);

        assert!(
            leading >= 240 && trailing >= 240,
            "the reviewed audio carries {leading} leading and {trailing} trailing zero samples, \
             so it is not what the cache publishes"
        );
    }

    #[test]
    fn t1_e1_a_revoked_consent_refuses_the_render_before_any_synthesis() {
        // The other half: the instrument read `profile.json` by hand and took
        // its digest on trust, so a voice whose consent had been revoked
        // could be rendered and reviewed. The count is what makes this an
        // *ordering* rather than a refusal that happened to arrive eventually.
        let (_workspace, executor, takes) = rendered_offline(&qualification_spec("revoked"));

        let refusal = takes
            .expect_err("revoked consent must refuse the render")
            .to_string();
        assert!(
            refusal.contains("consent"),
            "the refusal does not name consent: {refusal}"
        );
        assert_eq!(
            executor.synthesis_count(),
            0,
            "the gate ran after synthesis rather than before it"
        );
    }

    #[test]
    fn t1_e1_shuffling_preserves_every_take_exactly_once() {
        // A shuffle that dropped or duplicated an index would silently review
        // one take twice and another never.
        let mut order: Vec<usize> = (0..12).collect();
        shuffle(&mut order).expect("the operating system provides randomness");
        order.sort_unstable();

        assert_eq!(order, (0..12).collect::<Vec<usize>>());
    }
}
