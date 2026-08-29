//! Regenerates the committed deterministic-audio fixtures at `fixtures/audio/`.
//!
//! ```text
//! cargo run --package study-tts-testkit --example generate-audio-fixtures
//! ```
//!
//! The fixtures are committed rather than generated inside each test on
//! purpose. A test that writes a WAV with `hound` and then validates it with
//! `hound` proves the two agree with each other, not that either agrees with
//! the canonical format ADR-0001 §13.1 fixes. Pinned bytes are what let a
//! change in either direction show up as a failing test.
//!
//! Every sample is synthetic: a generated tone, no human voice, in keeping with
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`.

use std::{f32::consts::TAU, path::PathBuf};

use study_tts_core::{CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE};

/// One tenth of a second, which is long enough to hold a frame count and short
/// enough that the committed file stays small.
const FIXTURE_FRAMES: u32 = CANONICAL_SAMPLE_RATE / 10;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/audio");
    std::fs::create_dir_all(&directory)?;

    write(
        &directory.join("e1-s1-canonical-tone.wav"),
        CANONICAL_SAMPLE_RATE,
        hound::SampleFormat::Float,
        CANONICAL_BITS_PER_SAMPLE,
    )?;
    // Differs from the canonical fixture in exactly one property, so a test
    // that refuses it has refused the rate rather than the file.
    write(
        &directory.join("e1-s1-noncanonical-48k.wav"),
        48_000,
        hound::SampleFormat::Float,
        CANONICAL_BITS_PER_SAMPLE,
    )?;
    // Same width as the canonical format but integer samples: a validator that
    // checked only `bits_per_sample` would accept this one.
    write(
        &directory.join("e1-s1-noncanonical-integer.wav"),
        CANONICAL_SAMPLE_RATE,
        hound::SampleFormat::Int,
        CANONICAL_BITS_PER_SAMPLE,
    )?;
    Ok(())
}

fn write(
    path: &std::path::Path,
    sample_rate: u32,
    sample_format: hound::SampleFormat,
    bits_per_sample: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let spec = hound::WavSpec {
        channels: CANONICAL_CHANNELS,
        sample_rate,
        bits_per_sample,
        sample_format,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for frame in 0..FIXTURE_FRAMES {
        let phase = TAU * 440.0 * frame as f32 / sample_rate as f32;
        match sample_format {
            hound::SampleFormat::Float => writer.write_sample(phase.sin() * 0.2)?,
            hound::SampleFormat::Int => {
                writer.write_sample((phase.sin() * 0.2 * f32::from(i16::MAX)) as i32)?;
            }
        }
    }
    writer.finalize()?;
    println!("wrote {}", path.display());
    Ok(())
}
