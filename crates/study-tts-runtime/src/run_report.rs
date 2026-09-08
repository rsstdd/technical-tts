//! The run report: what one build measured, and what reading it means.
//!
//! `DELIVERY-PLAN.md` E2-S4 task 4 requires every measured field to declare its
//! unit, clock or sampling source, measured process, aggregation, missing-value
//! semantics, and whether it is exact, sampled, or approximate under WSL2. Five
//! of those six are properties of the *field*, identical on every run, so they
//! live in [`ReportField::semantics`] and are mirrored by
//! `docs/observability/RUN-REPORT-FIELDS.md` rather than written into every
//! document. Only the sixth varies per run, which is why [`Measured`] is the
//! one piece of vocabulary that reaches the wire.
//!
//! This module defines the document and measures nothing. E2-S4's later steps
//! time the build and fill these values in.
//!
//! **Redaction is structural.** §Storage and access of
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` allows logs to carry
//! "hashes, IDs, timings, states, and error classes" and excludes source text,
//! spoken text, and voice-reference paths. No type below has a field any of
//! those could land in, so
//! `t4_e2_run_report_excludes_sensitive_fixture_content` will check an
//! invariant rather than a scrubber.

use serde::{Deserialize, Serialize};
use study_tts_core::{CANONICAL_SAMPLE_RATE, SchemaVersion};

/// File-name stem of the published run-report schema.
pub const RUN_REPORT_SCHEMA_STEM: &str = "run-report";

/// Version of the published run-report schema.
///
/// `1.0` at its first publication, carrying the `1.0-skeleton` layout label
/// [`RUN_REPORT_LAYOUT_VERSION`] writes. The suffix follows the precedent
/// `MANIFEST_SCHEMA_VERSION` sets: later E2-S4 steps add measured stages to
/// this document, so the label must not claim a stability they are going to
/// take away.
pub const RUN_REPORT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0);

/// The `schema_version` a `run-report.json` this build writes carries.
pub const RUN_REPORT_LAYOUT_VERSION: &str = "1.0-skeleton";

/// Microseconds in one millisecond, the scale a milli-ratio is expressed in.
const MICROSECONDS_PER_MILLISECOND: u64 = 1_000;

/// What a measured number counts.
///
/// A closed vocabulary rather than a string, so a field cannot declare a unit
/// nothing else uses. Field names carry the same unit as a suffix — `_micros`,
/// `_frames`, `_kib` — and the published-schema suite holds the two spellings
/// together.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementUnit {
    /// Microseconds of elapsed time.
    Microseconds,
    /// Audio frames at [`CANONICAL_SAMPLE_RATE`].
    Frames,
    /// A count of discrete things.
    Count,
    /// Kibibytes of resident memory.
    Kibibytes,
    /// A ratio multiplied by one thousand, so it needs no floating point.
    MilliRatio,
}

impl MeasurementUnit {
    /// The suffix a published field name carries so its unit is not implied.
    ///
    /// One definition, read against the property names `schemars` generates by
    /// `t3_e2_every_published_run_report_number_names_its_unit`. Keeping the
    /// unit in the name is the convention `total_frames` already set.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Microseconds => "_micros",
            Self::Frames => "_frames",
            Self::Count => "_count",
            Self::Kibibytes => "_kib",
            Self::MilliRatio => "_milli",
        }
    }
}

/// Where a measured number was read from.
///
/// ADR-0001 §14's "clock or sampling source", as a closed set. A reader that
/// knows a value came from `/proc` treats it differently from one taken off a
/// monotonic clock, and neither is a wall-clock stamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementClock {
    /// A monotonic interval, immune to a wall-clock adjustment mid-render.
    MonotonicElapsed,
    /// Counted from audio this build produced, not timed.
    FrameCount,
    /// Sampled from `/proc/<pid>/status`.
    ProcStatus,
    /// Counted from `/proc/<pid>/fd`.
    ProcDescriptors,
    /// Computed from other fields in the same document.
    Derived,
    /// Fixed by this build's architecture rather than observed.
    Structural,
}

/// Which process a number describes.
///
/// The supervisor is this Rust binary; the worker is the Python child holding
/// Torch. `docs/perf/BUDGETS.md`'s ratified real-time factor is a worker
/// measurement, so a supervisor-measured ratio could not be read against it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasuredProcess {
    /// The Rust process that orchestrates the build.
    Supervisor,
    /// The Python synthesis worker.
    Worker,
}

/// How a number combines the observations behind it.
///
/// This is the axis `DELIVERY-PLAN.md` E2-S4 task 3 left open by asking for an
/// "aggregate RTF" while `docs/perf/BUDGETS.md` registers a worst-of-ten
/// per-utterance baseline. Both statistics are published, and this enum is what
/// keeps them from being read as one number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Aggregation {
    /// One observation, or a sum presented as the run's own figure.
    Total,
    /// A quotient of two sums over the whole run.
    Aggregate,
    /// The single worst segment, which carries its own audio length.
    WorstSegment,
    /// The smallest observation over the run's segments.
    Minimum,
    /// The largest observation over the run's segments.
    Maximum,
}

/// How much a number can be trusted on this platform.
///
/// ADR-0001 §14 admits peak RAM only "where the operating environment exposes
/// them reliably", and `docs/operations/REFERENCE-ENVIRONMENT.md` records that
/// this build runs under a constrained WSL2 allocation.
/// [`Fidelity::Approximate`] is what that concession looks like as a value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Fidelity {
    /// Counted or timed exactly.
    Exact,
    /// A point sample, which a longer or shorter run would move.
    Approximate,
}

/// Everything ADR-0001 §14 and E2-S4 task 4 require to be said about one field.
///
/// Deliberately not serialized. A static annotation cannot be wrong at runtime,
/// so writing these five into every document would repeat the same constants
/// per run and grow the published schema without making it more truthful.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldSemantics {
    /// What the number counts.
    pub unit: MeasurementUnit,
    /// Where it was read from.
    pub clock: MeasurementClock,
    /// Which process it describes.
    pub measured_process: MeasuredProcess,
    /// How it combines the observations behind it.
    pub aggregation: Aggregation,
    /// How much it can be trusted on this platform.
    pub fidelity: Fidelity,
}

/// Every measured field this document publishes.
///
/// One variant per number, so [`ReportField::semantics`] is an exhaustive
/// `match` and a field added without semantics fails to compile. The names are
/// mirrored by `docs/observability/RUN-REPORT-FIELDS.md`, which names this enum
/// in return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportField {
    /// Elapsed time for the whole build.
    WallTime,
    /// Summed time inside the worker's synthesis calls.
    SynthesisWallTime,
    /// Summed audio the worker generated.
    GeneratedAudio,
    /// Segments the worker actually synthesized, as opposed to reused.
    SegmentsSynthesized,
    /// Real-time factor over the whole run.
    AggregateRealTimeFactor,
    /// Real-time factor of the single worst segment.
    WorstSegmentRealTimeFactor,
    /// Shortest segment audio the worker generated.
    SegmentAudioMinimum,
    /// Longest segment audio the worker generated.
    SegmentAudioMaximum,
    /// Times the worker was restarted during the build.
    WorkerRestarts,
    /// High-water resident memory of the worker.
    PeakResidentMemory,
    /// Open file descriptors held by the worker.
    OpenHandles,
}

impl ReportField {
    /// Every measured field, in publication order.
    ///
    /// A new variant is a compile error in [`ReportField::index`], which is
    /// the one place an author is forced to look and is where this array is
    /// extended too. Rust cannot enumerate an enum's variants without a derive
    /// this workspace does not carry, so the compiler guarantees the
    /// *acknowledgement* and the test below guarantees the array is
    /// self-consistent — it does not prove the array is complete. That
    /// remaining gap is why the two live adjacent rather than apart, and it is
    /// the same gap the hand-maintained remedy samples in `error` carry.
    pub const ALL: [Self; 11] = [
        Self::WallTime,
        Self::SynthesisWallTime,
        Self::GeneratedAudio,
        Self::SegmentsSynthesized,
        Self::AggregateRealTimeFactor,
        Self::WorstSegmentRealTimeFactor,
        Self::SegmentAudioMinimum,
        Self::SegmentAudioMaximum,
        Self::WorkerRestarts,
        Self::PeakResidentMemory,
        Self::OpenHandles,
    ];

    /// This field's position in [`ReportField::ALL`].
    ///
    /// Exhaustive on purpose. The round trip
    /// `t1_e2_run_report_units_and_missing_values_follow_schema` asserts is
    /// what makes the array above provably complete. Test-only: it exists to
    /// fail compilation on a new variant, and publishing it would add a
    /// position to the API that no consumer asked for.
    #[cfg(test)]
    const fn index(self) -> usize {
        match self {
            Self::WallTime => 0,
            Self::SynthesisWallTime => 1,
            Self::GeneratedAudio => 2,
            Self::SegmentsSynthesized => 3,
            Self::AggregateRealTimeFactor => 4,
            Self::WorstSegmentRealTimeFactor => 5,
            Self::SegmentAudioMinimum => 6,
            Self::SegmentAudioMaximum => 7,
            Self::WorkerRestarts => 8,
            Self::PeakResidentMemory => 9,
            Self::OpenHandles => 10,
        }
    }

    /// What reading this field means.
    ///
    /// Exhaustive, so a new field is a compile error here rather than an
    /// undeclared number in a published document.
    #[must_use]
    pub const fn semantics(self) -> FieldSemantics {
        let elapsed = FieldSemantics {
            unit: MeasurementUnit::Microseconds,
            clock: MeasurementClock::MonotonicElapsed,
            measured_process: MeasuredProcess::Worker,
            aggregation: Aggregation::Total,
            fidelity: Fidelity::Exact,
        };
        let audio = FieldSemantics {
            unit: MeasurementUnit::Frames,
            clock: MeasurementClock::FrameCount,
            measured_process: MeasuredProcess::Worker,
            aggregation: Aggregation::Total,
            fidelity: Fidelity::Exact,
        };
        let ratio = FieldSemantics {
            unit: MeasurementUnit::MilliRatio,
            clock: MeasurementClock::Derived,
            measured_process: MeasuredProcess::Worker,
            aggregation: Aggregation::Aggregate,
            fidelity: Fidelity::Exact,
        };

        match self {
            Self::WallTime => FieldSemantics {
                measured_process: MeasuredProcess::Supervisor,
                ..elapsed
            },
            Self::SynthesisWallTime => elapsed,
            Self::GeneratedAudio => audio,
            Self::SegmentsSynthesized => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::Structural,
                ..audio
            },
            Self::AggregateRealTimeFactor => ratio,
            Self::WorstSegmentRealTimeFactor => FieldSemantics {
                aggregation: Aggregation::WorstSegment,
                ..ratio
            },
            Self::SegmentAudioMinimum => FieldSemantics {
                aggregation: Aggregation::Minimum,
                ..audio
            },
            Self::SegmentAudioMaximum => FieldSemantics {
                aggregation: Aggregation::Maximum,
                ..audio
            },
            // E5-S2 owns the worker pool. Until it exists no code path
            // restarts a worker, so zero is this build's shape rather than
            // an observation, and the clock says so.
            Self::WorkerRestarts => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::Structural,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::Total,
                fidelity: Fidelity::Exact,
            },
            // A high-water mark over the run, not a sum: `VmHWM` is the
            // largest resident size the worker reached, so a reader told
            // `Total` could legitimately add two runs together.
            Self::PeakResidentMemory => FieldSemantics {
                unit: MeasurementUnit::Kibibytes,
                clock: MeasurementClock::ProcStatus,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::Maximum,
                fidelity: Fidelity::Approximate,
            },
            Self::OpenHandles => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::ProcDescriptors,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::Total,
                fidelity: Fidelity::Approximate,
            },
        }
    }
}

/// Why a measurement this document expected is not present.
///
/// A stated reason rather than a null, because a reader handed `0` learns
/// something false: a run whose peak resident memory could not be sampled did
/// not use zero kibibytes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum Unavailable {
    /// ADR-0001 §14 admits peak RAM and handle counts only where the operating
    /// environment exposes them reliably. This is that concession's negative
    /// case.
    NotExposedByEnvironment,
    /// The build ended before the stage that would have measured this.
    StageNotReached,
    /// No audio was generated, so a ratio over it has no value. Distinct from a
    /// ratio of zero, which would claim synthesis took no time.
    NoAudioGenerated,
}

/// A number this environment may or may not have been able to observe.
///
/// Two variants rather than a nullable number, so a document cannot say
/// "missing" and "zero" with the same bytes. No `#[serde(other)]`: an
/// observation state this build does not know is a parse error.
/// Not generic. Every measurement this document publishes is a `u64`, and a
/// type parameter with one instantiation buys nothing while costing a real
/// hazard: `schemars` names generic instantiations positionally, so a second
/// one would publish `$defs/Measured2` and reordering fields could swap which
/// definition a reader is looking at.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "observation")]
pub enum Measured {
    /// The value was measured.
    Observed {
        /// What was measured, in the unit its field name names.
        value: u64,
    },
    /// The value was not measured, and this is why.
    Unavailable {
        /// Why no value is present.
        reason: Unavailable,
    },
}

