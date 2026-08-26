//! Tier 4 compatibility coverage for the E0-S3 float-WAV path.

use std::{fs::File, io::Write, path::Path, process::Command};

use tempfile::TempDir;

const SAMPLE_RATE_HZ: u32 = 24_000;
const WORKER_SAMPLES: [f32; 4] = [0.0, 0.25, -0.25, 0.0];

fn write_u16(output: &mut File, value: u16) {
    output
        .write_all(&value.to_le_bytes())
        .expect("write u16 field");
}

fn write_u32(output: &mut File, value: u32) {
    output
        .write_all(&value.to_le_bytes())
        .expect("write u32 field");
}

fn write_libsndfile_float_variant(path: &Path) {
    let data_bytes = u32::try_from(WORKER_SAMPLES.len() * size_of::<f32>())
        .expect("test sample data fits a RIFF chunk");
    let riff_bytes = 72_u32 + data_bytes;
    let mut output = File::create(path).expect("create worker WAV variant");

    output.write_all(b"RIFF").expect("write RIFF marker");
    write_u32(&mut output, riff_bytes);
    output.write_all(b"WAVE").expect("write WAVE marker");

    output.write_all(b"fmt ").expect("write fmt marker");
    write_u32(&mut output, 16);
    write_u16(&mut output, 3);
    write_u16(&mut output, 1);
    write_u32(&mut output, SAMPLE_RATE_HZ);
    write_u32(&mut output, SAMPLE_RATE_HZ * 4);
    write_u16(&mut output, 4);
    write_u16(&mut output, 32);

    output.write_all(b"fact").expect("write fact marker");
    write_u32(&mut output, 4);
    write_u32(
        &mut output,
        u32::try_from(WORKER_SAMPLES.len()).expect("test sample count fits u32"),
    );

    output.write_all(b"PEAK").expect("write PEAK marker");
    write_u32(&mut output, 16);
    write_u32(&mut output, 1);
    write_u32(&mut output, 0);
    write_u32(&mut output, 0.25_f32.to_bits());
    write_u32(&mut output, 1);

    output.write_all(b"data").expect("write data marker");
    write_u32(&mut output, data_bytes);
    for sample in WORKER_SAMPLES {
        write_u32(&mut output, sample.to_bits());
    }
    output.flush().expect("flush worker WAV variant");
}

fn decode_float_wav(path: &Path, expected_frames: usize) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("hound must open the float WAV variant");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "variant `{}`", path.display());
    assert_eq!(
        spec.sample_rate,
        SAMPLE_RATE_HZ,
        "variant `{}`",
        path.display()
    );
    assert_eq!(spec.bits_per_sample, 32, "variant `{}`", path.display());
    assert_eq!(
        spec.sample_format,
        hound::SampleFormat::Float,
        "variant `{}`",
        path.display()
    );
    let samples = reader
        .samples::<f32>()
        .map(|sample| sample.expect("hound must decode every float sample"))
        .collect::<Vec<_>>();
    assert_eq!(
        samples.len(),
        expected_frames,
        "variant `{}`",
        path.display()
    );
    assert!(
        samples
            .iter()
            .all(|sample| sample.is_finite() && sample.abs() <= 1.0),
        "variant `{}` contained an invalid float sample",
        path.display()
    );
    samples
}

fn write_assembled_master(path: &Path, first: &[f32], second: &[f32]) -> usize {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE_HZ,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create assembled master");
    for sample in first {
        writer.write_sample(*sample).expect("write first segment");
    }
    writer
        .write_sample(0.0_f32)
        .expect("write first silence frame");
    writer
        .write_sample(0.0_f32)
        .expect("write second silence frame");
    for sample in second {
        writer.write_sample(*sample).expect("write second segment");
    }
    writer.finalize().expect("finalize assembled master");
    first.len() + 2 + second.len()
}

#[test]
fn t4_e0_pipeline_wav_variants_round_trip() {
    let workspace = TempDir::new().expect("create WAV-variant workspace");
    let worker = workspace.path().join("worker.wav");
    write_libsndfile_float_variant(&worker);
    let worker_samples = decode_float_wav(&worker, WORKER_SAMPLES.len());
    assert_eq!(worker_samples, WORKER_SAMPLES);

    let cached = workspace.path().join("cache-preserved.wav");
    std::fs::copy(&worker, &cached).expect("copy worker WAV into the cache variant");
    assert_eq!(
        std::fs::read(&worker).expect("read worker WAV bytes"),
        std::fs::read(&cached).expect("read cached WAV bytes")
    );
    let cached_samples = decode_float_wav(&cached, WORKER_SAMPLES.len());
    assert_eq!(cached_samples, WORKER_SAMPLES);

    let master = workspace.path().join("assembled-master.wav");
    let master_frames = write_assembled_master(&master, &worker_samples, &cached_samples);
    let master_samples = decode_float_wav(&master, master_frames);
    assert_eq!(master_samples.len(), WORKER_SAMPLES.len() * 2 + 2);

    let converted = workspace.path().join("ffmpeg-pcm-f32le.wav");
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&worker)
        .args([
            "-map_metadata",
            "-1",
            "-ac",
            "1",
            "-ar",
            "24000",
            "-c:a",
            "pcm_f32le",
        ])
        .arg(&converted)
        .output()
        .expect("execute FFmpeg float-WAV conversion");
    assert!(
        output.status.success(),
        "FFmpeg conversion failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let converted_samples = decode_float_wav(&converted, WORKER_SAMPLES.len());
    assert_eq!(converted_samples, WORKER_SAMPLES);
}
