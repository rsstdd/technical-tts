//! Builds one lesson into a private preview: gating, planning, synthesis
//! caching, assembly, encoding, and the manifest that records what was
//! produced.
//!
//! Every gate runs before any tool or synthesis work, so a refusal names the
//! policy that refused rather than the first thing that happened to break.

mod assembly;
mod cache;
mod error;
mod export;
mod manifest;
mod pipeline;
mod synthesis;
mod tools;
mod voice_gate;

pub use error::{AudioFault, BuildError, CacheEntryFault};
pub use pipeline::{
    BuildRequest, BuildResult, build_preview, publish, validate_encoded_output,
    validate_production_manifest,
};
pub use synthesis::{SegmentSynthesizer, SynthesisError, SynthesisReport};

use std::path::{Path, PathBuf};

use study_tts_core::CacheKey;

/// Re-exported at the root so every module keeps constructing these the same
/// way, rather than half of them reaching into `error` directly.
pub(crate) use error::{audio_error, io_error};

/// Directory holding one cache entry.
///
/// Exposed so integration tests can corrupt a specific entry without
/// duplicating the sharding scheme. Changing the shard width in
/// `cache::entry_dir` updates the tests automatically.
///
/// Takes a parsed `CacheKey` rather than a string: this is the one cache path
/// that crosses the crate boundary, and a caller reading a key out of a
/// manifest should be told the key is malformed there rather than have it panic
/// inside the shard slice.
#[doc(hidden)]
pub fn cache_entry_dir(cache_root: &Path, cache_key: &CacheKey) -> PathBuf {
    cache::entry_dir(cache_root, cache_key)
}
