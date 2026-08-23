use std::{fs, io, path::Path};

use study_tts_core::{VoiceConsent, VoiceProfile, VoiceUse, validate_profile_for_use};

use crate::{BuildError, cache};

/// Loads a voice profile directory fail-closed for `requested` use.
///
/// Enforces `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` ("Profile load
/// fails closed") over the ADR-0001 §12.1 layout: `profile.json`,
/// `consent.json`, `reference.wav`, `conditionals.pt`. A missing record,
/// non-granted consent, non-approved rights decision, a use outside the
/// recorded consent scope, or a checksum mismatch refuses the profile before
/// any tool or synthesis work runs.
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

/// Reads a required record, distinguishing "the policy requires this and it is
/// absent" from an ordinary IO failure, because the two have different
/// remedies.
fn read_record(dir: &Path, record: &'static str) -> Result<Vec<u8>, BuildError> {
    let path = dir.join(record);
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

fn verify_checksum(dir: &Path, name: &str, recorded: &str) -> Result<(), BuildError> {
    let path = dir.join(name);
    let computed = cache::hash_file(&path)?;
    if computed != recorded {
        return Err(BuildError::VoiceChecksumMismatch {
            profile_dir: dir.to_path_buf(),
            path,
        });
    }
    Ok(())
}
