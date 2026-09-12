//! What this machine can and cannot do, before a build depends on it.
//!
//! ADR-0001 §14 lists fourteen things `study-tts doctor` verifies. Ten are
//! reported here and four cannot be answered by this build — they **name the
//! story that owns them** rather than being omitted, because a reader cannot
//! otherwise tell a check that passed from one that never ran. That
//! distinction is what §14's enumeration exists to protect, and the same
//! reason a check missing its `--model-root` or `--bundle-root` is reported
//! refused rather than skipped.
//!
//! The four that wait: ASR identities and the ASR thread budget are E4's, pool
//! size above one is E5-S2's, and the end-to-end smoke render needs the
//! governed model.

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
pub fn diagnose(
    workspace: &Path,
    model_root: Option<&Path>,
    bundle_root: Option<&Path>,
) -> Result<Vec<Finding>, BuildError> {
    let mut findings = vec![host_finding(), architecture_finding()];
    findings.push(filesystem_finding(workspace)?);
    findings.push(writable_finding(workspace));
    findings.push(disk_space_finding(workspace));
    findings.extend(tool_findings());
    findings.push(model_checksum_finding(model_root));
    findings.push(worker_bundle_finding(bundle_root));
    findings.push(device_finding(bundle_root));
    findings.push(core_budget_finding());
    findings.push(offline_finding());
    findings.extend(blocked_findings());
    Ok(findings)
}

/// WSL2, and the Ubuntu it is running.
fn host_finding() -> Finding {
    const CHECK: &str = "WSL2 and supported Ubuntu version";
    let kernel = std::fs::read_to_string("/proc/version").unwrap_or_default();
    let release = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let named = |key: &str| {
        release
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim_matches('"').to_owned())
    };
    let distribution = named("NAME=").unwrap_or_else(|| "unknown".to_owned());
    let version = named("VERSION_ID=").unwrap_or_else(|| "unknown".to_owned());

    // `microsoft-standard-WSL2` is what the WSL2 kernel calls itself. Matched
    // on the kernel string rather than an environment variable, which a shell
    // can set and a kernel cannot.
    Finding {
        check: CHECK,
        verdict: if kernel.contains("WSL2") {
            Verdict::Ok(format!("{distribution} {version} on WSL2"))
        } else {
            Verdict::Refused(format!(
                "{distribution} {version}, and the kernel does not report WSL2; \
                 ADR-0001 §2 fixes WSL2 as the deployment target"
            ))
        },
    }
}

/// The operating system and architecture this binary was built for.
fn architecture_finding() -> Finding {
    const CHECK: &str = "supported OS and architecture";
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    Finding {
        check: CHECK,
        verdict: if os == "linux" && arch == "x86_64" {
            Verdict::Ok(format!("{os} {arch}"))
        } else {
            Verdict::Refused(format!("{os} {arch}; this build targets linux x86_64"))
        },
    }
}

/// Whether durable state could actually be written where it is meant to go.
///
/// Probed by writing rather than by reading permission bits: a mode that looks
/// writable and a filesystem mounted read-only disagree, and the second is the
/// one that stops a build. The probe file is removed immediately, and a
/// workspace that does not exist yet is reported rather than created — `doctor`
/// answers questions and does not prepare the ground.
fn writable_finding(workspace: &Path) -> Finding {
    const CHECK: &str = "the workspace is writable";
    if !workspace.is_dir() {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(format!("`{}` is not a directory", workspace.display())),
        };
    }
    let probe = workspace.join(".study-tts-doctor-probe");
    Finding {
        check: CHECK,
        verdict: match std::fs::write(&probe, b"") {
            Ok(()) => {
                let _ = std::fs::remove_file(&probe);
                Verdict::Ok(format!("`{}` accepts a write", workspace.display()))
            }
            Err(error) => Verdict::Refused(format!("`{}`: {error}", workspace.display())),
        },
    }
}

