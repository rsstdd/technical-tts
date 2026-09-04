//! Edge analysis and conditioning for one canonical segment.
//!
//! ADR-0001 §13.4 has Rust measure 5 ms edge frames, insert any missing zero
//! padding, apply raised-cosine transition ramps no longer than 5 ms, and
//! require exposed endpoints to be exactly zero. §12.6 makes those checks a
//! condition of publishing a cache entry at all.
//!
//! # Why this carries a provisional threshold
//!
//! ADR-0001 delegates the *silence threshold* to ADR-0003, which is
//! **Proposed** and records it as `Pending` in its own calibration table.
//! `CLAUDE.md` says a Proposed ADR authorizes nothing, so the threshold this
//! module applies is not a ratified value and must never be mistaken for one.
//!
//! The conflict was raised and the project owner directed that E1-S3 implement
//! the conditioning now against a provisional threshold rather than wait for
//! calibration. The deviation record
//! `ADR-0001-D007-provisional-edge-conditioning.md` in `docs/adr/deviations/`
//! records that decision, what it overrides, and what it owes. Everything the
//! decision does *not* settle is kept mechanically separate:
//! [`SilenceThreshold::production`] refuses to hand a provisional value to
//! anything that asks for a production reference, which is what keeps a
//! preview-grade constant from silently becoming the calibrated one.
//!
//! The geometry — 5 ms frames, 10 ms of edge silence, a ramp no longer than
//! 5 ms — is fixed by ADR-0001 itself and is not provisional.

use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

/// Width of one edge-analysis frame, in milliseconds.
///
/// ADR-0001 §13.4: edges are analyzed in 5 ms RMS frames. Fixed by ADR-0001
/// rather than delegated, so this is not a provisional value.
pub const EDGE_ANALYSIS_FRAME_MS: u32 = 5;

/// Silence each exposed edge must have, in milliseconds.
///
/// ADR-0001 §13.4: at least 10 ms. Fixed by ADR-0001.
pub const REQUIRED_EDGE_SILENCE_MS: u32 = 10;

/// Longest raised-cosine transition ramp, in milliseconds.
///
/// ADR-0001 §13.4: no longer than 5 ms. Fixed by ADR-0001. The ramp covers the
/// first samples of signal, which is the only side of the transition with
/// anything to attenuate — the silence side is below the audio-profile
/// threshold by definition, so scaling it would change nothing audible. Note
/// that "below the threshold" is not "zero": [`condition_edges`] normalizes
/// the measured silent region to zero before it applies the ramp.
pub const MAX_TRANSITION_RAMP_MS: u32 = 5;

/// Longest segment audio this build will condition or publish.
///
/// A security ceiling on what one segment may hand the conditioner, not a
/// performance budget: the samples are held in memory to be conditioned, and a
/// worker that returned an unbounded file would otherwise be handed the
/// process. Ten minutes of canonical float mono is about 57 MB.
///
/// It bounds conditioning's output as well as its input, because
/// [`condition_edges`] adds up to 10 ms of padding at each exposed edge:
/// `crates/study-tts-runtime/src/cache.rs` applies it on both sides, so an
/// entry is never published that the same build would afterwards refuse to
/// read.
///
/// `docs/architecture/WALKING-SKELETON.md` §Provisional resource ceilings
/// records it and names this constant in return.
pub const MAX_SEGMENT_AUDIO_MS: u32 = 10 * 60 * 1_000;

/// Where a threshold's value came from.
///
/// The distinction exists because this build applies a threshold ADR-0003 has
/// not frozen. A number with no provenance attached is one that gets copied
/// into a production path by the next person who needs a threshold.
///
/// Serialized because a cache entry records the calibration its conditioning
/// was produced under: without it, an entry conditioned against the provisional
/// threshold is indistinguishable from one conditioned against the frozen value
/// ADR-0003 will publish. No `#[serde(other)]`: a calibration this build does
/// not know is a parse error, never a silent default.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CalibrationSource {
    /// Chosen for preview use while ADR-0003 is Proposed and its value Pending.
    Provisional,
    /// Frozen by an accepted ADR-0003 calibration table.
    Frozen,
}

