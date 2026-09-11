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
use std::num::NonZeroU32;

use study_tts_core::{CANONICAL_SAMPLE_RATE, MAX_LESSON_SEGMENTS, SchemaVersion, WorkerBundleHash};
use thiserror::Error;

use crate::BuildErrorClass;

/// File-name stem of the published run-report schema.
pub const RUN_REPORT_SCHEMA_STEM: &str = "run-report";

/// The file name this document carries wherever it is written.
///
/// One spelling for both places it lands: `jobs/<job-id>/` while a build is in
/// progress or has failed, and inside the published package once it is sealed.
/// They are the same document at two moments, so they share a name.
pub(crate) const RUN_REPORT_NAME: &str = "run-report.json";

/// Maximum serialized run-report size accepted before JSON decoding.
pub const MAX_RUN_REPORT_JSON_BYTES: usize = 16 * 1024 * 1024;

/// Version of the published run-report schema.
///
/// First drafted at `1.0`; moved to `2.0` before acceptance when E2-S4 added
/// the breaking per-segment, package-stage, typed-completion, and advisory-join
/// fields. The suffix follows `MANIFEST_SCHEMA_VERSION`: the layout remains
/// provisional even though the breaking change required a major increment.
///
/// `3.0` follows for accepted ADR-0002's waiver: `worker_bundle_hash`,
/// `hardware_environment_id`, and `resources.thread_budget` are required, and
/// §Change classes calls a required field a **Breaking contract**. The major
/// moves rather than folding into `2.0`, because `2.0` is published on `main`
/// and `E2-INTERFACE-CHANGE-001` settled that an unsigned version is kept out
/// of the charter's effective history without becoming reusable for a
/// different layout.
pub const RUN_REPORT_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(3, 0);

/// The `schema_version` a `run-report.json` this build writes carries.
pub const RUN_REPORT_LAYOUT_VERSION: &str = "3.0-skeleton";

/// The layout published before ADR-0002's three waiver facts were required.
///
/// Read, never written. A package sealed under it stays readable so
/// reconciliation can recover the workspace it sits in, and can never be
/// reused, because its report cannot answer what the waiver now requires.
/// `manifest::validate_run_report` is where both halves happen.
pub const LEGACY_RUN_REPORT_LAYOUT_VERSION: &str = "2.0-skeleton";

/// Microseconds in one millisecond, the scale a milli-ratio is expressed in.
pub(crate) const MICROSECONDS_PER_MILLISECOND: u64 = 1_000;

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
    /// One segment's own observation, combining nothing. Distinct from
    /// [`Aggregation::Total`]: the run's totals already sum these rows, so a
    /// reader who adds them again double-counts the build.
    Segment,
    /// One worker's own allowance, not a sum across the pool. Distinct from
    /// [`Aggregation::Total`]: multiplying by the process count gives the
    /// run's ceiling, so a reader who treats this as the total under-reports
    /// it whenever the pool grows past one.
    PerWorker,
    /// One reading taken at a stated moment, with no claim about any other.
    /// Unlike [`Aggregation::Maximum`], nothing guarantees the run never went
    /// higher: the kernel keeps a high-water mark for resident memory and none
    /// for open descriptors.
    PointInTime,
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
    /// Elapsed time from build entry through package production.
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
    /// Time inside the worker's synthesis call for one segment.
    SegmentSynthesisWallTime,
    /// Audio the worker generated for one segment.
    SegmentGeneratedAudio,
    /// Times one segment's synthesis was retried.
    SegmentRetries,
    /// Time the backend spent starting and loading its model.
    ModelLoadDuration,
    /// Time spent assembling segment audio into the master.
    AssemblyDuration,
    /// Time spent normalizing that master's loudness.
    NormalizeDuration,
    /// Time spent encoding both lossy outputs from that master.
    EncodeDuration,
    /// Worker processes this build was allowed to run at once.
    WorkerProcesses,
    /// Native threads each worker was allowed.
    NativeThreadsPerWorker,
    /// Interop threads each worker was allowed.
    InteropThreadsPerWorker,
}

