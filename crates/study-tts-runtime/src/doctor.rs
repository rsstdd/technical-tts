//! What this machine can and cannot do, before a build depends on it.
//!
//! ADR-0001 §14 lists fourteen things `study-tts doctor` verifies. This
//! reports the four `DELIVERY-PLAN.md` E2-S5's named test pins — the
//! filesystem beneath the workspace, the external tools, the governed
//! checksums, and the core budget — and **names the rest rather than omitting
//! them**, because a reader cannot otherwise tell a check that passed from one
//! that never ran. That distinction is what §14 exists to protect.
//!
//! Four of the fourteen cannot be implemented here at all and say which story
//! owns them: ASR identities and ASR threads are E4's, pool size above one is
//! E5-S2's, and the end-to-end smoke render needs the governed model.

use std::{
    fmt,
    path::{Path, PathBuf},
};

use crate::{BuildError, io_error, model_gate, tools};

/// Filesystem types that must not host durable state.
///
/// ADR-0001 §14 requires that "job, cache, staging, and output directories
/// reside on the WSL2 Linux filesystem and are not DrvFS mounts such as
/// `/mnt/c`", and §12.5's recovery list names "refusal to place durable job,
/// cache, staging, or output roots on DrvFS". `DELIVERY-PLAN.md:1230` adds the
/// network case: "State is not shared across machines and does not reside on
/// DrvFS or a network filesystem."
///
/// `9p` is what WSL2 actually mounts `/mnt/c` as, which is why the name
/// `drvfs` alone would miss it on this machine — checked rather than assumed,
/// against `/proc/mounts`.
const REFUSED_FILESYSTEMS: [&str; 5] = ["9p", "drvfs", "cifs", "nfs", "nfs4"];

/// One thing `doctor` looked at, and what it found.
#[derive(Debug)]
pub struct Finding {
    /// What was checked, in ADR-0001 §14's own words where it has them.
    pub check: &'static str,
    /// What was found.
    pub verdict: Verdict,
}

/// The outcome of one check.
#[derive(Debug)]
pub enum Verdict {
    /// The environment satisfies it, with what was observed.
    Ok(String),
    /// The environment does not, with what is wrong.
    Refused(String),
    /// This build cannot answer it yet, and which story will.
    Blocked {
        /// The story that owns the check.
        owner: &'static str,
    },
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.verdict {
            Verdict::Ok(detail) => write!(formatter, "  ok       {}: {detail}", self.check),
            Verdict::Refused(detail) => write!(formatter, "  REFUSED  {}: {detail}", self.check),
            Verdict::Blocked { owner } => {
                write!(formatter, "  blocked  {}: owned by {owner}", self.check)
            }
        }
    }
}

/// Reports what this machine can do, without changing anything.
///
/// Read-only by construction: every path is resolved rather than created, and
/// the tools are identified by the preflight discipline `tools::inspect`
/// already applies — resolved and version-probed before use, with the binary
/// that answered recorded rather than the one requested.
///
/// # Errors
///
/// None from the checks themselves: a check that cannot be satisfied is a
/// [`Verdict::Refused`] rather than an error, because `doctor` exists to report
/// a bad environment rather than to fail in one. Only reading `/proc/mounts`
/// can fail, and it fails as [`crate::IoError::FileSystem`].
pub fn diagnose(workspace: &Path, model_root: Option<&Path>) -> Result<Vec<Finding>, BuildError> {
    let mut findings = vec![filesystem_finding(workspace)?];
    findings.extend(tool_findings());
    findings.push(model_checksum_finding(model_root));
    findings.push(core_budget_finding());
    findings.extend(blocked_findings());
    Ok(findings)
}

/// Whether the governed model's declared artifacts still hash to what this
/// build pinned.
///
/// `verify_model_artifacts` is the same gate a build passes, so a `doctor`
/// that reported clean here and a build that then refused would be the one
/// outcome this check exists to prevent.
///
/// Without `--model-root` the answer is refused rather than assumed: a check
/// that silently skipped would read exactly like one that passed.
fn model_checksum_finding(model_root: Option<&Path>) -> Finding {
    const CHECK: &str = "model and voice-profile checksums";
    let Some(root) = model_root else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "not checked; pass `--model-root` to verify the governed model".to_owned(),
            ),
        };
    };
    Finding {
        check: CHECK,
        verdict: match model_gate::verify_model_artifacts(root) {
            Ok(proven) => Verdict::Ok(format!(
                "revision {}, artifacts {}",
                proven.revision.as_str(),
                proven.artifacts_hash.as_str()
            )),
            Err(error) => Verdict::Refused(error.to_string()),
        },
    }
}

