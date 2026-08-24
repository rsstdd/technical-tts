//! Concatenation of validated cache entries into one canonical master WAV.
//!
//! The expected frame total is derived from cache metadata before the write
//! loop runs, so a master that came out the wrong length is caught against
//! what the plan said rather than against whatever was produced. Frame
//! arithmetic is checked throughout: a pause long enough to overflow the
//! counter is a refusal, never a wrap.

use std::path::Path;

use study_tts_core::{CANONICAL_BITS_PER_SAMPLE, CANONICAL_CHANNELS, CANONICAL_SAMPLE_RATE};
use tempfile::Builder;

use crate::{
    BuildError, CacheEntryFault, audio_error,
    cache::{CachedSegment, hash_file, rejected},
    io_error,
};

/// Milliseconds in one second, for turning a declared pause into frames.
const MILLISECONDS_PER_SECOND: u64 = 1_000;

/// Frames of silence written after a segment. Shared by the expected-total
/// calculation and the write loop so the two cannot drift apart.
fn pause_frames(segment: &CachedSegment) -> Result<u64, BuildError> {
    Ok(u64::from(segment.pause_after_ms)
        .checked_mul(u64::from(CANONICAL_SAMPLE_RATE))
        .ok_or_else(|| BuildError::PauseFrameOverflow {
            segment_id: segment.segment_id.clone(),
            pause_after_ms: segment.pause_after_ms,
        })?
        / MILLISECONDS_PER_SECOND)
}

/// Total frames the master must contain, derived from validated cache metadata
/// rather than from what the write loop happens to produce.
fn expected_frames(segments: &[CachedSegment]) -> Result<u64, BuildError> {
    let mut expected = 0_u64;
    for segment in segments {
        let pause = pause_frames(segment)?;
        expected = expected
            .checked_add(u64::from(segment.frames))
            .and_then(|running| running.checked_add(pause))
            .ok_or(BuildError::PlannedLengthOverflow)?;
    }
    Ok(expected)
}

/// Confirms a segment's audio still hashes to what its cache entry recorded.
///
/// ADR-0001 §13.2 requires it: "It verifies each checksum before reading."
/// `cache` hashes an entry when it validates or publishes it, but assembly
/// consumes the file afterwards, and every other check here would pass on a
/// file whose bytes changed while its frame count did not — leaving altered
/// audio in the master and the digest of the audio that was validated in the
/// manifest.
///
/// Narrows the window rather than closing it: the file is hashed and then
/// reopened to decode. Closing it needs a single handle read twice, which
/// belongs with the directory-relative containment work deferred to E5-S4.
fn verify_recorded_audio(segment: &CachedSegment) -> Result<(), BuildError> {
    let computed = hash_file(&segment.audio_path)?;
    if computed != segment.audio_blake3 {
        return Err(rejected(
            &segment.entry_dir,
            &segment.segment_id,
            CacheEntryFault::ChecksumMismatch {
                found: computed,
                declared: segment.audio_blake3.clone(),
            },
        ));
    }
    Ok(())
}

