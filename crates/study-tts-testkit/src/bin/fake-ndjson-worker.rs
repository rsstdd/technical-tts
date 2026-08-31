//! Executable protocol fake for worker supervision and fault-injection tests.
//!
//! It speaks the provisional frames and emits synthetic WAVs from a loaded
//! synthetic backend; command-line behaviors expose failures for supervisor
//! tests without loading model weights.

use std::{
    collections::BTreeMap,
    error::Error,
    io::{BufRead, Write},
    path::Path,
    time::Duration,
};

use study_tts_core::{
    CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE, DeliveryStyle,
};
use study_tts_runtime::{
    DriftedIdentity, MAX_WORKER_FRAME_BYTES, THREAD_ENVIRONMENT, WorkerCapabilities,
    WorkerFailureCode, WorkerInitializationIdentities, WorkerRequestFrame, WorkerResponseFrame,
    parse_worker_request,
};
use study_tts_testkit::{
    DETERMINISTIC_TONE_BUNDLE_HASH, FIXTURE_VOICE_PROFILES, deterministic_tone_conditioning,
};

const FAKE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

/// The one profile this fake's synthetic voice root holds.
const FAKE_VOICE_PROFILE: &str = "synthetic-test-voice-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Behavior {
    Deterministic,
    Delay,
    Failure,
    Hang,
    /// Answers the startup exchanges, then never answers a synthesis.
    ///
    /// [`Behavior::Hang`] parks on the first frame it sees, which is
    /// `initialize` — so it can only exercise a deadline during `start`, where
    /// the client is dropped on the way out. This one reaches a *live*
    /// executor, which is where the timeout path has to reap the tree itself.
    HangOnSynthesis,
    MalformedFrame,
    TruncatedAudio,
    Stderr,
    Exit,
    EscapeStaging,
    /// Writes the assigned take, and a second file beside it.
    ///
    /// The staging directory becomes the published cache entry, so anything
    /// left in it is published too. This is the worker that leaves a scratch
    /// file behind — a likelier accident than a traversal, and one the
    /// assigned path being correct does not catch.
    LitterStaging,
    /// Writes the assigned take, and creates a directory beside it.
    ///
    /// The same defect as [`Behavior::LitterStaging`] in the shape a check
    /// written with `is_file` would have missed.
    LitterStagingDirectory,
    /// Declares an envelope this build cannot publish.
    ///
    /// A worker rendering at another rate is a session that can only end in
    /// refused takes, so it is refused when it opens rather than per segment.
    NonCanonicalFormat,
    OversizedFrame,
    ForeignRequestId,
    /// Reports one identity a success frame restates as something else.
    ///
    /// Carries [`DriftedIdentity`] rather than a behavior per field so the fake
    /// and the executor name the drift with one enum: a fifth identity cannot
    /// be added on one side and forgotten on the other.
    Drift(DriftedIdentity),
}