/// Free space where durable state will land.
///
/// `df` for the same reason `lscpu` answers topology: the number wanted is the
/// one the operating system reports for that mount, and `std` exposes no
/// syscall for it. Resolved through the same preflight path, so a `df` that is
/// absent is reported rather than guessed around.
fn disk_space_finding(workspace: &Path) -> Finding {
    const CHECK: &str = "free disk space";
    let Some(resolved) = tools::resolve_executable(Path::new("df")) else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused("`df` is not on PATH, so free space is unknown".to_owned()),
        };
    };
    let output = std::process::Command::new(resolved)
        .arg("-h")
        .arg("--output=avail,target")
        .arg(workspace)
        .output();
    Finding {
        check: CHECK,
        verdict: match output {
            Ok(result) if result.status.success() => String::from_utf8(result.stdout)
                .ok()
                .and_then(|listing| listing.lines().nth(1).map(str::trim).map(ToOwned::to_owned))
                .map_or_else(
                    || Verdict::Refused("`df` reported nothing for this path".to_owned()),
                    Verdict::Ok,
                ),
            _ => Verdict::Refused("`df` could not report free space".to_owned()),
        },
    }
}

/// The worker bundle's identity, which every synthesis key depends on.
fn worker_bundle_finding(bundle_root: Option<&Path>) -> Finding {
    const CHECK: &str = "worker runtime and locked dependencies";
    let Some(root) = bundle_root else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "not checked; pass `--bundle-root` to verify the worker bundle".to_owned(),
            ),
        };
    };
    Finding {
        check: CHECK,
        verdict: match crate::worker_bundle::WorkerBundle::load(root)
            .and_then(|bundle| bundle.verified_hash())
        {
            Ok(hash) => Verdict::Ok(format!("bundle identity {}", hash.as_str())),
            Err(error) => Verdict::Refused(error.to_string()),
        },
    }
}

/// The device the launcher declares, which is what the worker will use.
fn device_finding(bundle_root: Option<&Path>) -> Finding {
    const CHECK: &str = "GPU or CPU device compatibility";
    let Some(root) = bundle_root else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "not checked; pass `--bundle-root` to read the launcher".to_owned(),
            ),
        };
    };
    let launcher = root.join("worker").join("launcher.json");
    Finding {
        check: CHECK,
        verdict: match std::fs::read(&launcher)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|document| document.get("device")?.as_str().map(ToOwned::to_owned))
        {
            Some(device) => {
                Verdict::Ok(format!("`{device}`, as `{}` declares", launcher.display()))
            }
            None => Verdict::Refused(format!("`{}` declares no device", launcher.display())),
        },
    }
}

/// That this build reaches no network, which is a property rather than a probe.
///
/// ADR-0001 §2 makes the render path offline and nothing here opens a socket,
/// so there is no state to sample: a check that pinged something would be
/// testing the network rather than this build. What is reported is the fact and
/// where it is fixed, so a reader can verify the claim rather than take it.
fn offline_finding() -> Finding {
    Finding {
        check: "offline mode",
        verdict: Verdict::Ok(
            "the render path opens no socket; ADR-0001 §2 fixes it and the T1–T4 suites run \
             offline"
                .to_owned(),
        ),
    }
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

/// Every external binary ADR-0001 §14 names, resolved and version-probed.
///
/// `gcc`, `cmake`, and `python3` join FFmpeg and ffprobe because §14 asks for
/// "successful execution and parsed versions" of all of them: the first three
/// build the worker's locked dependencies, and a machine that cannot run them
/// cannot restore the bundle it renders through.
fn tool_findings() -> Vec<Finding> {
    [
        ("FFmpeg reports a version", "ffmpeg", "-version"),
        ("ffprobe reports a version", "ffprobe", "-version"),
        ("cmake reports a version", "cmake", "-version"),
        // Both refuse the single-dash form outright, which is why the flag is
        // carried per tool rather than assumed.
        ("gcc reports a version", "gcc", "--version"),
        ("python3 reports a version", "python3", "--version"),
    ]
    .into_iter()
    .map(|(check, tool, flag)| Finding {
        check,
        verdict: match tools::inspect_with_flag(tool, Path::new(tool), flag) {
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