/// The single worst segment by real-time factor, with the audio that produced
/// it.
///
/// Shaped differently from the aggregate deliberately.
/// `evidence/gates/g0/e0-s3/e0-s3-g0-requalification-torch-2-10-0-v1.md` found
/// that "a single RTF number is not meaningful for this backend unless the
/// utterance length is stated beside it" — the fitted cost is a fixed
/// per-take charge plus a marginal rate — so the worst ratio is a fact about
/// one utterance and carries that utterance's length.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorstSegment {
    /// Identifier of the segment this describes.
    pub segment_id: String,
    /// Take of that segment the build synthesized.
    pub take: u32,
    /// Time inside the worker's synthesis call for this segment.
    pub wall_micros: u64,
    /// Audio the worker generated for this segment.
    pub audio_frames: u64,
    /// This segment's real-time factor, multiplied by one thousand.
    pub real_time_factor_milli: u64,
}

/// What the worker did across the whole build.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct SynthesisTotals {
    /// Segments the worker synthesized, as opposed to segments reused from
    /// cache.
    pub segments_synthesized_count: u32,
    /// Summed time inside the worker's synthesis calls.
    pub wall_micros: u64,
    /// Summed audio the worker generated, excluding inter-segment pauses the
    /// timeline inserts.
    pub audio_frames: u64,
    /// Real-time factor over the whole run: summed synthesis time divided by
    /// summed generated audio, per ADR-0001 §3.4.
    pub aggregate_real_time_factor_milli: Measured,
    /// Shortest segment audio the worker generated.
    pub segment_audio_minimum_frames: Measured,
    /// Longest segment audio the worker generated.
    pub segment_audio_maximum_frames: Measured,
    /// The worst single segment, absent when nothing was synthesized.
    pub worst_segment: Option<WorstSegment>,
}