/// Longest publishable hardware environment identifier, in bytes.
///
/// Generous against the one identifier that exists —
/// `reference-wsl2-d9d550f06b783405` is 32 bytes — and small enough that a
/// caller who reached for a path or a command line is refused rather than
/// truncated.
pub const MAX_HARDWARE_ENVIRONMENT_ID_BYTES: usize = 128;

/// The governed environment a build ran in, as a label.
///
/// **Configured provenance, not a machine attestation.** The value is supplied
/// by whoever launched the build; `docs/operations/REFERENCE-ENVIRONMENT.md` is
/// what proves a given label describes a real qualified machine. This type
/// proves only that the label is safe to publish.
///
/// That safety is the point. ADR-0002's waiver requires a hardware identity in
/// every run report, and the obvious way to produce one — a directory name, a
/// hostname, a path — is exactly what
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access keeps
/// out of published documents and what issue #82 spent a change removing. A
/// caller reaching for a path cannot construct one of these.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct HardwareEnvironmentId(String);

impl HardwareEnvironmentId {
    /// Accepts a bounded, path-free, printable label.
    ///
    /// # Errors
    ///
    /// [`MalformedHardwareEnvironmentId`] when the label is empty, longer than
    /// [`MAX_HARDWARE_ENVIRONMENT_ID_BYTES`], or carries anything but printable
    /// ASCII — which excludes whitespace — or carries a path separator. The
    /// remedy is the project owner's: name the governed environment record
    /// rather than the directory it happens to sit in.
    pub fn parse(label: &str) -> Result<Self, MalformedHardwareEnvironmentId> {
        let usable = !label.is_empty()
            && label.len() <= MAX_HARDWARE_ENVIRONMENT_ID_BYTES
            && label.bytes().all(|byte| byte.is_ascii_graphic())
            && !label.contains(['/', '\\']);
        if usable {
            return Ok(Self(label.to_owned()));
        }
        Err(MalformedHardwareEnvironmentId(label.to_owned()))
    }

    /// The label as it is written into a run report.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for HardwareEnvironmentId {
    type Error = MalformedHardwareEnvironmentId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<HardwareEnvironmentId> for String {
    fn from(value: HardwareEnvironmentId) -> Self {
        value.0
    }
}

impl schemars::JsonSchema for HardwareEnvironmentId {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "HardwareEnvironmentId".into()
    }

    /// The published pattern is the parser's rule, so a consumer validating
    /// against the schema alone refuses the same labels the type does rather
    /// than discovering the difference at parse time.
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Governed environment label: bounded printable ASCII with no \
                            whitespace and no path separator. Configured provenance, proved by \
                            docs/operations/REFERENCE-ENVIRONMENT.md rather than by this value.",
            // Printable ASCII without the space, the slash, or the
            // backslash — the parser's rule as a character class. The
            // absolute-end guard is required as it is on
            // `BLAKE3_HEX_PATTERN`: ECMAScript `$` also matches before a
            // trailing newline, so a label ending in one would pass.
                "pattern": r"^[!-.0-9:-\[\]-~]+$(?![\s\S])",
            "maxLength": MAX_HARDWARE_ENVIRONMENT_ID_BYTES,
        })
    }
}

/// A hardware environment label that cannot be published.
#[derive(Debug, Error)]
#[error(
    "hardware environment id `{0}` is not a bounded path-free printable label; name the governed \
 environment record rather than a directory, host, or path"
)]
pub struct MalformedHardwareEnvironmentId(String);

