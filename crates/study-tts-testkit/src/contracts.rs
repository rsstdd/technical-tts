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
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
    thread,
};

use study_tts_core::{ProvisionalJobSnapshot, SelectedPackageIdentity};
use study_tts_runtime::{
    BackendDescriptor, BackendError, BuildError, CachePublisher, CacheResolveRequest,
    FileSystemCachePublisher, IoError, JobOwnership, JobRepository, PackagePreflightRequest,
    PackagePrepareRequest, PackagePublication, PackageWriteRequest, PackageWriter,
    PreparedPackageWriter, StagedAudioProducer, SynthesisReport, SynthesisRequest, TtsExecutor,
    ValidatedCachedArtifact,
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

    fn load(
        &self,
        workspace: &Path,
        job_id: &str,
    ) -> Result<Option<ProvisionalJobSnapshot>, BuildError> {
        self.events.record("job.load");
        self.inner.load(workspace, job_id)
    }

    fn replace(
        &self,
        workspace: &Path,
        snapshot: &ProvisionalJobSnapshot,
    ) -> Result<(), BuildError> {
        self.events
            .record(format!("job.replace:{:?}", snapshot.stage));
        self.inner.replace(workspace, snapshot)
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
        let master_wav = package_dir.join("lesson.wav");
        let m4a = package_dir.join("lesson.m4a");
        let manifest = package_dir.join("manifest.json");
        let publication_record = self.root.join("current.json");
        fs::write(&master_wav, b"fake master").map_err(|source| file_error(&master_wav, source))?;
        fs::write(&m4a, b"fake m4a").map_err(|source| file_error(&m4a, source))?;
        let manifest_bytes =
            format!("{{\"release_status\":\"private_preview\",\"plan_hash\":\"{plan_hash}\"}}");
        fs::write(&manifest, manifest_bytes.as_bytes())
            .map_err(|source| file_error(&manifest, source))?;
        let manifest_blake3 = blake3::hash(manifest_bytes.as_bytes()).to_hex().to_string();
        fs::write(&publication_record, manifest_blake3.as_bytes())
            .map_err(|source| file_error(&publication_record, source))?;
        let publication = PackagePublication {
            package_dir,
            publication_record,
            master_wav,
            m4a,
            manifest,
            identity: SelectedPackageIdentity {
                package_id: plan_hash.clone(),
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
    /// The named stage replaced the authoritative snapshot.
    Replace(study_tts_core::ProvisionalJobStage),
}

/// In-memory provisional job repository with complete replacement history.
#[derive(Debug, Default)]
pub struct InMemoryJobRepository {
    current: Mutex<BTreeMap<String, ProvisionalJobSnapshot>>,
    history: Mutex<Vec<ProvisionalJobSnapshot>>,
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

    /// Returns every snapshot passed to [`JobRepository::replace`].
    pub fn snapshots(&self) -> Vec<ProvisionalJobSnapshot> {
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

    fn load(
        &self,
        _workspace: &Path,
        job_id: &str,
    ) -> Result<Option<ProvisionalJobSnapshot>, BuildError> {
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

    fn replace(
        &self,
        _workspace: &Path,
        snapshot: &ProvisionalJobSnapshot,
    ) -> Result<(), BuildError> {
        self.calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(FakeJobCall::Replace(snapshot.stage));
        self.history
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(snapshot.clone());
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(snapshot.job_id.clone(), snapshot.clone());
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
    snapshot: &ProvisionalJobSnapshot,
) -> Result<ProvisionalJobSnapshot, BuildError> {
    let _ownership = repository.claim(workspace, &snapshot.job_id)?;
    repository.replace(workspace, snapshot)?;
    repository
        .load(workspace, &snapshot.job_id)?
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
