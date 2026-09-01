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

use crate::{BuildError, IoError, VoiceProfileError, cache, error::io_error};

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
/// keys, because `speaker` is a key input of its own. The single load is
/// observed by
/// `t1_e1_one_voice_profile_is_loaded_once_however_many_speakers_name_it`.
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
    resolve_by_profile(lesson, |profile_id| {
        resolve_voice_conditioning(root, profile_id, requested)
    })
}

/// Maps every speaker a segment names to the conditioning identity of the
/// profile it declares, loading each profile once.
///
/// The loader is a parameter because it is the only seam from which loading
/// once is observable. The returned map says which hash each speaker received
/// and nothing about how many times a profile was read, so the rewrite
/// [`resolve_speakers`] warns about — resolving per speaker — would return
/// exactly this map and refuse exactly the same documents.
/// `t1_e1_one_voice_profile_is_loaded_once_however_many_speakers_name_it`
/// counts the calls instead.
fn resolve_by_profile(
    lesson: &ValidatedLesson,
    mut load: impl FnMut(&str) -> Result<VoiceConditioningHash, BuildError>,
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
                let conditioning = load(profile_id)?;
                by_profile.insert(profile_id, conditioning.clone());
                conditioning
            }
        };
        resolved.insert(segment.speaker.clone(), conditioning);
    }
    Ok(resolved)
}

/// Gates every profile a worker will load, before one can be started.
///
/// The worker deserializes *every* `conditionals.pt` beneath the governed root
/// during `initialize`, not only the profile a later request names, so the
/// blast radius of an ungated root is the whole root. This runs the same
/// fail-closed check [`resolve_voice_conditioning`] runs over each of them,
/// from [`crate::WorkerConfiguration::for_bundle`] — the only constructor that
/// is *given* a voice root, and therefore the only one that could gate it.
/// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` requires a profile load to
/// fail closed, and ADR-0001 §11.2 puts that validation *before*
/// initialization; a gate that ran afterwards would refuse a revoked voice only
/// after its bytes had been through `torch.load`.
///
/// The other constructor,
/// [`crate::WorkerConfiguration::for_protocol_fake`], takes a caller-chosen
/// program and environment, so being the only *gating* constructor is not by
/// itself the property that matters: a caller could point that fake at the
/// bundle interpreter and hand it a governed root, and `initialize` would load
/// every profile before the synthetic identity refused the session. That route
/// is closed at the fake, which refuses the environment a governed root reaches
/// a worker through. The two together are what make it true that no
/// configuration this crate builds launches a process that can find a governed
/// root ungated.
///
/// The consequence is worth stating out loud: every profile beneath the root
/// must be rights-clean or no worker starts. A revoked profile has to be moved
/// out of the governed root, which is what the rights policy's revocation path
/// asks for anyway.
///
/// **The skip list is load-bearing and mirrors
/// `worker/study_tts_worker/worker.py` `_voice_conditioning`,** whose docstring
/// names this function in return. It skips an entry that is not a directory, is
/// a symlink, or holds no `profile.json` — and this must skip *at most* those,
/// because anything skipped here that the worker still loads is a profile that
/// reached `torch.load` ungated. Skipping more is a false refusal; skipping
/// less is the defect. Where the two cannot agree — a directory name the worker
/// can spell and this build cannot — the entry is refused rather than skipped,
/// which keeps that rule intact in the only direction that is safe.
///
/// # Errors
///
/// Whatever [`resolve_voice_conditioning`] returns for the first profile it
/// refuses, [`VoiceProfileError::VoiceProfileNameNotUtf8`] for a loadable
/// profile whose directory name this build cannot spell but the worker can, and
/// [`IoError::FileSystem`] when the root itself cannot be listed — the variant
/// every other directory walk in this crate reports, because enumerating a
/// governed root is an operation the build performs rather than an input it was
/// told to read.
pub fn admit_voice_root(root: &Path, requested: VoiceUse) -> Result<(), BuildError> {
    let listing = fs::read_dir(root).map_err(|source| io_error(root, source))?;
    let mut candidates = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|source| io_error(root, source))?;
        candidates.push(entry.path());
    }
    // Sorted so a root with two bad profiles refuses the same one every time.
    // A refusal that depends on directory order is a refusal an operator
    // cannot reproduce from the message they were given.
    candidates.sort();

    for candidate in candidates {
        if !holds_a_loadable_profile(&candidate) {
            continue;
        }
        // Refused rather than skipped, which is the one place this filter may
        // not mirror the worker's. A name that is not UTF-8 was assumed to be
        // one no `profile_id` could equal; it is not. Python reads the same
        // directory name through `surrogateescape`, so `voice-\xff-v1` reaches
        // `_voice_conditioning` as a string holding a lone surrogate — and JSON
        // carries that surrogate too, so a record stating it compares equal,
        // which is the only agreement the worker asks for before
        // `_load_backend` deserializes the artifact. An entry this build cannot
        // name is an entry the worker can, so skipping it is the ungated
        // `torch.load` this gate exists to prevent.
        let Some(profile_id) = candidate.file_name().and_then(|name| name.to_str()) else {
            return Err(VoiceProfileError::VoiceProfileNameNotUtf8 {
                root: root.to_path_buf(),
                name: candidate
                    .file_name()
                    .unwrap_or(candidate.as_os_str())
                    .to_string_lossy()
                    .into_owned(),
            }
            .into());
        };
        resolve_voice_conditioning(root, profile_id, requested)?;
    }
    Ok(())
}

