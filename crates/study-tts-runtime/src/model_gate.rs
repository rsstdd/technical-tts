//! The governed model root's bytes, proven before a worker is started.
//!
//! ADR-0001 §12.5 keys every cache entry on the model revision, and until this
//! existed that revision was a *string* — one the worker read out of
//! `bundle-manifest.json` in the governed root and reported back. Nothing
//! hashed the weights it actually loaded, so replacing them under an unchanged
//! record left every cache key where it was: stale audio would be reused, and
//! new audio published under a key describing bytes that did not produce it. A
//! content-addressed cache must never do that. Raised as issue #66 and as the
//! model half of the 2026-08-31 audit's sixth finding.
//!
//! # Why the digests live here and not beside the weights
//!
//! `bundle-manifest.json` in the governed model root already declares each
//! artifact's SHA-256, and checking against it alone would be trust on first
//! use: whoever can replace the weights can replace the record describing
//! them. The authoritative list therefore has to be in Git, and once it is,
//! parsing the governed record adds nothing a reader could rely on — it only
//! adds a second parser of a format `worker/study_tts_worker/worker.py`
//! already reads, which is a drift surface rather than a control.
//!
//! Pinning these digests in Git leaks nothing.
//! `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` keeps governed *locations*
//! and *bytes* out of the repository, not the checksums of public third-party
//! weights. **It does not extend to voice digests**, which sit beside personal
//! data and stay in the governed voice root where `voice_gate` reads them.
//!
//! # The key term, and why it exists as well as the refusal
//!
//! Refusing unproven bytes and keying on them are separable, and for a long
//! time only the first was done here: a legitimate weights change moves
//! [`PINNED_MODEL_REVISION`] and therefore the key already, while an
//! illegitimate one is refused outright. The project owner directed on
//! 2026-08-31 that the key term be deferred on exactly that reasoning.
//!
//! That direction was reversed before G1, and the gap it leaves is narrow but
//! real: the revision names an *acquisition* and the digests name its *bytes*,
//! and a commit that edits [`DECLARED_MODEL_ARTIFACTS`] without moving
//! [`PINNED_MODEL_REVISION`] moves the bytes while the key stands still. Audio
//! rendered from the old weights is then reused for the new ones.
//! [`model_artifacts_hash`] closes that, and reaches
//! [`study_tts_core::SynthesisContext`]. The ADR-0001 §12.5 amendment the same
//! 2026-08-31 direction required is
//! `docs/adr/deviations/ADR-0001-D011-model-artifacts-key-input.md`.
//!
//! # What this does not do
//!
//! It also cannot close the window between hashing a file and the worker
//! opening it. The model root is governed and read-only in practice, and an
//! attacker who can rewrite it during a build can rewrite it before one.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use sha2::{Digest, Sha256};
use study_tts_core::{CanonicalValue, ModelArtifactsHash, Revision, canonical_digest};

use crate::error::{BuildError, ModelArtifactError};

/// The acquisition this build runs against, transcribed from the governed
/// `bundle-manifest.json` `model.revision`.
///
/// Two-sided with `docs/operations/REVIEW-AND-ACCEPT-CYCLE.md` §The model root
/// is pinned in Git, and that is deliberate, which names this constant in
/// return. ADR-0002 owns the qualified revision: changing this is a
/// governed-backend change and re-qualification, never an edit made to get a
/// failing gate to pass.
pub const PINNED_MODEL_REVISION: &str = "1b475dffa71fb191cb6d5901215eb6f55635a9b6";

/// One inference-affecting file the model root must hold, and its bytes.
///
/// Size as well as digest, so a truncated or padded file is refused by the
/// cheap check before 3 GB are read — and so a mismatch can say which of the
/// two failed rather than only that something did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredArtifact {
    /// File name beneath the revision directory.
    pub name: &'static str,
    /// Exact size in bytes.
    pub bytes: u64,
    /// Lowercase hexadecimal SHA-256 of the file's bytes.
    ///
    /// SHA-256 rather than the BLAKE3 `voice_gate` uses, because these are
    /// transcribed from a governed acquisition record that recorded SHA-256.
    /// Re-hashing a ratified record in another algorithm to suit this crate's
    /// dependency list would be the wrong way round.
    pub sha256: &'static str,
}

