//! T5 qualification instrument for `DELIVERY-PLAN.md` E1-S3.
//!
//! ```text
//! cargo build --package study-tts-testkit --example worker-qualification
//!
//! unshare --user --map-root-user --net \
//!   ./target/debug/examples/worker-qualification \
//!     --bundle-root <repo> --model-root <governed> --voice-root <governed> \
//!     --output-root <fresh directory>
//! ```
//!
//! **The namespace is required**, and [`NetworkIsolation::require`] refuses the
//! run without it. Built outside it and run inside it, because a build may
//! legitimately reach a network and a qualification run may not.
//!
//! **Run by an operator on the reference machine, never by a workflow.** The
//! criteria below need real weights, a lawful voice profile, and the
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
//! Rust rather than a Python harness because three of the five criteria are
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
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, VoiceConditioningHash,
    VoiceUse,
};
use study_tts_runtime::{
    SynthesisRequest, TtsExecutor, WorkerBundle, WorkerConfiguration, WorkerLauncher,
    WorkerTtsExecutor, resolve_voice_conditioning,
};
use study_tts_testkit::{run_tts_executor_contract_scenario, run_worker_restart_contract_scenario};

/// Takes rendered while proving the model loads once.
///
/// Three rather than two: two takes prove a session survives one request, which
/// the T4 suite already proves against the fake. Three is the smallest count
/// where a per-request reload would have had to happen twice, so a single
/// mis-scoped load cannot be read as a startup cost.
const LIFETIME_TAKES: usize = 3;

/// The sentence both lifetimes of the reproducibility criterion render.
///
/// Deliberately not the listening script. What that script exists to ask is
/// whether audio sounds right to a person; what this asks is whether two
/// processes agree, and agreement on one sentence is agreement on the sampler
/// rather than on the words.
const REPRODUCIBILITY_TEXT: &str =
    "Two workers given one seed and one bundle render this sentence identically.";

