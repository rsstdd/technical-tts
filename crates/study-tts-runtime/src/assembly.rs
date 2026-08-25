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
    AudioError, BuildError, CacheEntryFault, ManagedPathError, audio_error,
    cache::{CachedSegment, hash_file, rejected},
    io_error,
};

/// Milliseconds in one second, for turning a declared pause into frames.
const MILLISECONDS_PER_SECOND: u64 = 1_000;

/// Frames of silence written after a segment. Shared by the expected-total
/// calculation and the write loop so the two cannot drift apart.
///
/// No pause reaches the overflow: the widest value a `u32` can hold, times a
/// 24 kHz sample rate, is near `1.0e14` against a `u64` ceiling near `1.8e19`.
/// The check is kept because it binds that headroom to the constant rather
/// than to a reader's memory of it. Overflow would need `CANONICAL_SAMPLE_RATE`
/// above roughly `4.3e9` Hz or a wider pause field, and a change that large
/// should be refused here rather than wrap into a master whose length every
/// downstream duration inherits.
/// `t1_e0_the_widest_pause_a_segment_can_declare_does_not_overflow` pins the
/// frame count that headroom rests on.
fn pause_frames(segment: &CachedSegment) -> Result<u64, BuildError> {
    Ok(u64::from(segment.pause_after_ms)
        .checked_mul(u64::from(CANONICAL_SAMPLE_RATE))
        .ok_or_else(|| AudioError::PauseFrameOverflow {
            segment_id: segment.segment_id.clone(),
            pause_after_ms: segment.pause_after_ms,
        })?
        / MILLISECONDS_PER_SECOND)
}

/// Total frames the master must contain, derived from validated cache metadata
/// rather than from what the write loop happens to produce.
///
/// No plan reaches the overflow. With every field at its `u32` maximum a
/// segment contributes 107,374,182,375 frames, so a `u64` holds 171,798,691 of
/// them; under the ten-second pause cap `study_tts_core` enforces the figure
/// rises to 4,294,727,310. Neither is a segment count a machine could hold in
/// memory.
///
/// The check is kept because those are properties of the current field widths
/// rather than of the lesson format: the total is a `u64` fed by counts that
/// could be widened, and this crate's arithmetic should not rest on a bound
/// another crate enforces.
/// `t1_e0_the_widest_plan_the_types_allow_leaves_frame_headroom` pins both
/// figures, so widening a field fails a test rather than eroding the margin
/// quietly.
fn expected_frames(segments: &[CachedSegment]) -> Result<u64, BuildError> {
    let mut expected = 0_u64;
    for segment in segments {
        let pause = pause_frames(segment)?;
        expected = expected
            .checked_add(u64::from(segment.frames))
            .and_then(|running| running.checked_add(pause))
            .ok_or(AudioError::PlannedLengthOverflow)?;
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
/// [`ManagedPathError::UnrootedDestination`] when `destination` has no parent;
/// [`crate::CacheError::UnusableCacheEntry`] when an entry's digest or frame
/// count disagrees with its record; [`AudioError::PauseFrameOverflow`],
/// [`AudioError::PlannedLengthOverflow`], or
/// [`AudioError::AssembledLengthOverflow`] when arithmetic would wrap;
/// [`AudioError::AssembledLengthMismatch`] when the total disagrees with the
/// plan; otherwise [`crate::IoError::AudioAt`] or
/// [`crate::IoError::FileSystem`].
pub(crate) fn assemble(segments: &[CachedSegment], destination: &Path) -> Result<u64, BuildError> {
    let parent = destination
        .parent()
        .ok_or_else(|| ManagedPathError::UnrootedDestination {
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
            // One WAV cannot wrap a `u64` frame counter: a data chunk
            // declares its length in 32 bits, so `hound` yields at most about
            // `1.1e9` f32 samples from the file, and the equality check below
            // refuses anything past the `u32` the entry recorded long before
            // the counter is in danger. Checked so the write loop does not
            // depend on the container's field width for its arithmetic.
            segment_frames = segment_frames.checked_add(1).ok_or_else(|| {
                AudioError::AssembledLengthOverflow {
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

        // Unreachable while `expected_frames` succeeded above: each segment
        // contributes exactly the count that pre-pass already summed without
        // overflow. Checked because the two totals are built by separate
        // loops, and holding both to the same limit is what makes the
        // comparison below meaningful.
        total_frames = total_frames
            .checked_add(segment_frames)
            .and_then(|running| running.checked_add(pause))
            .ok_or_else(|| AudioError::AssembledLengthOverflow {
                destination: destination.to_path_buf(),
            })?;
    }

    // The aggregate check is redundant while every per-segment check passes,
    // and it is retained because it is the invariant the manifest and every
    // downstream duration derive from.
    if total_frames != expected {
        return Err(AudioError::AssembledLengthMismatch {
            destination: destination.to_path_buf(),
            assembled: total_frames,
            expected,
        }
        .into());
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

    /// The pause arithmetic has room at the widest value the field can hold,
    /// so `PauseFrameOverflow` guards the sample rate rather than the input.
    /// Pins the figure `pause_frames` argues from: the multiplication peaks
    /// near `1.0e14`, five orders below a `u64`, and the widest declarable
    /// pause is 103,079,215,080 frames.
    ///
    /// Not a `DELIVERY-PLAN.md` §E0 name: it pins the headroom behind a check
    /// no input reaches, rather than a planned behavior.
    #[test]
    fn t1_e0_the_widest_pause_a_segment_can_declare_does_not_overflow() {
        let workspace = TempDir::new().expect("create assembly workspace");
        // The frame arithmetic reads the declared counts and never the audio,
        // so these segments deliberately name a file that was never written.
        let widest = segment(workspace.path().join("absent.wav"), 0, u32::MAX);

        let frames = pause_frames(&widest).expect("the widest declarable pause must not overflow");

        assert_eq!(frames, 103_079_215_080);
    }

    /// A plan whose every field sits at its maximum still sums without
    /// overflowing, so `PlannedLengthOverflow` guards the field widths rather
    /// than refusing any lesson a person could write. Pins the figure
    /// `expected_frames` argues from: one such segment contributes
    /// 107,374,182,375 frames, of which a `u64` holds 171,798,691.
    ///
    /// Not a `DELIVERY-PLAN.md` §E0 name: it pins the headroom behind a check
    /// no input reaches, rather than a planned behavior.
    #[test]
    fn t1_e0_the_widest_plan_the_types_allow_leaves_frame_headroom() {
        let workspace = TempDir::new().expect("create assembly workspace");
        let absent = workspace.path().join("absent.wav");
        let segments = vec![
            segment(absent.clone(), u32::MAX, u32::MAX),
            segment(absent, u32::MAX, u32::MAX),
        ];

        let total =
            expected_frames(&segments).expect("the widest declarable plan must not overflow");

        assert_eq!(total, 214_748_364_750);
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
        let BuildError::Cache(crate::CacheError::UnusableCacheEntry { fault, .. }) = &error else {
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
        assert!(
            message.contains("runtime reconciliation"),
            "message was `{message}`"
        );
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

        let BuildError::Cache(crate::CacheError::UnusableCacheEntry { fault, .. }) = &error else {
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
            matches!(error, BuildError::Io(crate::IoError::FileSystem { .. })),
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
