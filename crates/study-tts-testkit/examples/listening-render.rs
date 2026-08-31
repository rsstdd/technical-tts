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

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use study_tts_core::{CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE};
use study_tts_runtime::{SynthesisRequest, TtsExecutor, WorkerConfiguration, WorkerTtsExecutor};
use study_tts_testkit::run_tts_executor_contract_scenario;

/// The committed text this review is taken against.
///
/// Relative to the repository root, which is what `--bundle-root` names.
const LISTENING_SCRIPT: &str = "fixtures/listening/e1-s3-listening-script.json";

/// Layout version of the sheet a reviewer fills in.
const REVIEW_SHEET_SCHEMA: &str = "1.0-e1-s3-listening-review";

/// Layout version of the mapping the sheet is blind to.
const RANDOMIZATION_KEY_SCHEMA: &str = "1.0-e1-s3-randomization-key";

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
        &configuration.staging_root,
    )?;
    let executor = WorkerTtsExecutor::start(&launch)?;
    let bundle = executor.descriptor().worker_bundle_hash.as_str().to_owned();
    let voice = governed_voice(&configuration.voice_root)?;

    let mut takes = Vec::with_capacity(script.lines.len());
    for (index, line) in script.lines.iter().enumerate() {
        let destination = configuration
            .staging_root
            .join(format!("take-{index:02}.wav"));
        run_tts_executor_contract_scenario(
            &executor,
            request(&voice, index, &line.text, &script.style)?,
            &destination,
        )?;
        takes.push(Take {
            line_id: line.id.clone(),
            digest: digest_of(&destination)?,
            path: destination,
        });
    }
    // Read before the shutdown joins the reader threads, then shut down through
    // the protocol rather than by dropping the executor.
    let diagnostics = executor.diagnostics().len();
    executor.shutdown()?;

    let blinded = blind(&configuration.listening_root, &takes)?;
    let sheet = configuration.listening_root.join("review-sheet.json");
    let key = configuration.listening_root.join("randomization-key.json");
    fs::write(&sheet, render_review_sheet(&bundle, &voice.0, &blinded))?;
    fs::write(&key, render_randomization_key(&blinded))?;

    println!("worker bundle identity: {bundle}");
    println!("voice profile: {}", voice.0);
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

/// One rendered take, before it is blinded.
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
    /// The one directory the worker is told it may write inside.
    staging_root: PathBuf,
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
            staging_root: std::path::absolute(output_root.join("takes"))?,
            listening_root: std::path::absolute(output_root.join("listening"))?,
        };
        fs::create_dir_all(&configuration.staging_root)?;
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
fn render_review_sheet(bundle: &str, voice_profile: &str, blinded: &[Blinded]) -> String {
    let samples: Vec<String> = blinded
        .iter()
        .map(|sample| {
            let findings: Vec<String> = REVIEW_CRITERIA
                .iter()
                .map(|criterion| format!("        \"{criterion}\": null"))
                .collect();
            format!(
                "    {{\n      \"blind_id\": \"{}\",\n      \"wav\": \"{}.wav\",\n      \
                 \"sha256\": \"{}\",\n      \"findings\": {{\n{}\n      }},\n      \
                 \"disposition\": null\n    }}",
                sample.blind_id,
                sample.blind_id,
                sample.digest,
                findings.join(",\n")
            )
        })
        .collect();
    format!(
        "{{\n  \"schema_version\": \"{REVIEW_SHEET_SCHEMA}\",\n  \"status\": \
         \"pending_human_review\",\n  \"instructions\": \"Answer every criterion for every \
         sample before opening randomization-key.json. Record `none` where nothing was heard, \
         and a description otherwise. Set each disposition to `accept` or `reject`.\",\n  \
         \"worker_bundle_hash\": \"{bundle}\",\n  \"voice_profile\": \"{voice_profile}\",\n  \
         \"reviewer\": null,\n  \"playback_environment\": null,\n  \"reviewed_at\": null,\n  \
         \"samples\": [\n{}\n  ],\n  \"overall_finding\": null\n}}\n",
        samples.join(",\n")
    )
}