/// Every artifact `ChatterboxTTS.from_local` reads, for
/// [`PINNED_MODEL_REVISION`].
///
/// Transcribed from `model.artifacts` in the governed model root's
/// `bundle-manifest.json`, which `scripts/qualification/chatterbox_spike.py`
/// wrote at acquisition. Undeclared extra files in the root are ignored rather
/// than refused: the layout belongs to ResembleAI, not to this project, and a
/// file the loader never reads cannot change what it renders.
pub const DECLARED_MODEL_ARTIFACTS: [DeclaredArtifact; 4] = [
    DeclaredArtifact {
        name: "s3gen.safetensors",
        bytes: 1_056_484_620,
        sha256: "2b78103c654207393955e4900aac14a12de8ef25f4b09424f1ef91941f161d4e",
    },
    DeclaredArtifact {
        name: "t3_cfg.safetensors",
        bytes: 2_129_653_744,
        sha256: "914cb1696f47527fe8852ca8f1fe1fa63cb34f76f9c715e84e067b744dd0da81",
    },
    DeclaredArtifact {
        name: "tokenizer.json",
        bytes: 25_470,
        sha256: "d71e3a44eabb1784df9a68e9f95b251ecbf1a7af6a9f50835856b2ca9d8c14a5",
    },
    DeclaredArtifact {
        name: "ve.safetensors",
        bytes: 5_695_784,
        sha256: "f0921cab452fa278bc25cd23ffd59d36f816d7dc5181dd1bef9751a7fb61f63c",
    },
];

/// Proves the governed model root holds the bytes this build is pinned to.
///
/// Called before a worker is started, never after: a worker that has loaded
/// unproven weights has already produced audio under an identity nothing
/// checked, and refusing then would only decide what to do with it.
///
/// **The ordering is the type system's, not a test's.** Every launch field of
/// [`crate::WorkerConfiguration`] is private and
/// [`crate::WorkerConfiguration::for_bundle`] is the only way to obtain one for
/// a bundle, so a configuration that could start a real worker cannot exist
/// unless this returned `Ok`. `rust-testing` puts an invariant the compiler can
/// make unrepresentable ahead of one a test asserts, and this is one — which is
/// also why no test calls `for_bundle`: doing so would need the restored
/// interpreter and the governed model root, neither of which CI has.
///
/// Returns the revision *and* the artifact identity it verified, so a caller
/// carries the values it proved rather than ones it read somewhere else.
/// [`crate::WorkerTtsExecutor::start`] compares the revision against what the
/// worker answers with, which is what stops the worker loading a different
/// revision than the one hashed here. The artifact identity has no such
/// counterpart and needs none: the worker reads a record and cannot answer for
/// bytes, which is why this side is where it comes from.
///
/// # Errors
///
/// [`ModelArtifactError::MissingModelArtifact`] when a declared file is absent,
/// [`ModelArtifactError::ModelArtifactNotRegularFile`] when its name holds
/// something else, [`ModelArtifactError::ModelArtifactSizeMismatch`] and
/// [`ModelArtifactError::ModelArtifactChecksumMismatch`] when the bytes are not
/// the pinned ones, or [`crate::IoError::FileSystem`] when artifact metadata
/// cannot be read for another reason. Each routes to the engineering and
/// project owners, who decide a model revision per
/// `docs/governance/ROUTING-TABLES.md` §Decision routing.
pub fn verify_model_artifacts(model_root: &Path) -> Result<ProvenModel, BuildError> {
    let revision_root = model_root.join(format!("model-{PINNED_MODEL_REVISION}"));
    for artifact in &DECLARED_MODEL_ARTIFACTS {
        verify_artifact(&revision_root, artifact)?;
    }
    Ok(ProvenModel {
        revision: PINNED_MODEL_REVISION
            .parse()
            .expect("the pinned model revision is a well-formed revision"),
        artifacts_hash: model_artifacts_hash(),
    })
}