impl CalibrationSource {
    /// This calibration's name, in the spelling a cache entry records.
    ///
    /// Pinned to the serde representation by
    /// `t1_e1_calibration_source_spelling_matches_its_serde_form`, so a
    /// refusal quotes what a reader will find in the file rather than a
    /// second spelling that drifted from it.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Provisional => "provisional",
            Self::Frozen => "frozen",
        }
    }
}

/// The RMS level at or below which an edge frame counts as silence.
///
/// Carries its own provenance so a caller cannot use it without deciding what
/// it is allowed to do with it — see [`SilenceThreshold::production`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SilenceThreshold {
    rms: f32,
    source: CalibrationSource,
}

/// The provisional silence RMS threshold, about -60 dBFS.
///
/// Preview-grade and deliberately conservative: a threshold set too low treats
/// faint room tone as signal and pads less than it should, which is a smaller
/// error than trimming into speech. ADR-0003's calibration table owns the
/// frozen value; this is what stands in until it does.
const PROVISIONAL_SILENCE_RMS: f32 = 0.001;

impl SilenceThreshold {
    /// The provisional threshold this build applies while ADR-0003 is Proposed.
    #[must_use]
    pub const fn provisional() -> Self {
        Self {
            rms: PROVISIONAL_SILENCE_RMS,
            source: CalibrationSource::Provisional,
        }
    }

    /// The level itself, for conditioning that is explicitly preview-grade.
    #[must_use]
    pub const fn rms(&self) -> f32 {
        self.rms
    }

    /// Where the value came from.
    #[must_use]
    pub const fn source(&self) -> CalibrationSource {
        self.source
    }

    /// The level, for a caller that requires a calibrated production reference.
    ///
    /// # Errors
    ///
    /// [`ProvisionalCalibration`] when the threshold is
    /// [`CalibrationSource::Provisional`]. ADR-0001 §13.3 says preview loudness
    /// references remain provisional and cannot become production references
    /// without calibration; this is that rule made mechanical, so a
    /// preview-grade constant cannot reach a production path by being passed
    /// along until nobody remembers where it came from.
    pub const fn production(&self) -> Result<f32, ProvisionalCalibration> {
        match self.source {
            CalibrationSource::Frozen => Ok(self.rms),
            CalibrationSource::Provisional => Err(ProvisionalCalibration),
        }
    }
}

/// One side of a join, as the segment that meets there was rendered.
#[derive(Clone, Copy, Debug)]
pub struct JoinSide<'a> {
    /// The segment's canonical samples.
    pub samples: &'a [f32],
    /// Rate those samples were rendered at.
    pub sample_rate: u32,
    /// Characters of spoken text the segment was rendered from.
    pub characters: usize,
}

/// A measured comparison of the two segments meeting at one join.
///
/// ADR-0001 §11.4: "After a mid-lesson retake, automated loudness and
/// speaking-rate comparisons plus a listening check evaluate both joins." This
/// is the automated half, and it is deliberately only the measurement.
///
/// Nothing here is a verdict. ADR-0003 owns the join-discontinuity threshold
/// and the loudness target, it is **Proposed**, and both rows of its
/// calibration table read `Pending`; a `Proposed` ADR authorizes nothing, so a
/// pass or fail derived here would be an unratified threshold on a production
/// path. [`JoinContinuity::production`] is what keeps that from happening by
/// being passed along.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct JoinContinuity {
    /// The later segment's speech level over the earlier segment's.
    ///
    /// Published as non-negative and finite, which is what [`assess_join`]
    /// guarantees and nothing narrower: a plausible range for this ratio is
    /// precisely what ADR-0003's `Pending` calibration table has not settled,
    /// so a tighter published bound would be that unratified threshold wearing
    /// a schema.
    #[schemars(range(min = 0.0, max = 3.402_823_5e38))]
    pub loudness_ratio: f32,
    /// The later segment's speaking rate over the earlier segment's.
    ///
    /// Bounded on the same terms, and for the same reason, as
    /// [`JoinContinuity::loudness_ratio`].
    #[schemars(range(min = 0.0, max = 3.402_823_5e38))]
    pub rate_ratio: f32,
    /// Where the interpretation of these numbers would have to come from.
    pub calibration_source: CalibrationSource,
}