/// The mapping the sheet is blind to.
fn render_randomization_key(blinded: &[Blinded]) -> String {
    let mapping: Vec<String> = blinded
        .iter()
        .map(|sample| {
            format!(
                "    {{\"blind_id\": \"{}\", \"line_id\": \"{}\", \"sha256\": \"{}\"}}",
                sample.blind_id, sample.line_id, sample.digest
            )
        })
        .collect();
    format!(
        "{{\n  \"schema_version\": \"{RANDOMIZATION_KEY_SCHEMA}\",\n  \"mapping\": \
         [\n{}\n  ]\n}}\n",
        mapping.join(",\n")
    )
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
    voice: &(String, String),
    take: usize,
    text: &str,
    style: &str,
) -> Result<SynthesisRequest, Box<dyn Error>> {
    let (voice_profile, conditioning) = voice;
    let take = u32::try_from(take)?;
    Ok(SynthesisRequest {
        request_id: format!("e1-s3-listening-{take}"),
        segment_id: format!("listening-{take}"),
        spoken_text: text.to_owned(),
        voice: voice_profile.to_owned(),
        voice_profile: voice_profile.to_owned(),
        voice_conditioning_hash: conditioning.parse()?,
        style: style.to_owned(),
        language: "en".parse()?,
        take,
        cache_key: "0".repeat(64).parse()?,
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
    })
}

/// The voice profile this run renders with, and the digest its record states.
///
/// Read from the governed voice root, the same record
/// `voice_gate::load_profile` verifies `conditionals.pt` against before any
/// synthesis runs.
fn governed_voice(voice_root: &Path) -> Result<(String, String), Box<dyn Error>> {
    let mut profiles: Vec<PathBuf> = fs::read_dir(voice_root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.join("profile.json").is_file())
        .collect();
    profiles.sort();
    let profile = profiles
        .first()
        .ok_or("the governed voice root holds no profile record")?;

    let record: serde_json::Value =
        serde_json::from_slice(&fs::read(profile.join("profile.json"))?)?;
    let field = |name: &str| -> Result<String, Box<dyn Error>> {
        Ok(record
            .get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("the voice profile record states no `{name}`"))?
            .to_owned())
    };
    Ok((field("profile_id")?, field("conditionals_blake3")?))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Both records are formatted by hand rather than serialized, so an
        // unbalanced brace or a missing comma is a defect that would otherwise
        // surface only on the reference machine, after a model load, against
        // governed roots. `scripts/qualification/check_listening_review.py` is
        // the other end of this shape.
        let rendered =
            render_review_sheet("c".repeat(64).as_str(), "owner-fallback-v1", &blinded());
        let sheet: serde_json::Value =
            serde_json::from_str(&rendered).expect("the review sheet is JSON");

        assert_eq!(sheet["schema_version"], REVIEW_SHEET_SCHEMA);
        assert_eq!(sheet["status"], "pending_human_review");
        let samples = sheet["samples"].as_array().expect("samples is an array");
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0]["wav"], "sample-01.wav");
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
        let rendered = render_randomization_key(&blinded());
        let key: serde_json::Value =
            serde_json::from_str(&rendered).expect("the randomization key is JSON");

        assert_eq!(key["schema_version"], RANDOMIZATION_KEY_SCHEMA);
        let mapping = key["mapping"].as_array().expect("mapping is an array");
        assert_eq!(mapping.len(), 2);
        assert_eq!(mapping[0]["blind_id"], "sample-01");
        assert_eq!(mapping[0]["line_id"], "line-03");
    }

    #[test]
    fn t1_e1_the_sheet_never_names_the_line_a_sample_came_from() {
        // The blinding itself. A sheet that carried the line id would let a
        // reviewer infer the ordering without opening the key at all.
        let rendered =
            render_review_sheet("c".repeat(64).as_str(), "owner-fallback-v1", &blinded());

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
