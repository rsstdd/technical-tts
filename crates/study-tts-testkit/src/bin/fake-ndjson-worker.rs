//! Executable protocol fake for worker supervision and fault-injection tests.
//!
//! It speaks the provisional frames and emits synthetic WAVs but never loads a
//! model or voice; command-line behaviors expose failures for supervisor tests.

use std::{
    collections::BTreeMap,
    error::Error,
    io::{BufRead, Write},
    path::Path,
    time::Duration,
};

use study_tts_core::{CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE};
use study_tts_runtime::{
    WorkerCapabilities, WorkerFailureCode, WorkerRequestFrame, WorkerResponseFrame,
    parse_worker_request,
};

const FAKE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Behavior {
    Deterministic,
    Delay,
    Failure,
    Hang,
    MalformedFrame,
    TruncatedAudio,
    Stderr,
    Exit,
}

fn main() -> Result<(), Box<dyn Error>> {
    let behavior = parse_behavior(std::env::args().nth(1).as_deref())?;
    if behavior == Behavior::Exit {
        std::process::exit(17);
    }
    if behavior == Behavior::Stderr {
        eprintln!("fake worker diagnostic on stderr");
    }

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let frame = parse_worker_request(line.as_bytes())?;
        if behavior == Behavior::Hang {
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

        let response = respond(frame, behavior)?;
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}

fn parse_behavior(value: Option<&str>) -> Result<Behavior, Box<dyn Error>> {
    match value.unwrap_or("deterministic") {
        "deterministic" => Ok(Behavior::Deterministic),
        "delay" => Ok(Behavior::Delay),
        "failure" => Ok(Behavior::Failure),
        "hang" => Ok(Behavior::Hang),
        "malformed-frame" => Ok(Behavior::MalformedFrame),
        "truncated-audio" => Ok(Behavior::TruncatedAudio),
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
        } => Ok(WorkerResponseFrame::Initialized {
            protocol_version,
            request_id,
            identities: BTreeMap::from([
                ("backend_revision".to_owned(), "fake-backend-v1".to_owned()),
                (
                    "worker_bundle_hash".to_owned(),
                    parameters.worker_bundle_hash.to_string(),
                ),
            ]),
        }),
        WorkerRequestFrame::Capabilities {
            protocol_version,
            request_id,
        } => Ok(WorkerResponseFrame::Capabilities {
            protocol_version,
            request_id,
            capabilities: WorkerCapabilities {
                languages: vec!["en".to_owned()],
                max_text_bytes: 64 * 1024,
                voices: vec!["synthetic-test-voice-v1".to_owned()],
                styles: vec!["calm".to_owned()],
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                sample_format: "f32le".to_owned(),
                deterministic_seed: true,
                device: "cpu".to_owned(),
            },
        }),
        WorkerRequestFrame::Synthesize {
            protocol_version,
            request_id,
            parameters,
        } => {
            if behavior == Behavior::TruncatedAudio {
                std::fs::write(&parameters.output, b"RIFF")?;
            } else {
                write_tone(Path::new(&parameters.output), parameters.seed)?;
            }
            Ok(WorkerResponseFrame::SynthesisSucceeded {
                protocol_version,
                request_id,
                sample_rate: CANONICAL_SAMPLE_RATE,
                channels: CANONICAL_CHANNELS,
                frames: FAKE_FRAMES,
                model_revision: "fake-model-v1".to_owned(),
                codec_revision: "fake-codec-v1".to_owned(),
                worker_bundle_hash: blake3::hash(b"fake-worker-bundle").into(),
                voice_profile_hash: blake3::hash(parameters.voice.as_bytes()).into(),
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
        } => Ok(WorkerResponseFrame::Shutdown {
            protocol_version,
            request_id,
        }),
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