/// Measures the two segments meeting at one join against each other.
///
/// Loudness is the RMS of each side's *speech* — the samples left once the
/// measured edge silence is excluded — rather than of a fixed window, so no
/// window length has to be invented while ADR-0003's calibration table is
/// `Pending`. Speaking rate is speech frames per character of spoken text, a
/// **provisional proxy**: no ratified document in this repository defines a
/// speaking-rate measure, and `docs/architecture/E2-S2-INTERFACE-CHANGE-001.md`
/// §Open questions records that for the audio owner.
///
/// A ratio whose denominator is zero is reported as `0.0`. That is a
/// correctness constraint rather than a preference: JSON has no infinity, so a
/// non-finite ratio is a manifest `serde_json` cannot write at all.
#[must_use]
pub fn assess_join(earlier: &JoinSide<'_>, later: &JoinSide<'_>) -> JoinContinuity {
    let earlier_speech = speech_of(earlier);
    let later_speech = speech_of(later);
    JoinContinuity {
        loudness_ratio: ratio(frame_rms(later_speech), frame_rms(earlier_speech)),
        rate_ratio: ratio(
            speaking_rate(later_speech.len(), later.characters),
            speaking_rate(earlier_speech.len(), earlier.characters),
        ),
        calibration_source: CalibrationSource::Provisional,
    }
}

impl JoinContinuity {
    /// This comparison, for a caller that requires a calibrated production
    /// reference.
    ///
    /// # Errors
    ///
    /// [`ProvisionalCalibration`] while
    /// [`JoinContinuity::calibration_source`] is
    /// [`CalibrationSource::Provisional`], which it always is until ADR-0003 is
    /// accepted. The same rule [`SilenceThreshold::production`] applies, for
    /// the same reason: ADR-0001 §13.3 says preview references cannot become
    /// production references without calibration.
    pub const fn production(&self) -> Result<Self, ProvisionalCalibration> {
        match self.calibration_source {
            CalibrationSource::Frozen => Ok(*self),
            CalibrationSource::Provisional => Err(ProvisionalCalibration),
        }
    }
}

/// The samples of one side that are not its measured edge silence.
///
/// Through [`measure_edge_silence`] rather than a second scan, for the reason
/// that function's own doc gives: two implementations of one measurement stop
/// agreeing, and a join would then be measured over a region the conditioner
/// treated as silence.
fn speech_of<'a>(side: &JoinSide<'a>) -> &'a [f32] {
    let (leading, trailing) = measure_edge_silence(
        side.samples,
        side.sample_rate,
        SilenceThreshold::provisional(),
    );
    side.samples
        .get(leading..side.samples.len().saturating_sub(trailing))
        .unwrap_or_default()
}

/// Speech frames per character of the text they were rendered from.
fn speaking_rate(frames: usize, characters: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "both counts are bounded by MAX_SEGMENT_AUDIO_MS and the lesson's text \
                  ceiling, far below f32's exact-integer range"
    )]
    let rate = ratio(frames as f32, characters as f32);
    rate
}

/// `numerator / denominator`, or zero where the denominator is.
///
/// Zero is the only denominator that has to be guarded, and that is an argument
/// rather than an assumption. Both ratios divide measurements of a *speech*
/// region, and [`speech_of`] returns an empty slice for a side whose every
/// frame is below [`SilenceThreshold`] — so a wholly silent side arrives as an
/// exact `0.0` rather than as a denormal. Any non-empty region contains a frame
/// whose RMS exceeded that threshold, which floors the region's own RMS far
/// above the `2.9e-39` a unit numerator would need to overflow `f32`. Widening
/// the region without that floor would reintroduce the case, which is why the
/// reasoning is recorded next to the guard rather than left to be rederived.
fn ratio(numerator: f32, denominator: f32) -> f32 {
    if denominator == 0.0 {
        return 0.0;
    }
    numerator / denominator
}

/// A provisional value was asked to serve as a production reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error(
    "this build's silence threshold is provisional while ADR-0003 is Proposed and its \
     calibration table records the value as pending; a production reference needs an accepted \
     ADR-0003, and the audio owner must complete that calibration before this is asked for again"
)]
pub struct ProvisionalCalibration;