fn main() -> Result<(), Box<dyn Error>> {
    let behavior = parse_behavior(std::env::args().nth(1).as_deref())?;
    if behavior == Behavior::Exit {
        std::process::exit(17);
    }
    if behavior == Behavior::Stderr {
        eprintln!("fake worker diagnostic on stderr");
    }
    // Reported unconditionally, and on stderr because stdout is the protocol
    // channel. ADR-0001 §10.1 has the launching parent cap every native
    // numerical pool at the same per-worker value, and a variable that never
    // reached the child is a cap that exists only in the parent's intention.
    // The real worker reports what it applied for the same reason
    // (`_apply_offline_environment` in `worker/study_tts_worker/worker.py`);
    // this is the observable that lets a test read the caps back out.
    let threads: Vec<String> = THREAD_ENVIRONMENT
        .iter()
        .map(|name| {
            format!(
                "{name}={}",
                std::env::var(name).unwrap_or_else(|_| "unset".to_owned())
            )
        })
        .collect();
    eprintln!("fake worker thread environment: {}", threads.join(" "));

    // Names only, never values: a governed root reaches this child as a value,
    // and `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps a governed
    // location out of a log. The names are enough for a test to prove the child
    // holds exactly what the parent declared and nothing it inherited, which is
    // the observable side of `Command::env_clear` in `WorkerClient::spawn`.
    let mut names: Vec<String> = std::env::vars().map(|(name, _)| name).collect();
    names.sort();
    eprintln!("fake worker environment names: {}", names.join(","));

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let frame = parse_worker_request(line.as_bytes())?;
        if behavior == Behavior::Hang
            || (behavior == Behavior::HangOnSynthesis
                && matches!(frame, WorkerRequestFrame::Synthesize { .. }))
        {
            loop {
                std::thread::park();
            }
        }
        if behavior == Behavior::Delay {
            std::thread::sleep(Duration::from_millis(100));
        }
        if behavior == Behavior::MalformedFrame {
            stdout.write_all(b"{malformed\n")?;
            stdout.flush()?;
            continue;
        }
        // One byte past the protocol ceiling and no newline at all, so a reader
        // that grows a buffer until a line ends is handed memory without limit.
        // ADR-0001 §10.3 caps the message length; this is the case that walks
        // at it.
        if behavior == Behavior::OversizedFrame {
            stdout.write_all(&vec![b'x'; MAX_WORKER_FRAME_BYTES + 1])?;
            stdout.flush()?;
            continue;
        }
        // A well-formed frame answering a request nobody asked. ADR-0001 §10.3
        // puts a request ID on every frame so a supervisor can correlate; a
        // supervisor that skipped the check would attribute this answer — and
        // one day another segment's audio — to the request in flight.
        if behavior == Behavior::ForeignRequestId {
            let mut response = respond(frame, Behavior::Deterministic)?;
            if let WorkerResponseFrame::SynthesisSucceeded { request_id, .. }
            | WorkerResponseFrame::Initialized { request_id, .. }
            | WorkerResponseFrame::Capabilities { request_id, .. } = &mut response
            {
                *request_id = "a-request-nobody-made".to_owned();
            }
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
            continue;
        }

        let response = respond(frame, behavior)?;
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

/// `honest`, unless this run is the one drifting `identity`.
///
/// The spoiled value only has to differ, so it is derived from the honest one
/// rather than written out: a literal would be a second place to edit when the
/// honest value changes, and a drift case that silently stopped differing would
/// pass.
fn drifted_or(honest: &str, drift: Option<DriftedIdentity>, identity: DriftedIdentity) -> String {
    if drift == Some(identity) {
        return format!("{honest}-drifted");
    }
    honest.to_owned()
}

fn parse_behavior(value: Option<&str>) -> Result<Behavior, Box<dyn Error>> {
    match value.unwrap_or("deterministic") {
        "deterministic" => Ok(Behavior::Deterministic),
        "delay" => Ok(Behavior::Delay),
        "failure" => Ok(Behavior::Failure),
        "hang" => Ok(Behavior::Hang),
        "hang-on-synthesis" => Ok(Behavior::HangOnSynthesis),
        "malformed-frame" => Ok(Behavior::MalformedFrame),
        "truncated-audio" => Ok(Behavior::TruncatedAudio),
        "escape-staging" => Ok(Behavior::EscapeStaging),
        "litter-staging" => Ok(Behavior::LitterStaging),
        "litter-staging-directory" => Ok(Behavior::LitterStagingDirectory),
        "non-canonical-format" => Ok(Behavior::NonCanonicalFormat),
        "oversized-frame" => Ok(Behavior::OversizedFrame),
        "foreign-request-id" => Ok(Behavior::ForeignRequestId),
        "drift-bundle" => Ok(Behavior::Drift(DriftedIdentity::WorkerBundle)),
        "drift-model" => Ok(Behavior::Drift(DriftedIdentity::Model)),
        "drift-codec" => Ok(Behavior::Drift(DriftedIdentity::Codec)),
        "drift-voice" => Ok(Behavior::Drift(DriftedIdentity::VoiceProfile)),
        "stderr" => Ok(Behavior::Stderr),
        "exit" => Ok(Behavior::Exit),
        unknown => Err(format!("unknown fake-worker behavior `{unknown}`").into()),
    }
}

fn respond(
    frame: WorkerRequestFrame,
    behavior: Behavior,
) -> Result<WorkerResponseFrame, Box<dyn Error>> {
    if behavior == Behavior::Failure {
        return Ok(failure_response(&frame));
    }
    match frame {
        WorkerRequestFrame::Initialize {
            protocol_version,
            request_id,
            parameters,
        } => {
            let worker_bundle_hash = DETERMINISTIC_TONE_BUNDLE_HASH.parse()?;
            if parameters.worker_bundle_hash != worker_bundle_hash {
                return Ok(WorkerResponseFrame::Failure {
                    protocol_version,
                    request_id,
                    code: WorkerFailureCode::InitializationFailed,
                    message: "requested bundle identity does not match the deterministic fake"
                        .to_owned(),
                    recoverable: false,
                });
            }
            Ok(WorkerResponseFrame::Initialized {
                protocol_version,
                request_id,
                identities: WorkerInitializationIdentities {
                    model_revision: "v1".parse()?,
                    tokenizer_revision: "none".parse()?,
                    worker_bundle_hash,
                    // Read out of the synthetic voice root this fake stands
                    // in for, not handed to it: `initialize` names no voice, so
                    // this is what the worker went and looked at.
                    voice_conditioning_hashes: BTreeMap::from([(
                        FAKE_VOICE_PROFILE.to_owned(),
                        deterministic_tone_conditioning(FAKE_VOICE_PROFILE),
                    )]),
                },
            })
        }
        WorkerRequestFrame::Capabilities {
            protocol_version,
            request_id,
        } => Ok(WorkerResponseFrame::Capabilities {
            protocol_version,
            request_id,
            capabilities: WorkerCapabilities {
                languages: vec!["en".to_owned()],
                max_text_bytes: 64 * 1024,
                // The profiles a synthetic voice root stands in for, and
                // the style the fixture lessons actually bind. Both were
                // fiction until the executor started enforcing the declared
                // envelope: this fake advertised one profile of two and the
                // style `calm`, which no `DeliveryStyle` spells, while every
                // request it accepted named something else. A capability list
                // nothing checks is a list that drifts.
                voices: FIXTURE_VOICE_PROFILES.map(str::to_owned).to_vec(),
                styles: vec![DeliveryStyle::CalmExplanatory.as_str().to_owned()],
                sample_rate: if behavior == Behavior::NonCanonicalFormat {
                    CANONICAL_SAMPLE_RATE * 2
                } else {
                    CANONICAL_SAMPLE_RATE
                },
                channels: CANONICAL_CHANNELS,
                sample_format: "f32le".to_owned(),
                deterministic_seed: true,
                device: "cpu".to_owned(),
            },
        }),
        WorkerRequestFrame::Health {
            protocol_version,
            request_id,
        } => Ok(WorkerResponseFrame::Health {
            protocol_version,
            request_id,
            ready: true,
            model_loaded: false,
        }),
        WorkerRequestFrame::Synthesize {
            protocol_version,
            request_id,
            parameters,
        } => {
            match behavior {
                Behavior::TruncatedAudio => std::fs::write(&parameters.output, b"RIFF")?,
                // A worker that writes somewhere other than the path it was
                // assigned, and reports success anyway. ADR-0001 §10.3 confines
                // worker writes to the assigned staging root, and a control is
                // only as good as the case that tries to walk past it: this is
                // that case, and the E1-S3 test
                // `t4_e1_worker_output_outside_the_assigned_path_is_refused`
                // is what reads it.
                Behavior::EscapeStaging => {
                    let assigned = Path::new(&parameters.output);
                    let escaped = assigned
                        .parent()
                        .and_then(Path::parent)
                        .unwrap_or(assigned)
                        .join("escaped-take.wav");
                    write_tone(&escaped, parameters.seed)?;
                }
                Behavior::LitterStagingDirectory => {
                    let assigned = Path::new(&parameters.output);
                    write_tone(assigned, parameters.seed)?;
                    std::fs::create_dir_all(
                        assigned
                            .parent()
                            .unwrap_or(assigned)
                            .join("scratch-directory"),
                    )?;
                }
                Behavior::LitterStaging => {
                    let assigned = Path::new(&parameters.output);
                    write_tone(assigned, parameters.seed)?;
                    let beside = assigned
                        .parent()
                        .unwrap_or(assigned)
                        .join("scratch-take.wav");
                    write_tone(&beside, parameters.seed)?;
                }
                _ => write_tone(Path::new(&parameters.output), parameters.seed)?,
            }
            // On stderr, because stdout is the protocol channel. The identity a
            // worker actually received is not observable from outside
            // otherwise, and ADR-0001 §10.3 requires it to be unique per worker
            // lifetime — a rule the supervisor can only be shown to keep by
            // reading back what arrived. `FAKE_SYNTHESIZING_PREFIX` in
            // `crates/study-tts-testkit/tests/worker_contract.rs` is the other
            // end of this line.
            eprintln!("fake worker synthesizing request {request_id}");
            // The honest frame first, then exactly one field spoiled. Built
            // this way round so a drift case differs from the accepted case in
            // the one value it is about, and a test that passes cannot be
            // passing for a second reason nobody named.
            let drift = match behavior {
                Behavior::Drift(identity) => Some(identity),
                _ => None,
            };
            Ok(WorkerResponseFrame::SynthesisSucceeded {
                protocol_version,
                request_id,
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                frames: FAKE_FRAMES,
                model_revision: drifted_or("v1", drift, DriftedIdentity::Model),
                codec_revision: drifted_or("none", drift, DriftedIdentity::Codec),
                worker_bundle_hash: if drift == Some(DriftedIdentity::WorkerBundle) {
                    // A well-formed digest that is not this fake's, so the
                    // refusal is the comparison rather than the parse.
                    "0".repeat(DETERMINISTIC_TONE_BUNDLE_HASH.len()).parse()?
                } else {
                    DETERMINISTIC_TONE_BUNDLE_HASH.parse()?
                },
                // Resolved from the profile the request named, so a plan that
                // expected another voice derives another key and is refused by
                // the cache rather than published.
                voice_conditioning_hash: deterministic_tone_conditioning(&parameters.voice),
                voice_profile: drifted_or(&parameters.voice, drift, DriftedIdentity::VoiceProfile),
            })
        }
        WorkerRequestFrame::Cancel {
            protocol_version,
            request_id,
            active_request_id,
        } => Ok(WorkerResponseFrame::Cancelled {
            protocol_version,
            request_id,
            active_request_id,
        }),
        WorkerRequestFrame::Shutdown {
            protocol_version,
            request_id,
        } => {
            // On stderr because stdout is the protocol channel. This is how a
            // test tells a worker that was *asked* to leave from one that was
            // killed where it stood: the two look identical from outside, and
            // only the graceful one gets to finish what it was doing.
            eprintln!("fake worker leaving on a shutdown frame");
            Ok(WorkerResponseFrame::Shutdown {
                protocol_version,
                request_id,
            })
        }
    }
}

fn failure_response(frame: &WorkerRequestFrame) -> WorkerResponseFrame {
    let (protocol_version, request_id) = frame_identity(frame);
    WorkerResponseFrame::Failure {
        protocol_version: protocol_version.to_owned(),
        request_id: request_id.to_owned(),
        code: WorkerFailureCode::SynthesisFailed,
        message: "injected fake-worker failure".to_owned(),
        recoverable: false,
    }
}

fn frame_identity(frame: &WorkerRequestFrame) -> (&str, &str) {
    match frame {
        WorkerRequestFrame::Initialize {
            protocol_version,
            request_id,
            ..
        }
        | WorkerRequestFrame::Capabilities {
            protocol_version,
            request_id,
        }
        | WorkerRequestFrame::Health {
            protocol_version,
            request_id,
        }
        | WorkerRequestFrame::Synthesize {
            protocol_version,
            request_id,
            ..
        }
        | WorkerRequestFrame::Cancel {
            protocol_version,
            request_id,
            ..
        }
        | WorkerRequestFrame::Shutdown {
            protocol_version,
            request_id,
        } => (protocol_version, request_id),
    }
}

fn write_tone(path: &Path, seed: u64) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: CANONICAL_CHANNELS,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    let frequency = 300.0 + (seed % 400) as f32;
    for frame in 0..FAKE_FRAMES {
        let phase = std::f32::consts::TAU * frequency * frame as f32 / CANONICAL_SAMPLE_RATE as f32;
        writer.write_sample(phase.sin() * 0.2)?;
    }
    writer.finalize()
}