/// The worker thread allowance a build declared, in its three dimensions.
///
/// **Declared, never sampled**, which is why these are plain counts rather
/// than [`Measured`]: the build chose them before the worker started, so an
/// "unavailable" reading is not a state this can be in.
///
/// Three numbers rather than one, because one would hide which limit binds.
/// `worker/launcher.json` sets the native allowance, ADR-0001 §10.1 keeps pool
/// size at one until measured evidence authorizes more, and the worker pins its
/// own interop count — a single figure could not say which of those changed.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct WorkerThreadBudget {
    /// Worker processes the build was allowed to run at once.
    pub worker_processes_count: NonZeroU32,
    /// Native threads each worker was allowed, from `worker/launcher.json`.
    pub native_threads_per_worker_count: NonZeroU32,
    /// Interop threads each worker was allowed, pinned by the worker itself.
    pub interop_threads_per_worker_count: NonZeroU32,
}

/// Whether a worker thread allowance applies to this build at all.
///
/// An in-process backend has no worker process, so it has no worker thread
/// budget — and saying so is different from declaring 1/1/1, which would
/// publish an allowance nothing enforces. `Measured` is not the vehicle for
/// this: these are declared counts, and "not applicable" is not a failure to
/// observe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DeclaredThreadBudget {
    /// A worker process ran under this declared allowance.
    Worker(WorkerThreadBudget),
    /// The backend ran inside the supervisor, so no worker allowance applies.
    InProcess,
}

/// The environment one build ran in, as ADR-0002's waiver requires it kept.
///
/// Three facts the accepted waiver names separately — worker identity,
/// hardware identity, and the declared thread budget. They travel together
/// because they are read together, once, at the gate: reading them later would
/// let a mutable executor answer for an environment the build did not use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunEnvironment {
    /// Identity of the executable worker bundle behind this build.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Governed label of the machine it ran on.
    pub hardware_environment_id: HardwareEnvironmentId,
    /// Thread allowance the build declared, or that none applies.
    pub thread_budget: DeclaredThreadBudget,
}

#[cfg(test)]
impl RunEnvironment {
    /// A recognizable environment for tests that do not exercise one.
    ///
    /// Test-only: production builds take theirs from the gate, and a
    /// constructor that invented one would let a real report carry a label
    /// nobody configured.
    pub(crate) fn fixture() -> Self {
        Self {
            worker_bundle_hash: "1"
                .repeat(64)
                .try_into()
                .expect("a 64-character hexadecimal digest is a worker bundle hash"),
            hardware_environment_id: HardwareEnvironmentId::parse(
                "reference-wsl2-d9d550f06b783405",
            )
            .expect("the governed reference environment's identifier is publishable"),
            thread_budget: DeclaredThreadBudget::Worker(WorkerThreadBudget {
                worker_processes_count: NonZeroU32::MIN,
                native_threads_per_worker_count: NonZeroU32::new(4)
                    .expect("four is a non-zero thread allowance"),
                interop_threads_per_worker_count: NonZeroU32::MIN,
            }),
        }
    }
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
    pub const ALL: [Self; 21] = [
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
        Self::SegmentSynthesisWallTime,
        Self::SegmentGeneratedAudio,
        Self::SegmentRetries,
        Self::ModelLoadDuration,
        Self::AssemblyDuration,
        Self::NormalizeDuration,
        Self::EncodeDuration,
        Self::WorkerProcesses,
        Self::NativeThreadsPerWorker,
        Self::InteropThreadsPerWorker,
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
            Self::SegmentSynthesisWallTime => 11,
            Self::SegmentGeneratedAudio => 12,
            Self::SegmentRetries => 13,
            Self::ModelLoadDuration => 14,
            Self::AssemblyDuration => 15,
            Self::NormalizeDuration => 16,
            Self::EncodeDuration => 17,
            Self::WorkerProcesses => 18,
            Self::NativeThreadsPerWorker => 19,
            Self::InteropThreadsPerWorker => 20,
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
            // `/proc/<pid>/fd` lists what is open at the instant it is read
            // and the kernel keeps no high-water mark for descriptors, so this
            // is one reading and not a run-wide figure of any kind.
            Self::OpenHandles => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::ProcDescriptors,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::PointInTime,
                fidelity: Fidelity::Approximate,
            },
            Self::SegmentSynthesisWallTime => FieldSemantics {
                aggregation: Aggregation::Segment,
                ..elapsed
            },
            Self::SegmentGeneratedAudio => FieldSemantics {
                aggregation: Aggregation::Segment,
                ..audio
            },
            // E5-S3 owns retry, timeout, and lifecycle. No code path retries a
            // segment yet, so zero is this build's shape rather than an
            // observation, and the clock says so — the same reading
            // `ReportField::WorkerRestarts` carries for the same reason.
            Self::SegmentRetries => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::Structural,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::Segment,
                fidelity: Fidelity::Exact,
            },
            // Deliberately not inside any real-time factor: ADR-0001 §3.4
            // excludes one-time installation and model download from the
            // ratio, so this is the figure that explains a slow build without
            // moving the number the budget is written against.
            Self::ModelLoadDuration => elapsed,
            // Supervisor, not worker: assembly is this binary's own PCM work
            // and encoding is FFmpeg running under it. Reading either against
            // `docs/perf/BUDGETS.md`'s worker figures would compare two
            // different machines' worth of work.
            Self::AssemblyDuration | Self::NormalizeDuration | Self::EncodeDuration => {
                FieldSemantics {
                    measured_process: MeasuredProcess::Supervisor,
                    ..elapsed
                }
            }
            // Declared before the worker started rather than read from it, so
            // the clock is structural and the fidelity exact: these are the
            // numbers the build *chose*, and a build cannot be wrong about its
            // own configuration the way it can be wrong about a sample.
            Self::WorkerProcesses => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::Structural,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::Total,
                fidelity: Fidelity::Exact,
            },
            Self::NativeThreadsPerWorker | Self::InteropThreadsPerWorker => FieldSemantics {
                unit: MeasurementUnit::Count,
                clock: MeasurementClock::Structural,
                measured_process: MeasuredProcess::Worker,
                aggregation: Aggregation::PerWorker,
                fidelity: Fidelity::Exact,
            },
        }
    }
}