/// What conditioning did to one segment, in samples.
///
/// ADR-0001 §13.4 requires the padding and ramp sample counts to be recorded
/// rather than merely applied, so a reviewer can tell audio that needed no work
/// from audio that was rebuilt at both ends.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EdgeConditioning {
    /// Zero samples added before the first sample the worker wrote.
    pub leading_padding: u32,
    /// Zero samples added after the last sample the worker wrote.
    pub trailing_padding: u32,
    /// Samples the leading raised-cosine ramp covers.
    pub leading_ramp: u32,
    /// Samples the trailing raised-cosine ramp covers.
    pub trailing_ramp: u32,
}

/// Samples in `milliseconds` at `sample_rate`.
///
/// Saturating rather than wrapping, and computed in `u64` so the multiplication
/// cannot overflow before the narrowing: a rate and a duration that are both in
/// range can still multiply past `u32`.
///
/// Public because cache acceptance converts the same ratified durations —
/// [`REQUIRED_EDGE_SILENCE_MS`], [`MAX_TRANSITION_RAMP_MS`] — into the sample
/// counts it checks a published entry against. A second conversion is how the
/// publisher and the acceptor stop agreeing on what the ADR asked for.
#[must_use]
pub fn samples_for(milliseconds: u32, sample_rate: u32) -> usize {
    let samples = u64::from(milliseconds) * u64::from(sample_rate) / 1_000;
    usize::try_from(samples).unwrap_or(usize::MAX)
}

/// Root mean square of one frame.
///
/// Computed in `f64` so a long frame of small values does not lose the sum to
/// rounding before the square root sees it.
fn frame_rms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let sum: f64 = frame.iter().map(|sample| f64::from(*sample).powi(2)).sum();
    // The mean of squares of finite samples is finite and non-negative, so the
    // root is real; `validate_wav` has already refused a non-finite sample.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "an RMS of canonical samples is within f32 by construction: every sample \
                  satisfies |x| <= 1.0, so the mean of squares does too"
    )]
    let rms = (sum / frame.len() as f64).sqrt() as f32;
    rms
}

/// Leading silent samples, measured in whole [`EDGE_ANALYSIS_FRAME_MS`] frames.
///
/// Whole frames rather than individual samples because that is what ADR-0001
/// §13.4 specifies: a per-sample scan would call a single zero crossing inside
/// speech "silence" and place a ramp in the middle of a word.
fn leading_silent_samples(samples: &[f32], sample_rate: u32, threshold: f32) -> usize {
    let frame = samples_for(EDGE_ANALYSIS_FRAME_MS, sample_rate).max(1);
    let mut silent = 0;
    while silent + frame <= samples.len() {
        if frame_rms(&samples[silent..silent + frame]) > threshold {
            break;
        }
        silent += frame;
    }
    // A remainder shorter than one frame is measured as a partial frame rather
    // than left unmeasured. Without this a segment shorter than 5 ms measures
    // as having no silence at all even when every sample in it is zero, and the
    // conditioner then treats silence as an edge to ramp out of.
    if samples.len() - silent < frame && frame_rms(&samples[silent..]) <= threshold {
        silent = samples.len();
    }
    silent
}

/// Trailing silent samples, measured the same way.
fn trailing_silent_samples(samples: &[f32], sample_rate: u32, threshold: f32) -> usize {
    let frame = samples_for(EDGE_ANALYSIS_FRAME_MS, sample_rate).max(1);
    let mut silent = 0;
    while silent + frame <= samples.len() {
        let end = samples.len() - silent;
        if frame_rms(&samples[end - frame..end]) > threshold {
            break;
        }
        silent += frame;
    }
    if samples.len() - silent < frame && frame_rms(&samples[..samples.len() - silent]) <= threshold
    {
        silent = samples.len();
    }
    silent
}