/// What the build cost the machine.
///
/// No VRAM field. ADR-0001 §14 lists "peak RAM and VRAM where the operating
/// environment exposes them reliably", and ADR-0002 pins a CPU-only backend, so
/// a VRAM field would be permanently unavailable — a field whose only value is
/// "not applicable". `docs/architecture/E2-S4-INTERFACE-CHANGE-001.md` records
/// the omission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RunResources {
    /// High-water resident memory of the worker.
    pub peak_resident_kib: Measured,
    /// Open file descriptors held by the worker.
    pub open_handles_count: Measured,
    /// Times the worker was restarted. Structurally zero at pool size one.
    pub worker_restarts_count: u32,
}

/// The layout label a `run-report.json` this build writes carries.
///
/// A newtype rather than a `String` so a foreign layout is refused where the
/// document is parsed rather than compared somewhere downstream.
/// `study_tts_core::JobDocument` gates its own version the same way: a
/// document is not readable merely because its fields happen to parse.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RunReportLayout(String);

impl RunReportLayout {
    /// The one layout this build writes and reads.
    #[must_use]
    pub fn current() -> Self {
        Self(RUN_REPORT_LAYOUT_VERSION.to_owned())
    }

    /// The label, as it appears in the document.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for RunReportLayout {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let declared = String::deserialize(deserializer)?;
        if declared == RUN_REPORT_LAYOUT_VERSION {
            return Ok(Self(declared));
        }

