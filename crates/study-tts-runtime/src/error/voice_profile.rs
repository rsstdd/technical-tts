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

    /// The lesson names a voice profile that the voice-profile root does not
    /// hold.
    ///
    /// Distinct from [`VoiceProfileError::MissingVoiceRecord`], which is a
    /// profile that exists but is incomplete: this one was never installed, so
    /// the remedy is to install it rather than to repair it.
    #[error(
        "voice profile `{profile_id}` is refused: `{root}` holds no directory for it; the \
         project owner must install the profile, or the lesson's author must name one that \
         exists"
    )]
    MissingVoiceProfileDirectory {
        /// The voice-profile root the build was given.
        root: PathBuf,
        /// The profile the lesson declared.
        profile_id: String,
    },

    /// The voice-profile root holds an entry for the profile that is not a
    /// directory.
    ///
    /// Distinct from [`VoiceProfileError::MissingVoiceProfileDirectory`]: the
    /// name is taken, so installing the profile is not the remedy. A symlink
    /// lands here too, and is refused for the reason `voice_gate::record_path`
    /// gives one level down — the gate would otherwise read and hash the same
    /// artifacts through the link and agree with itself about a voice the
    /// consent record never covered.
    #[error(
        "voice profile `{profile_id}` is refused: `{root}` holds an entry of that name that is \
         not a directory; the project owner must remove or replace it before the profile is used"
    )]
    VoiceProfileNotDirectory {
        /// The voice-profile root the build was given.
        root: PathBuf,
        /// The profile the lesson declared.
        profile_id: String,
    },

    /// A profile record names an identity other than the directory it was
    /// resolved through.
    ///
    /// Fails closed because the recorded identity is what reaches a manifest
    /// and a worker frame: accepting the mismatch would attribute one voice's
    /// consent record to another voice's audio.
    #[error(
        "voice profile `{declared}` is refused: the record in that directory calls itself \
         `{recorded}`; the project owner must correct the record or the directory name before \
         the profile is used"
    )]
    VoiceProfileIdMismatch {
        /// The profile identity the lesson declared and the directory carries.
        declared: String,
        /// The identity the record claims for itself.
        recorded: String,
    },

    /// The voice-profile root holds a loadable profile whose directory name is
    /// not UTF-8.
    ///
    /// Refused rather than skipped, and the reason is the whole point of the
    /// gate. The worker reads the same name through Python's
    /// `surrogateescape`, so `voice-\xff-v1` reaches it as a string holding a
    /// lone surrogate — and a `profile.json` whose `profile_id` carries that
    /// same surrogate compares equal to it, which is all
    /// `worker.py::_voice_conditioning` requires before `_load_backend`
    /// deserializes the artifact. An entry this build cannot name is therefore
    /// an entry the worker can, so skipping it would leave exactly one profile
    /// reaching `torch.load` with no consent, rights, scope, or checksum check.
    #[error(
        "voice profile root `{root}` is refused: it holds a profile directory named `{name}`, \
         which is not UTF-8 and so cannot be gated, while the worker would still load it; the \
         project owner must rename the directory or move it out of the governed root before any \
         profile in it is used"
    )]
    VoiceProfileNameNotUtf8 {
        /// The voice-profile root the build was given.
        root: PathBuf,
        /// The entry's name, rendered lossily so it can be found on disk.
        name: String,
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
            | Self::MissingVoiceProfileDirectory { .. }
            | Self::VoiceProfileNotDirectory { .. }
            | Self::VoiceProfileIdMismatch { .. }
            | Self::VoiceProfileNameNotUtf8 { .. }
            | Self::VoiceChecksumMismatch { .. } => Some(RemedyAdvice::new(
                RemedyOwner::ProjectOwner,
                "supply or correct the voice profile record before use",
                Some("Voice consent/checksum mismatch"),
            )),
        }
    }
}
