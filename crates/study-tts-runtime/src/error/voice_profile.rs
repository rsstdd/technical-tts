//! Runtime refusals for required on-disk voice-profile records.
//!
//! No variant carries the profile directory or a record's path. ADR-0001 §14
//! and `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access
//! exclude raw voice-reference paths from logs by default, and a refusal's
//! `Display` is printed by default — `study-tts` writes it to standard error
//! and into the `--json` record unchanged. A profile is named by its identity
//! and a record by its name; the operator who supplied `--voice-root` can find
//! `<root>/<profile>/<record>` from those without the log spelling it.
//! `t1_e2_no_voice_profile_refusal_names_a_path` pins that.

use std::path::PathBuf;

use thiserror::Error;

use super::{RemedyAdvice, RemedyOwner};

/// Why the runtime refused an on-disk voice profile.
#[derive(Debug, Error)]
pub enum VoiceProfileError {
    /// A record the voice policy requires is absent.
    #[error(
        "voice profile `{profile_id}` is refused: required record `{record}` is missing; \
         profile load fails closed and the project owner must supply the record before use"
    )]
    MissingVoiceRecord {
        /// The profile the record was expected in.
        profile_id: String,
        /// Which required record is absent.
        record: &'static str,
    },

    /// A required record name holds something other than a regular file.
    #[error(
        "voice profile `{profile_id}` is refused: required record `{record}` is not a regular \
         file; profile load fails closed and the project owner must supply the record itself \
         before use"
    )]
    VoiceRecordNotRegularFile {
        /// The profile the record was expected in.
        profile_id: String,
        /// Which required record is not a regular file.
        record: &'static str,
    },

    /// A required record exists and could not be read.
    ///
    /// Distinct from [`VoiceProfileError::MissingVoiceRecord`] because the
    /// remedy differs — nothing to supply, something to repair — and carried
    /// here rather than as [`crate::IoError`] because that error names the
    /// path it failed on, which for `reference.wav` is the raw voice
    /// reference this module's header keeps out of every message.
    #[error(
        "voice profile `{profile_id}` is refused: required record `{record}` could not be read \
         ({source}); profile load fails closed and the project owner must repair the record \
         before use"
    )]
    VoiceRecordUnreadable {
        /// The profile the record belongs to.
        profile_id: String,
        /// Which required record failed to read.
        record: &'static str,
        /// What the filesystem reported.
        #[source]
        source: std::io::Error,
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
        "voice profile `{profile_id}` is refused: `{record}` does not match its recorded \
         checksum; do not use this profile until the project owner re-verifies it against its \
         rights record"
    )]
    VoiceChecksumMismatch {
        /// The profile whose record no longer holds.
        profile_id: String,
        /// The record whose contents disagree with what was recorded.
        record: &'static str,
    },
}

impl VoiceProfileError {
    /// Returns the governed recovery advice for this voice-profile refusal.
    pub(super) fn remedy(&self) -> Option<RemedyAdvice> {
        match self {
            Self::MissingVoiceRecord { .. }
            | Self::VoiceRecordNotRegularFile { .. }
            | Self::VoiceRecordUnreadable { .. }
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::VoiceProfileError;

    /// No refusal about a profile's contents spells where that profile lives.
    ///
    /// The directory is built from a root and a profile identity that appear
    /// nowhere else in the message, so a variant that carried it would print a
    /// substring no other field produces. The variants about the *root* —
    /// a profile that was never installed, an entry that is not a directory,
    /// a name this build cannot spell — name the root the operator typed,
    /// which is theirs to see; none of them locates a raw voice reference,
    /// because in each case there is no readable profile beneath it.
    #[test]
    fn t1_e2_no_voice_profile_refusal_names_a_path() {
        const ROOT: &str = "/governed/voices";
        const PROFILE: &str = "narrator-v1";
        let profile_dir = PathBuf::from(ROOT).join(PROFILE);
        let record_path = profile_dir.join("reference.wav");

        let refusals = [
            VoiceProfileError::MissingVoiceRecord {
                profile_id: PROFILE.to_owned(),
                record: "reference.wav",
            },
            VoiceProfileError::VoiceRecordNotRegularFile {
                profile_id: PROFILE.to_owned(),
                record: "reference.wav",
            },
            VoiceProfileError::VoiceRecordUnreadable {
                profile_id: PROFILE.to_owned(),
                record: "reference.wav",
                source: std::io::Error::other("permission denied"),
            },
            VoiceProfileError::VoiceChecksumMismatch {
                profile_id: PROFILE.to_owned(),
                record: "reference.wav",
            },
        ];

        for refusal in refusals {
            let message = refusal.to_string();
            assert!(
                !message.contains(&profile_dir.display().to_string())
                    && !message.contains(&record_path.display().to_string()),
                "a refusal about a profile's contents must not locate it: `{message}`"
            );
            assert!(
                message.contains(PROFILE) && message.contains("reference.wav"),
                "the profile and the record are what the owner navigates by: `{message}`"
            );
        }
    }
}