        Err(serde::de::Error::custom(format!(
            "run report declares layout `{declared}`, and this build reads only \
             `{RUN_REPORT_LAYOUT_VERSION}`"
        )))
    }
}

impl schemars::JsonSchema for RunReportLayout {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "RunReportLayout".into()
    }

    /// Published as a `const`, so the schema refuses a foreign layout at the
    /// same field the parser does instead of deferring to it.
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "const": RUN_REPORT_LAYOUT_VERSION,
        })
    }
}

/// What one build measured.
///
/// Carries identifiers, timings, and counts only. It names no file, quotes no
/// lesson text, and holds no voice-reference path.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RunReport {
    /// Layout this document was written against.
    pub schema_version: RunReportLayout,
    /// Job this build belongs to.
    pub job_id: String,
    /// Attempt within that job.
    pub build_attempt: u32,
    /// Lesson this build rendered.
    pub lesson_id: String,
    /// Plan hash the build rendered from.
    pub plan_hash: String,
    /// Elapsed time for the whole build.
    pub wall_micros: u64,
    /// What the worker did.
    pub synthesis: SynthesisTotals,
    /// What the build cost the machine.
    pub resources: RunResources,
}

/// Returns the real-time factor of one synthesis, multiplied by one thousand.
///
/// ADR-0001 §3.4 defines the ratio as "synthesis wall time divided by
/// generated-audio duration, excluding one-time installation and model
/// download". Both operands are exact integers here — elapsed microseconds and
/// audio frames at [`CANONICAL_SAMPLE_RATE`] — so the quotient needs no
/// floating point, and a consumer can check ADR-0001's `<= 6.0` gate against
/// `6_000` in integer arithmetic.
///
/// The algebra, with `r` the sample rate: `(micros / 1e6) / (frames / r)`
/// scaled by `1000` is `micros * r / (frames * 1000)`.
///
/// Saturating rather than wrapping on the multiply. A render long enough to
/// overflow would be some five million years, so the saturation is a guard
/// against a nonsense argument rather than a reachable case.
#[must_use]
pub fn milli_real_time_factor(wall_micros: u64, audio_frames: u64) -> Measured {
    if audio_frames == 0 {
        return Measured::Unavailable {
            reason: Unavailable::NoAudioGenerated,
        };
    }

    let scaled = wall_micros.saturating_mul(u64::from(CANONICAL_SAMPLE_RATE));
    Measured::Observed {
        value: scaled / (audio_frames * MICROSECONDS_PER_MILLISECOND),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every measured field, checked against the report that publishes them.
    ///
    /// Destructures the report with no rest pattern, so a struct field added
    /// without a [`ReportField`] beside it is a compile error;
    /// `manifest::expected_executions` uses the same mechanism. The list itself
    /// comes from [`ReportField::ALL`], which [`ReportField::index`] holds
    /// complete, so a new *variant* cannot slip past either.
    fn declared_fields(report: &RunReport) -> Vec<ReportField> {
        let RunReport {
            schema_version: _,
            job_id: _,
            build_attempt: _,
            lesson_id: _,
            plan_hash: _,
            wall_micros: _,
            synthesis,
            resources,
        } = report;
        let SynthesisTotals {
            segments_synthesized_count: _,
            wall_micros: _,
            audio_frames: _,
            aggregate_real_time_factor_milli: _,
            segment_audio_minimum_frames: _,
            segment_audio_maximum_frames: _,
            worst_segment: _,
        } = synthesis;
        let RunResources {
            peak_resident_kib: _,
            open_handles_count: _,
            worker_restarts_count: _,
        } = resources;

        ReportField::ALL.to_vec()
    }

    fn unmeasured_report() -> RunReport {
        RunReport {
            schema_version: RunReportLayout::current(),
            job_id: "job".to_owned(),
            build_attempt: 1,
            lesson_id: "lesson".to_owned(),
            plan_hash: "0".repeat(64),
            wall_micros: 0,
            synthesis: SynthesisTotals {
                segments_synthesized_count: 0,
                wall_micros: 0,
                audio_frames: 0,
                aggregate_real_time_factor_milli: Measured::Unavailable {
                    reason: Unavailable::NoAudioGenerated,
                },
                segment_audio_minimum_frames: Measured::Unavailable {
                    reason: Unavailable::StageNotReached,
                },
                segment_audio_maximum_frames: Measured::Unavailable {
                    reason: Unavailable::StageNotReached,
                },
                worst_segment: None,
            },
            resources: RunResources {
                peak_resident_kib: Measured::Unavailable {
                    reason: Unavailable::NotExposedByEnvironment,
                },
                open_handles_count: Measured::Unavailable {
                    reason: Unavailable::NotExposedByEnvironment,
                },
                worker_restarts_count: 0,
            },
        }
    }

    #[test]
    fn t1_e2_run_report_units_and_missing_values_follow_schema() {
        // Task 4 is discharged by every field declaring all five static
        // semantics, so the check is that the vocabulary is total rather than
        // that any one value is a particular constant.
        // Every entry claims its own slot: this catches a reorder and a
        // duplicate. It cannot catch a variant missing from `ALL` — see that
        // constant's doc for why nothing here can.
        for (position, field) in ReportField::ALL.iter().enumerate() {
            assert_eq!(
                field.index(),
                position,
                "{field:?} is not at its declared position in ReportField::ALL"
            );
        }

        // Only a derived or structural number may claim `Exact` without a
        // clock behind it; anything read off `/proc` is a point sample.
        for field in declared_fields(&unmeasured_report()) {
            let semantics = field.semantics();
            let sampled = matches!(
                semantics.clock,
                MeasurementClock::ProcStatus | MeasurementClock::ProcDescriptors
            );
            assert_eq!(
                sampled,
                semantics.fidelity == Fidelity::Approximate,
                "{field:?} declares clock {:?} with fidelity {:?}",
                semantics.clock,
                semantics.fidelity
            );
        }

        // ADR-0001 §3.4 fixes the window and the denominator, so only the
        // aggregation separates the two published ratios.
        let aggregate = ReportField::AggregateRealTimeFactor.semantics();
        let worst = ReportField::WorstSegmentRealTimeFactor.semantics();
        assert_eq!(aggregate.aggregation, Aggregation::Aggregate);
        assert_eq!(worst.aggregation, Aggregation::WorstSegment);
        assert_eq!(
            FieldSemantics {
                aggregation: worst.aggregation,
                ..aggregate
            },
            worst,
            "the two real-time factors must differ only in aggregation"
        );

        // `docs/perf/BUDGETS.md` registers a worker measurement, so a
        // supervisor-measured ratio could not be read against it at all.
        assert_eq!(aggregate.measured_process, MeasuredProcess::Worker);

        // Nothing but those two is a ratio, so no other field invites a
        // comparison with ADR-0001 §3.4's `<= 6.0` gate.
        let ratios: Vec<ReportField> = declared_fields(&unmeasured_report())
            .into_iter()
            .filter(|field| field.semantics().unit == MeasurementUnit::MilliRatio)
            .collect();
        assert_eq!(
            ratios,
            vec![
                ReportField::AggregateRealTimeFactor,
                ReportField::WorstSegmentRealTimeFactor,
            ],
            "whole-build wall time is a duration, not a second real-time factor"
        );

        // E5-S2 owns the pool, so a restart count of zero is this build's
        // shape rather than something anybody observed.
        assert_eq!(
            ReportField::WorkerRestarts.semantics().clock,
            MeasurementClock::Structural
        );

        // ADR-0001 §14 admits these two only where the environment exposes
        // them reliably, and WSL2 is why that qualifier is in the sentence.
        for sampled in [ReportField::PeakResidentMemory, ReportField::OpenHandles] {
            assert_eq!(
                sampled.semantics().fidelity,
                Fidelity::Approximate,
                "{sampled:?} is a point sample under WSL2"
            );
        }
    }

    #[test]
    fn t1_e2_the_real_time_factor_matches_the_ratio_the_adr_defines() {
        // Read against ADR-0001 §3.4 and `docs/perf/BUDGETS.md`, not
        // recomputed here: a test that reran the implementation would agree
        // with a wrong one.
        const CASES: [(u64, u64, u64); 4] = [
            // ADR-0002's registered baseline: 14.9804 over its 5.88 s
            // utterance, which is 141,120 frames.
            (88_084_752, 141_120, 14_980),
            // The gate itself: 6.0 over one second of audio.
            (6_000_000, 24_000, 6_000),
            // The M2 acceptance render's upper bound, from
            // `m2-acceptance-lesson-render-v1`: 1,963 s over 281.60 s.
            (1_963_000_000, 6_758_400, 6_970),
            (0, 24_000, 0),
        ];

        for (wall_micros, audio_frames, expected) in CASES {
            assert_eq!(
                milli_real_time_factor(wall_micros, audio_frames),
                Measured::Observed { value: expected },
                "{wall_micros} us over {audio_frames} frames"
            );
        }
    }

    #[test]
    fn t1_e2_a_real_time_factor_over_no_audio_is_unavailable_rather_than_zero() {
        assert_eq!(
            milli_real_time_factor(1_000_000, 0),
            Measured::Unavailable {
                reason: Unavailable::NoAudioGenerated
            },
            "a ratio of zero would claim synthesis took no time"
        );
    }

    #[test]
    fn t1_e2_an_unavailable_measurement_carries_no_value_on_the_wire() {
        let absent: Measured = Measured::Unavailable {
            reason: Unavailable::NotExposedByEnvironment,
        };
        let encoded = serde_json::to_value(absent).expect("a measurement serializes");

        assert_eq!(encoded["observation"], "unavailable");
        assert_eq!(encoded["reason"], "not_exposed_by_environment");
        assert!(
            encoded.get("value").is_none(),
            "an unavailable measurement must not carry a number, got {encoded}"
        );

        let present = serde_json::to_value(Measured::Observed { value: 7_u64 })
            .expect("a measurement serializes");
        assert_eq!(present["observation"], "observed");
        assert_eq!(present["value"], 7);
        assert!(present.get("reason").is_none());
    }
}