/// Whether the worker would read `candidate` as a voice profile.
///
/// `symlink_metadata` rather than `metadata`, so a symlinked directory is a
/// symlink here as it is to `_voice_conditioning`'s `candidate.is_symlink()`.
fn holds_a_loadable_profile(candidate: &Path) -> bool {
    fs::symlink_metadata(candidate).is_ok_and(|metadata| metadata.is_dir())
        && candidate.join("profile.json").is_file()
}

/// Loads one profile from the root and returns the conditioning identity that
/// reaches the cache key.
///
/// Public because the committed instruments under
/// `crates/study-tts-testkit/examples/` render against a governed voice root
/// without a lesson to resolve speakers from, and the alternative is what they
/// did before: read `profile.json` by hand and skip consent, the rights
/// decision, the permitted-use scope, and both checksums. An instrument whose
/// output a gate record cites must pass the same gate a build does.
/// [`resolve_speakers`] stays crate-private, because it takes a
/// [`ValidatedLesson`] and neither instrument has one.
///
/// # Errors
///
/// [`VoiceProfileError::MissingVoiceProfileDirectory`] when `root` holds no
/// entry for `profile_id`, [`VoiceProfileError::VoiceProfileNotDirectory`] when
/// the entry is not a directory, [`VoiceProfileError::VoiceProfileIdMismatch`]
/// when the record calls itself something else,
/// [`VoiceProfileError::VoiceChecksumMismatch`] when an artifact does not hash
/// to the digest its record states, [`BuildError::Voice`] for a withdrawn
/// consent, an unapproved rights decision, or a use outside the recorded
/// scope, and [`IoError::ReadFile`] when a record cannot be read at all.
pub fn resolve_voice_conditioning(
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

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use study_tts_core::{ValidatedLesson, VoiceUse};
    use tempfile::TempDir;

    use super::{admit_voice_root, resolve_by_profile};
    use crate::{BuildError, VoiceProfileError};

    /// Two speakers naming one profile load that profile once.
    ///
    /// T1 because nothing here reaches the filesystem: the lesson is compiled
    /// in and the loader is a closure, which is what makes the load count
    /// observable at all. The half that runs the real build is
    /// `t4_e1_two_speakers_may_share_one_voice_profile` in `study-tts-testkit`,
    /// which proves a shared profile resolves through the pipeline but cannot
    /// count reads.
    /// A synthetic profile directory whose consent carries `consent_status`.
    ///
    /// Written by hand rather than through `study-tts-testkit`, which depends
    /// on this crate and so cannot be depended on back. Nothing here is real
    /// voice material: `reference.wav` is a fixed byte string that is never
    /// decoded, because the gate hashes both artifacts and parses neither.
    /// `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and access
    /// keeps real references out of the suite entirely.
    fn write_profile(root: &Path, profile_id: &str, consent_status: &str) {
        let dir = root.join(profile_id);
        fs::create_dir_all(&dir).expect("create a profile directory");

        let reference = b"synthetic-reference-v1".as_slice();
        let conditionals = b"synthetic-conditionals-v1".as_slice();
        fs::write(dir.join("reference.wav"), reference).expect("write a reference");
        fs::write(dir.join("conditionals.pt"), conditionals).expect("write conditioning");
        let reference_hash = blake3::hash(reference).to_hex().to_string();

        let profile = serde_json::json!({
            "schema_version": "0.1-voice",
            "profile_id": profile_id,
            "reference_wav_blake3": reference_hash,
            "conditionals_blake3": blake3::hash(conditionals).to_hex().to_string(),
            "extractor_identity": "synthetic-extractor-v1",
            "approval": "approved",
        });
        let consent = serde_json::json!({
            "schema_version": "0.1-voice",
            "declaration": "Synthetic test fixture; no human voice.",
            "permitted_use": ["voice_qualification"],
            "reference_wav_blake3": reference_hash,
            "created": "2026-08-31",
            "consent_status": consent_status,
            "rights_record_id": "rights-voice-owner-fallback-v1",
        });
        fs::write(
            dir.join("profile.json"),
            serde_json::to_vec(&profile).expect("serialize a profile record"),
        )
        .expect("write a profile record");
        fs::write(
            dir.join("consent.json"),
            serde_json::to_vec(&consent).expect("serialize a consent record"),
        )
        .expect("write a consent record");
    }

    #[test]
    fn t1_e1_a_root_whose_every_profile_is_rights_clean_is_admitted() {
        let root = TempDir::new().expect("create a voice root");
        write_profile(root.path(), "first-voice-v1", "granted");
        write_profile(root.path(), "second-voice-v1", "granted");

        admit_voice_root(root.path(), VoiceUse::VoiceQualification)
            .expect("two rights-clean profiles are admitted");
    }

    #[test]
    fn t1_e1_a_revoked_profile_the_request_never_names_refuses_the_root() {
        // The defect this closes: the worker loads every `conditionals.pt`
        // beneath the root during `initialize`, so a revoked profile is
        // deserialized whether or not any later request names it. Gating only
        // the selected voice left the rest of the root ungated.
        let root = TempDir::new().expect("create a voice root");
        write_profile(root.path(), "selected-voice-v1", "granted");
        write_profile(root.path(), "unrelated-voice-v1", "revoked");

        let error = admit_voice_root(root.path(), VoiceUse::VoiceQualification)
            .expect_err("a revoked profile anywhere in the root must refuse the run");

        assert!(
            matches!(error, BuildError::Voice(_)),
            "the refusal must come from the rights gate: {error:?}"
        );
    }

    #[test]
    fn t1_e1_an_altered_profile_the_request_never_names_refuses_the_root() {
        let root = TempDir::new().expect("create a voice root");
        write_profile(root.path(), "selected-voice-v1", "granted");
        write_profile(root.path(), "unrelated-voice-v1", "granted");
        fs::write(
            root.path()
                .join("unrelated-voice-v1")
                .join("conditionals.pt"),
            b"conditioning nobody approved",
        )
        .expect("alter the conditioning artifact");

        let error = admit_voice_root(root.path(), VoiceUse::VoiceQualification)
            .expect_err("conditioning that is not the recorded bytes must refuse the run");

        assert!(
            matches!(
                error,
                BuildError::VoiceProfile(VoiceProfileError::VoiceChecksumMismatch { .. })
            ),
            "the refusal must name the checksum: {error:?}"
        );
    }

    #[test]
    fn t1_e1_the_gate_skips_exactly_what_the_worker_skips() {
        // The other end of `_voice_conditioning`'s filter. Anything skipped
        // here that the worker still loads is a profile that reached
        // `torch.load` ungated, so these three cases pin the skip list rather
        // than the refusals: a stray file, a directory that is not a profile,
        // and a symlink the worker will not follow either.
        let root = TempDir::new().expect("create a voice root");
        write_profile(root.path(), "real-voice-v1", "granted");
        fs::write(root.path().join("README.txt"), b"not a profile").expect("write a stray file");
        fs::create_dir(root.path().join("scratch")).expect("create a non-profile directory");
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            root.path().join("real-voice-v1"),
            root.path().join("linked-voice-v1"),
        )
        .expect("link a profile directory");

        admit_voice_root(root.path(), VoiceUse::VoiceQualification)
            .expect("entries the worker never reads are skipped, not refused");
    }

    #[test]
    #[cfg(unix)]
    fn t1_e1_a_profile_name_that_is_not_utf8_refuses_the_root() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        // Skipping this entry was the defect. The worker reads the same name
        // through Python's `surrogateescape`, so `voice-\xff-v1` reaches it as
        // a string holding a lone surrogate — and a `profile.json` whose
        // `profile_id` carries that same surrogate compares equal to it, which
        // is the only agreement `_voice_conditioning` asks for before
        // `_load_backend` deserializes the artifact. The record is `{}` here
        // precisely because the refusal must precede reading it.
        let root = TempDir::new().expect("create a voice root");
        write_profile(root.path(), "real-voice-v1", "granted");
        let unnameable = root.path().join(OsStr::from_bytes(b"voice-\xff-v1"));
        fs::create_dir(&unnameable).expect("create a profile directory this build cannot spell");
        fs::write(unnameable.join("profile.json"), b"{}").expect("write a profile record");

        let error = admit_voice_root(root.path(), VoiceUse::VoiceQualification)
            .expect_err("a profile name this gate cannot read must refuse the root");

        assert!(
            matches!(
                error,
                BuildError::VoiceProfile(VoiceProfileError::VoiceProfileNameNotUtf8 { .. })
            ),
            "the refusal must name the profile directory this build cannot spell: {error:?}"
        );
    }

    #[test]
    fn t1_e1_one_voice_profile_is_loaded_once_however_many_speakers_name_it() {
        // `nadia` and `tom` both name `synthetic-test-voice-v1` over six
        // segments, so resolving per speaker loads twice and resolving per
        // segment loads six times — the two shapes this asserts against.
        let lesson = ValidatedLesson::from_json(
            "e0-s0-cache-identity.json",
            include_bytes!("../../../fixtures/lessons/e0-s0-cache-identity.json"),
        )
        .expect("the cache-identity fixture is a valid lesson");
        let mut loaded: Vec<String> = Vec::new();

        let resolved = resolve_by_profile(&lesson, |profile_id| {
            loaded.push(profile_id.to_owned());
            Ok(blake3::hash(profile_id.as_bytes()).into())
        })
        .expect("the counting loader resolves every speaker");

        assert_eq!(
            loaded,
            ["synthetic-test-voice-v1"],
            "profiles loaded, in order"
        );
        assert_eq!(
            resolved["nadia"], resolved["tom"],
            "two speakers naming one profile must receive one conditioning identity"
        );
    }
}
