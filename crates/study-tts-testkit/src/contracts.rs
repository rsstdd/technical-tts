//! Observable fakes, recording adapters, and reusable E0 contract scenarios.
//!
//! These adapters expose calls without weakening the safety invariants owned
//! by production ports. The E0-S4 inventory and G1 parity requirement live in
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md`.

use std::{
    collections::BTreeMap,
    fmt, fs,
    future::Future,
    path::{Path, PathBuf},
    pin::pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Wake, Waker},
    thread,
};

use study_tts_core::{JobDocument, JobState, ManifestDigest, RenderPlan, SelectedPackageIdentity};
use study_tts_runtime::{
    BackendDescriptor, BackendError, BuildError, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, FileSystemJobRepository, IoError, JobOwnership, JobRepository,
    PackagePreflightRequest, PackagePrepareRequest, PackagePublication, PackageWriteRequest,
    PackageWriter, PreparedPackageWriter, StagedAudioProducer, SynthesisReport, SynthesisRequest,
    TtsExecutor, ValidatedCachedArtifact, WorkerConfiguration, WorkerTtsExecutor,
};

/// Thread-safe ordered observations shared by recording seam adapters.
#[derive(Clone, Debug, Default)]
pub struct SeamEventLog {
    events: Arc<Mutex<Vec<String>>>,
}

impl SeamEventLog {
    /// Returns recorded event labels in call order.
    pub fn events(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(event.into());
    }
}

/// Executor adapter that records calls before delegating unchanged.
#[derive(Debug)]
pub struct RecordingTtsExecutor<E> {
    inner: E,
    events: SeamEventLog,
}

impl<E> RecordingTtsExecutor<E> {
    /// Wraps `inner` with ordered seam observations.
    pub fn new(inner: E, events: SeamEventLog) -> Self {
        Self { inner, events }
    }

    /// Returns the wrapped executor for implementation-specific observations.
    pub fn inner(&self) -> &E {
        &self.inner
    }
}

impl<E: TtsExecutor> TtsExecutor for RecordingTtsExecutor<E> {
    fn descriptor(&self) -> BackendDescriptor {
        self.events.record("executor.descriptor");
        self.inner.descriptor()
    }

    fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    fn validate(&self, request: &SynthesisRequest) -> Result<(), BackendError> {
        self.events
            .record(format!("executor.validate:{}", request.segment_id));
        self.inner.validate(request)
    }

    fn synthesize<'a>(
        &'a self,
        request: SynthesisRequest,
        destination: &'a Path,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<SynthesisReport, BackendError>> + Send + 'a>>
    {
        self.events
            .record(format!("executor.synthesize:{}", request.segment_id));
        self.inner.synthesize(request, destination)
    }
}

/// Cache adapter that records calls before delegating unchanged.
#[derive(Debug)]
pub struct RecordingCachePublisher<P> {
    inner: P,
    events: SeamEventLog,
}

impl<P> RecordingCachePublisher<P> {
    /// Wraps `inner` with ordered seam observations.
    pub fn new(inner: P, events: SeamEventLog) -> Self {
        Self { inner, events }
    }
}

impl<P: CachePublisher> CachePublisher for RecordingCachePublisher<P> {
    fn resolve(
        &self,
        request: &CacheResolveRequest,
        producer: &mut dyn StagedAudioProducer,
    ) -> Result<ValidatedCachedArtifact, BuildError> {
        self.events
            .record(format!("cache.resolve:{}", request.segment.id));
        self.inner.resolve(request, producer)
    }
}

/// Package adapter that records calls before delegating unchanged.
#[derive(Debug)]
pub struct RecordingPackageWriter<P> {
    inner: P,
    events: SeamEventLog,
}

impl<P> RecordingPackageWriter<P> {
    /// Wraps `inner` with ordered seam observations.
    pub fn new(inner: P, events: SeamEventLog) -> Self {
        Self { inner, events }
    }
}

impl<P: PackageWriter> PackageWriter for RecordingPackageWriter<P> {
    fn preflight(
        &self,
        request: &PackagePreflightRequest<'_>,
    ) -> Result<Box<dyn PreparedPackageWriter>, BuildError> {
        self.events.record("package.preflight");
        Ok(Box::new(RecordingPreparedPackageWriter {
            inner: self.inner.preflight(request)?,
            events: self.events.clone(),
        }))
    }
}

#[derive(Debug)]
struct RecordingPreparedPackageWriter {
    inner: Box<dyn PreparedPackageWriter>,
    events: SeamEventLog,
}

impl PreparedPackageWriter for RecordingPreparedPackageWriter {
    fn prepare(&self, request: &PackagePrepareRequest<'_>) -> Result<(), BuildError> {
        self.events.record("package.prepare");
        self.inner.prepare(request)
    }

    fn write(&self, request: &PackageWriteRequest<'_>) -> Result<PackagePublication, BuildError> {
        self.events.record("package.write");
        self.inner.write(request)
    }
}

/// Job repository that records calls before delegating unchanged.
#[derive(Debug)]
pub struct RecordingJobRepository<R> {
    inner: R,
    events: SeamEventLog,
}

impl<R> RecordingJobRepository<R> {
    /// Wraps `inner` with ordered seam observations.
    pub fn new(inner: R, events: SeamEventLog) -> Self {
        Self { inner, events }
    }
}

impl<R: JobRepository> JobRepository for RecordingJobRepository<R> {
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        self.events.record("job.claim");
        self.inner.claim(workspace, job_id)
    }

    fn load(&self, workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError> {
        self.events.record("job.load");
        self.inner.load(workspace, job_id)
    }

    fn replace(&self, workspace: &Path, document: &JobDocument) -> Result<(), BuildError> {
        self.events
            .record(format!("job.replace:{:?}", document.state));
        self.inner.replace(workspace, document)
    }

    fn retain_inputs(
        &self,
        workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError> {
        self.events.record("job.retain_inputs");
        self.inner.retain_inputs(workspace, job_id, lesson, plan)
    }

    fn retained_lesson(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        self.events.record("job.retained_lesson");
        self.inner.retained_lesson(workspace, job_id)
    }

    fn retained_plan(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError> {
        self.events.record("job.retained_plan");
        self.inner.retained_plan(workspace, job_id)
    }

    fn validate_preview_selection(
        &self,
        workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError> {
        self.events.record("job.validate_preview_selection");
        self.inner.validate_preview_selection(workspace, document)
    }
}

/// Interrupts a real build at one durable job-state write.
///
/// Delegates everything to [`FileSystemJobRepository`] and fails the first
/// `replace` that would record the chosen state, before it happens. That
/// leaves the workspace exactly as a process killed at that moment would:
/// every artifact the earlier stages published is on disk, and `job.json`
/// never advanced past the previous state. That is the on-disk shape ADR-0001
/// §12.7 step 4 reconciles, and it is reachable through the public
/// `PreviewServiceBundle` seam with no filesystem plumbing.
#[derive(Debug)]
pub struct InterruptingJobRepository {
    inner: FileSystemJobRepository,
    fail_before: JobState,
    interrupted: AtomicUsize,
}

impl InterruptingJobRepository {
    /// Fails the first `replace` whose document is in `state`.
    pub fn failing_before(state: JobState) -> Self {
        Self {
            inner: FileSystemJobRepository,
            fail_before: state,
            interrupted: AtomicUsize::new(0),
        }
    }

    /// How many times the injected interruption fired.
    pub fn interruptions(&self) -> usize {
        self.interrupted.load(Ordering::SeqCst)
    }
}

impl JobRepository for InterruptingJobRepository {
    fn claim(&self, workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        self.inner.claim(workspace, job_id)
    }

    fn load(&self, workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError> {
        self.inner.load(workspace, job_id)
    }

    fn replace(&self, workspace: &Path, document: &JobDocument) -> Result<(), BuildError> {
        if document.state == self.fail_before
            && self.interrupted.fetch_add(1, Ordering::SeqCst) == 0
        {
            return Err(file_error(
                workspace,
                std::io::Error::other("injected interruption before the job state advanced"),
            ));
        }
        self.inner.replace(workspace, document)
    }

    fn retain_inputs(
        &self,
        workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError> {
        self.inner.retain_inputs(workspace, job_id, lesson, plan)
    }

    fn retained_lesson(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        self.inner.retained_lesson(workspace, job_id)
    }

    fn retained_plan(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError> {
        self.inner.retained_plan(workspace, job_id)
    }

    fn validate_preview_selection(
        &self,
        workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError> {
        self.inner.validate_preview_selection(workspace, document)
    }
}

/// Records requests while delegating cache validation and publication.
#[derive(Debug, Default)]
pub struct FakeCachePublisher {
    requests: Mutex<Vec<CacheResolveRequest>>,
}

impl FakeCachePublisher {
    /// Returns cache requests in call order.
    pub fn requests(&self) -> Vec<CacheResolveRequest> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl CachePublisher for FakeCachePublisher {
    fn resolve(
        &self,
        request: &CacheResolveRequest,
        producer: &mut dyn StagedAudioProducer,
    ) -> Result<ValidatedCachedArtifact, BuildError> {
        self.requests
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(request.clone());
        FileSystemCachePublisher.resolve(request, producer)
    }
}

/// Calls observed by [`FakePackageWriter`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FakePackageCall {
    /// Package dependencies were accepted without invoking external tools.
    Preflight,
    #[allow(missing_docs)]
    Prepare,
    #[allow(missing_docs)]
    Write,
}

/// Deterministic package fake with immutable plan-hash selection.
#[derive(Clone, Debug)]
pub struct FakePackageWriter {
    root: PathBuf,
    calls: Arc<Mutex<Vec<FakePackageCall>>>,
    selected: Arc<Mutex<BTreeMap<String, PackagePublication>>>,
}

impl FakePackageWriter {
    /// Creates a package fake beneath `root`.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            calls: Arc::new(Mutex::new(Vec::new())),
            selected: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Returns package calls in order.
    pub fn calls(&self) -> Vec<FakePackageCall> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl PackageWriter for FakePackageWriter {
    fn preflight(
        &self,
        _request: &PackagePreflightRequest<'_>,
    ) -> Result<Box<dyn PreparedPackageWriter>, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakePackageCall::Preflight);
        Ok(Box::new(self.clone()))
    }
}

impl PreparedPackageWriter for FakePackageWriter {
    fn prepare(&self, _request: &PackagePrepareRequest<'_>) -> Result<(), BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakePackageCall::Prepare);
        Ok(())
    }

    fn write(&self, request: &PackageWriteRequest<'_>) -> Result<PackagePublication, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakePackageCall::Write);
        let plan_hash = request.plan.plan_hash.as_str().to_owned();
        if let Some(publication) = self
            .selected
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&plan_hash)
            .cloned()
        {
            return Ok(publication);
        }

        let package_dir = self.root.join("packages").join(&plan_hash);
        fs::create_dir_all(&package_dir).map_err(|source| file_error(&package_dir, source))?;
        // Every artifact the real writer publishes, so a consumer written
        // against the fake cannot compile against a package the real path does
        // not produce.
        let artifact = |name: &str, contents: &str| -> Result<PathBuf, BuildError> {
            let path = package_dir.join(name);
            fs::write(&path, contents.as_bytes()).map_err(|source| file_error(&path, source))?;
            Ok(path)
        };
        let master_wav = artifact("lesson.wav", "fake master")?;
        let m4a = artifact("lesson.m4a", "fake m4a")?;
        let mp3 = artifact("lesson.mp3", "fake mp3")?;
        let transcript = artifact("transcript.txt", "fake transcript")?;
        let captions = artifact("transcript.vtt", "WEBVTT\n")?;
        let chapters = artifact("chapters.ffmetadata", ";FFMETADATA1\n")?;
        let manifest = package_dir.join("manifest.json");
        let publication_record = self.root.join("current.json");
        let manifest_bytes =
            format!("{{\"release_status\":\"private_preview\",\"plan_hash\":\"{plan_hash}\"}}");
        fs::write(&manifest, manifest_bytes.as_bytes())
            .map_err(|source| file_error(&manifest, source))?;
        let manifest_blake3 = ManifestDigest::from(blake3::hash(manifest_bytes.as_bytes()));
        fs::write(&publication_record, manifest_blake3.as_str().as_bytes())
            .map_err(|source| file_error(&publication_record, source))?;
        let publication = PackagePublication {
            package_dir,
            publication_record,
            master_wav,
            m4a,
            mp3,
            transcript,
            captions,
            chapters,
            manifest,
            identity: SelectedPackageIdentity {
                // The fake names the package after its manifest, as the real
                // writer does; the plan hash is this fake's map key, not an
                // identity a package carries.
                package_id: manifest_blake3.clone(),
                manifest_blake3,
            },
        };
        self.selected
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(plan_hash, publication.clone());
        Ok(publication)
    }
}

/// Calls observed by [`InMemoryJobRepository`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FakeJobCall {
    /// Exclusive ownership was claimed for the named job.
    Claim(String),
    /// Current state was loaded for the named job.
    Load(String),
    /// A document in the named state replaced the authoritative one.
    Replace(JobState),
    /// The lesson and plan were retained for the named job.
    RetainInputs(String),
    /// The retained lesson was read back for the named job.
    RetainedLesson(String),
    /// The retained plan was read back for the named job.
    RetainedPlan(String),
    /// Recorded and selected preview identities were compared.
    ValidatePreviewSelection(String),
}

/// In-memory job repository with complete replacement history.
#[derive(Debug, Default)]
pub struct InMemoryJobRepository {
    current: Mutex<BTreeMap<String, JobDocument>>,
    history: Mutex<Vec<JobDocument>>,
    retained: Mutex<BTreeMap<String, Vec<u8>>>,
    plans: Mutex<BTreeMap<String, RenderPlan>>,
    calls: Mutex<Vec<FakeJobCall>>,
}

impl InMemoryJobRepository {
    /// Returns repository calls in order.
    pub fn calls(&self) -> Vec<FakeJobCall> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Returns every document passed to [`JobRepository::replace`].
    pub fn documents(&self) -> Vec<JobDocument> {
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl JobRepository for InMemoryJobRepository {
    fn claim(&self, _workspace: &Path, job_id: &str) -> Result<Box<dyn JobOwnership>, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::Claim(job_id.to_owned()));
        Ok(Box::new(FakeJobOwnership))
    }

    fn load(&self, _workspace: &Path, job_id: &str) -> Result<Option<JobDocument>, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::Load(job_id.to_owned()));
        Ok(self
            .current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .cloned())
    }

    fn replace(&self, _workspace: &Path, document: &JobDocument) -> Result<(), BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::Replace(document.state));
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(document.clone());
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(document.job_id.clone(), document.clone());
        Ok(())
    }

    fn retain_inputs(
        &self,
        _workspace: &Path,
        job_id: &str,
        lesson: &[u8],
        plan: &RenderPlan,
    ) -> Result<(), BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::RetainInputs(job_id.to_owned()));
        self.retained
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(job_id.to_owned(), lesson.to_vec());
        self.plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(job_id.to_owned(), plan.clone());
        Ok(())
    }

    fn retained_lesson(
        &self,
        _workspace: &Path,
        job_id: &str,
    ) -> Result<Option<Vec<u8>>, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::RetainedLesson(job_id.to_owned()));
        Ok(self
            .retained
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .cloned())
    }

    fn retained_plan(
        &self,
        _workspace: &Path,
        job_id: &str,
    ) -> Result<Option<RenderPlan>, BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::RetainedPlan(job_id.to_owned()));
        Ok(self
            .plans
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .cloned())
    }

    fn validate_preview_selection(
        &self,
        _workspace: &Path,
        document: &JobDocument,
    ) -> Result<(), BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::ValidatePreviewSelection(
                document.job_id.clone(),
            ));
        Ok(())
    }
}

#[derive(Debug)]
struct FakeJobOwnership;

impl JobOwnership for FakeJobOwnership {}

/// Runs synthesis validation and rendering for fake and real executors.
///
/// # Errors
///
/// [`BackendError::InvalidRequest`] when validation fails,
/// [`BackendError::Destination`] for a staging write,
/// [`BackendError::Execution`] for inference failure,
/// [`BackendError::Timeout`] for a bounded deadline, or
/// [`BackendError::Protocol`] for a violated protocol invariant.
pub fn run_tts_executor_contract_scenario(
    executor: &dyn TtsExecutor,
    request: SynthesisRequest,
    destination: &Path,
) -> Result<SynthesisReport, BackendError> {
    executor.validate(&request)?;
    block_on(executor.synthesize(request, destination))
}

/// What one worker lifetime reported, for comparison against another's.
///
/// Named rather than a tuple so a reader of
/// [`run_worker_restart_contract_scenario`] can tell the two lifetimes apart.
#[derive(Debug)]
pub struct WorkerLifetimeOutcome {
    /// What the worker rendered, as it reported it.
    pub report: SynthesisReport,
    /// Everything the worker wrote to its standard error that lifetime.
    pub diagnostics: String,
}

/// Starts a worker, renders, shuts it down, and does all of it again.
///
/// ADR-0001 §17.7 asks a worker to survive being restarted, and nothing shared
/// between the fake and the real worker exercised it: both suites started one
/// worker, rendered once, and dropped it. Two lifetimes driven through one
/// function is what makes "restartable" a property rather than an assumption —
/// a worker that leaves a lock, a staged file, or a resident model behind
/// fails the second lifetime, and only the second.
///
/// The shutdown between them is the graceful one:
/// [`WorkerTtsExecutor::shutdown`] sends the protocol's `shutdown` frame and
/// gives the worker its grace period before the process group is killed. A
/// restart that only worked after a `SIGKILL` would not be the property this
/// scenario claims.
///
/// # Errors
///
/// [`BuildError::Synthesis`] when either worker cannot be started or
/// initialized, and everything
/// [`run_tts_executor_contract_scenario`] reports for either render.
pub fn run_worker_restart_contract_scenario(
    configuration: &WorkerConfiguration,
    request: &SynthesisRequest,
    first_destination: &Path,
    second_destination: &Path,
) -> Result<[WorkerLifetimeOutcome; 2], BuildError> {
    let mut lifetimes = Vec::with_capacity(2);
    for destination in [first_destination, second_destination] {
        let executor = WorkerTtsExecutor::start(configuration)?;
        let report = run_tts_executor_contract_scenario(&executor, request.clone(), destination)?;
        executor.shutdown()?;
        // Read after the shutdown, not before: the reader threads are joined by
        // it, so anything the worker wrote on its way out — including its
        // answer to the `shutdown` frame — is all there only once it returned.
        let diagnostics = executor.diagnostics();
        lifetimes.push(WorkerLifetimeOutcome {
            report,
            diagnostics,
        });
    }
    let [first, second] = lifetimes
        .try_into()
        .unwrap_or_else(|_| unreachable!("the loop above runs exactly twice"));
    Ok([first, second])
}

/// Runs cache miss then hit through the same cache adapter and producer.
///
/// # Errors
///
/// [`BuildError::ManagedPath`], [`BuildError::Cache`],
/// [`BuildError::Audio`], [`BuildError::DurableState`], [`BuildError::Io`], or
/// [`BuildError::Synthesis`] when either resolution fails.
pub fn run_cache_contract_scenario(
    cache: &dyn CachePublisher,
    request: &CacheResolveRequest,
    producer: &mut dyn StagedAudioProducer,
) -> Result<[ValidatedCachedArtifact; 2], BuildError> {
    let first = cache.resolve(request, producer)?;
    let second = cache.resolve(request, producer)?;
    Ok([first, second])
}

/// Runs repository ownership, replacement, and strict load as one scenario.
///
/// # Errors
///
/// [`BuildError::ManagedPath`] for an unsafe job path,
/// [`BuildError::DurableState`] for untrusted state or ownership, or
/// [`BuildError::Io`] for snapshot storage or a missing replacement.
pub fn run_job_repository_contract_scenario(
    repository: &dyn JobRepository,
    workspace: &Path,
    document: &JobDocument,
    lesson: &[u8],
    plan: &RenderPlan,
) -> Result<JobDocument, BuildError> {
    let _ownership = repository.claim(workspace, &document.job_id)?;
    repository.retain_inputs(workspace, &document.job_id, lesson, plan)?;
    if repository
        .retained_lesson(workspace, &document.job_id)?
        .as_deref()
        != Some(lesson)
    {
        return Err(file_error(
            workspace,
            std::io::Error::other("retained lesson did not load back unchanged"),
        ));
    }
    if repository
        .retained_plan(workspace, &document.job_id)?
        .is_none_or(|retained| retained.plan_hash != plan.plan_hash)
    {
        return Err(file_error(
            workspace,
            std::io::Error::other("retained plan did not load back unchanged"),
        ));
    }
    repository.validate_preview_selection(workspace, document)?;
    repository.replace(workspace, document)?;
    repository
        .load(workspace, &document.job_id)?
        .ok_or_else(|| {
            file_error(
                workspace,
                std::io::Error::other("snapshot was not retained"),
            )
        })
}

/// Runs package reconciliation, first publication, and immutable reuse.
///
/// # Errors
///
/// [`BuildError::Tool`] for preflight, [`BuildError::DurableState`] for
/// reconciliation or selection, [`BuildError::ManagedPath`] for containment,
/// [`BuildError::Cache`] for plan disagreement, [`BuildError::Audio`] for
/// assembly, or [`BuildError::Io`] for package storage.
pub fn run_package_writer_contract_scenario(
    writer: &dyn PackageWriter,
    preflight: &PackagePreflightRequest<'_>,
    prepare: &PackagePrepareRequest<'_>,
    write: &PackageWriteRequest<'_>,
) -> Result<[PackagePublication; 2], BuildError> {
    let writer = writer.preflight(preflight)?;
    writer.prepare(prepare)?;
    let first = writer.write(write)?;
    let second = writer.write(write)?;
    Ok([first, second])
}

fn block_on<F: Future>(future: F) -> F::Output {
    let parker = Arc::new(ThreadParker {
        thread: thread::current(),
    });
    let waker = Waker::from(parker);
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

#[derive(Debug)]
struct ThreadParker {
    thread: thread::Thread,
}

impl Wake for ThreadParker {
    fn wake(self: Arc<Self>) {
        self.thread.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.thread.unpark();
    }
}

fn file_error(path: &Path, source: std::io::Error) -> BuildError {
    IoError::FileSystem {
        path: path.to_path_buf(),
        source,
    }
    .into()
}

impl fmt::Display for FakePackageCall {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}
