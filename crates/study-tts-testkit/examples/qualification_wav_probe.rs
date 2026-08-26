//! Qualification-only hound probe for private E0-S3 float-WAV artifacts.

use std::{
    env,
    error::Error,
    fs::File,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use serde_json::json;
use sha2::{Digest, Sha256};

const SAMPLE_RATE_HZ: u32 = 24_000;

struct Arguments {
    worker: PathBuf,
    cached: PathBuf,
    ffmpeg: PathBuf,
    master: PathBuf,
    report: PathBuf,
}

struct InspectedWav {
    name: String,
    frames: usize,
    maximum_absolute_sample: f32,
    sha256: String,
    samples: Vec<f32>,
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn parse_arguments() -> Result<Arguments, Box<dyn Error>> {
    let values = env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    let [worker, cached, ffmpeg, master, report] = values.as_slice() else {
        return Err(invalid_input(
            "usage: qualification_wav_probe WORKER CACHE FFMPEG MASTER REPORT",
        )
        .into());
    };
    Ok(Arguments {
        worker: worker.clone(),
        cached: cached.clone(),
        ffmpeg: ffmpeg.clone(),
        master: master.clone(),
        report: report.clone(),
    })
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn inspect_wav(path: &Path) -> Result<InspectedWav, Box<dyn Error>> {
    if path.is_symlink() || !path.is_file() {
        return Err(invalid_input("qualification WAV must be a regular file").into());
    }
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.sample_rate != SAMPLE_RATE_HZ
        || spec.bits_per_sample != 32
        || spec.sample_format != hound::SampleFormat::Float
    {
        return Err(invalid_input("qualification WAV has an unsupported media format").into());
    }
    let mut samples = Vec::new();
    let mut maximum_absolute_sample = 0.0_f32;
    for sample in reader.samples::<f32>() {
        let sample = sample?;
        if !sample.is_finite() || sample.abs() > 1.0 {
            return Err(invalid_input("qualification WAV contains an invalid float sample").into());
        }
        maximum_absolute_sample = maximum_absolute_sample.max(sample.abs());
        samples.push(sample);
    }
    if samples.is_empty() {
        return Err(invalid_input("qualification WAV contains no frames").into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid_input("qualification WAV name is not UTF-8"))?
        .to_owned();
    Ok(InspectedWav {
        name,
        frames: samples.len(),
        maximum_absolute_sample,
        sha256: sha256_file(path)?,
        samples,
    })
}

fn write_master(path: &Path, worker: &[f32], cached: &[f32]) -> Result<(), Box<dyn Error>> {
    if path.exists() || path.is_symlink() {
        return Err(invalid_input("qualification master output already exists").into());
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE_HZ,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for sample in worker {
        writer.write_sample(*sample)?;
    }
    for _ in 0..SAMPLE_RATE_HZ {
        writer.write_sample(0.0_f32)?;
    }
    for sample in cached {
        writer.write_sample(*sample)?;
    }
    writer.finalize()?;
    Ok(())
}

fn report_row(wav: &InspectedWav) -> serde_json::Value {
    json!({
        "name": wav.name,
        "sha256": wav.sha256,
        "sample_rate_hz": SAMPLE_RATE_HZ,
        "channels": 1,
        "bits_per_sample": 32,
        "sample_format": "float",
        "frames": wav.frames,
        "maximum_absolute_sample": wav.maximum_absolute_sample,
        "all_samples_decoded": true,
        "finite_and_in_range": true,
    })
}

fn write_report(path: &Path, value: &serde_json::Value) -> Result<(), Box<dyn Error>> {
    if path.exists() || path.is_symlink() {
        return Err(invalid_input("qualification WAV report already exists").into());
    }
    let mut output = File::create(path)?;
    serde_json::to_writer_pretty(&mut output, value)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let arguments = parse_arguments()?;
    let worker = inspect_wav(&arguments.worker)?;
    let cached = inspect_wav(&arguments.cached)?;
    let ffmpeg = inspect_wav(&arguments.ffmpeg)?;
    if worker.sha256 != cached.sha256 || worker.samples != cached.samples {
        return Err(invalid_input("cache-preserved WAV changed worker bytes or samples").into());
    }
    if worker.samples != ffmpeg.samples {
        return Err(invalid_input("FFmpeg pcm_f32le conversion changed samples").into());
    }

    write_master(&arguments.master, &worker.samples, &cached.samples)?;
    let master = inspect_wav(&arguments.master)?;
    let expected_master_frames = worker.frames + usize::try_from(SAMPLE_RATE_HZ)? + cached.frames;
    if master.frames != expected_master_frames {
        return Err(invalid_input("Rust-assembled master frame count is incorrect").into());
    }
    let report = json!({
        "schema_version": "1.0-e0-s3-hound-probe",
        "decoder": "hound-3.5.1",
        "variants": [
            report_row(&worker),
            report_row(&cached),
            report_row(&master),
            report_row(&ffmpeg),
        ],
        "cache_copy_byte_identical": true,
        "ffmpeg_conversion_sample_identical": true,
        "master_inserted_silence_frames": SAMPLE_RATE_HZ,
        "result": "pass",
    });
    write_report(&arguments.report, &report)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    run()
}