/// Take number the reproducibility renders carry.
///
/// Distinct from the restart criterion's only so the two runs' request
/// identifiers differ in a diagnostic log. `take` reaches no seed: the seed
/// every request carries is `worker/launcher.json`'s, applied by
/// `WorkerConfiguration::for_bundle` and unchanged by the take.
const REPRODUCIBILITY_TAKE: usize = 1;

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
    // Before the output root is even created: a `qualification-result.json`
    // that exists at all should be one from a run whose IPv4 egress was denied,
    // the same principle as the output root that must not already exist.
    let isolation = NetworkIsolation::require()?;
    let configuration = Configuration::from_arguments()?;
    // Read from `worker/launcher.json` rather than from the configuration
    // built below, which keeps its launch fields private on purpose: together
    // they are what starts a process under a claimed identity. This is the same
    // declared bundle input the worker itself reads, so the seed recorded in
    // the result is the one both ends used rather than one this file chose.
    let seed = WorkerLauncher::read(&configuration.bundle_root)?.seed;
    let mut outcomes = Vec::new();

    outcomes.push(bundle_identity_is_stable(&configuration)?);

    let launch = WorkerConfiguration::for_bundle(
        &configuration.bundle_root,
        &configuration.model_root,
        &configuration.voice_root,
        &configuration.staging_root,
        // Gates every profile the worker will deserialize, before the child
        // exists — not only the one `governed_voice` selects below, which is
        // why that call can no longer be the first governed read. Same scope:
        // a run that qualifies a worker and never reaches a lesson.
        VoiceUse::VoiceQualification,
    )?;
    let executor = WorkerTtsExecutor::start(&launch)?;
    let descriptor = executor.descriptor();
    let voice = governed_voice(&configuration.voice_root)?;

    outcomes.push(model_loads_once(&configuration, &executor, &voice)?);
    outcomes.push(worker_restarts_and_stays_offline(
        &configuration,
        &launch,
        &voice,
        &isolation,
    )?);
    outcomes.push(lifetimes_render_identical_audio(
        &configuration,
        &launch,
        &voice,
        seed,
    )?);
    outcomes.push(output_stayed_in_the_staging_root(
        &configuration,
        &executor,
        &voice,
    )?);

    // Shut down before the stdout criterion is recorded, because shutdown is
    // where the *end* of the stream is read: what a worker writes past its
    // last frame is exactly what no check sees while the session is still
    // open. A refusal here is the criterion's answer, not the instrument's
    // failure, so it becomes an outcome rather than ending the run.
    let refusal = executor.shutdown().err().map(|error| error.to_string());
    outcomes.push(protocol_stdout_stayed_clean(&executor, refusal.as_deref()));

    let passed = outcomes.iter().all(|outcome| outcome.passed);
    let result = render_result(
        descriptor.worker_bundle_hash.as_str(),
        seed,
        &isolation,
        &outcomes,
    )?;
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
/// worker that merely says it loaded once. `loaded PerthNet` is the exact
/// diagnostic token pinned as the qualification contract for
/// `chatterbox-tts==0.1.2`; a reload would print it again.
fn model_loads_once(
    configuration: &Configuration,
    executor: &WorkerTtsExecutor,
    voice: &GovernedVoice,
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
/// Every take above proves the middle of the session: the executor parses each
/// frame off standard output and refuses anything it cannot read, so a session
/// that completed is a session whose stdout carried nothing but frames. Two
/// things that proof does not cover are added here.
///
/// The *end* of the stream, which `refusal` carries. Nothing reads the response
/// channel after the last request, so until `shutdown` drained it a worker
/// could write anything it liked past its final frame — an unterminated tail
/// most of all, which a line-oriented reader discards at end of input — and the
/// session still looked clean. This criterion passed for such a worker, which
/// is the tenth-audit shape of a check named more strongly than its predicate.
///
/// And that the proof is not vacuous: the backend really does write
/// diagnostics, and they really did land on the other channel.
fn protocol_stdout_stayed_clean(executor: &WorkerTtsExecutor, refusal: Option<&str>) -> Outcome {
    const CRITERION: &str = "t5_e1_worker_protocol_stdout_remains_clean";

    let diagnostics = executor.diagnostics();
    let Some(refusal) = refusal else {
        return Outcome::new(
            CRITERION,
            !diagnostics.trim().is_empty(),
            format!(
                "every frame of a completed session parsed off standard output and nothing \
                 followed the last one, while {} bytes of backend diagnostics went to standard \
                 error",
                diagnostics.len()
            ),
        );
    };
    Outcome::new(
        CRITERION,
        false,
        format!("the session's standard output was refused at shutdown: {refusal}"),
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
/// Offline has two independent halves, and this criterion needs both.
///
/// **IPv4 egress was denied**, which [`NetworkIsolation`] established before
/// the run began: a loopback-only namespace with no IPv4 route. That is the
/// half the worker cannot attest to. Until it existed, this criterion asserted
/// "operates without network access" while measuring nothing of the kind.
///
/// **The worker configured itself**, read out of its own diagnostics rather
/// than assumed: `_apply_offline_environment` prints the variables it *applied*
/// in that process, so a launcher that merely names them does not satisfy this.
/// Since `WorkerClient::spawn` now clears the environment, the second lifetime
/// also proves the declared set is enough to start a worker from nothing.
///
/// Neither half implies the other. Flags steer `huggingface_hub` and
/// `transformers` and bind no socket; a namespace says nothing about what the
/// worker would have tried to fetch had it been able to.
fn worker_restarts_and_stays_offline(
    configuration: &Configuration,
    launch: &WorkerConfiguration,
    voice: &GovernedVoice,
    isolation: &NetworkIsolation,
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
            "two lifetimes reported identical synthesis identities; offline settings applied in \
             {} of 2 lifetimes, inside a network namespace holding {:?} with no IPv4 route",
            offline.iter().filter(|applied| **applied).count(),
            isolation.interfaces
        ),
    ))
}

