//! Published synthesis-cache lookup and staged-audio publication port.
//!
//! The port accepts only a producer writing to the cache-owned staging path.
//! Its filesystem adapter retains containment, structural validation,
//! checksums, no-replace publication, key locking, and quarantine.
//! `docs/architecture/PROVISIONAL-CONTRACT-BASELINE.md` records its consumers,
//! fake, identity effects, and G1 stabilization condition.

use std::path::{Path, PathBuf};

use study_tts_core::PlannedSegment;

use crate::{
    BackendError, BuildError, SynthesisReport,
    cache::{self, ValidatedCachedArtifact},
    durable::OsDurableFileSystem,
    managed, preview,
};

/// Mirrors the cache version in the E0-S4 provisional contract baseline.
pub const CACHE_PUBLICATION_CONTRACT_VERSION: &str = "e0.cache-publication.2.0";

/// Owned cache lookup and publication inputs for one planned segment.
#[derive(Clone, Debug)]
pub struct CacheResolveRequest {
    /// Canonical managed workspace root.
    pub workspace: PathBuf,
    /// Provisional job identity used for quarantine ownership.
    pub job_id: String,
    /// Planned synthesis identity and assembly metadata.
    pub segment: PlannedSegment,
}

/// Produces audio only at a cache-assigned staging destination.
pub trait StagedAudioProducer {
    /// Writes one staged WAV and reports what was written.
    ///
    /// # Errors
    ///
    /// [`BackendError::InvalidRequest`] for request validation,
    /// [`BackendError::Destination`] for staging writes,
    /// [`BackendError::Execution`] for backend inference,
    /// [`BackendError::Timeout`] for a bounded deadline, or
    /// [`BackendError::Protocol`] for a violated protocol invariant.
    fn produce(&mut self, destination: &Path) -> Result<SynthesisReport, BackendError>;
}

impl<F> StagedAudioProducer for F
where
    F: FnMut(&Path) -> Result<SynthesisReport, BackendError>,
{
    fn produce(&mut self, destination: &Path) -> Result<SynthesisReport, BackendError> {
        self(destination)
    }
}

/// Cache lookup and immutable publication boundary.
pub trait CachePublisher: Send + Sync {
    /// Reuses one validated artifact or publishes validated staged audio.
    ///
    /// # Errors
    ///
    /// [`BuildError::ManagedPath`] for containment, [`BuildError::Cache`] for
    /// an unusable entry, [`BuildError::Audio`] for staged-WAV rejection,
    /// [`BuildError::DurableState`] for locking, publication, or quarantine,
    /// [`BuildError::Io`] for filesystem failure, or
    /// [`BuildError::Synthesis`] when `producer` fails.
    fn resolve(
        &self,
        request: &CacheResolveRequest,
        producer: &mut dyn StagedAudioProducer,
    ) -> Result<ValidatedCachedArtifact, BuildError>;
}

/// Filesystem cache adapter used by the walking skeleton.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileSystemCachePublisher;

impl CachePublisher for FileSystemCachePublisher {
    fn resolve(
        &self,
        request: &CacheResolveRequest,
        producer: &mut dyn StagedAudioProducer,
    ) -> Result<ValidatedCachedArtifact, BuildError> {
        let filesystem = OsDurableFileSystem;
        let cache_root = managed::subdirectory(&request.workspace, "cache")?;
        let roots = preview::roots(&request.workspace, &request.job_id)?;
        cache::resolve(
            &filesystem,
            &cache_root,
            &roots.quarantine_root,
            &request.job_id,
            &request.segment,
            producer,
        )
    }
}
