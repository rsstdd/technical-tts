//! Fail-closed loading of a voice profile directory.
//!
//! Applies `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load
//! fails closed") to the ADR-0001 §12.1 layout, and adds what the core gate
//! cannot: reading `reference.wav` and `conditionals.pt` and checking their
//! bytes against the digests the profile records, and refusing a record name
//! that holds anything other than a regular file.
//!
//! A refusal here precedes every tool and synthesis call, so an unconsented or
//! altered voice cannot reach audio.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use study_tts_core::{VoiceConsent, VoiceProfile, VoiceUse, validate_profile_for_use};

use crate::{BuildError, cache};

/// Loads a voice profile directory fail-closed for `requested` use.
///
/// Enforces `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load
/// fails closed") over the ADR-0001 §12.1 layout: `profile.json`,
/// `consent.json`, `reference.wav`, `conditionals.pt`. A missing record, a
/// record that is not a regular file, non-granted consent, non-approved rights
/// decision, a use outside the recorded consent scope, or a checksum mismatch
/// refuses the profile before any tool or synthesis work runs.
///
/// Returns the loaded identity. The skeleton worker discards it; the
/// real-worker story consumes it and adds the per-build audit event required by
/// ADR-0001 §15.3.
pub(crate) fn load_profile(dir: &Path, requested: VoiceUse) -> Result<VoiceProfile, BuildError> {
    let profile_bytes = read_record(dir, "profile.json")?;
    let profile = VoiceProfile::from_json(&profile_bytes).map_err(BuildError::Voice)?;

    let consent_bytes = read_record(dir, "consent.json")?;
    let consent = VoiceConsent::from_json(&consent_bytes).map_err(BuildError::Voice)?;

    validate_profile_for_use(&profile, &consent, requested).map_err(BuildError::Voice)?;

    verify_checksum(dir, "reference.wav", &profile.reference_wav_blake3)?;
    verify_checksum(dir, "conditionals.pt", &profile.conditionals_blake3)?;

    Ok(profile)
}

/// Resolves one required record and refuses a name that does not hold a
/// regular file.
///
/// Both readers go through this, so the bytes and the digest of an artifact
/// can never be taken through two differently-resolved names. A link here is
/// not a write that escapes: `verify_checksum` hashes the same name it reads,
/// so a link would supply both sides of the comparison from one file outside
/// the profile and the gate would agree with itself, admitting audio the
/// consent record never covered. A FIFO is refused by the same check, because
/// hashing one never returns.
///
/// Deliberately not [`crate::managed::leaf`], which resolves names beneath a
/// root this crate created and canonicalized. A profile directory is
/// operator-supplied input, so the containment its errors claim would not be
/// true here; only the symlink refusal transfers, and it arrives with the
/// rights routing rather than the runtime's.
///
/// A link planted between this check and the read that follows is still
/// followed. Closing that window needs the directory-relative operations
/// `managed` defers to the E5-S4 containment story, and the same
/// proportionality argument applies: an attacker who can write into the
/// profile directory can already replace the record outright.
///
/// # Errors
///
/// [`BuildError::VoiceRecordNotRegularFile`] when the name holds a symlink, a
/// directory, or anything else that is not a regular file. An absent record
/// resolves successfully, because reporting it is the caller's job:
/// [`BuildError::MissingVoiceRecord`] belongs to whichever of the two readers
/// discovers it. Otherwise [`BuildError::ReadFile`] carrying what the
/// filesystem reported.
fn record_path(dir: &Path, record: &'static str) -> Result<PathBuf, BuildError> {
    let path = dir.join(record);
    match fs::symlink_metadata(&path) {
        // `symlink_metadata` reports the link's own type rather than its
        // target's, which is what lets a planted link be seen at all.
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(BuildError::VoiceRecordNotRegularFile {
                profile_dir: dir.to_path_buf(),
                record,
            })
        }
        Ok(_) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path),
        Err(source) => Err(BuildError::ReadFile { path, source }),
    }
}

/// Reads a required record, distinguishing "the policy requires this and it is
/// absent" from an ordinary IO failure, because the two have different
/// remedies.
fn read_record(dir: &Path, record: &'static str) -> Result<Vec<u8>, BuildError> {
    let path = record_path(dir, record)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(BuildError::MissingVoiceRecord {
                profile_dir: dir.to_path_buf(),
                record,
            })
        }
        Err(source) => Err(BuildError::ReadFile { path, source }),
    }
}

/// Verifies one recorded artifact against its checksum.
///
/// An absent artifact is reported as a missing record rather than as an IO
/// failure. `reference.wav` and `conditionals.pt` are required by the ADR-0001
/// §12.1 layout exactly as `profile.json` and `consent.json` are, so their
/// absence is the same class of refusal and deserves the same remedy owner —
/// not "filesystem operation failed", which names no policy and no person.
fn verify_checksum(dir: &Path, record: &'static str, recorded: &str) -> Result<(), BuildError> {
    let path = record_path(dir, record)?;
    // Absence is mapped from the real read rather than from the resolution
    // above, which is why `record_path` passes a missing record through. A
    // separate existence check would leave a window for the artifact to vanish
    // between the two and be reported as the wrong failure.
    let computed = cache::hash_file(&path).map_err(|error| match &error {
        BuildError::FileSystem { source, .. } if source.kind() == io::ErrorKind::NotFound => {
            BuildError::MissingVoiceRecord {
                profile_dir: dir.to_path_buf(),
                record,
            }
        }
        _ => error,
    })?;
    if computed != recorded {
        return Err(BuildError::VoiceChecksumMismatch {
            profile_dir: dir.to_path_buf(),
            path,
        });
    }
    Ok(())
}