/// The `2.0-skeleton` report, frozen, for reading a package this build did not
/// write.
///
/// Every field the superseded layout carried, so the boundary refuses an
/// unknown one exactly as `StoredManifestV2` and `LegacyStoredManifest` do.
/// A superseded layout is still a format this project defines, and
/// `rust-review` §Types, traits, coherence allows one lenient boundary in this
/// crate — `export::ProbeResponse`, for tool output — and no other.
///
/// Only `join_run_report`'s inputs are read from here. The rest is modelled to
/// be *refused if absent or unknown*, not to be used, which is the difference
/// between a frozen decoder and a lenient one.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct LegacyRunReport {
    /// Layout label, already matched before this shape was selected.
    pub(crate) schema_version: String,
    /// Job this build belonged to.
    pub(crate) job_id: String,
    /// Attempt within that job.
    pub(crate) build_attempt: u32,
    /// Lesson it rendered.
    pub(crate) lesson_id: String,
    /// Plan it rendered from.
    pub(crate) plan_hash: String,
    /// Whether it finished.
    pub(crate) completion: ReportCompletion,
    /// Elapsed time from build entry through package production.
    pub(crate) wall_micros: u64,
    /// Time the backend spent starting and loading its model.
    pub(crate) model_load_micros: Measured,
    /// Time spent assembling segment audio into the master.
    pub(crate) assembly_micros: Measured,
    /// Time spent normalizing that master's loudness.
    pub(crate) normalize_micros: Measured,
    /// Time spent encoding both lossy outputs.
    pub(crate) encode_micros: Measured,
    /// Per-segment rows, unchanged by the `3.0` move.
    pub(crate) segments: Vec<RunReportSegment>,
    /// Advisory join findings, unchanged by the `3.0` move.
    pub(crate) join_findings: Vec<JoinFinding>,
    /// Run totals, unchanged by the `3.0` move.
    pub(crate) synthesis: SynthesisTotals,
    /// Process figures, which the `3.0` move gave a fourth member.
    pub(crate) resources: LegacyRunResources,
}