/// Silence at each exposed edge, in samples, leading first.
///
/// The measurement [`condition_edges`] pads from, exposed so cache acceptance
/// can re-derive it. ADR-0001 §12.6 makes the silence check a condition of
/// *using* an entry as well as of writing one, and a second implementation of
/// the same measurement is how the two ends stop agreeing: an entry would then
/// be publishable under one reading and unusable under the other.
///
/// Returns `(leading, trailing)`. Audio that is silent throughout reports its
/// whole length as both, which is what [`condition_edges`] does with it.
#[must_use]
pub fn measure_edge_silence(
    samples: &[f32],
    sample_rate: u32,
    threshold: SilenceThreshold,
) -> (usize, usize) {
    let level = threshold.rms();
    let leading = leading_silent_samples(samples, sample_rate, level);
    // A wholly silent segment reports its whole length as leading silence, and
    // measuring the trailing edge again would count the same samples twice.
    let trailing = if leading == samples.len() {
        leading
    } else {
        trailing_silent_samples(samples, sample_rate, level)
    };
    (leading, trailing)
}

/// Raised-cosine gain at `offset` of a ramp `length` samples long.
///
/// Exactly zero at offset 0 and rising toward unity, with zero slope at both
/// ends — which is what makes a transition smooth rather than merely gradual.
fn raised_cosine_gain(offset: usize, length: usize) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a ramp is at most 5 ms of samples, far inside f32's exact integer range"
    )]
    let position = offset as f32 / length as f32;
    0.5 * (1.0 - (PI * position).cos())
}

/// Fades the signal in across the `length` samples starting at `onset`.
///
/// The ramp covers *signal*, not the silence before it. ADR-0001 §13.4 requires
/// each silence-to-signal transition to be smoothed, and after padding the
/// silence side is exactly zero: scaling zero by any gain leaves zero, so a
/// ramp confined to the silence leaves the very step it exists to remove. The
/// sample at `onset` is scaled by exactly zero, so the transition out of the
/// padding is continuous and the exposed endpoint stays exactly zero.
fn apply_leading_ramp(samples: &mut [f32], onset: usize, length: usize) {
    for offset in 0..length {
        samples[onset + offset] *= raised_cosine_gain(offset, length);
    }
}

/// The mirror at the other end, fading out across the `length` samples ending
/// at `end`, which is one past the last signal sample.
fn apply_trailing_ramp(samples: &mut [f32], end: usize, length: usize) {
    for offset in 0..length {
        samples[end - 1 - offset] *= raised_cosine_gain(offset, length);
    }
}

/// Pads and ramps both exposed edges, reporting what it did.
///
/// ADR-0001 §13.4, in the order that document states: analyze each edge in 5 ms
/// RMS frames, add zero samples until each edge has at least 10 ms of silence,
/// then smooth each silence-to-signal transition with a raised-cosine ramp no
/// longer than 5 ms.
///
/// **Smoothing attenuates the first and last 5 ms of signal**, because that is
/// the only side of the transition with anything to attenuate: the silence side
/// is exactly zero once padded. `DELIVERY-PLAN.md` E1-S3 task 3 previously
/// required the ramp to apply "without entering speech", which cannot smooth
/// anything at all; the project owner resolved the conflict in ADR-0001's
/// favour under the conflict order in `CLAUDE.md`, and the plan now carries
/// ADR-0001's wording and names
/// `t1_e2_ramp_smooths_the_silence_to_signal_transition` in return.
///
/// Ramp length is capped at half the signal, so on a segment shorter than two
/// full ramps the two abut rather than overlapping into a hole in its middle.
///
/// Audio that is silent throughout is padded and left unramped: there is no
/// silence-to-signal transition to smooth, and inventing one would fabricate a
/// boundary the audio does not have.
#[must_use]
pub fn condition_edges(
    samples: &mut Vec<f32>,
    sample_rate: u32,
    threshold: SilenceThreshold,
) -> EdgeConditioning {
    let required = samples_for(REQUIRED_EDGE_SILENCE_MS, sample_rate);
    let max_ramp = samples_for(MAX_TRANSITION_RAMP_MS, sample_rate);

    let (leading_silence, trailing_silence) = measure_edge_silence(samples, sample_rate, threshold);
    let wholly_silent = leading_silence == samples.len();

    samples[..leading_silence].fill(0.0);
    let trailing_start = samples.len().saturating_sub(trailing_silence);
    samples[trailing_start..].fill(0.0);

    let leading_padding = required.saturating_sub(leading_silence);
    let trailing_padding = required.saturating_sub(trailing_silence);
    if leading_padding > 0 {
        samples.splice(0..0, std::iter::repeat_n(0.0, leading_padding));
    }
    samples.extend(std::iter::repeat_n(0.0, trailing_padding));

    let mut conditioning = EdgeConditioning {
        leading_padding: u32::try_from(leading_padding).unwrap_or(u32::MAX),
        trailing_padding: u32::try_from(trailing_padding).unwrap_or(u32::MAX),
        ..EdgeConditioning::default()
    };
    if wholly_silent {
        return conditioning;
    }

    // The signal is what lies between the two silences, and it is what the
    // ramps cover. Half of it apiece, so two ramps on a segment shorter than
    // 10 ms of signal abut instead of overlapping and multiplying.
    let leading_boundary = leading_silence + leading_padding;
    let trailing_boundary = samples.len() - (trailing_silence + trailing_padding);
    let ramp = max_ramp.min((trailing_boundary - leading_boundary) / 2);
    apply_leading_ramp(samples, leading_boundary, ramp);
    apply_trailing_ramp(samples, trailing_boundary, ramp);

    let recorded = u32::try_from(ramp).unwrap_or(u32::MAX);
    conditioning.leading_ramp = recorded;
    conditioning.trailing_ramp = recorded;
    conditioning
}

