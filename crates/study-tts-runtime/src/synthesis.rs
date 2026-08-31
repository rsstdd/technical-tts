//! The provisional asynchronous port a speech backend implements.
//!
//! The representation mirrors ADR-0001 §10.4 and the E0-S4 baseline in
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`. Backend fields live
//! here instead of in lesson or planning types, and `&self` permits the E1
//! executor to dispatch concurrent requests without changing this boundary.

use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::Path,
    pin::Pin,
};

use study_tts_core::{
    CANONICAL_CHANNELS, CANONICAL_SAMPLE_FORMAT, CANONICAL_SAMPLE_RATE, CacheKey, DeterminismClass,
    LanguageTag, Revision, SynthesisContext, VoiceConditioningHash, WorkerBundleHash,
};
use thiserror::Error;

/// Mirrors the executor version in the E0-S4 provisional contract baseline.
///
/// Raised to a major version by
/// `docs/architecture/E1-S1-INTERFACE-CHANGE-001.md`:
/// [`BackendDescriptor`] replaced one opaque `synthesis_identity` string with
/// the ADR-0001 §12.5 inputs, which is a required-field change and therefore
/// breaking under `ContractDescriptor::assess_successor`. Raised again by
/// `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md`, which made
/// [`SynthesisRequest::voice_conditioning_hash`] required for the same reason.
/// Raised again by `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md`:
/// [`SynthesisRequest::voice_profile`] became required so the worker receives
/// the profile identity its protocol asks for, and
/// [`SynthesisReport::voice_conditioning_hash`] replaced a reported profile
/// hash the worker cannot compute, which is what makes the cache's identity
/// gate evidence rather than a tautology.
pub const TTS_EXECUTOR_CONTRACT_VERSION: &str = "e1.tts-executor.3.0";

/// Stable identity and supported request envelope of one backend.
///
/// Every field but `contract_version` and `max_text_bytes` is a
/// speech-affecting input the backend contributes to each synthesis key. They
/// are named separately rather than folded into one identity string so that a
/// model upgrade, a tokenizer change, and a rebuilt worker are distinguishable
/// in a manifest instead of all reading as "the backend changed".
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDescriptor {
    /// Executor contract version implemented by this backend.
    pub contract_version: String,
    /// Identity of the executable worker bundle behind this backend.
    pub worker_bundle_hash: WorkerBundleHash,
    /// Model repository the backend loads from.
    pub model_repository: String,
    /// Pinned model revision.
    ///
    /// A [`Revision`] rather than a `String` because it reaches every cache key
    /// this backend's audio is stored under, and "never a moving tag" is a
    /// promise only a type can keep.
    pub model_revision: Revision,
    /// Tokenizer or codec revision the backend applies.
    pub tokenizer_revision: Revision,
    /// Languages this backend declares it speaks.
    ///
    /// A set rather than a list because declaring `en` twice says nothing more
    /// than declaring it once, and because the order a backend happens to
    /// enumerate its languages in is not part of what it supports.
    ///
    /// This is the boundary that answers "can this backend say these words".
    /// [`LanguageTag`] answers only whether the tag is well formed; a
    /// well-formed tag for a language the backend has no weights for is exactly
    /// the request that must be refused before synthesis, not after.
    pub languages: BTreeSet<LanguageTag>,
    /// Whether a fixed seed is expected to reproduce bytes for this backend.
    pub determinism_class: DeterminismClass,
    /// Seed the backend samples with.
    pub seed: u64,
    /// Backend generation parameters, by name, in their configured spelling.
    pub generation_parameters: BTreeMap<String, String>,
    /// Maximum UTF-8 bytes accepted as spoken text in one request.
    pub max_text_bytes: usize,
}

impl BackendDescriptor {
    /// The synthesis context this backend contributes to every cache key.
    ///
    /// The lesson supplies its language and the voice gate supplies the
    /// conditioning hashes, because neither is the backend's to decide; the
    /// backend owns the rest.
    pub fn synthesis_context(
        &self,
        language: LanguageTag,
        voice_conditioning_hashes: BTreeMap<String, VoiceConditioningHash>,
    ) -> SynthesisContext {
        SynthesisContext {
            worker_bundle_hash: self.worker_bundle_hash.clone(),
            model_repository: self.model_repository.clone(),
            model_revision: self.model_revision.clone(),
            tokenizer_revision: self.tokenizer_revision.clone(),
            language,
            determinism_class: self.determinism_class,
            seed: self.seed,
            generation_parameters: self.generation_parameters.clone(),
            voice_conditioning_hashes,
        }
    }

    /// Whether this backend declares it speaks `language`.
    ///
    /// A declared primary subtag covers its regional forms — a backend
    /// declaring `en` speaks `en-US` — but not the reverse: a backend that
    /// declared only `en-US` has not claimed `en-GB`, and treating it as though
    /// it had would ship the wrong accent under a key that says otherwise.
    fn speaks(&self, language: &LanguageTag) -> bool {
        self.languages.contains(language)
            || self
                .languages
                .iter()
                .any(|declared| declared.as_str() == language.primary())
    }
}

/// Backend-owned synthesis inputs derived from one planned segment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisRequest {
    /// Request identity unique within a worker lifetime.
    pub request_id: String,
    /// Segment identity used for diagnostics and staging ownership.
    pub segment_id: String,
    /// Exact reviewed text to speak.
    pub spoken_text: String,
    /// Speaker the plan selected, which is also a synthesis-key input.
    ///
    /// The speaker's *name*, not the profile identity: two speakers may share
    /// one voice profile and must not share a cache entry.
    /// [`SynthesisRequest::voice_profile`] carries the identity the worker
    /// protocol's `voice` field wants.
    pub voice: String,
    /// Voice profile the plan resolved that speaker to.
    ///
    /// What the worker is actually asked to load, and what the protocol's
    /// `synthesize` frame means by "voice profile identity". Carried rather
    /// than derived, because resolving a speaker to a profile needs the
    /// lesson's bindings and a worker has never seen a lesson.
    ///
    /// Not a synthesis-key input: ADR-0001 §12.5 keys on the conditioning
    /// artifact, which [`SynthesisRequest::voice_conditioning_hash`] carries.
    pub voice_profile: String,
    /// Conditioning artifact the resolved voice profile carries.
    ///
    /// Required rather than implied, because it is an ADR-0001 §12.5
    /// synthesis-key input that the *planner* resolved: an executor that
    /// reports a different one in [`SynthesisReport::context`] is refused by
    /// the cache's identity gate, and it can only make that comparison
    /// meaningful if it was told which artifact the key names.
    pub voice_conditioning_hash: VoiceConditioningHash,
    /// Delivery style selected by the plan.
    pub style: String,
    /// Language the segment is to be spoken in.
    ///
    /// Carried on the request rather than left implicit in the backend's
    /// configuration because it is a synthesis-key input (ADR-0001 §12.5):
    /// without it, two lessons in two languages would send byte-identical
    /// requests under two different keys, and the executor could not tell that
    /// it was being asked for something it does not speak.
    pub language: LanguageTag,
    /// Take of this segment the plan selects.
    ///
    /// Carried rather than left implicit at the worker protocol's boundary,
    /// which requires it on every `synthesize` frame, because it is a term of
    /// `cache_key` itself: two takes of one segment differ in
    /// nothing else, so an executor handed the key without the take is being
    /// asked to reproduce a distinction it cannot see.
    pub take: u32,
    /// Synthesis identity of the requested take.
    pub cache_key: CacheKey,
    /// Required output sample rate in hertz.
    pub sample_rate: u32,
    /// Required output channel count.
    pub channels: u16,
    /// Required output sample format.
    pub sample_format: String,
}

/// What an executor says it wrote, checked against the staged file itself.
///
/// Every field is a *claim* the cache verifies before publishing: the format
/// fields against the WAV on disk, and [`SynthesisReport::context`] against the
/// key the plan derived. An unverified report field would be a field that can
/// disagree with the artifact it describes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisReport {
    /// Sample rate the executor claims for the file, in hertz.
    pub sample_rate: u32,
    /// Channel count the executor claims for the file.
    pub channels: u16,
    /// Frame count the executor claims for the file.
    pub frames: u32,
    /// Backend revision that produced the audio.
    ///
    /// Diagnostic rather than an identity input: it names a build of the
    /// backend for a human reading a manifest, and ADR-0001 §12.5 keys audio on
    /// the model, tokenizer, and bundle identities in `context` instead.
    pub backend_revision: String,
    /// The speech-affecting inputs the executor actually used.
    ///
    /// Reported as one value rather than as loose fields so the cache can
    /// recompute the synthesis key from it and compare that against the key the
    /// plan derived. Comparing a whole identity catches a field that a
    /// field-by-field check would stop covering the moment somebody adds one.
    pub context: SynthesisContext,
    /// The conditioning artifact the executor's worker actually resolved.
    ///
    /// **The field the cache's identity gate rests on.** It is reported by the
    /// worker from the voice root the worker itself read, never echoed from the
    /// request, so a worker whose voice root disagrees with the planner's
    /// derives a different key and is refused before it can publish. An
    /// executor that echoed the requested value would satisfy every test in
    /// this workspace while leaving that gate doing nothing —
    /// `docs/architecture/E1-S2-INTERFACE-CHANGE-001.md` §Limits this change
    /// does not close records that as owed to E1-S3, and this field is how it
    /// is paid.
    ///
    /// Replaces a reported `VoiceProfileHash`, which no Python worker can
    /// produce: the worker environment has no BLAKE3 and
    /// `docs/architecture/E1-S3-INTERFACE-CHANGE-001.md` records why a
    /// dependency was not added for one.
    pub voice_conditioning_hash: VoiceConditioningHash,
    /// Voice profile the worker resolved that artifact through.
    ///
    /// Names the record a reviewer follows to a consent decision. Diagnostic
    /// rather than an identity input, like `backend_revision`.
    pub voice_profile: String,
}

/// A descriptor with every identity input populated, for tests in this crate.
///
/// Lives beside the definition so a new backend-owned identity input has to be
/// given a value here, rather than defaulting into every runtime test at once.
#[cfg(test)]
pub(crate) fn sample_descriptor() -> BackendDescriptor {
    BackendDescriptor {
        contract_version: TTS_EXECUTOR_CONTRACT_VERSION.to_owned(),
        worker_bundle_hash: "1".repeat(64).parse().expect("a digest of ones parses"),
        model_repository: "example/standard-chatterbox".to_owned(),
        model_revision: "0123456789abcdef0123456789abcdef01234567"
            .parse()
            .expect("a hex revision parses"),
        tokenizer_revision: "tokenizer-2026-01"
            .parse()
            .expect("a dated tokenizer revision parses"),
        languages: BTreeSet::from(["en".parse().expect("`en` is a well-formed language tag")]),
        determinism_class: DeterminismClass::SeededNondeterministic,
        seed: 42,
        generation_parameters: BTreeMap::from([("cfg_weight".to_owned(), "0.5".to_owned())]),
        max_text_bytes: 4096,
    }
}

/// A request invariant the executor can reject before synthesis begins.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BackendValidationError {
    /// The executor implements a different contract version.
    #[error("executor contract `{found}` is incompatible with required `{required}`")]
    IncompatibleContract {
        /// Contract version the executor reports.
        found: String,
        /// Contract version this build requires.
        required: &'static str,
    },
    /// Capacity zero cannot accept any request.
    #[error("executor capacity must be at least one")]
    ZeroCapacity,
    /// Request IDs are required for correlation and cancellation.
    #[error("synthesis request ID is empty")]
    EmptyRequestId,
    /// Segment IDs are required for diagnostics and containment.
    #[error("synthesis segment ID is empty")]
    EmptySegmentId,
    /// The backend does not declare the requested language.
    ///
    /// Refused before synthesis rather than after: a backend with no weights
    /// for a language does not fail, it produces confident nonsense, and that
    /// nonsense would be published under a key that names the language it does
    /// not speak.
    #[error(
        "executor does not speak `{requested}`; it declares {supported}, and the lesson owner \
         must set the lesson language to one of those or select a backend that speaks the \
         authored language"
    )]
    UnsupportedLanguage {
        /// Language the request asks for.
        requested: LanguageTag,
        /// Languages the backend declares, rendered for the message above.
        supported: String,
    },
    /// Spoken text exceeded the backend-declared request envelope.
    #[error("spoken text is {found} bytes but the executor accepts at most {maximum}")]
    TextTooLarge {
        /// Bytes in the requested text.
        found: usize,
        /// Backend-declared maximum.
        maximum: usize,
    },
    /// The backend does not declare the requested delivery style.
    ///
    /// Refused before synthesis for the reason an unspoken language is: a
    /// backend handed a style it has no parameters for does not fail, it
    /// renders the delivery it does have — and that take is published under a
    /// key naming the style nobody rendered.
    #[error(
        "executor does not declare the style `{requested}`; it declares {declared}, and the lesson \
         owner must set the segment style to one of those or select a backend that offers the \
         authored delivery"
    )]
    UndeclaredStyle {
        /// Style the request asks for.
        requested: String,
        /// Styles the backend declares, rendered for the message above.
        declared: String,
    },
    /// The backend has not loaded the requested voice profile.
    ///
    /// The sharpest case of the same rule: ADR-0001 §12.5 keys every entry on
    /// the conditioning artifact, so a take rendered with whatever voice the
    /// backend happened to hold would be filed under the artifact the plan
    /// resolved and never loaded.
    #[error(
        "executor has not loaded the voice profile `{requested}`; it holds {declared}, and the \
         project owner must attach the profile's governed root or bind the speaker to a profile \
         the worker holds"
    )]
    UndeclaredVoiceProfile {
        /// Voice profile the request asks for.
        requested: String,
        /// Voice profiles the backend declares, rendered for the message above.
        declared: String,
    },
    /// The requested intermediate format differs from the canonical format.
    #[error(
        "requested audio format is {sample_rate} Hz, {channels} channels, `{sample_format}`; the \
         provisional executor requires 24000 Hz, one channel, `f32le`"
    )]
    NonCanonicalFormat {
        /// Requested sample rate.
        sample_rate: u32,
        /// Requested channel count.
        channels: u16,
        /// Requested sample format.
        sample_format: String,
    },
}

/// Why an executor could not produce one staged audio artifact.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BackendError {
    /// Request validation failed before backend work began.
    #[error("synthesis request `{request_id}` is invalid: {source}")]
    InvalidRequest {
        /// Request the refusal belongs to.
        request_id: String,
        /// Exact violated request invariant.
        source: BackendValidationError,
    },
    /// The assigned destination was not writable by the backend.
    #[error("executor could not write request `{request_id}` to `{destination}`: {message}")]
    Destination {
        /// Request the failure belongs to.
        request_id: String,
        /// Assigned staging destination.
        destination: std::path::PathBuf,
        /// Backend diagnostic without source text.
        message: String,
    },
    /// Backend inference failed after accepting the request.
    #[error("executor failed request `{request_id}` with `{code}`: {message}")]
    Execution {
        /// Request the failure belongs to.
        request_id: String,
        /// Stable backend failure code.
        code: String,
        /// Redacted backend diagnostic.
        message: String,
    },
    /// Backend work exceeded its bounded deadline.
    #[error("executor timed out request `{request_id}` after {timeout_ms} ms")]
    Timeout {
        /// Request the timeout belongs to.
        request_id: String,
        /// Enforced deadline in milliseconds.
        timeout_ms: u64,
    },
    /// Executor or orchestration protocol invariants failed.
    #[error("executor protocol failed request `{request_id}`: {message}")]
    Protocol {
        /// Request the protocol failure belongs to.
        request_id: String,
        /// Redacted protocol diagnostic.
        message: String,
    },
    /// A success frame restated an identity the executor did not initialize
    /// with.
    #[error(
        "executor refused request `{request_id}`: the worker synthesized under a different \
         {identity} than it initialized with ({message})"
    )]
    IdentityDrift {
        /// Request the refusal belongs to.
        request_id: String,
        /// Which of the reported identities disagreed.
        identity: DriftedIdentity,
        /// The disagreement, as expected and found.
        message: String,
    },
}

/// An identity a synthesis success frame restates, and may disagree on.
///
/// A distinct variant per identity rather than one opaque refusal: each is a
/// separate ADR-0001 §12.5 key input, so a test asserting "the codec drifted"
/// must not pass for a build that only noticed the model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriftedIdentity {
    /// The worker bundle the audio was produced by.
    WorkerBundle,
    /// The speech model revision.
    Model,
    /// The tokenizer or codec revision.
    Codec,
    /// The voice profile the conditioning artifact was resolved through.
    VoiceProfile,
}

impl DriftedIdentity {
    /// How this identity is named in a refusal an operator reads.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkerBundle => "worker bundle identity",
            Self::Model => "model revision",
            Self::Codec => "codec revision",
            Self::VoiceProfile => "voice profile",
        }
    }
}

impl std::fmt::Display for DriftedIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Object-safe asynchronous speech executor from ADR-0001 §10.4.
pub trait TtsExecutor: Send + Sync {
    /// Returns stable backend identity and validation limits.
    fn descriptor(&self) -> BackendDescriptor;

    /// Returns the number of requests this executor may run concurrently.
    fn capacity(&self) -> usize;

    /// Refuses a request that this executor cannot honor without starting work.
    ///
    /// # Errors
    ///
    /// [`BackendError::InvalidRequest`] for the exact request invariant that
    /// failed.
    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError>;

    /// Renders one request to the assigned managed staging destination.
    ///
    /// # Errors
    ///
    /// [`BackendError::InvalidRequest`] when validation fails,
    /// [`BackendError::Destination`] for a destination write failure,
    /// [`BackendError::Execution`] for backend inference failure,
    /// [`BackendError::Timeout`] for a bounded deadline, or
    /// [`BackendError::Protocol`] when protocol interaction cannot be trusted.
    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
        destination: &'a Path,
    ) -> Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>>;
}

/// Applies the request invariants shared by every provisional executor.
///
/// # Errors
///
/// [`BackendValidationError::IncompatibleContract`],
/// [`BackendValidationError::ZeroCapacity`],
/// [`BackendValidationError::EmptyRequestId`],
/// [`BackendValidationError::EmptySegmentId`],
/// [`BackendValidationError::UnsupportedLanguage`],
/// [`BackendValidationError::TextTooLarge`], or
/// [`BackendValidationError::NonCanonicalFormat`] when the named invariant
/// does not hold.
pub fn validate_executor_request(
    descriptor: &BackendDescriptor,
    capacity: usize,
    request: &SynthesisRequest,
) -> Result<(), BackendValidationError> {
    if descriptor.contract_version != TTS_EXECUTOR_CONTRACT_VERSION {
        return Err(BackendValidationError::IncompatibleContract {
            found: descriptor.contract_version.clone(),
            required: TTS_EXECUTOR_CONTRACT_VERSION,
        });
    }
    if capacity == 0 {
        return Err(BackendValidationError::ZeroCapacity);
    }
    if request.request_id.is_empty() {
        return Err(BackendValidationError::EmptyRequestId);
    }
    if request.segment_id.is_empty() {
        return Err(BackendValidationError::EmptySegmentId);
    }
    if !descriptor.speaks(&request.language) {
        return Err(BackendValidationError::UnsupportedLanguage {
            requested: request.language.clone(),
            supported: render_languages(&descriptor.languages),
        });
    }
    if request.spoken_text.len() > descriptor.max_text_bytes {
        return Err(BackendValidationError::TextTooLarge {
            found: request.spoken_text.len(),
            maximum: descriptor.max_text_bytes,
        });
    }
    if request.sample_rate != CANONICAL_SAMPLE_RATE
        || request.channels != CANONICAL_CHANNELS
        || request.sample_format != CANONICAL_SAMPLE_FORMAT
    {
        return Err(BackendValidationError::NonCanonicalFormat {
            sample_rate: request.sample_rate,
            channels: request.channels,
            sample_format: request.sample_format.clone(),
        });
    }
    Ok(())
}

/// Renders declared languages for a refusal message.
///
/// A backend that declares none is worth saying out loud: the message would
/// otherwise read as an empty list and leave the reader guessing whether the
/// field was omitted or the backend really speaks nothing.
fn render_languages(languages: &BTreeSet<LanguageTag>) -> String {
    if languages.is_empty() {
        return "no languages at all".to_owned();
    }
    languages
        .iter()
        .map(LanguageTag::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}