/// Concatenates validated cache entries into the master WAV, returning the
/// frames written.
///
/// Each entry's audio is re-hashed against its recorded digest before a sample
/// of it is read, so a tampered entry is refused rather than assembled. The
/// master is staged beside its destination and renamed, so a failure part way
/// through leaves no partial master for a later step to treat as finished.
///
/// # Errors
///
/// [`BuildError::UnrootedDestination`] when `destination` has no parent;
/// [`BuildError::UnusableCacheEntry`] naming the entry whose digest or frame
/// count disagrees with its record; [`BuildError::PauseFrameOverflow`] or
/// [`BuildError::AssembledLengthOverflow`] when the frame arithmetic would
/// wrap; [`BuildError::AssembledLengthMismatch`] when the total disagrees with
/// what the plan required; otherwise [`BuildError::AudioAt`] or
/// [`BuildError::FileSystem`].
pub(crate) fn assemble(segments: &[CachedSegment], destination: &Path) -> Result<u64, BuildError> {
    let parent = destination
        .parent()
        .ok_or_else(|| BuildError::UnrootedDestination {
            path: destination.to_path_buf(),
        })?;
    let expected = expected_frames(segments)?;

    let mut staged = Builder::new()
        .prefix("master-")
        .suffix(".wav")
        .tempfile_in(parent)
        .map_err(|error| io_error(parent, error))?;
    let spec = hound::WavSpec {
        channels: CANONICAL_CHANNELS,
        sample_rate: CANONICAL_SAMPLE_RATE,
        bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::new(staged.as_file_mut(), spec)
        .map_err(|error| audio_error(destination, error))?;
    let mut total_frames = 0_u64;

    for segment in segments {
        verify_recorded_audio(segment)?;
        let mut reader = hound::WavReader::open(&segment.audio_path)
            .map_err(|error| audio_error(&segment.audio_path, error))?;
        let mut segment_frames = 0_u64;

        for sample in reader.samples::<f32>() {
            // The read and the write fail for different reasons and name
            // different files, so they are mapped separately rather than
            // through one nested `?`.
            let sample = sample.map_err(|error| audio_error(&segment.audio_path, error))?;
            writer
                .write_sample(sample)
                .map_err(|error| audio_error(destination, error))?;
            segment_frames = segment_frames.checked_add(1).ok_or_else(|| {
                BuildError::AssembledLengthOverflow {
                    destination: destination.to_path_buf(),
                }
            })?;
        }

        // Fail on the offending segment rather than on the aggregate, because a
        // per-segment mismatch names the cache entry that needs regenerating.
        if segment_frames != u64::from(segment.frames) {
            return Err(rejected(
                &segment.entry_dir,
                &segment.segment_id,
                CacheEntryFault::FrameCountMismatch {
                    found: segment_frames,
                    declared: segment.frames,
                },
            ));
        }

        let pause = pause_frames(segment)?;
        for _ in 0..pause {
            writer
                .write_sample(0.0_f32)
                .map_err(|error| audio_error(destination, error))?;
        }

        total_frames = total_frames
            .checked_add(segment_frames)
            .and_then(|running| running.checked_add(pause))
            .ok_or_else(|| BuildError::AssembledLengthOverflow {
                destination: destination.to_path_buf(),
            })?;
    }

    // The aggregate check is redundant while every per-segment check passes,
    // and it is retained because it is the invariant the manifest and every
    // downstream duration derive from.
    if total_frames != expected {
        return Err(BuildError::AssembledLengthMismatch {
            destination: destination.to_path_buf(),
            assembled: total_frames,
            expected,
        });
    }

    writer
        .finalize()
        .map_err(|error| audio_error(destination, error))?;
    staged
        .persist(destination)
        .map_err(|error| io_error(destination, error.error))?;
    Ok(total_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use study_tts_core::CacheKey;
    use tempfile::TempDir;

    fn write_tone(path: &Path, frames: u32) {
        let spec = hound::WavSpec {
            channels: CANONICAL_CHANNELS,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create test WAV");
        for _ in 0..frames {
            writer.write_sample(0.25_f32).expect("write test sample");
        }
        writer.finalize().expect("finalize test WAV");
    }

    fn segment(audio_path: PathBuf, declared_frames: u32, pause_after_ms: u32) -> CachedSegment {
        CachedSegment {
            segment_id: "seg-0001".to_owned(),
            entry_dir: audio_path
                .parent()
                .expect("test audio lives in a directory")
                .to_path_buf(),
            cache_key: format!("{:0<width$}", "cafebabe", width = CacheKey::LENGTH)
                .parse()
                .expect("test label pads to a well-formed key"),
            audio_blake3: hash_file(&audio_path)
                // The missing-audio test names a file that was never written,
                // so there is nothing to hash; that case fails on the read.
                .unwrap_or_else(|_| String::new()),
            audio_path,
            frames: declared_frames,
            pause_after_ms,
        }
    }

    #[test]
    fn t1_e0_assembled_frame_count_matches_the_plan() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let first_audio = workspace.path().join("first.wav");
        let second_audio = workspace.path().join("second.wav");
        write_tone(&first_audio, 2_400);
        write_tone(&second_audio, 2_400);
        let segments = vec![
            segment(first_audio, 2_400, 75),
            segment(second_audio, 2_400, 125),
        ];
        let master = workspace.path().join("lesson.wav");

        let total = assemble(&segments, &master).expect("assembly should succeed");

        assert_eq!(total, 9_600);
        let reader = hound::WavReader::open(&master).expect("open assembled master");
        assert_eq!(reader.duration(), 9_600);
    }

    #[test]
    fn t1_e0_declared_frame_count_mismatch_is_rejected_before_persisting() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let audio = workspace.path().join("short.wav");
        write_tone(&audio, 1_200);
        // The artifact claims 2,400 frames while the WAV holds 1,200, which is
        // the shape a truncated synthesis or a partially written cache entry
        // would take.
        let segments = vec![segment(audio, 2_400, 75)];
        let master = workspace.path().join("lesson.wav");

        let error = assemble(&segments, &master).expect_err("truncated segment must be rejected");

        // A truncated entry found while assembling is the same violated
        // invariant `cache` reports when loading one, so it must arrive as the
        // same fault with the same remedy.
        let BuildError::UnusableCacheEntry { fault, .. } = &error else {
            panic!("error was `{error}`");
        };
        assert!(
            matches!(
                **fault,
                CacheEntryFault::FrameCountMismatch {
                    found: 1_200,
                    declared: 2_400,
                }
            ),
            "fault was `{fault}`"
        );
        let message = error.to_string();
        assert!(message.contains("seg-0001"), "message was `{message}`");
        assert!(message.contains("delete"), "message was `{message}`");
        assert!(
            !master.exists(),
            "a rejected assembly must not persist a master WAV"
        );
    }

    #[test]
    fn t1_e0_altered_segment_audio_is_refused_before_it_reaches_the_master() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let audio = workspace.path().join("tampered.wav");
        write_tone(&audio, 2_400);
        let segment = segment(audio.clone(), 2_400, 75);

        // Same frame count, different samples: the shape that survives every
        // other check the assembler makes. Without a checksum comparison the
        // altered bytes reach the master while the manifest keeps recording the
        // digest of the audio that was validated.
        let spec = hound::WavSpec {
            channels: CANONICAL_CHANNELS,
            sample_rate: CANONICAL_SAMPLE_RATE,
            bits_per_sample: CANONICAL_BITS_PER_SAMPLE,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&audio, spec).expect("rewrite test WAV");
        for _ in 0..2_400 {
            writer.write_sample(-0.5_f32).expect("write altered sample");
        }
        writer.finalize().expect("finalize altered WAV");

        let master = workspace.path().join("lesson.wav");
        let error = assemble(&[segment], &master).expect_err("altered audio must be refused");

        let BuildError::UnusableCacheEntry { fault, .. } = &error else {
            panic!("altered audio produced `{error}`");
        };
        assert!(
            matches!(**fault, CacheEntryFault::ChecksumMismatch { .. }),
            "altered audio produced fault `{fault}`"
        );
        assert!(
            !master.exists(),
            "a refused assembly must not persist a master WAV"
        );
    }

    #[test]
    fn t1_e0_missing_segment_audio_names_the_file() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let missing = workspace.path().join("does-not-exist.wav");
        let segments = vec![segment(missing.clone(), 2_400, 75)];
        let master = workspace.path().join("lesson.wav");

        let error = assemble(&segments, &master).expect_err("missing segment audio must fail");

        // Reported as a filesystem failure rather than an audio one since the
        // checksum verification reaches the file first. That is the accurate
        // description: there is no audio operation to fail on a file that is
        // not there. What this test guards is the name — the refusal must say
        // which file — and that is asserted below rather than implied by the
        // variant.
        assert!(
            matches!(error, BuildError::FileSystem { .. }),
            "missing segment audio produced `{error}`"
        );
        assert!(
            error.to_string().contains(&missing.display().to_string()),
            "error was `{error}`"
        );
    }

    #[test]
    fn t1_e0_pause_frames_are_exact_for_the_canonical_rate() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let audio = workspace.path().join("tone.wav");
        write_tone(&audio, 100);

        assert_eq!(
            pause_frames(&segment(audio.clone(), 100, 0)).expect("zero"),
            0
        );
        assert_eq!(
            pause_frames(&segment(audio.clone(), 100, 1)).expect("1 ms"),
            24
        );
        assert_eq!(
            pause_frames(&segment(audio, 100, 10_000)).expect("10 s"),
            240_000
        );
    }
}