#[cfg(test)]
mod tests {
    use study_tts_core::CANONICAL_SAMPLE_RATE;

    use super::*;

    const RATE: u32 = 24_000;
    /// 10 ms at the canonical rate, which is [`REQUIRED_EDGE_SILENCE_MS`].
    const REQUIRED: usize = 240;
    /// 5 ms at the canonical rate, which is [`MAX_TRANSITION_RAMP_MS`].
    const RAMP: usize = 120;

    /// Loud enough that every 5 ms frame covering it exceeds the threshold.
    fn speech(length: usize) -> Vec<f32> {
        (0..length).map(|_| 0.5).collect()
    }

    #[test]
    fn t1_e2_short_edge_is_padded_to_ten_milliseconds() {
        // Signal from the first sample: both edges are exposed and both must be
        // padded to the full requirement.
        let mut samples = speech(2_400);

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_padding, REQUIRED as u32);
        assert_eq!(conditioning.trailing_padding, REQUIRED as u32);
        assert_eq!(samples.len(), 2_400 + REQUIRED * 2);
    }

    #[test]
    fn t1_e2_sufficient_edge_receives_no_extra_padding() {
        // Already quiet at both ends by more than the requirement, so
        // conditioning must add nothing: padding audio that does not need it
        // would move the segment's duration on every republication.
        let mut samples = vec![0.0; REQUIRED * 2];
        samples.extend(speech(2_400));
        samples.extend(std::iter::repeat_n(0.0, REQUIRED * 2));
        let before = samples.len();

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_padding, 0);
        assert_eq!(conditioning.trailing_padding, 0);
        assert_eq!(samples.len(), before);
    }

    #[test]
    fn t1_e2_ramp_smooths_the_silence_to_signal_transition() {
        // ADR-0001 §13.4's actual requirement, and the one a ramp confined to
        // the padding cannot meet: scaling inserted zeros is arithmetically
        // inert, so the 0.0 -> 0.5 step at the onset survived untouched. The
        // ramp therefore covers signal, and this pins both halves of the rule —
        // that the transition is smooth, and that the smoothing is bounded.
        let mut samples = speech(2_400);

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_padding, REQUIRED as u32);
        assert_eq!(conditioning.leading_ramp, RAMP as u32);
        assert_eq!(
            samples[REQUIRED], 0.0,
            "the transition must start from exactly zero"
        );
        for index in REQUIRED..REQUIRED + RAMP - 1 {
            assert!(
                samples[index] < samples[index + 1],
                "the ramp must rise at {index}: {} then {}",
                samples[index],
                samples[index + 1]
            );
        }
        for (index, sample) in samples
            .iter()
            .enumerate()
            .skip(REQUIRED + RAMP)
            .take(2_400 - 2 * RAMP)
        {
            assert!(
                (*sample - 0.5).abs() < f32::EPSILON,
                "signal beyond the ramp was attenuated at {index} to {sample}"
            );
        }
    }

    #[test]
    fn t1_e2_ramp_is_capped_by_the_signal_it_rises_into() {
        // A segment holding less signal than two full ramps gets shorter ramps
        // rather than two that overlap and multiply into a hole in its middle.
        let mut samples = speech(100);

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_ramp, 50);
        assert_eq!(conditioning.trailing_ramp, 50);
    }

    #[test]
    fn t1_e2_a_loud_burst_before_long_silence_is_not_measured_as_wholly_silent() {
        // The partial-frame branch measures a remainder *shorter than one
        // frame*. Unbounded, it instead measured the whole remainder: a quiet
        // burst followed by a second of silence averages below the threshold,
        // the segment is called wholly silent, and conditioning returns before
        // ramping any of the real signal. The burst is above the threshold in
        // its own 5 ms frame and below it once diluted across the remainder,
        // which is exactly the shape that distinguishes the two readings.
        let mut samples = vec![0.01; 120];
        samples.extend(std::iter::repeat_n(0.0, 24_000));

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_padding, REQUIRED as u32);
        assert_eq!(
            conditioning.leading_ramp, 60,
            "the burst is signal, so its onset must be smoothed"
        );
    }

    #[test]
    fn t1_e2_exposed_endpoints_are_exactly_zero() {
        // Exactly zero, not merely small: ADR-0001 §13.4 requires it so that
        // assembly can concatenate segments without introducing a step at the
        // join.
        let mut samples = speech(2_400);

        let _ = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(samples.first(), Some(&0.0));
        assert_eq!(samples.last(), Some(&0.0));
    }

    #[test]
    fn t1_e2_a_quiet_but_nonzero_edge_is_normalized_without_padding() {
        // Quiet model output already satisfies the measured-silence duration,
        // so conditioning normalizes that measured region instead of changing
        // the segment duration merely to make its endpoints exact zeros.
        const QUIET: f32 = 4.312_751e-6;
        let mut samples = vec![QUIET; REQUIRED * 2];
        samples.extend(speech(2_400));
        samples.extend(std::iter::repeat_n(QUIET, REQUIRED * 2));
        let before = samples.len();

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(
            samples.first(),
            Some(&0.0),
            "the first sample must be exactly zero"
        );
        assert_eq!(
            samples.last(),
            Some(&0.0),
            "the last sample must be exactly zero"
        );
        assert_eq!(conditioning.leading_padding, 0);
        assert_eq!(conditioning.trailing_padding, 0);
        assert_eq!(samples.len(), before);
    }

    #[test]
    fn t1_e2_wholly_silent_audio_is_padded_but_not_ramped() {
        // There is no silence-to-signal transition to smooth, and inventing one
        // would fabricate a boundary the audio does not have.
        let mut samples = vec![0.0; 100];

        let conditioning = condition_edges(&mut samples, RATE, SilenceThreshold::provisional());

        assert_eq!(conditioning.leading_ramp, 0);
        assert_eq!(conditioning.trailing_ramp, 0);
        assert_eq!(samples.len(), 100 + (REQUIRED - 100) * 2);
    }

    #[test]
    fn t3_e2_provisional_measurement_cannot_satisfy_production_calibration() {
        // The guard that keeps this build's stand-in from becoming ADR-0003's
        // frozen value by being passed along. Conditioning may use it; anything
        // asking for a production reference may not.
        let threshold = SilenceThreshold::provisional();

        assert_eq!(threshold.source(), CalibrationSource::Provisional);
        assert!(
            threshold.rms() > 0.0,
            "conditioning still has a level to use"
        );
        assert_eq!(threshold.production(), Err(ProvisionalCalibration));
    }

    #[test]
    fn t1_e2_edge_geometry_matches_the_ratified_constants() {
        // ADR-0001 §13.4 fixes this geometry itself rather than delegating
        // it to ADR-0003, so these three are not provisional and a change to
        // any of them is a change to the ADR.
        assert_eq!(EDGE_ANALYSIS_FRAME_MS, 5);
        assert_eq!(REQUIRED_EDGE_SILENCE_MS, 10);
        assert_eq!(MAX_TRANSITION_RAMP_MS, 5);
        assert_eq!(samples_for(REQUIRED_EDGE_SILENCE_MS, RATE), REQUIRED);
        assert_eq!(samples_for(MAX_TRANSITION_RAMP_MS, RATE), RAMP);
    }

    #[test]
    fn t1_e2_a_join_is_measured_against_the_speech_on_each_side() {
        // Hand-built sides rather than rendered audio: the arithmetic is the
        // subject, and a reviewer can read each expectation off the inputs.
        // Every side carries the ADR-0001 §13.4 edge silence a published entry
        // must have, so the measurement is over speech in both directions.
        let side = |level: f32, speech_frames: usize, characters: usize| {
            let silence = samples_for(REQUIRED_EDGE_SILENCE_MS, CANONICAL_SAMPLE_RATE);
            let mut samples = vec![0.0_f32; silence];
            samples.extend(std::iter::repeat_n(level, speech_frames));
            samples.extend(std::iter::repeat_n(0.0_f32, silence));
            (samples, characters)
        };

        /// One side of a case: signal level, speech samples, and characters.
        type Rendered = (f32, usize, usize);
        /// A case: the earlier side, the later side, and the two ratios the
        /// pair must measure.
        type Case = (Rendered, Rendered, f32, f32);

        const CASES: [Case; 3] = [
            // Identical sides compare as one.
            ((0.5, 2_400, 10), (0.5, 2_400, 10), 1.0, 1.0),
            // Twice the level, same rate.
            ((0.25, 2_400, 10), (0.5, 2_400, 10), 2.0, 1.0),
            // Same level, half the frames per character: twice as fast.
            ((0.5, 2_400, 10), (0.5, 2_400, 20), 1.0, 0.5),
        ];

        for (earlier, later, loudness, rate) in CASES {
            let (earlier_samples, earlier_characters) = side(earlier.0, earlier.1, earlier.2);
            let (later_samples, later_characters) = side(later.0, later.1, later.2);

            let measured = assess_join(
                &JoinSide {
                    samples: &earlier_samples,
                    sample_rate: CANONICAL_SAMPLE_RATE,
                    characters: earlier_characters,
                },
                &JoinSide {
                    samples: &later_samples,
                    sample_rate: CANONICAL_SAMPLE_RATE,
                    characters: later_characters,
                },
            );

            let case = format!("{earlier:?} then {later:?}");
            assert!(
                (measured.loudness_ratio - loudness).abs() < 1e-3,
                "{case}: loudness {} is not {loudness}",
                measured.loudness_ratio
            );
            assert!(
                (measured.rate_ratio - rate).abs() < 1e-3,
                "{case}: rate {} is not {rate}",
                measured.rate_ratio
            );
            assert_eq!(measured.calibration_source, CalibrationSource::Provisional);
        }
    }

    #[test]
    fn t1_e2_a_join_measurement_is_never_a_production_reference() {
        let silent = JoinSide {
            samples: &[0.0; 480],
            sample_rate: CANONICAL_SAMPLE_RATE,
            characters: 4,
        };

        let measured = assess_join(&silent, &silent);

        // A side with no measurable speech divides by zero, and JSON has no
        // infinity: a non-finite ratio is a manifest `serde_json` refuses to
        // write at all.
        assert!(measured.loudness_ratio.is_finite());
        assert!(measured.rate_ratio.is_finite());
        assert_eq!(measured.production(), Err(ProvisionalCalibration));
    }

    #[test]
    fn t1_e1_calibration_source_spelling_matches_its_serde_form() {
        // A cache entry records the serde form; a refusal quotes `name`. An
        // exhaustive list makes a new calibration a compile error here rather
        // than a spelling nobody checked.
        for source in [CalibrationSource::Provisional, CalibrationSource::Frozen] {
            let serialized =
                serde_json::to_string(&source).expect("a calibration source serializes");

            assert_eq!(serialized, format!("\"{}\"", source.name()));
        }
    }
}