/// The `2.0-skeleton` resources block: the three members before the budget.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct LegacyRunResources {
    /// High-water resident memory of the worker.
    pub(crate) peak_resident_kib: Measured,
    /// Open file descriptors held by the worker.
    pub(crate) open_handles_count: Measured,
    /// Times the worker was restarted.
    pub(crate) worker_restarts_count: u32,
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
    /// The work was reused rather than performed: a segment came from the
    /// cache and no synthesis ran, or a package an earlier build produced was
    /// selected and nothing was assembled or encoded. Distinct from a duration
    /// of zero, which would claim the work happened instantly.
    ReusedFromCache,
    /// The executor ran synthesis with no separate worker process, so there
    /// was nothing to sample. Distinct from
    /// [`Unavailable::NotExposedByEnvironment`], which says the platform
    /// withheld a counter: here the platform would have supplied one and no
    /// process existed to ask about.
    NoWorkerProcess,
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

/// Whether this build's worker produced a segment or reused one.
///
/// `DELIVERY-PLAN.md` E2-S4 task 2 requires the outcome per segment. It is
/// read from whether the cache ran the producer closure, which is the only
/// place the distinction survives: [`crate::CachePublisher::resolve`] returns
/// the same validated artifact either way, deliberately, so that reused audio
/// and fresh audio are indistinguishable to everything downstream of it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CacheOutcome {
    /// The worker synthesized this segment during this build.
    Synthesized,
    /// A published cache entry supplied it and no synthesis ran.
    Reused,
    /// Cache resolution or synthesis failed before audio was available.
    Failed,
}

/// What one planned segment cost this build.
///
/// The five facts `DELIVERY-PLAN.md` E2-S4 task 2 names. Built from planned
/// identities and the resolved cache artifact, never from the validated
/// lesson: `spoken_text` and `display_text` sit on `PlannedSegment` beside the
/// `id` this carries, and the guarantee that neither reaches a published
/// document is that this type has nowhere to put them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct RunReportSegment {
    /// Identity of the segment within its lesson.
    pub segment_id: String,
    /// Take this build rendered.
    pub take: u32,
    /// Whether the worker produced this segment or the cache supplied it.
    pub cache_outcome: CacheOutcome,
    /// Times this segment's synthesis was retried.
    pub retry_count: u32,
    /// Time inside the worker's synthesis call, absent for a reused segment.
    pub synthesis_wall_micros: Measured,
    /// Audio this segment supplied, excluding the pause written after it.
    pub audio_frames: Measured,
}

/// One provisional join-discontinuity finding requiring human review.
///
/// The ratios remain in the manifest. This report carries only the segment
/// pair whose measured continuity is outside the provisional band, so it does
/// not publish a second copy of the measurement or promote that band to a
/// production threshold.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct JoinFinding {
    /// Segment ending at the join.
    pub earlier_segment_id: String,
    /// Segment beginning at the join.
    pub later_segment_id: String,
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
    /// Thread allowance this build declared, in its three dimensions.
    ///
    /// The third fact accepted ADR-0002's waiver retains. Declared rather than
    /// sampled, so unlike its neighbours here it is not [`Measured`].
    pub thread_budget: DeclaredThreadBudget,
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