/// What one governed model root was proven to hold.
///
/// Returned together because they are one fact about one verified root, and a
/// caller that took the revision alone would be free to pair it with a hash
/// derived somewhere else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenModel {
    /// The revision whose directory was hashed.
    pub revision: Revision,
    /// The identity of the bytes that were hashed.
    pub artifacts_hash: ModelArtifactsHash,
}

/// The identity of the artifact bytes this build is pinned to.
///
/// Derived from [`DECLARED_MODEL_ARTIFACTS`] rather than from the files, and
/// that is not a shortcut: `verify_model_artifacts` has already proven the
/// files *are* these bytes, so hashing them again would re-read 3 GB to learn
/// what the constant already states. What reaches the key is the declaration
/// the gate enforced.
///
/// Derived rather than pinned as a second constant, for the same reason. A
/// pinned value would be a third thing to keep in step with the digests and the
/// revision, and the first edit that moved the digests and forgot it would
/// reintroduce exactly the defect this closes.
///
/// Hashed through [`canonical_digest`] like every other identity here, so the
/// byte form is owned by `study-tts-core` rather than invented at this call
/// site. The name is included beside each digest: two artifacts swapping names
/// is a different model root, and hashing the digests alone would not see it.
pub fn model_artifacts_hash() -> ModelArtifactsHash {
    artifacts_hash(&DECLARED_MODEL_ARTIFACTS)
}

/// The identity of one declared artifact list.
///
/// Split from [`model_artifacts_hash`] so its properties can be driven with
/// synthetic lists. The constant cannot be varied, and a test that asserted the
/// real hash against a copied literal would be a second copy of this function
/// rather than a check on it.
fn artifacts_hash(artifacts: &[DeclaredArtifact]) -> ModelArtifactsHash {
    let declared = artifacts
        .iter()
        .map(|artifact| {
            CanonicalValue::object([
                ("name", CanonicalValue::Text(artifact.name.to_owned())),
                ("sha256", CanonicalValue::Text(artifact.sha256.to_owned())),
                ("bytes", CanonicalValue::Unsigned(artifact.bytes)),
            ])
        })
        .collect();

    canonical_digest(&CanonicalValue::object([(
        "artifacts",
        CanonicalValue::Array(declared),
    )]))
    .to_hex()
    .to_string()
    .parse()
    .expect("a BLAKE3 hex digest parses as a model artifacts hash")
}

/// Proves one declared artifact, cheapest check first.
fn verify_artifact(revision_root: &Path, artifact: &DeclaredArtifact) -> Result<(), BuildError> {
    let path = revision_root.join(artifact.name);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Err(ModelArtifactError::MissingModelArtifact {
                root: revision_root.to_path_buf(),
                artifact: artifact.name,
            }
            .into());
        }
        Err(error) => return Err(crate::io_error(&path, error)),
    };
    // A link would supply both sides of the comparison from one file outside
    // the root, exactly as `voice_gate::read_record` refuses it to: the gate
    // would then agree with itself about bytes the acquisition never approved.
    if !metadata.is_file() {
        return Err(ModelArtifactError::ModelArtifactNotRegularFile {
            root: revision_root.to_path_buf(),
            artifact: artifact.name,
        }
        .into());
    }
    if metadata.len() != artifact.bytes {
        return Err(ModelArtifactError::ModelArtifactSizeMismatch {
            path,
            declared: artifact.bytes,
            found: metadata.len(),
        }
        .into());
    }

    let digest = sha256_of(&path)?;
    if digest != artifact.sha256 {
        return Err(ModelArtifactError::ModelArtifactChecksumMismatch { path }.into());
    }
    Ok(())
}

