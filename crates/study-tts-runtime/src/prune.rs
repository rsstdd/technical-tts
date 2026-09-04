//! Which synthesis-cache entries are still referenced, and which are not.
//!
//! ADR-0001 §12.2: "Cache pruning treats every artifact referenced by an
//! accepted takes file or published manifest as live." This module computes
//! that reachability and reports it. It deletes nothing.
//!
//! Report-only is the whole scope, and deliberately so. ADR-0001 §15.4 makes
//! prune operations dry-run by default and says published outputs are never
//! pruned by a cache command; `AGENTS.md` §Autonomy puts prune without
//! `--dry-run` behind an explicit human decision. The `study-tts cache prune`
//! command that acts on this report belongs to E2-S5.
//!
//! # Compatibility limitation
//!
//! Every published manifest is a retention root, and a root this build cannot
//! decode is an error rather than an empty contribution. A workspace holding
//! **one** package written before `manifest` `2.0` therefore refuses retention
//! reporting for the **whole workspace**, not merely for that lesson, until it
//! is rebuilt. `docs/operations/UPGRADE-RUNBOOK.md` §Known compatibility
//! limitations records that for operators and names this module.
//!
//! Softening it to a skip is the one change that must not be made here:
//! treating an unreadable root as "references nothing" reports live artifacts
//! as prunable.
//!
//! Two-sided coupling: `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
//! §Retention records "Synthesis cache — Retain while referenced by accepted
//! takes or published manifests", and its §Enforcement table names
//! `t4_e2_selected_artifact_survives_prune`. That document names this module in
//! return.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use study_tts_core::{CacheKey, ValidatedTakes};

use crate::{BuildError, cache, managed, manifest, preview};

/// One cache entry no retention root refers to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PruneCandidate {
    /// The synthesis identity the entry is filed under.
    pub cache_key: CacheKey,
    /// The entry directory a prune command would act on.
    pub entry_dir: PathBuf,
}

/// Every cache entry a retention root refers to.
///
/// The union ADR-0001 §12.2 names: the selected artifact of every accepted
/// takes document, and every segment of every published manifest. Accepted
/// takes documents are passed in rather than discovered, because ADR-0001
/// §12.1 puts them beside the *lesson* — outside the workspace this reads, and
/// known only to the operator invoking the command.
///
/// # Errors
///
/// Whatever [`preview::published_manifests`] and
/// [`manifest::referenced_cache_keys`] report. A root that cannot be read is an
/// error rather than an empty contribution: reading an unreadable manifest as
/// "references nothing" would report live artifacts as prunable, which is a
/// report today and data loss once E2-S5 makes prune destructive.
pub fn live_cache_keys(
    workspace: &Path,
    accepted_takes: &[ValidatedTakes],
) -> Result<BTreeSet<CacheKey>, BuildError> {
    let mut live: BTreeSet<CacheKey> = accepted_takes
        .iter()
        .flat_map(ValidatedTakes::selections)
        .map(|selection| selection.selected_cache_key.clone())
        .collect();

    for manifest_path in preview::published_manifests(workspace)? {
        live.extend(manifest::referenced_cache_keys(&manifest_path)?);
    }
    Ok(live)
}

/// Every cache entry no retention root refers to, in a stable order.
///
/// A report. Nothing is deleted, moved, or created, and the caller decides what
/// to do with the answer — see this module's own documentation for why that
/// separation is the scope rather than an omission.
///
/// # Errors
///
/// Whatever [`live_cache_keys`] and [`cache::published_entries`] report,
/// including [`crate::CacheError::UnrecognizedCacheEntry`] for a directory
/// inside the cache tree that no cache key names.
pub fn prune_candidates(
    workspace: &Path,
    accepted_takes: &[ValidatedTakes],
) -> Result<Vec<PruneCandidate>, BuildError> {
    let live = live_cache_keys(workspace, accepted_takes)?;
    let cache_root = managed::directory_candidate(workspace, "cache")?;
    if !cache_root.is_dir() {
        return Ok(Vec::new());
    }

    Ok(cache::published_entries(&cache_root)?
        .into_iter()
        .filter(|(cache_key, _)| !live.contains(cache_key))
        .map(|(cache_key, entry_dir)| PruneCandidate {
            cache_key,
            entry_dir,
        })
        .collect())
}