/// Whether a report describes a build that finished.
///
/// A reader cannot otherwise tell a report sealed into a published package
/// from one written where a build stopped, and the two carry the same fields
/// with very different meanings: an incomplete report's totals cover the work
/// that happened before the failure, not the work the lesson asked for.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ReportCompletion {
    /// The build finished and this report was sealed into its package.
    Complete,
    /// The build stopped, and this is what it had measured.
    Incomplete {
        /// Closed class of the failure that ended the build.
        error_class: BuildErrorClass,
    },
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
    /// Identity of the executable worker bundle behind this build.
    ///
    /// Required by accepted ADR-0002's waiver, which retains "worker identity"
    /// in every run report until it expires. Separate from
    /// [`RunReport::hardware_environment_id`] because that sentence names the
    /// two independently, and a build can change one without the other.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Governed label of the machine this build ran on.
    ///
    /// The other half of the same waiver sentence. See
    /// [`HardwareEnvironmentId`] for why it is a label rather than a probe.
    pub hardware_environment_id: HardwareEnvironmentId,
    /// Whether the build finished, with a required class only when it failed.
    pub completion: ReportCompletion,
    /// Elapsed time from build entry through package production, sampled
    /// immediately before this report is sealed.
    pub wall_micros: u64,
    /// Time the backend spent starting and loading its model, where the
    /// backend has a process that can report it.
    pub model_load_micros: Measured,
    /// Time spent assembling segment audio into the master, absent when this
    /// build selected a package an earlier one produced.
    pub assembly_micros: Measured,
    /// Time spent normalizing that master's loudness, absent for the same
    /// reason.
    pub normalize_micros: Measured,
    /// Time spent encoding both lossy outputs, absent for the same reason.
    pub encode_micros: Measured,
    /// One row per planned segment, in the order the plan renders them.
    #[schemars(length(max = MAX_LESSON_SEGMENTS))]
    pub segments: Vec<RunReportSegment>,
    /// Provisional join discontinuities requiring human review.
    #[schemars(length(max = MAX_LESSON_SEGMENTS))]
    pub join_findings: Vec<JoinFinding>,
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

/// Rolls one build's segment rows up into the totals the document publishes.
///
/// Only the segments this build's worker produced contribute. A reused segment
/// is real audio in the master but no work by this run, so counting it would
/// report a real-time factor for synthesis that never happened — and the
/// second build of any lesson is all reuse, which is where that error would
/// have lived unnoticed.
#[must_use]
pub(crate) fn synthesis_totals(segments: &[RunReportSegment]) -> SynthesisTotals {
    let mut synthesized_count = 0_u32;
    let mut wall_micros = 0_u64;
    let mut audio_frames = 0_u64;
    let mut shortest: Option<u64> = None;
    let mut longest: Option<u64> = None;
    let mut worst: Option<WorstSegment> = None;

    for segment in segments {
        if segment.cache_outcome != CacheOutcome::Synthesized {
            continue;
        }
        let Measured::Observed { value: micros } = segment.synthesis_wall_micros else {
            continue;
        };
        let Measured::Observed { value: frames } = segment.audio_frames else {
            continue;
        };

        synthesized_count = synthesized_count.saturating_add(1);
        wall_micros = wall_micros.saturating_add(micros);
        audio_frames = audio_frames.saturating_add(frames);
        shortest = Some(shortest.map_or(frames, |held| held.min(frames)));
        longest = Some(longest.map_or(frames, |held| held.max(frames)));

        let Measured::Observed { value: ratio } = milli_real_time_factor(micros, frames) else {
            continue;
        };
        if worst
            .as_ref()
            .is_none_or(|held| ratio > held.real_time_factor_milli)
        {
            worst = Some(WorstSegment {
                segment_id: segment.segment_id.clone(),
                take: segment.take,
                wall_micros: micros,
                audio_frames: frames,
                real_time_factor_milli: ratio,
            });
        }
    }

    SynthesisTotals {
        segments_synthesized_count: synthesized_count,
        wall_micros,
        audio_frames,
        aggregate_real_time_factor_milli: milli_real_time_factor(wall_micros, audio_frames),
        segment_audio_minimum_frames: generated_or_absent(shortest),
        segment_audio_maximum_frames: generated_or_absent(longest),
        worst_segment: worst,
    }
}

