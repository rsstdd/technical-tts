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
//!
//! A check is labeled with exactly what it examined. §14's "model and
//! voice-profile checksums" is two findings here, each refused when its root
//! was not given, because one label over two checks would let the model
//! verifying read as the voices having been.

use std::{
    ffi::OsStr,
    fmt,
    path::{Path, PathBuf},
};

use study_tts_core::VoiceUse;

use crate::{
    BuildError, ToolInvocation, ToolOperation, WORKER_LAUNCHER_PATH, WorkerLauncher, io_error,
    model_gate,
    process::{self, VERSION_PROBE_POLICY},
    tools, voice_gate,
};

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
    /// This build cannot answer it yet, and which story owns the check.
    Blocked(&'static str),
}

impl fmt::Display for Finding {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.verdict {
            Verdict::Ok(detail) => write!(formatter, "  ok       {}: {detail}", self.check),
            Verdict::Refused(detail) => write!(formatter, "  REFUSED  {}: {detail}", self.check),
            Verdict::Blocked(owner) => {
                write!(formatter, "  blocked  {}: owned by {owner}", self.check)
            }
        }
    }
}

/// Reports what this machine can do, without changing anything.
///
/// Read-only by construction: every path is resolved rather than created, and
/// every binary — the five version probes, `df`, and `lscpu` — is resolved
/// through `tools::resolve_executable` and run under `process::run`'s
/// supervision, with the binary that answered recorded rather than the one
/// requested.
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
    voice_root: Option<&Path>,
) -> Result<Vec<Finding>, BuildError> {
    let mut findings = vec![host_finding(), architecture_finding()];
    findings.push(filesystem_finding(workspace)?);
    findings.push(writable_finding(workspace));
    findings.push(disk_space_finding(workspace));
    findings.extend(tool_findings());
    findings.push(model_checksum_finding(model_root));
    findings.push(voice_checksum_finding(voice_root));
    findings.push(worker_bundle_finding(bundle_root));
    findings.push(device_finding(bundle_root));
    let (topology, available) = core_budget_finding();
    findings.push(topology);
    findings.push(thread_budget_finding(bundle_root, available));
    findings.push(offline_finding());
    findings.extend(blocked_findings());
    Ok(findings)
}