/// `t5_e1_two_lifetimes_render_identical_audio_under_one_seed`.
///
/// **Not a `DELIVERY-PLAN.md` name.** It has exactly the standing
/// `t5_e1_worker_survives_restart_and_starts_offline` has: a helper criterion
/// covering a property nothing shared between the fake and the real worker
/// measures. `DELIVERY-PLAN.md` is digest-pinned by accepted evidence under
/// `evidence/gates/`, so a criterion name is not added to it here; adding one
/// is a project-owner edit with a provenance recomputation behind it.
///
/// **This is the criterion `capabilities.deterministic_seed` turns on, and the
/// only thing that can turn it on.** `worker/study_tts_worker/worker.py` seeds
/// every generator before the model is constructed as well as before each take,
/// which removes the defect that made the answer necessarily `False` — the
/// vocoder's noise used to be drawn once per process from an unseeded
/// generator, so each lifetime was internally consistent and lifetimes
/// disagreed with each other. That is a mechanism. Whether the answer is now
/// `True` is a measurement, and this is it.
/// `docs/architecture/E1-S3-INTERFACE-CHANGE-004.md` §Limits carries what the
/// flip then moves.
///
/// The two lifetimes are driven through
/// [`run_worker_restart_contract_scenario`] rather than a loop written here,
/// for the reason this whole instrument is a Rust example: it is the same
/// function the T4 suite drives the protocol fake through, so the shipped path
/// is what is qualified. It costs two more model loads, which is why the
/// criterion is stated in `scripts/qualification/README.md` as a run-time cost
/// rather than discovered as one.
fn lifetimes_render_identical_audio(
    configuration: &Configuration,
    launch: &WorkerConfiguration,
    voice: &GovernedVoice,
    seed: u64,
) -> Result<Outcome, Box<dyn Error>> {
    const CRITERION: &str = "t5_e1_two_lifetimes_render_identical_audio_under_one_seed";

    let staging = configuration.staging_root.join("reproducibility");
    fs::create_dir_all(&staging)?;
    let first = staging.join("first.wav");
    let second = staging.join("second.wav");

    let lifetimes = run_worker_restart_contract_scenario(
        launch,
        &request(voice, REPRODUCIBILITY_TAKE, REPRODUCIBILITY_TEXT)?,
        &first,
        &second,
    )?;

    // The identity half before the audio half. Two lifetimes reporting
    // different synthesis contexts would be two different bundles, and
    // identical bytes out of those would be a coincidence rather than the
    // property this criterion names -- while different bytes would be explained
    // by the identity rather than by the sampler.
    let [first_lifetime, second_lifetime] = &lifetimes;
    if first_lifetime.report.context != second_lifetime.report.context {
        return Ok(Outcome::new(
            CRITERION,
            false,
            "the two lifetimes reported different synthesis identities, so their renders \
             are not comparable and nothing about reproducibility was measured"
                .to_owned(),
        ));
    }

    let comparison = compare_renders(&first, &second)?;
    Ok(Outcome::new(
        CRITERION,
        comparison.passed(),
        comparison.observed(seed),
    ))
}

/// How two renders of one request compare, as canonical bytes and as samples.
#[derive(Debug)]
struct RenderComparison {
    /// SHA-256 of each take's canonical re-encoding, in lifetime order.
    digests: [String; 2],
    /// SHA-256 of each take as the worker wrote it, in lifetime order.
    ///
    /// Reported and never judged. See [`RenderComparison::passed`].
    staged_digests: [String; 2],
    /// Frames each file decoded to, in lifetime order.
    frames: [usize; 2],
    /// Whether the two files are byte-identical.
    identical_bytes: bool,
    /// Frames whose sample bits differ, plus the difference in frame counts.
    differing_frames: usize,
    /// The largest absolute sample difference over the frames both hold.
    largest_difference: f32,
}

impl RenderComparison {
    /// Whether the criterion holds.
    ///
    /// Byte equality of the **canonical** encoding, which is the artifact a
    /// cache entry is validated and addressed by. Not the bytes the worker
    /// wrote: `soundfile` stamps a wall-clock time into libsndfile's `PEAK`
    /// chunk, so two staged takes of identical audio differ by one byte at
    /// offset 61 and comparing those measures the clock rather than the
    /// sampler. That is not a hypothetical — it is what the first run of this
    /// criterion found, and the reason it reads this way.
    ///
    /// The timestamp reaches nothing downstream:
    /// `cache::write_canonical_samples` decodes, conditions, and rewrites every
    /// take through `hound` from the samples and a fixed spec, so a published
    /// entry carries no `PEAK` chunk.
    /// Re-encoding both takes the same way is therefore not an approximation of
    /// the gate — it *is* the artifact the gate names.
    fn passed(&self) -> bool {
        self.identical_bytes
    }