/// An extreme over the segments the worker produced, or the reason there is
/// none.
///
/// A build that reused everything generated no audio, which is a different
/// statement from a shortest segment of zero frames.
fn generated_or_absent(frames: Option<u64>) -> Measured {
    frames.map_or(
        Measured::Unavailable {
            reason: Unavailable::NoAudioGenerated,
        },
        |value| Measured::Observed { value },
    )
}

impl RunReport {
    /// A report for a build that has measured nothing yet.
    ///
    /// Every measurement is absent with a reason rather than zero, because a
    /// build that has not run has observed nothing — which is a different
    /// statement from observing none. The package writer takes a report to
    /// seal, so this is what a caller hands it before any stage has run.
    #[must_use]
    pub fn unmeasured(
        job_id: &str,
        lesson_id: &str,
        plan_hash: &str,
        build_attempt: u32,
        completion: ReportCompletion,
        environment: &RunEnvironment,
    ) -> Self {
        let absent = Measured::Unavailable {
            reason: Unavailable::StageNotReached,
        };
        Self {
            schema_version: RunReportLayout::current(),
            job_id: job_id.to_owned(),
            build_attempt,
            lesson_id: lesson_id.to_owned(),
            plan_hash: plan_hash.to_owned(),
            worker_bundle_hash: environment.worker_bundle_hash.clone(),
            hardware_environment_id: environment.hardware_environment_id.clone(),
            completion,
            wall_micros: 0,
            model_load_micros: absent,
            assembly_micros: absent,
            normalize_micros: absent,
            encode_micros: absent,
            segments: Vec::new(),
            join_findings: Vec::new(),
            synthesis: synthesis_totals(&[]),
            resources: RunResources {
                peak_resident_kib: absent,
                open_handles_count: absent,
                worker_restarts_count: 0,
                thread_budget: environment.thread_budget,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0002's waiver retains three facts, and this report keeps them apart.
    ///
    /// The accepted sentence names "thread-budget, **worker identity, and
    /// hardware identity**" as three things. `ADR-0001-D002` summarizes them as
    /// one "environment identity", but it is approved *through* ADR-0002 and
    /// cannot merge what the controlling decision separates.
    ///
    /// Rejects the wrong implementation this story nearly shipped: one
    /// composite identity carrying the worker bundle hash, with the hardware
    /// half deferred to a follow-up. That satisfies the summary and leaves an
    /// accepted obligation false, so the assertion is that the two identities
    /// are present, distinct, and separately named.
    #[test]
    fn t1_e2_run_report_retains_distinct_waiver_identities_and_thread_budget() {
        let report = RunReport::unmeasured(
            "job",
            "lesson",
            &"0".repeat(64),
            1,
            ReportCompletion::Complete,
            &RunEnvironment::fixture(),
        );
        let document = serde_json::to_value(&report).expect("a run report serializes");

        assert_eq!(
            document["worker_bundle_hash"],
            serde_json::Value::from("1".repeat(64)),
            "the worker identity must be published under its own name"
        );
        assert_eq!(
            document["hardware_environment_id"],
            serde_json::Value::from("reference-wsl2-d9d550f06b783405"),
            "the hardware identity must be published under its own name"
        );
        assert_ne!(
            document["worker_bundle_hash"], document["hardware_environment_id"],
            "one value standing for both identities is the reading ADR-0002 forbids"
        );

        let budget = &document["resources"]["thread_budget"]["worker"];
        assert_eq!(budget["worker_processes_count"], 1);
        assert_eq!(budget["native_threads_per_worker_count"], 4);
        assert_eq!(budget["interop_threads_per_worker_count"], 1);
    }

    /// A hardware identity is a label, and only a safe one may be published.
    ///
    /// `RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access keeps host paths
    /// out of published documents and issue #82 closed the last one. The type
    /// is what keeps this field from becoming the next: a caller who reaches
    /// for a directory name cannot construct one.
    #[test]
    fn t1_e2_a_hardware_environment_id_refuses_a_path_or_an_unbounded_label() {
        HardwareEnvironmentId::parse("reference-wsl2-d9d550f06b783405")
            .expect("the governed reference environment's own identifier is accepted");

        let oversized = "x".repeat(MAX_HARDWARE_ENVIRONMENT_ID_BYTES + 1);
        let refused: [&str; 7] = [
            "",
            " ",
            "has space",
            "relative/path",
            "windows\\path",
            "/absolute/path",
            &oversized,
        ];
        for candidate in refused {
            assert!(
                HardwareEnvironmentId::parse(candidate).is_err(),
                "`{candidate}` must not be publishable as a hardware identity"
            );
        }
    }

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
            worker_bundle_hash: _,
            hardware_environment_id: _,
            completion: _,
            wall_micros: _,
            model_load_micros: _,
            assembly_micros: _,
            normalize_micros: _,
            encode_micros: _,
            segments,
            join_findings: _,
            synthesis,
            resources,
        } = report;
        for segment in segments {
            let RunReportSegment {
                segment_id: _,
                take: _,
                cache_outcome: _,
                retry_count: _,
                synthesis_wall_micros: _,
                audio_frames: _,
            } = segment;
        }
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
            thread_budget,
        } = resources;
        match thread_budget {
            DeclaredThreadBudget::InProcess => {}
            DeclaredThreadBudget::Worker(WorkerThreadBudget {
                worker_processes_count: _,
                native_threads_per_worker_count: _,
                interop_threads_per_worker_count: _,
            }) => {}
        }

        ReportField::ALL.to_vec()
    }

