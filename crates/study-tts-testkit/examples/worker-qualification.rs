//! T5 qualification instrument for `DELIVERY-PLAN.md` E1-S3.
//!
//! ```text
//! cargo run --package study-tts-testkit --example worker-qualification -- \
//!     --bundle-root <repo> --model-root <governed> --voice-root <governed> \
//!     --output-root <fresh directory>
//! ```
//!
//! **Run by an operator on the reference machine, never by a workflow.** The
//! four criteria below need real weights, a lawful voice profile, and the
//! qualified interpreter, none of which a hosted runner has;
//! `.github/workflows/qualification.yml` says in its own words why the
//! real-model steps are not invoked from it, and
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` is why the governed roots
//! are arguments with no defaults rather than paths written into this file.
//!
//! This is how a `t5_` name is discharged in this repository: `grep 'fn t5_'`
//! across `crates/` returns nothing, because every one of them is an acceptance
//! criterion answered by an instrument like this one plus an evidence record
//! citing its hashed output — the shape E0-S3 used, recorded in
//! `evidence/gates/g0/e0-s3/e0-s3-g0-qualification-report-v1.md`.
//!
//! Rust rather than a Python harness because three of the four criteria are
//! about the *executor* driving a real worker. A Python harness would
//! re-implement the protocol client and then qualify the re-implementation
//! instead of the shipped path. Every take goes through
//! [`run_tts_executor_contract_scenario`], the same seam the fake worker is
//! driven through, which is what E1-S3 task 7 asks for: one suite over both.
//!
//! The result is one JSON object on standard output. Hash it, file it under
//! `evidence/gates/g1/e1-s3/`, and cite it per `evidence/README.md`.

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use study_tts_core::{CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE};
use study_tts_runtime::{
    SynthesisRequest, TtsExecutor, WorkerBundle, WorkerConfiguration, WorkerTtsExecutor,
};
use study_tts_testkit::{run_tts_executor_contract_scenario, run_worker_restart_contract_scenario};

/// Takes rendered while proving the model loads once.
///
/// Three rather than two: two takes prove a session survives one request, which
/// the T4 suite already proves against the fake. Three is the smallest count
/// where a per-request reload would have had to happen twice, so a single
/// mis-scoped load cannot be read as a startup cost.
const LIFETIME_TAKES: usize = 3;

/// What one criterion observed, in the shape an evidence table reads.
struct Outcome {
    /// The `DELIVERY-PLAN.md` name this discharges, character for character.
    criterion: &'static str,
    /// Whether the criterion holds.
    passed: bool,
    /// What was measured, stated so a reader can check the verdict.
    observed: String,
}

impl Outcome {
    fn new(criterion: &'static str, passed: bool, observed: String) -> Self {
        Self {
            criterion,
            passed,
            observed,
        }
    }
}

/// The name the instrument writes its machine-readable result under.
///
/// Fixed rather than chosen per run, so the operator procedure in
/// `scripts/qualification/README.md` can name one path and the evidence record
/// can cite one filename.
const QUALIFICATION_RESULT_FILE: &str = "qualification-result.json";

/// Lowercase hexadecimal, the spelling every digest in this project is written
/// in.
fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

/// Governed locations this run was pointed at.
struct Configuration {
    bundle_root: PathBuf,
    model_root: PathBuf,
    voice_root: PathBuf,
    output_root: PathBuf,
    /// The one directory the worker is told it may write inside.
    ///
    /// A subdirectory of `output_root` rather than the root itself, so this
    /// run's own scratch — the directories a refused take must not reach — can
    /// sit beside it under one root the operator deletes. A staging root equal
    /// to `output_root` would put those inside the boundary and make
    /// `t5_e1_worker_output_cannot_escape_staging_root` assert nothing.
    staging_root: PathBuf,
}

impl Configuration {
    /// Reads the four roots, refusing anything it was not given.
    ///
    /// No defaults, and that is deliberate: a default governed path would put
    /// one into a committed file, and a default output root would let a rerun
    /// overwrite the artifacts a previous result was hashed from.
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

        let configuration = Self {
            bundle_root: bundle_root.ok_or("--bundle-root is required")?,
            model_root: model_root.ok_or("--model-root is required")?,
            voice_root: voice_root.ok_or("--voice-root is required")?,
            output_root: output_root.clone().ok_or("--output-root is required")?,
            staging_root: output_root
                .ok_or("--output-root is required")?
                .join("staging"),
        };
        // Absolute from here on. Every path below is handed to the worker,
        // whose working directory is the bundle's import root rather than this
        // process's: a relative `--output-root` becomes a directory the worker
        // cannot find, and it refuses the render rather than writing somewhere
        // nobody meant. `WorkerConfiguration::for_bundle` resolves the roots it
        // is given for the same reason.
        let configuration = Self {
            bundle_root: std::path::absolute(&configuration.bundle_root)?,
            model_root: std::path::absolute(&configuration.model_root)?,
            voice_root: std::path::absolute(&configuration.voice_root)?,
            output_root: std::path::absolute(&configuration.output_root)?,
            staging_root: std::path::absolute(&configuration.staging_root)?,
        };
        if configuration.output_root.exists() {
            return Err("--output-root must not exist; a rerun takes a new root".into());
        }
        fs::create_dir_all(&configuration.staging_root)?;
        Ok(configuration)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let configuration = Configuration::from_arguments()?;
    let mut outcomes = Vec::new();

    outcomes.push(bundle_identity_is_stable(&configuration)?);

    let launch = WorkerConfiguration::for_bundle(
        &configuration.bundle_root,
        &configuration.model_root,
        &configuration.voice_root,
        &configuration.staging_root,
    )?;
    let executor = WorkerTtsExecutor::start(&launch)?;
    let descriptor = executor.descriptor();
    let voice = governed_voice(&configuration.voice_root)?;

    outcomes.push(model_loads_once(&configuration, &executor, &voice)?);
    outcomes.push(protocol_stdout_stayed_clean(&executor));
    outcomes.push(worker_restarts_and_stays_offline(
        &configuration,
        &launch,
        &voice,
    )?);
    outcomes.push(output_stayed_in_the_staging_root(
        &configuration,
        &executor,
        &voice,
    )?);

    executor.shutdown()?;

    let passed = outcomes.iter().all(|outcome| outcome.passed);
    let result = render_result(descriptor.worker_bundle_hash.as_str(), &outcomes);
    println!("{result}");

    // Written and hashed, not only printed. The E1-S3 story record requires the
    // instrument's output to be "hashed and cited", and a result that exists
    // only in a terminal cannot be either: an evidence record citing a digest
    // needs bytes on disk that the digest is of. The digest is reported here so
    // the operator transcribes one value rather than re-deriving it, and it is
    // SHA-256 because that is what `scripts/check-evidence-provenance.py`
    // verifies every citation with.
    let result_path = configuration.output_root.join(QUALIFICATION_RESULT_FILE);
    fs::write(&result_path, result.as_bytes())?;
    println!(
        "qualification result: {} (SHA-256 {})",
        result_path.display(),
        hex(&Sha256::digest(result.as_bytes()))
    );

    if !passed {
        return Err("one or more qualification criteria failed".into());
    }
    Ok(())
}

/// `t5_e1_worker_bundle_hash_matches_when_all_declared_bundle_inputs_match`.
///
/// Two derivations from one unchanged tree, on the qualified interpreter, must
/// agree. That is the criterion as written, and the reference machine is the
/// only place it means anything: `verified_hash` probes the interpreter that
/// would run the bundle before it returns a digest, and ordinary CI has no
/// restored `worker/.venv` to probe.
///
/// The converse — that a *moved* declared input moves the identity — is not
/// measured here because it is already mechanized at T1, where it costs no
/// weights: `t1_e1_worker_bundle_hash_changes_on_owned_runtime_input` and
/// `t1_e1_worker_bundle_hash_ignores_unrelated_repository_files` in
/// `crates/study-tts-runtime/src/worker_bundle.rs`. Repeating it here would
/// need a second bundle copy carrying a whole interpreter.
fn bundle_identity_is_stable(configuration: &Configuration) -> Result<Outcome, Box<dyn Error>> {
    const CRITERION: &str =
        "t5_e1_worker_bundle_hash_matches_when_all_declared_bundle_inputs_match";

    let first = WorkerBundle::load(&configuration.bundle_root)?.verified_hash()?;
    let second = WorkerBundle::load(&configuration.bundle_root)?.verified_hash()?;

    Ok(Outcome::new(
        CRITERION,
        first == second,
        format!(
            "two derivations on the qualified interpreter agreed at {}; sensitivity to a moved \
             input is pinned at T1 by t1_e1_worker_bundle_hash_changes_on_owned_runtime_input",
            first.as_str()
        ),
    ))
}

/// `t5_e1_model_load_occurs_once_per_worker_lifetime`.
///
/// Counted from the worker's own diagnostics rather than from a protocol field,
/// so the observation costs the protocol nothing and cannot be satisfied by a
/// worker that merely says it loaded once. `Loading` and `loaded` are what the
/// backend's own load path prints; a reload would print them again.
fn model_loads_once(
    configuration: &Configuration,
    executor: &WorkerTtsExecutor,
    voice: &(String, String),
) -> Result<Outcome, Box<dyn Error>> {
    const CRITERION: &str = "t5_e1_model_load_occurs_once_per_worker_lifetime";

    let staging = configuration.staging_root.join("lifetime");
    fs::create_dir_all(&staging)?;
    for take in 0..LIFETIME_TAKES {
        let destination = staging.join(format!("take-{take}.wav"));
        run_tts_executor_contract_scenario(
            executor,
            request(voice, take, "One. Two. Three.")?,
            &destination,
        )?;
        if !destination.is_file() {
            return Ok(Outcome::new(
                CRITERION,
                false,
                format!("take {take} reported success and wrote nothing"),
            ));
        }
    }

    let loads = executor
        .diagnostics()
        .lines()
        .filter(|line| line.contains("loaded PerthNet"))
        .count();
    Ok(Outcome::new(
        CRITERION,
        loads == 1,
        format!("{LIFETIME_TAKES} takes through one worker reported {loads} model load(s)"),
    ))
}

/// `t5_e1_worker_protocol_stdout_remains_clean`.
///
/// Every take above already proves it: the executor parses each frame off
/// standard output and refuses anything it cannot read, so a session that
/// completed is a session whose stdout carried nothing but frames. What this
/// adds is that the proof is not vacuous — the backend really does write
/// diagnostics, and they really did land on the other channel.
fn protocol_stdout_stayed_clean(executor: &WorkerTtsExecutor) -> Outcome {
    const CRITERION: &str = "t5_e1_worker_protocol_stdout_remains_clean";

    let diagnostics = executor.diagnostics();
    Outcome::new(
        CRITERION,
        !diagnostics.trim().is_empty(),
        format!(
            "every frame of a completed session parsed off standard output, while {} bytes of \
             backend diagnostics went to standard error",
            diagnostics.len()
        ),
    )
}

/// `t5_e1_worker_survives_restart_and_starts_offline`.
///
/// Two lifetimes rather than one, driven by the same
/// [`run_worker_restart_contract_scenario`] the T4 suite drives the protocol
/// fake through. ADR-0001 §17.7 asks a worker to be restartable and to run
/// offline, and until this existed both suites started one worker, rendered
/// once and dropped it — which cannot tell a restartable worker from one that
/// only ever ran once.
///
/// Offline is read out of the worker's own diagnostics rather than assumed:
/// `_apply_offline_environment` prints the variables it *applied* in that
/// process, so a launcher that merely names them does not satisfy this. Since
/// `WorkerClient::spawn` now clears the environment, the second lifetime also
/// proves the declared set is enough to start a worker from nothing.
fn worker_restarts_and_stays_offline(
    configuration: &Configuration,
    launch: &WorkerConfiguration,
    voice: &(String, String),
) -> Result<Outcome, Box<dyn Error>> {
    const CRITERION: &str = "t5_e1_worker_survives_restart_and_starts_offline";

    let staging = configuration.staging_root.join("restart");
    fs::create_dir_all(&staging)?;
    let lifetimes = run_worker_restart_contract_scenario(
        launch,
        &request(voice, 0, "A restarted take.")?,
        &staging.join("first.wav"),
        &staging.join("second.wav"),
    )?;

    let [first, second] = &lifetimes;
    if first.report.context != second.report.context {
        return Ok(Outcome::new(
            CRITERION,
            false,
            "a restarted worker reported different identities than the first one".to_owned(),
        ));
    }
    let offline: Vec<bool> = lifetimes
        .iter()
        .map(|lifetime| {
            lifetime
                .diagnostics
                .contains("offline environment applied:")
        })
        .collect();
    Ok(Outcome::new(
        CRITERION,
        offline.iter().all(|applied| *applied),
        format!(
            "two lifetimes reported identical synthesis identities; offline settings applied \
             in {} of 2 lifetimes",
            offline.iter().filter(|applied| **applied).count()
        ),
    ))
}

/// `t5_e1_worker_output_cannot_escape_staging_root`.
///
/// The worker is given its staging root at `initialize` and decides containment
/// against the *resolved parent* of every path it is assigned, so this measures
/// the property the criterion names rather than a weaker one: an absolute path
/// somewhere else entirely is refused, and so is every escape shape that stays
/// inside the root by spelling. Earlier revisions of this instrument recorded
/// that they could not cover the first of those, because the worker was told
/// one path and no root and could only inspect the spelling it was handed.
fn output_stayed_in_the_staging_root(
    configuration: &Configuration,
    executor: &WorkerTtsExecutor,
    voice: &(String, String),
) -> Result<Outcome, Box<dyn Error>> {
    const CRITERION: &str = "t5_e1_worker_output_cannot_escape_staging_root";

    // `staging` is inside the root the worker was given; `outside` is not, and
    // sits under `output_root` only so one deletion cleans up after this run.
    let staging = configuration.staging_root.join("containment");
    let outside = configuration.output_root.join("outside");
    let root = configuration.output_root.clone();
    fs::create_dir_all(&staging)?;
    fs::create_dir_all(&outside)?;

    // A legitimate take first: only the assigned file may appear anywhere
    // beneath the root, so a worker writing a scratch file is caught here too.
    let before = inventory(&root)?;
    let assigned = staging.join("audio.wav");
    run_tts_executor_contract_scenario(
        executor,
        request(voice, 0, "A contained take.")?,
        &assigned,
    )?;
    let after = inventory(&root)?;
    let appeared: Vec<_> = after.difference(&before).cloned().collect();
    if appeared != vec![assigned.clone()] {
        return Ok(Outcome::new(
            CRITERION,
            false,
            format!("a contained take made {appeared:?} appear, not only its assigned path"),
        ));
    }

    // Then each escape shape the worker can see for itself. Every one must be
    // refused, and none may leave anything behind.
    let planted = outside.join("planted.wav");
    let link = staging.join("linked.wav");
    std::os::unix::fs::symlink(&planted, &link)?;
    // The shape no lexical check can see: every component is inside the root by
    // name, and the parent resolves out of it.
    let bridge = staging.join("bridge");
    std::os::unix::fs::symlink(&outside, &bridge)?;
    let traversal = staging
        .join("..")
        .join("..")
        .join("outside")
        .join("traversed.wav");
    let existing = staging.join("audio.wav");

    let mut refused = Vec::new();
    for (shape, destination) in [
        ("a symlink planted at the assigned path", link),
        ("an assigned path that walks upward", traversal),
        ("an assigned path that already exists", existing),
        (
            "an absolute path outside the staging root",
            outside.join("absolute.wav"),
        ),
        (
            "an assigned path whose parent is a symlink out of the root",
            bridge.join("bridged.wav"),
        ),
    ] {
        let outcome = run_tts_executor_contract_scenario(
            executor,
            request(voice, 0, "An escaping take.")?,
            &destination,
        );
        if outcome.is_ok() {
            return Ok(Outcome::new(
                CRITERION,
                false,
                format!("{shape} was accepted rather than refused"),
            ));
        }
        refused.push(shape);
    }

    let leaked = inventory(&outside)?;
    Ok(Outcome::new(
        CRITERION,
        leaked.is_empty(),
        format!(
            "a contained take wrote only its assigned path; {} refused ({}); {} file(s) outside \
             the staging root",
            refused.len(),
            refused.join("; "),
            leaked.len()
        ),
    ))
}

/// Every regular file beneath `root`, so two inventories can be differenced.
fn inventory(root: &Path) -> Result<BTreeSet<PathBuf>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                found.insert(entry.path());
            }
        }
    }
    Ok(found)
}

/// One synthesis request for the qualified worker.
///
/// The cache key is a well-formed placeholder: nothing in the executor reads
/// it, and deriving a real one would need a lesson and a plan this instrument
/// has no reason to carry. What the key is *made of* is E1-S3's cache tests;
/// what this instrument measures is the worker session.
fn request(
    voice: &(String, String),
    take: usize,
    text: &str,
) -> Result<SynthesisRequest, Box<dyn Error>> {
    let (voice_profile, conditioning) = voice;
    let take = u32::try_from(take)?;
    Ok(SynthesisRequest {
        request_id: format!("e1-s3-qualification-{take}"),
        segment_id: format!("qualification-{take}"),
        spoken_text: text.to_owned(),
        voice: voice_profile.to_owned(),
        voice_profile: voice_profile.to_owned(),
        voice_conditioning_hash: conditioning.parse()?,
        style: "calm_explanatory".to_owned(),
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
/// Read from the governed voice root, which is the same record
/// `voice_gate::load_profile` verifies `conditionals.pt` against before any
/// synthesis runs. Reading it here rather than asking the worker is what makes
/// the request an independent claim: the executor compares the profile the
/// worker reports back against the one the request named, so a request built
/// from the worker's own answer would make that comparison pass by
/// construction.
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

    let record = fs::read_to_string(profile.join("profile.json"))?;
    let identity = json_string(&record, "profile_id")?;
    let digest = json_string(&record, "conditionals_blake3")?;
    Ok((identity, digest))
}

/// One string field out of a governed record.
///
/// Read by hand rather than with a deserializer: this crate has `serde_json`,
/// but a struct for two fields of a record this instrument does not own would
/// have to be kept in step with a schema E0-S2 governs, and getting that wrong
/// silently is worse than a missing field failing loudly here.
fn json_string(record: &str, field: &str) -> Result<String, Box<dyn Error>> {
    let value: serde_json::Value = serde_json::from_str(record)?;
    Ok(value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("the voice profile record states no `{field}`"))?
        .to_owned())
}

/// The result object an evidence record cites, hashed as it stands.
fn render_result(worker_bundle_hash: &str, outcomes: &[Outcome]) -> String {
    let criteria: Vec<String> = outcomes
        .iter()
        .map(|outcome| {
            format!(
                "    {{\"criterion\": \"{}\", \"result\": \"{}\", \"observed\": \"{}\"}}",
                outcome.criterion,
                if outcome.passed { "pass" } else { "fail" },
                outcome.observed.replace('"', "'")
            )
        })
        .collect();
    format!(
        "{{\n  \"worker_bundle_hash\": \"{worker_bundle_hash}\",\n  \"criteria\": [\n{}\n  ]\n}}",
        criteria.join(",\n")
    )
}