/// Runs one host tool under the supervision every external binary gets.
///
/// `df` and `lscpu` answer questions `std` cannot, and they are still spawns.
/// `rust-production` gives every spawn a named deadline, its own process
/// group, and a bounded capture, and `process::run` is where those live —
/// a `df` hung on a stale network mount would otherwise hang `doctor` with it,
/// which is the one command an operator runs *because* something hangs.
fn probe(tool: &'static str, arguments: &[&OsStr]) -> Result<String, String> {
    let resolved = tools::resolve_executable(Path::new(tool))
        .ok_or_else(|| format!("`{tool}` is not on PATH"))?;
    let mut command = std::process::Command::new(&resolved);
    command.args(arguments);
    let invocation = ToolInvocation::new(tool, ToolOperation::HostProbe, &resolved);
    let output = process::run(invocation, command, VERSION_PROBE_POLICY)
        .map_err(|error| format!("`{tool}` could not be run: {error}"))?;
    if !output.status.success() {
        return Err(format!("`{tool}` exited {}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("`{tool}` reported non-UTF-8 output"))
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
///
/// §14 lists "writable job, cache, model, and output directories". The first,
/// second, and fourth are created beneath the workspace by the build, so one
/// probe at the root they inherit from is the check; the model root is
/// governed content this build reads and never writes, and a writable one
/// would be the surprise, so it is deliberately not probed.
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
    let arguments = [
        OsStr::new("-h"),
        OsStr::new("--output=avail,target"),
        workspace.as_os_str(),
    ];
    Finding {
        check: CHECK,
        verdict: match probe("df", &arguments) {
            Ok(listing) => listing.lines().nth(1).map(str::trim).map_or_else(
                || Verdict::Refused("`df` reported nothing for this path".to_owned()),
                |line| Verdict::Ok(line.to_owned()),
            ),
            Err(why) => Verdict::Refused(why),
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
///
/// Read through [`WorkerLauncher::read`] rather than as free JSON. The launcher
/// is a format this project defines, so its `schema_version` is a gate and not
/// a field to skip past: an untyped read answers `ok` for a launcher whose
/// major this build refuses, and an operator told the device is fine discovers
/// otherwise at the first spawn. Reading it the way the pipeline reads it is
/// also the only way this check can agree with the render it is clearing.
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
    Finding {
        check: CHECK,
        verdict: match WorkerLauncher::read(root) {
            Ok(launcher) => Verdict::Ok(format!(
                "`{}`, as `{}` declares",
                launcher.device,
                root.join(WORKER_LAUNCHER_PATH).display()
            )),
            // The same shape the model and tool checks above use: the typed
            // error already names the artifact and separates an absent file
            // from an unreadable one from an unsupported layout, so restating
            // it here would be a second, coarser wording of one failure.
            Err(error) => Verdict::Refused(error.to_string()),
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
    const CHECK: &str = "model checksums";
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

/// Every profile beneath the voice root, through the gate a render passes.
///
/// The other half of §14's "model and voice-profile checksums", as its own
/// finding: `admit_voice_root` is what `WorkerConfiguration::for_bundle` runs
/// before a worker starts, so a profile this refuses is one a render refuses,
/// and `doctor` answering from the same gate is the only way the two can
/// agree. Checked at [`VoiceUse::PrivateSynthesis`] because that is the use a
/// render asks for; a profile consented for qualification only is correctly
/// reported as unusable here.
///
/// The refusal it prints names the profile and the record, never their path —
/// `error/voice_profile.rs` keeps raw voice-reference paths out of every
/// message, and `doctor`'s output is printed by default like any other.
fn voice_checksum_finding(voice_root: Option<&Path>) -> Finding {
    const CHECK: &str = "voice-profile checksums";
    let Some(root) = voice_root else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "not checked; pass `--voice-root` to verify the governed profiles".to_owned(),
            ),
        };
    };
    Finding {
        check: CHECK,
        verdict: match voice_gate::admit_voice_root(root, VoiceUse::PrivateSynthesis) {
            Ok(()) => Verdict::Ok(format!(
                "every profile under `{}` matches its record",
                root.display()
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
fn core_budget_finding() -> (Finding, usize) {
    const CHECK: &str = "physical-core topology and reserved-core policy";
    let (physical, source) = match physical_cores() {
        Ok(cores) => (cores, "lscpu".to_owned()),
        Err(why) => (
            std::thread::available_parallelism().map_or(1, |logical| (logical.get() / 2).max(1)),
            format!("half the visible logical processors, topology being unavailable: {why}"),
        ),
    };
    let available = if physical > 1 { physical - 1 } else { physical };
    let finding = Finding {
        check: CHECK,
        verdict: Verdict::Ok(format!(
            "{physical} physical ({source}); {available} available after reserving one"
        )),
    };
    (finding, available)
}

/// Physical cores, from `lscpu -p=CORE`, or why it could not say.
///
/// The reason travels with the fallback rather than being dropped: §10.1
/// permits "half the visible logical processors" when topology is unavailable,
/// and a reader deciding whether to trust that number wants to know whether
/// `lscpu` was absent, timed out, or answered nothing.
fn physical_cores() -> Result<usize, String> {
    let listing = probe("lscpu", &[OsStr::new("-p=CORE")])?;
    let mut cores: Vec<&str> = listing
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    cores.sort_unstable();
    cores.dedup();
    if cores.is_empty() {
        return Err("`lscpu` listed no cores".to_owned());
    }
    Ok(cores.len())
}

/// ADR-0001 §10.1's preflight rule for the launcher this bundle declares:
/// `pool_size * threads_per_worker <= available_physical_cores`.
///
/// Pool size is one. §10.1 fixes the default and qualification size there,
/// and the configurable pool is E5-S2's, which `blocked_findings` names — so
/// the product is the launcher's own thread count, and the question is whether
/// this machine hosts one worker at that width without oversubscribing. §14
/// lists "per-worker threads" and "oversubscription result" beside topology;
/// they are this finding rather than a line in the topology one, because a
/// launcher that cannot be read refuses this check and not that one.
fn thread_budget_finding(bundle_root: Option<&Path>, available: usize) -> Finding {
    const CHECK: &str = "per-worker threads and oversubscription";
    let Some(root) = bundle_root else {
        return Finding {
            check: CHECK,
            verdict: Verdict::Refused(
                "not checked; pass `--bundle-root` to read the launcher".to_owned(),
            ),
        };
    };
    Finding {
        check: CHECK,
        verdict: match WorkerLauncher::read(root) {
            Ok(launcher) => {
                let threads = usize::try_from(launcher.threads.get()).unwrap_or(usize::MAX);
                if threads <= available {
                    Verdict::Ok(format!(
                        "{threads} per worker × 1 worker ≤ {available} available"
                    ))
                } else {
                    Verdict::Refused(format!(
                        "{threads} per worker × 1 worker exceeds {available} available; lower \
                         `threads` in the launcher or reserve fewer cores"
                    ))
                }
            }
            Err(error) => Verdict::Refused(error.to_string()),
        },
    }
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
        verdict: Verdict::Blocked(owner),
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::{Verdict, device_finding};
    use crate::WORKER_LAUNCHER_PATH;

    #[test]
    fn t2_e2_doctor_refuses_a_launcher_layout_this_build_cannot_read() -> Result<(), Box<dyn Error>>
    {
        // Deliberately partial — a device, a refused major, and nothing else.
        // The assertion is what makes the partiality safe: it names the layout
        // the launcher declares, so this passes only when the version gate
        // refused, never when the shape check tripped over an absent `seed`.
        // `WorkerBundleError::UnsupportedLauncher` documents that ordering as
        // load-bearing, and a build that checked shape first would keep a bare
        // `Refused(_)` assertion green. The verdict erases the typed error into
        // a string, so a fragment of the message is the discrimination
        // available at this boundary.
        let root = TempDir::new()?;
        let launcher = root.path().join(WORKER_LAUNCHER_PATH);
        std::fs::create_dir_all(launcher.parent().ok_or("the launcher has a parent")?)?;
        std::fs::write(
            &launcher,
            br#"{"schema_version": "99.0", "device": "cpu", "threads": 1}"#,
        )?;

        let finding = device_finding(Some(root.path()));

        let Verdict::Refused(detail) = &finding.verdict else {
            return Err(format!(
                "an unsupported launcher major must not clear the device check: {:?}",
                finding.verdict
            )
            .into());
        };
        assert!(
            detail.contains("99.0"),
            "the refusal names the layout the launcher declares: {detail}"
        );
        Ok(())
    }

    #[test]
    fn t2_e2_doctor_names_the_device_a_readable_launcher_declares() -> Result<(), Box<dyn Error>> {
        // The checked-in launcher rather than a copy of one. `worker_launcher`
        // keeps a single spelling of that record on purpose, and a second copy
        // here would keep passing after the layout moved underneath it — which
        // is the drift this check exists to catch on an operator's machine.
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

        let finding = device_finding(Some(&root));

        let Verdict::Ok(detail) = &finding.verdict else {
            return Err(format!(
                "the launcher this repository ships clears the check: {:?}",
                finding.verdict
            )
            .into());
        };
        assert!(
            detail.contains("cpu"),
            "the verdict names the device the launcher declares: {detail}"
        );
        Ok(())
    }
}
