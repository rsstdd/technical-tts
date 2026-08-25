//! Runtime refusals for required on-disk voice-profile records.

use std::path::PathBuf;

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why the runtime refused an on-disk voice profile.
#[derive(Debug, Error)]
pub enum VoiceProfileError {
    /// A record the voice policy requires is absent.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is missing; \
         profile load fails closed and the project owner must supply the record before use"
    )]
    MissingVoiceRecord {
        /// The profile directory the record was expected in.
        profile_dir: PathBuf,
        /// Which required record is absent.
        record: &'static str,
    },

    /// A required record name holds something other than a regular file.
    #[error(
        "voice profile at `{profile_dir}` is refused: required record `{record}` is not a regular \
         file; profile load fails closed and the project owner must supply the record itself \
         before use"
    )]
    VoiceRecordNotRegularFile {
        /// The profile directory the record was expected in.
        profile_dir: PathBuf,
        /// Which required record is not a regular file.
        record: &'static str,
    },

    /// A profile file no longer hashes to what its record says.
    #[error(
        "voice profile at `{profile_dir}` is refused: `{path}` does not match its recorded \
         checksum; do not use this profile until the project owner re-verifies it against its \
         rights record"
    )]
    VoiceChecksumMismatch {
        /// The profile directory whose record no longer holds.
        profile_dir: PathBuf,
        /// The file whose contents disagree with the record.
        path: PathBuf,
    },
}

impl VoiceProfileError {
    /// Returns the governed recovery advice for this voice-profile refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::MissingVoiceRecord { .. }
            | Self::VoiceRecordNotRegularFile { .. }
            | Self::VoiceChecksumMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "supply or correct the voice profile record before use",
                Some("Voice consent/checksum mismatch"),
            )),
        }
    }
}