/// Streams a file into SHA-256, in lowercase hexadecimal.
///
/// Streamed rather than read whole: the largest declared artifact is 2 GB, and
/// reading it into memory to hash it would be a resource ceiling this build has
/// no reason to spend.
fn sha256_of(path: &Path) -> Result<String, BuildError> {
    let file = fs::File::open(path).map_err(|source| crate::io_error(path, source))?;
    let mut reader = std::io::BufReader::new(file);
    let mut digest = Sha256::new();
    std::io::copy(&mut reader, &mut digest).map_err(|source| crate::io_error(path, source))?;
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use tempfile::TempDir;

    /// A synthetic model root holding `contents` for every declared artifact.
    ///
    /// Synthetic and tiny: these cases are about the gate's decisions, and
    /// `docs/testing/TEST-STRATEGY.md` keeps real weights out of the suite
    /// entirely. The sizes therefore never match, which is why every case here
    /// asserts a refusal — the accepting case is `t5_` on the reference
    /// machine, where the real bytes are.
    fn model_root(contents: &[u8]) -> TempDir {
        let root = TempDir::new().expect("create a model root");
        let revision = root.path().join(format!("model-{PINNED_MODEL_REVISION}"));
        fs::create_dir_all(&revision).expect("create the revision directory");
        for artifact in &DECLARED_MODEL_ARTIFACTS {
            let mut file =
                fs::File::create(revision.join(artifact.name)).expect("create an artifact");
            file.write_all(contents).expect("write an artifact");
        }
        root
    }

    /// The revision directory beneath a synthetic model root.
    fn revision_root(root: &TempDir) -> std::path::PathBuf {
        root.path().join(format!("model-{PINNED_MODEL_REVISION}"))
    }

    #[test]
    fn t1_e1_an_artifact_of_the_wrong_size_is_refused_before_it_is_hashed() {
        let root = model_root(b"not the governed weights");

        let error = verify_model_artifacts(root.path())
            .expect_err("bytes that are not the pinned ones must be refused");

        assert!(
            matches!(
                error,
                BuildError::ModelArtifacts(ModelArtifactError::ModelArtifactSizeMismatch { .. })
            ),
            "the refusal must name the size: {error:?}"
        );
    }

    #[test]
    fn t1_e1_an_absent_artifact_is_refused() {
        let root = model_root(b"");
        let missing = DECLARED_MODEL_ARTIFACTS[0].name;
        fs::remove_file(revision_root(&root).join(missing)).expect("remove an artifact");

        let error = verify_model_artifacts(root.path())
            .expect_err("a declared artifact that is absent must be refused");

        assert!(
            matches!(
                &error,
                BuildError::ModelArtifacts(ModelArtifactError::MissingModelArtifact {
                    artifact,
                    ..
                }) if *artifact == missing
            ),
            "the refusal must name the missing artifact: {error:?}"
        );
    }

    #[test]
    fn t4_e1_a_non_missing_metadata_failure_preserves_the_filesystem_error() {
        let root = TempDir::new().expect("create a model root");
        let revision_file = root.path().join("revision-is-a-file");
        fs::write(&revision_file, b"not a directory").expect("write the revision file");
        let artifact = &DECLARED_MODEL_ARTIFACTS[0];
        let artifact_path = revision_file.join(artifact.name);

        let error = verify_artifact(&revision_file, artifact)
            .expect_err("a non-missing metadata failure must remain an IO error");

        match error {
            BuildError::Io(crate::IoError::FileSystem { path, source }) => {
                assert_eq!(path, artifact_path);
                assert_eq!(source.kind(), ErrorKind::NotADirectory);
            }
            other => panic!("the filesystem error must be preserved: {other:?}"),
        }
    }

    #[test]
    fn t1_e1_an_artifact_that_is_a_symlink_is_refused_rather_than_followed() {
        // The link would supply both sides of the comparison from one file
        // outside the root, and the gate would agree with itself about bytes
        // the acquisition never approved. `voice_gate::read_record` refuses a
        // link for the same reason.
        let root = model_root(b"");
        let linked = DECLARED_MODEL_ARTIFACTS[0].name;
        let elsewhere = root.path().join("elsewhere.safetensors");
        fs::write(&elsewhere, b"whatever this build would have hashed")
            .expect("write the link target");
        let planted = revision_root(&root).join(linked);
        fs::remove_file(&planted).expect("remove an artifact");
        std::os::unix::fs::symlink(&elsewhere, &planted).expect("plant the link");

        let error = verify_model_artifacts(root.path())
            .expect_err("a declared artifact that is a link must be refused");

        assert!(
            matches!(
                &error,
                BuildError::ModelArtifacts(ModelArtifactError::ModelArtifactNotRegularFile {
                    artifact,
                    ..
                }) if *artifact == linked
            ),
            "the refusal must name the artifact that is not a regular file: {error:?}"
        );
    }

    #[test]
    fn t1_e1_the_declared_artifacts_match_the_governed_acquisition_record() {
        // A transcription check, not a behavior one: these four entries are
        // copied from `model.artifacts` in the governed root's
        // `bundle-manifest.json`, and a typo in one would refuse every real
        // build with a message pointing at the weights rather than at this
        // table. The record itself is not in Git, so this pins the shape and
        // the count; the values are proven against the bytes by `t5_`.
        assert_eq!(DECLARED_MODEL_ARTIFACTS.len(), 4);
        for artifact in &DECLARED_MODEL_ARTIFACTS {
            assert_eq!(
                artifact.sha256.len(),
                64,
                "{} carries a SHA-256 digest",
                artifact.name
            );
            assert!(
                artifact
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()),
                "{} is spelled in lowercase hexadecimal",
                artifact.name
            );
            assert!(artifact.bytes > 0, "{} declares a size", artifact.name);
        }
    }

    /// Two artifacts, enough to tell an ordering apart from a swap.
    const PAIR: [DeclaredArtifact; 2] = [
        DeclaredArtifact {
            name: "first.safetensors",
            bytes: 10,
            sha256: "a1",
        },
        DeclaredArtifact {
            name: "second.safetensors",
            bytes: 20,
            sha256: "b2",
        },
    ];

    #[test]
    fn t1_e1_every_declared_artifact_field_changes_the_model_identity() {
        // Each field separately, because each one is a way the model root can
        // differ: a digest is different bytes, a size is a truncated or padded
        // file, and a name is a different file entirely.
        let baseline = artifacts_hash(&PAIR);

        let mut digest_moved = PAIR;
        digest_moved[0].sha256 = "c3";
        let mut size_moved = PAIR;
        size_moved[0].bytes = 11;
        let mut name_moved = PAIR;
        name_moved[0].name = "third.safetensors";

        for (field, artifacts) in [
            ("sha256", digest_moved),
            ("bytes", size_moved),
            ("name", name_moved),
        ] {
            assert_ne!(
                artifacts_hash(&artifacts).as_str(),
                baseline.as_str(),
                "changing `{field}` must change the model identity"
            );
        }
    }

    #[test]
    fn t1_e1_two_artifacts_swapping_names_is_a_different_model_identity() {
        // The case that motivates hashing the name beside the digest rather
        // than the digests alone. The multiset of digests is unchanged here;
        // which file each one describes is not, and that is a different root.
        let mut swapped = PAIR;
        swapped[0].name = PAIR[1].name;
        swapped[1].name = PAIR[0].name;

        assert_ne!(
            artifacts_hash(&swapped).as_str(),
            artifacts_hash(&PAIR).as_str()
        );
    }

    #[test]
    fn t1_e1_the_model_identity_is_stable_across_derivations() {
        // It reaches every cache key, so a derivation that varied between two
        // calls in one process would re-key the cache from nothing.
        assert_eq!(model_artifacts_hash(), model_artifacts_hash());
        assert_eq!(model_artifacts_hash().as_str().len(), 64);
    }
}