    /// What was measured, in the shape an evidence table reads.
    fn observed(&self, seed: u64) -> String {
        if self.identical_bytes {
            return format!(
                "two fresh lifetimes under launcher seed {seed} produced byte-identical canonical \
                 WAVs of {} frames at SHA-256 {}; the takes as staged differ only in libsndfile's \
                 `PEAK` wall-clock stamp ({} and {}), which no published entry carries",
                self.frames[0], self.digests[0], self.staged_digests[0], self.staged_digests[1]
            );
        }
        if self.differing_frames == 0 {
            return format!(
                "two fresh lifetimes under launcher seed {seed} decoded to identical PCM over \
                 {} frames and still re-encoded to different canonical bytes, SHA-256 {} and {}: \
                 the sampler is reproducible and the canonical encoder is not",
                self.frames[0], self.digests[0], self.digests[1]
            );
        }
        format!(
            "two fresh lifetimes under launcher seed {seed} disagreed on {} frames, by at most \
             {:e}; they decoded to {} and {} frames, SHA-256 {} and {}",
            self.differing_frames,
            self.largest_difference,
            self.frames[0],
            self.frames[1],
            self.digests[0],
            self.digests[1],
        )
    }
}

/// Compares two rendered takes as bytes and as decoded samples.
///
/// Both, because they answer different questions and only one of them is the
/// gate. A sampler that drew different noise and a container that recorded a
/// different chunk produce the same verdict and are completely different
/// defects, and an operator who is told only "the bytes differ" has to go and
/// find out which.
///
/// Samples are compared as bit patterns rather than with `==`, so a NaN
/// compares equal to itself: an encoder that emitted one in both lifetimes is
/// reproducible, and float equality would report that as a difference forever.
///
/// # Errors
///
/// Whatever reading either file reports, and a refusal naming the file when it
/// is not a canonical-format take.
fn compare_renders(first: &Path, second: &Path) -> Result<RenderComparison, Box<dyn Error>> {
    let staged = [fs::read(first)?, fs::read(second)?];
    let samples = [decoded_samples(first)?, decoded_samples(second)?];
    let canonical = [
        canonical_encoding(&samples[0])?,
        canonical_encoding(&samples[1])?,
    ];

    let paired = || samples[0].iter().zip(&samples[1]);
    let differing_over_common_frames = paired()
        .filter(|(left, right)| left.to_bits() != right.to_bits())
        .count();

    Ok(RenderComparison {
        digests: [
            hex(&Sha256::digest(&canonical[0])),
            hex(&Sha256::digest(&canonical[1])),
        ],
        staged_digests: [
            hex(&Sha256::digest(&staged[0])),
            hex(&Sha256::digest(&staged[1])),
        ],
        frames: [samples[0].len(), samples[1].len()],
        identical_bytes: canonical[0] == canonical[1],
        // The length difference counts. Frames one lifetime produced and the
        // other did not are frames the two disagreed on, and zipping alone
        // would report a truncated render as perfect agreement over its own
        // length.
        differing_frames: differing_over_common_frames
            + samples[0].len().abs_diff(samples[1].len()),
        largest_difference: paired()
            .map(|(left, right)| (left - right).abs())
            .fold(0.0_f32, f32::max),
    })
}