    fn unmeasured_report() -> RunReport {
        RunReport {
            schema_version: RunReportLayout::current(),
            job_id: "job".to_owned(),
            build_attempt: 1,
            lesson_id: "lesson".to_owned(),
            plan_hash: "0".repeat(64),
            worker_bundle_hash: RunEnvironment::fixture().worker_bundle_hash,
            hardware_environment_id: RunEnvironment::fixture().hardware_environment_id,
            completion: ReportCompletion::Incomplete {
                error_class: BuildErrorClass::Io,
            },
            wall_micros: 0,
            model_load_micros: Measured::Unavailable {
                reason: Unavailable::NoWorkerProcess,
            },
            assembly_micros: Measured::Unavailable {
                reason: Unavailable::StageNotReached,
            },
            normalize_micros: Measured::Unavailable {
                reason: Unavailable::StageNotReached,
            },
            encode_micros: Measured::Unavailable {
                reason: Unavailable::StageNotReached,
            },
            // A build that reached no stage synthesized no segment, so the
            // rows are empty rather than fabricated.
            segments: Vec::new(),
            join_findings: Vec::new(),
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
                thread_budget: RunEnvironment::fixture().thread_budget,
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
    fn t1_e2_completion_and_error_class_are_one_closed_state() {
        let mut complete = serde_json::to_value(RunReport::unmeasured(
            "job",
            "lesson",
            &"0".repeat(64),
            1,
            ReportCompletion::Complete,
            &RunEnvironment::fixture(),
        ))
        .expect("serialize complete report");
        complete["error_class"] = serde_json::json!("io");
        assert!(
            serde_json::from_value::<RunReport>(complete).is_err(),
            "a complete report cannot carry an independent error class"
        );

        let mut incomplete =
            serde_json::to_value(unmeasured_report()).expect("serialize incomplete report");
        incomplete["completion"] = serde_json::json!({"incomplete": {}});
        assert!(
            serde_json::from_value::<RunReport>(incomplete.clone()).is_err(),
            "an incomplete report must carry an error class"
        );
        incomplete["completion"] = serde_json::json!({"incomplete": {"error_class": "invented"}});
        assert!(
            serde_json::from_value::<RunReport>(incomplete).is_err(),
            "an incomplete report cannot invent an error class"
        );
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
