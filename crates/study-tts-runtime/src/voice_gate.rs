//! Fail-closed resolution of the voices a lesson declares.
//!
//! Applies `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load
//! fails closed") to the ADR-0001 §12.1 layout, and adds what the core gate
//! cannot: reading `reference.wav` and `conditionals.pt` and checking their
//! bytes against the digests the profile records, and refusing a record name
//! that holds anything other than a regular file.
//!
//! A refusal here precedes every tool and synthesis call, so an unconsented or
//! altered voice cannot reach audio. It also precedes *planning*, because the
//! conditioning artifact it resolves is an ADR-0001 §12.5 synthesis-key input.

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use study_tts_core::{
    ValidatedLesson, VoiceConditioningHash, VoiceConsent, VoiceProfile, VoiceUse,
    validate_profile_for_use,
};

use crate::{BuildError, IoError, VoiceProfileError, cache};

/// Resolves every voice a lesson speaks with into its conditioning identity.
///
/// Only the speakers segments actually use are resolved, so an unused
/// declaration costs no rights check and no checksum.
///
/// Deduplication is by *profile*, not by speaker. Two speakers naming one
/// profile must receive the same hash, and loading it twice would read and
/// hash the same artifacts twice — and could return two different digests if
/// the profile changed between the reads, silently keying two segments of one
/// build on two versions of one voice. They still receive different cache
/// keys, because `speaker` is a key input of its own.
///
/// The returned map is what fills
/// [`study_tts_core::SynthesisContext::voice_conditioning_hashes`], so this
/// runs before planning: the hash is an ADR-0001 §12.5 input, and a plan
/// derived without it would name cache entries for a voice nobody resolved.
///
/// # Errors
///
/// [`VoiceProfileError::MissingVoiceProfileDirectory`] when `root` holds no
/// entry for a declared profile,
/// [`VoiceProfileError::VoiceProfileNotDirectory`] when the entry is not a
/// directory, [`VoiceProfileError::VoiceProfileIdMismatch`] when the record
/// calls itself something else, and [`IoError::ReadFile`] when the entry
/// cannot be inspected at all. Otherwise whatever [`load_profile`] returns for
/// that profile.
pub(crate) fn resolve_speakers(
    root: &Path,
    lesson: &ValidatedLesson,
    requested: VoiceUse,
) -> Result<BTreeMap<String, VoiceConditioningHash>, BuildError> {
    // Keyed by profile rather than by speaker, so one profile is loaded once
    // however many speakers name it.
    let mut by_profile: BTreeMap<&str, VoiceConditioningHash> = BTreeMap::new();
    let mut resolved: BTreeMap<String, VoiceConditioningHash> = BTreeMap::new();
    for segment in lesson.segments() {
        // Validation guarantees the key: `LessonError::UndeclaredSpeaker`
        // refuses a segment whose speaker the document never bound.
        let profile_id = lesson.speakers()[&segment.speaker].voice_profile.as_str();
        let conditioning = match by_profile.get(profile_id) {
            Some(conditioning) => conditioning.clone(),
            None => {
                let conditioning = load_conditioning(root, profile_id, requested)?;
                by_profile.insert(profile_id, conditioning.clone());
                conditioning
            }
        };
        resolved.insert(segment.speaker.clone(), conditioning);
    }
    Ok(resolved)
}

/// Loads one profile from the root and returns the conditioning identity that
/// reaches the cache key.
fn load_conditioning(
    root: &Path,
    profile_id: &str,
    requested: VoiceUse,
) -> Result<VoiceConditioningHash, BuildError> {
    let dir = root.join(profile_id);
    // `Path::is_dir` collapses "absent", "not a directory", and "could not be
    // read at all" into one `false`, which would report a permission failure
    // as a profile the owner never installed and send them to the wrong
    // remedy. `symlink_metadata` also reports a link as a link: a symlinked
    // profile directory is refused for the reason `record_path` gives about a
    // symlinked record, one level up.
    match fs::symlink_metadata(&dir) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return Err(VoiceProfileError::VoiceProfileNotDirectory {
                root: root.to_path_buf(),
                profile_id: profile_id.to_owned(),
            }
            .into());
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err(VoiceProfileError::MissingVoiceProfileDirectory {
                root: root.to_path_buf(),
                profile_id: profile_id.to_owned(),
            }
            .into());
        }
        Err(source) => return Err(IoError::ReadFile { path: dir, source }.into()),
    }

    let profile = load_profile(&dir, requested)?;
    if profile.profile_id != profile_id {
        return Err(VoiceProfileError::VoiceProfileIdMismatch {
            declared: profile_id.to_owned(),
            recorded: profile.profile_id,
        }
        .into());
    }

    // `VoiceProfile::validate` already refused a checksum that is not a
    // BLAKE3 digest, so this parse cannot fail on a loaded profile; it is
    // still a parse rather than an assertion, because the type is what keeps a
    // non-digest out of every cache key downstream.
    profile.conditionals_blake3.parse().map_err(|_| {
        VoiceProfileError::VoiceChecksumMismatch {
            profile_dir: dir.clone(),
            path: dir.join("conditionals.pt"),
        }
        .into()
    })
}

/// Loads a voice profile directory fail-closed for `requested` use.
///
/// Enforces `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load
/// fails closed") over the ADR-0001 §12.1 layout: `profile.json`,
/// `consent.json`, `reference.wav`, `conditionals.pt`. A missing record, a
/// record that is not a regular file, non-granted consent, non-approved rights
/// decision, a use outside the recorded consent scope, or a checksum mismatch
/// refuses the profile before any tool or synthesis work runs.
///
/// Returns the loaded identity. [`resolve_speakers`] takes its conditioning
/// hash; the real-worker story adds the per-build audit event required by
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
/// [`VoiceProfileError::VoiceRecordNotRegularFile`] when the name holds a
/// symlink, a directory, or anything else that is not a regular file. An
/// absent record resolves successfully, because reporting it is the caller's
/// job: [`VoiceProfileError::MissingVoiceRecord`] belongs to whichever reader
/// discovers it. Otherwise [`IoError::ReadFile`] carries what the filesystem
/// reported.
fn record_path(dir: &Path, record: &'static str) -> Result<PathBuf, BuildError> {
    let path = dir.join(record);
    match fs::symlink_metadata(&path) {
        // `symlink_metadata` reports the link's own type rather than its
        // target's, which is what lets a planted link be seen at all.
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            Err(VoiceProfileError::VoiceRecordNotRegularFile {
                profile_dir: dir.to_path_buf(),
                record,
            }
            .into())
        }
        Ok(_) => Ok(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path),
        Err(source) => Err(IoError::ReadFile { path, source }.into()),
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
            Err(VoiceProfileError::MissingVoiceRecord {
                profile_dir: dir.to_path_buf(),
                record,
            }
            .into())
        }
        Err(source) => Err(IoError::ReadFile { path, source }.into()),
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
        BuildError::Io(IoError::FileSystem { source, .. })
            if source.kind() == io::ErrorKind::NotFound =>
        {
            VoiceProfileError::MissingVoiceRecord {
                profile_dir: dir.to_path_buf(),
                record,
            }
            .into()
        }
        _ => error,
    })?;
    if computed != recorded {
        return Err(VoiceProfileError::VoiceChecksumMismatch {
            profile_dir: dir.to_path_buf(),
            path,
        }
        .into());
    }
    Ok(())
}