/// One take re-encoded exactly as the cache would publish it.
///
/// Mirrors `cache::write_canonical_samples`: the same fixed spec, written
/// through `hound` from the samples, with no chunk carrying anything but audio.
/// A change to that function without a matching change here would compare an
/// artifact the cache does not write, so the two are named in each other's
/// comments.
///
/// In memory rather than through a file, because nothing needs the bytes to
/// exist anywhere: what is wanted is whether two sample vectors encode alike.
///
/// # Errors
///
/// Whatever the encoder reports.
fn canonical_encoding(samples: &[f32]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(
        &mut buffer,
        hound::WavSpec {
            channels: CANONICAL_CHANNELS,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )?;
    for sample in samples {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(buffer.into_inner())
}

/// The canonical float samples one rendered take holds.
///
/// The format is checked rather than assumed: a take in some other shape would
/// decode to samples that compare fine and describe audio this project does not
/// publish, which is a passing criterion about the wrong thing.
///
/// # Errors
///
/// Whatever opening or decoding the file reports, and a refusal naming the file
/// when its declared format is not the canonical one.
fn decoded_samples(path: &Path) -> Result<Vec<f32>, Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != CANONICAL_CHANNELS
        || spec.sample_rate != CANONICAL_SAMPLE_RATE
        || spec.sample_format != hound::SampleFormat::Float
        || spec.bits_per_sample != 32
    {
        return Err(format!(
            "`{}` is not a canonical-format take: {spec:?}",
            path.display()
        )
        .into());
    }
    Ok(reader.samples::<f32>().collect::<Result<Vec<_>, _>>()?)
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
    voice: &GovernedVoice,
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
    voice: &GovernedVoice,
    take: usize,
    text: &str,
) -> Result<SynthesisRequest, Box<dyn Error>> {
    let take = u32::try_from(take)?;
    Ok(SynthesisRequest {
        request_id: format!("e1-s3-qualification-{take}"),
        segment_id: format!("qualification-{take}"),
        spoken_text: text.to_owned(),
        voice: voice.profile_id.clone(),
        voice_profile: voice.profile_id.clone(),
        voice_conditioning_hash: voice.conditioning.clone(),
        style: "calm_explanatory".to_owned(),
        language: "en".parse()?,
        take,
        cache_key: "0".repeat(64).parse()?,
        sample_rate: CANONICAL_SAMPLE_RATE,
        channels: CANONICAL_CHANNELS,
        sample_format: CANONICAL_SAMPLE_FORMAT.to_owned(),
    })
}

/// The voice this run renders with, resolved through the rights gate.
#[derive(Debug)]
struct GovernedVoice {
    profile_id: String,
    conditioning: VoiceConditioningHash,
}

/// The voice profile this run renders with, resolved through the rights gate.
///
/// Discovery is by directory name; the *load* goes through
/// [`resolve_voice_conditioning`], which is the gate a build passes before any
/// synthesis: consent status, rights decision, permitted-use scope, and the
/// bytes of both `reference.wav` and `conditionals.pt` against the digests the
/// record states. An earlier version of this read `profile.json` by hand and
/// took `conditionals_blake3` on trust, so this instrument could qualify a
/// worker against a voice whose consent had been revoked and whose artifact had
/// been swapped — while its own comment claimed the gate had verified it.
///
/// [`VoiceUse::VoiceQualification`] is the scope: this is a qualification run
/// that never reaches a lesson, which is the use that variant names. A governed
/// `consent.json` that does not permit it refuses the run, and that refusal is
/// the check working.
///
/// Reading from disk rather than asking the worker is still what makes the
/// request an independent claim: the executor compares the profile the worker
/// reports back against the one the request named, so a request built from the
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

/// The loopback-only network namespace this instrument refuses to run outside.
///
/// The two-sided other end is `validate_network_isolation` in
/// `scripts/qualification/chatterbox_spike.py`, which asks the same two
/// questions of the same two files for the E0-S3 harness; the operator
/// procedure is `scripts/qualification/README.md` §E1-S3. Kept as its own
/// type so the answers reach [`render_result`]: an evidence record should be
/// able to read the isolation off the artifact rather than take the run's word
/// for it.
struct NetworkIsolation {
    /// Interface names the namespace holds: `lo`, and nothing else.
    interfaces: Vec<String>,
    /// IPv4 routes the namespace carries, which is none.
    ///
    /// Counted and carried rather than written into the result as a literal:
    /// a constant printed where a reader expects a measurement is the shape
    /// this whole finding was about.
    ipv4_routes: usize,
    /// The namespace itself, so two runs can be told apart or matched up.
    namespace_inode: u64,
}

impl NetworkIsolation {
    /// Refuses unless this process is in a loopback-only network namespace.
    ///
    /// ADR-0001 §17.7 asks the worker to operate without network access, and
    /// the criterion below used to read that off the worker's own diagnostics:
    /// `_apply_offline_environment` prints the variables it applied, which
    /// proves the worker configured `huggingface_hub` and `transformers` and
    /// proves nothing about the backend, a transitive dependency, or a socket.
    /// Environment flags are a request; a namespace with no IPv4 route is a
    /// denial.
    ///
    /// Refused rather than reported, so the run has to be wrapped:
    ///
    /// ```text
    /// unshare --user --map-root-user --net <this instrument> ...
    /// ```
    ///
    /// # Errors
    ///
    /// When `/proc/net/dev` or `/proc/net/route` cannot be read, when any
    /// interface besides `lo` exists, or when the namespace carries an IPv4
    /// route. Every one names the wrapper as the remedy, and the operator is
    /// the remedy owner per `docs/governance/ROUTING-TABLES.md`.
    fn require() -> Result<Self, Box<dyn Error>> {
        // Both files carry header lines the kernel writes for `ip` and
        // `netstat` to skip: two in `/proc/net/dev`, one in `/proc/net/route`.
        let interfaces: Vec<String> = fs::read_to_string("/proc/net/dev")?
            .lines()
            .skip(2)
            .filter_map(|line| line.split_once(':'))
            .map(|(name, _)| name.trim().to_owned())
            .collect();
        if interfaces != ["lo"] {
            return Err(format!(
                "this instrument must run in a loopback-only network namespace, and this one \
                 holds {interfaces:?}; wrap the command in `unshare --user --map-root-user --net`"
            )
            .into());
        }
        let ipv4_routes = fs::read_to_string("/proc/net/route")?
            .lines()
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .count();
        if ipv4_routes > 0 {
            return Err(format!(
                "this instrument's network namespace carries {ipv4_routes} IPv4 route(s), so \
                 egress is reachable; wrap the command in `unshare --user --map-root-user --net`"
            )
            .into());
        }
        Ok(Self {
            interfaces,
            ipv4_routes,
            namespace_inode: fs::metadata("/proc/self/ns/net")?.ino(),
        })
    }
}

/// The result object an evidence record cites, hashed as it stands.
fn render_result(
    worker_bundle_hash: &str,
    launcher_seed: u64,
    isolation: &NetworkIsolation,
    outcomes: &[Outcome],
) -> Result<String, serde_json::Error> {
    let criteria: Vec<serde_json::Value> = outcomes
        .iter()
        .map(|outcome| {
            serde_json::json!({
                "criterion": outcome.criterion,
                "result": if outcome.passed { "pass" } else { "fail" },
                "observed": outcome.observed,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({
        "worker_bundle_hash": worker_bundle_hash,
        // Recorded because one criterion's claim is about this value. A result
        // asserting that two lifetimes agreed, without saying what they agreed
        // under, describes an experiment nobody can repeat.
        "launcher_seed": launcher_seed,
        "network_isolation": {
            "interfaces": isolation.interfaces,
            "ipv4_routes": isolation.ipv4_routes,
            "namespace_inode": isolation.namespace_inode,
        },
        "criteria": criteria,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    use study_tts_testkit::{VoiceProfileFixtureSpec, write_voice_profile_fixture};
    use tempfile::TempDir;

    /// A synthetic governed root whose consent carries `status`.
    ///
    /// `voice_qualification` scope, because that is what this instrument
    /// requests: a run that qualifies a worker and never reaches a lesson.
    fn voice_root(status: &str) -> TempDir {
        let workspace = TempDir::new().expect("create a qualification workspace");
        let spec = VoiceProfileFixtureSpec {
            consent_status: status.to_owned(),
            permitted_use: vec!["voice_qualification".to_owned()],
            ..VoiceProfileFixtureSpec::default()
        };
        write_voice_profile_fixture(&workspace.path().join(&spec.profile_id), &spec);
        workspace
    }

    #[test]
    fn t1_e1_a_revoked_consent_refuses_the_governed_voice() {
        // This instrument read `profile.json` by hand and took its digest on
        // trust, so it could qualify a worker against a voice whose consent had
        // been revoked — while its own comment claimed the gate had verified
        // the artifact. No worker is started here: the refusal is what the
        // instrument's first governed read must produce.
        let granted = voice_root("granted");
        governed_voice(granted.path()).expect("a rights-clean voice resolves");

        let revoked = voice_root("revoked");
        let refusal = governed_voice(revoked.path())
            .expect_err("revoked consent must refuse the run")
            .to_string();

        assert!(
            refusal.contains("consent"),
            "the refusal does not name consent: {refusal}"
        );
    }

    #[test]
    fn t1_e1_a_voice_outside_the_qualification_scope_is_refused() {
        // The scope this instrument renders under is not the one a build uses.
        // A profile consented only to `private_synthesis` is not consented to
        // being qualified, and the governed record is what must change rather
        // than the request.
        let workspace = TempDir::new().expect("create a qualification workspace");
        let spec = VoiceProfileFixtureSpec::default();
        write_voice_profile_fixture(&workspace.path().join(&spec.profile_id), &spec);

        let refusal = governed_voice(workspace.path())
            .expect_err("a voice outside the qualification scope must be refused")
            .to_string();

        assert!(
            refusal.contains("voice_qualification"),
            "the refusal does not name the requested use: {refusal}"
        );
    }

    /// Writes `samples` as a canonical-format take at `path`.
    fn write_take(path: &Path, samples: &[f32]) {
        let mut writer = hound::WavWriter::create(
            path,
            hound::WavSpec {
                channels: CANONICAL_CHANNELS,
                sample_rate: CANONICAL_SAMPLE_RATE,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .expect("create a canonical take");
        for sample in samples {
            writer.write_sample(*sample).expect("write a sample");
        }
        writer.finalize().expect("finalize the take");
    }

    /// Two takes written into one scratch directory, then compared.
    fn compare(first: &[f32], second: &[f32]) -> RenderComparison {
        let workspace = TempDir::new().expect("create a comparison workspace");
        let first_path = workspace.path().join("first.wav");
        let second_path = workspace.path().join("second.wav");
        write_take(&first_path, first);
        write_take(&second_path, second);
        compare_renders(&first_path, &second_path).expect("compare two canonical takes")
    }

    /// The T5 criterion's own logic, at a tier that costs no weights.
    ///
    /// `t5_e1_two_lifetimes_render_identical_audio_under_one_seed` can only run
    /// on the reference machine, so nothing would otherwise ever exercise the
    /// code deciding its verdict. These four cover the decision; the reference
    /// machine supplies the audio.
    #[test]
    fn t1_e1_two_identical_takes_compare_as_reproducible() {
        let comparison = compare(&[0.25, -0.5, 0.75], &[0.25, -0.5, 0.75]);

        assert!(comparison.passed(), "identical bytes are the criterion");
        assert_eq!(comparison.differing_frames, 0);
        assert_eq!(comparison.digests[0], comparison.digests[1]);
        assert_eq!(comparison.frames, [3, 3]);
    }

    #[test]
    fn t1_e1_one_differing_sample_is_counted_and_measured() {
        let comparison = compare(&[0.25, -0.5, 0.75], &[0.25, -0.25, 0.75]);

        assert!(!comparison.passed());
        assert_eq!(comparison.differing_frames, 1);
        assert!(
            (comparison.largest_difference - 0.25).abs() < f32::EPSILON,
            "the reported difference must be the one that was heard, got {}",
            comparison.largest_difference
        );
    }

    #[test]
    fn t1_e1_a_truncated_take_is_not_agreement_over_its_own_length() {
        // Zipping alone reports perfect agreement here, because every frame the
        // shorter take holds matches. The frames it does not hold are frames
        // the two lifetimes disagreed on.
        let comparison = compare(&[0.25, -0.5, 0.75], &[0.25]);

        assert!(!comparison.passed());
        assert_eq!(comparison.differing_frames, 2);
        assert_eq!(comparison.frames, [3, 1]);
    }

    #[test]
    fn t1_e1_a_take_that_is_not_canonical_format_is_refused() {
        let workspace = TempDir::new().expect("create a format workspace");
        let canonical = workspace.path().join("first.wav");
        let stereo = workspace.path().join("second.wav");
        write_take(&canonical, &[0.25]);
        let mut writer = hound::WavWriter::create(
            &stereo,
            hound::WavSpec {
                channels: 2,
                sample_rate: CANONICAL_SAMPLE_RATE,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .expect("create a two-channel take");
        writer.write_sample(0.25_f32).expect("write a sample");
        writer.write_sample(0.25_f32).expect("write a sample");
        writer.finalize().expect("finalize the take");

        let refusal = compare_renders(&canonical, &stereo)
            .expect_err("a take outside the canonical format must be refused")
            .to_string();

        assert!(
            refusal.contains("not a canonical-format take"),
            "the refusal does not name the format: {refusal}"
        );
    }

    #[test]
    fn t1_e1_a_take_that_is_not_a_number_still_compares_equal_to_itself() {
        // The reason samples are compared as bit patterns. Under `==` a NaN is
        // unequal to itself, so two lifetimes that both emitted one would be
        // reported as disagreeing forever -- while the bytes on disk, and the
        // verdict, said they agreed.
        let comparison = compare(&[f32::NAN, 0.25], &[f32::NAN, 0.25]);

        assert!(comparison.passed());
        assert_eq!(comparison.differing_frames, 0);
    }

    /// The case the first real run of this criterion found.
    ///
    /// `soundfile` writes libsndfile's `PEAK` chunk, which carries the
    /// wall-clock time of the write. Two lifetimes rendering bit-identical
    /// audio 35 seconds apart produced staged files differing by one byte at
    /// offset 61, and the criterion failed on it while the sampler was
    /// perfectly reproducible. Re-encoding canonically is what makes the
    /// verdict about the artifact a cache entry is addressed by; this is the
    /// regression that would catch a return to comparing staged bytes.
    #[test]
    fn t1_e1_a_container_timestamp_does_not_make_two_takes_irreproducible() {
        let workspace = TempDir::new().expect("create a timestamp workspace");
        let first = workspace.path().join("first.wav");
        let second = workspace.path().join("second.wav");
        let samples = [0.25_f32, -0.5, 0.75];
        write_take(&first, &samples);
        write_take(&second, &samples);

        // Standing in for the `PEAK` stamp: one byte of container metadata,
        // ahead of the sample data, differing between two identical renders.
        let mut bytes = std::fs::read(&second).expect("read the second take");
        bytes[16] ^= 0xff;
        std::fs::write(&second, &bytes).expect("restamp the second take");

        let comparison = compare_renders(&first, &second).expect("compare two takes");

        assert_ne!(
            comparison.staged_digests[0], comparison.staged_digests[1],
            "the staged bytes must differ, or this test exercises nothing"
        );
        assert_eq!(comparison.differing_frames, 0, "the audio is identical");
        assert!(
            comparison.passed(),
            "identical audio in a restamped container is reproducible: {}",
            comparison.observed(42)
        );
    }

    #[test]
    fn t1_e1_a_nondeterministic_canonical_encoder_is_named_as_such() {
        // Built by hand: the canonical encoder is a pure function of spec and
        // samples, so this state cannot be produced. It is still the state the
        // criterion would have to report if that ever stopped being true, and
        // the sentence must not blame the sampler for it.
        let comparison = RenderComparison {
            digests: ["a".repeat(64), "b".repeat(64)],
            staged_digests: ["c".repeat(64), "d".repeat(64)],
            frames: [96_000, 96_000],
            identical_bytes: false,
            differing_frames: 0,
            largest_difference: 0.0,
        };

        let observed = comparison.observed(42);

        assert!(!comparison.passed(), "canonical byte equality is the gate");
        assert!(
            observed.contains("the sampler is reproducible and the canonical encoder is not"),
            "the observation must name the encoder, not the sampler: {observed}"
        );
    }

    #[test]
    fn t1_e1_qualification_observations_are_json_escaped() {
        let isolation = NetworkIsolation {
            interfaces: vec!["lo".to_owned()],
            ipv4_routes: 0,
            namespace_inode: 1,
        };
        let outcomes = [Outcome::new(
            "criterion-escape",
            true,
            "quoted \"value\" with \\ and newline\n".to_owned(),
        )];

        let rendered = render_result("bundle", 42, &isolation, &outcomes)
            .expect("serialize the qualification result");
        let result: serde_json::Value =
            serde_json::from_str(&rendered).expect("the qualification result is JSON");

        assert_eq!(
            result["criteria"][0]["observed"],
            "quoted \"value\" with \\ and newline\n"
        );
    }

    #[test]
    fn t1_e1_route_evidence_names_its_ipv4_scope() {
        let isolation = NetworkIsolation {
            interfaces: vec!["lo".to_owned()],
            ipv4_routes: 2,
            namespace_inode: 1,
        };

        let rendered = render_result("bundle", 42, &isolation, &[])
            .expect("serialize the route measurement as evidence");
        let result: serde_json::Value =
            serde_json::from_str(&rendered).expect("the qualification result is JSON");

        assert_eq!(result["network_isolation"]["ipv4_routes"], 2);
    }
}