/// Whether durable state would land on a filesystem that must not hold it.
fn filesystem_finding(workspace: &Path) -> Result<Finding, BuildError> {
    const CHECK: &str = "durable roots are not DrvFS or a network filesystem";
    let mounts = PathBuf::from("/proc/mounts");
    let Ok(table) = std::fs::read_to_string(&mounts) else {
        return Ok(Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "/proc/mounts is unreadable, so the filesystem cannot be identified".to_owned(),
            ),
        });
    };
    let absolute = std::path::absolute(workspace).map_err(|error| io_error(workspace, error))?;

    // The longest mount point that is a prefix of the path is the one that
    // holds it. Shorter prefixes are ancestors, and `/` matches everything.
    let mut best: Option<(usize, &str)> = None;
    for line in table.lines() {
        let mut fields = line.split_whitespace();
        let (Some(_device), Some(point), Some(kind)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        if absolute.starts_with(point) && best.is_none_or(|(len, _)| point.len() > len) {
            best = Some((point.len(), kind));
        }
    }

    let Some((_, kind)) = best else {
        return Ok(Finding {
            check: CHECK,
            verdict: Verdict::Refused(format!("no mount holds `{}`", absolute.display())),
        });
    };
    Ok(Finding {
        check: CHECK,
        verdict: if REFUSED_FILESYSTEMS.contains(&kind) {
            Verdict::Refused(format!(
                "`{}` is on `{kind}`; durable state must live on the WSL2 Linux filesystem",
                absolute.display()
            ))
        } else {
            Verdict::Ok(format!("`{}` is on `{kind}`", absolute.display()))
        },
    })
}

/// FFmpeg and ffprobe, resolved and version-probed before anything runs.
fn tool_findings() -> Vec<Finding> {
    [
        ("FFmpeg is present and answers `-version`", "ffmpeg"),
        ("ffprobe is present and answers `-version`", "ffprobe"),
    ]
    .into_iter()
    .map(|(check, tool)| Finding {
        check,
        verdict: match tools::inspect(tool, Path::new(tool)) {
            // The binary that answered, not the name asked for: a `doctor`
            // that reported the request would not say which build ran.
            Ok(identity) => Verdict::Ok(format!(
                "{} — {}",
                identity.resolved_executable.display(),
                identity.version
            )),
            Err(error) => Verdict::Refused(error.to_string()),
        },
    })
    .collect()
}

/// The core budget ADR-0001 §10.1 fixes.
///
/// "Obtains WSL-visible physical-core topology from `lscpu`. If topology is
/// unavailable, it uses half the visible logical processors, with a minimum of
/// one. It reserves one physical core when more than one is available."
fn core_budget_finding() -> Finding {
    const CHECK: &str = "physical-core topology and reserved-core policy";
    let (physical, source) = match physical_cores() {
        Some(cores) => (cores, "lscpu"),
        None => (
            std::thread::available_parallelism().map_or(1, |logical| (logical.get() / 2).max(1)),
            "half the visible logical processors, topology being unavailable",
        ),
    };
    let available = if physical > 1 { physical - 1 } else { physical };
    Finding {
        check: CHECK,
        verdict: Verdict::Ok(format!(
            "{physical} physical ({source}); {available} available after reserving one"
        )),
    }
}

/// Physical cores, from `lscpu -p=CORE`, or `None` when it cannot say.
fn physical_cores() -> Option<usize> {
    let resolved = tools::resolve_executable(Path::new("lscpu"))?;
    let output = std::process::Command::new(resolved)
        .arg("-p=CORE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let listing = String::from_utf8(output.stdout).ok()?;
    let mut cores: Vec<&str> = listing
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    cores.sort_unstable();
    cores.dedup();
    (!cores.is_empty()).then_some(cores.len())
}

/// The checks this build cannot answer, and who will.
///
/// Present rather than omitted. A `doctor` that listed only what it could
/// check would let a reader mistake an unrun check for a passing one, which is
/// the failure ADR-0001 §14's enumeration exists to prevent.
fn blocked_findings() -> Vec<Finding> {
    [
        ("pinned whisper-rs and ASR-model identities", "E4"),
        ("ASR thread budget", "E4"),
        ("worker pool size above one", "E5-S2"),
        (
            "short end-to-end smoke render",
            "E6-S1; it needs the governed model",
        ),
    ]
    .into_iter()
    .map(|(check, owner)| Finding {
        check,
        verdict: Verdict::Blocked { owner },
    })
    .collect()
}
