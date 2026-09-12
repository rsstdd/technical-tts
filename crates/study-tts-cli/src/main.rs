//! `study-tts`, the authoring command line.
//!
//! `DELIVERY-PLAN.md` E1-S5 gives this binary two jobs: scaffold a lesson an
//! author can edit, and tell them whether the document they edited is one this
//! build will render. Every decision behind both lives in `study-tts-runtime`.
//! What is here is argument parsing and the rendering of a refusal — no
//! filesystem durability, no validation, and no second opinion about either.
//!
//! `docs/operations/AUTHORING.md` documents the loop these two commands open.

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand};
use study_tts_core::{
    ApprovalDisposition, LessonDiagnostic, MAX_TAKES_JSON_BYTES, ManifestDigest,
    REQUIRED_PRODUCTION_GATES, ReleaseClaim, ReleaseError, ValidatedTakes, VoiceUse,
};
use study_tts_runtime::{
    ApprovalRequest, BuildError, BuildRequest, BuildResult, FileSystemJobRepository,
    HardwareEnvironmentId, JobRepository, PublicationError, ResumeRequest, WorkerConfiguration,
    WorkerTtsExecutor, approve_preview, build_preview, current_run_report, live_cache_keys,
    load_lesson, prune_candidates, resume_preview, scaffold_lesson,
};

mod exit;
mod output;
mod recovery;

use exit::ExitClass;
use output::CommandOutput;

/// Turns reviewed technical lessons into study-guide audio.
#[derive(Debug, Parser)]
#[command(name = "study-tts", version, about, long_about = None)]
struct Cli {
    /// Emit one structured record instead of prose.
    ///
    /// Global rather than per-command: ADR-0001 §7.3 says "every command
    /// supports human-readable output and `--json`", and a flag declared once
    /// cannot be forgotten on the eleventh command.
    #[arg(long, global = true)]
    json: bool,

    /// The command to run.
    #[command(subcommand)]
    command: Command,
}

/// The command groups this build implements.
#[derive(Debug, Subcommand)]
enum Command {
    /// Author and check lesson documents.
    #[command(subcommand)]
    Lesson(LessonCommand),
    /// Publish a lesson for production. Always refused; see `DELIVERY-PLAN.md`
    /// §11.
    Publish {
        /// The lesson whose preview an operator wanted published.
        lesson_id: String,
    },
    /// Inspect and reclaim the synthesis cache.
    #[command(subcommand)]
    Cache(CacheCommand),
    /// Render a lesson into a private-preview package.
    ///
    /// A retake is this command with `--retake`, which is how
    /// `package-render` has always done it and what `BuildRequest::retakes`
    /// takes. ADR-0001 §7.3 spells a job-scoped `retake <job-id> --segment`
    /// instead, and that spelling is **not provided**: it would need the
    /// job's retained lesson, which `JobRepository::retained_lesson` returns
    /// as bytes while `build_preview` takes a path. Bridging the two is a
    /// runtime change rather than a CLI one, so it is recorded here instead of
    /// improvised.
    Render {
        /// The lesson document to render.
        lesson: PathBuf,
        /// The governed workspace to build beneath.
        #[arg(long)]
        workspace: PathBuf,
        #[command(flatten)]
        roots: WorkerRoots,
        /// Re-synthesize one segment at a named take, repeatable as
        /// `<segment-id>=<take>`.
        #[arg(long = "retake", value_parser = parse_retake)]
        retakes: Vec<(String, u32)>,
    },
    /// Resume a job that was interrupted.
    Resume {
        /// The job to resume.
        job_id: String,
        /// The governed workspace the job lives beneath.
        #[arg(long)]
        workspace: PathBuf,
        #[command(flatten)]
        roots: WorkerRoots,
    },
    /// Record a listening decision about a published package.
    ///
    /// This records a judgment already made against
    /// `docs/operations/PREVIEW-REVIEW-CHECKLIST.md`. ADR-0001 §17.5 makes the
    /// listening itself a human act, and no command can stand in for it.
    Review {
        /// The governed workspace holding `previews/`.
        #[arg(long)]
        workspace: PathBuf,
        /// The lesson whose package was reviewed.
        #[arg(long)]
        lesson_id: String,
        /// BLAKE3 of the reviewed `manifest.json`, which names its directory.
        #[arg(long, value_parser = parse_manifest_digest)]
        manifest: ManifestDigest,
        /// Who listened.
        #[arg(long)]
        reviewer: String,
        /// The role they signed in.
        #[arg(long)]
        reviewer_role: String,
        /// Hardware and room they listened on.
        #[arg(long)]
        playback_environment: String,
        /// What the reviewer decided. No default: a decision is durable, and a
        /// mistyped one must stop rather than record the opposite of what a
        /// reviewer meant.
        #[arg(long, value_enum)]
        disposition: Disposition,
    },
    /// Report the measurements the current package sealed.
    Report {
        /// The governed workspace holding `previews/`.
        #[arg(long)]
        workspace: PathBuf,
        /// The lesson whose current package to read.
        lesson_id: String,
    },
    /// Report what a job's durable record says about it.
    Inspect {
        /// The governed workspace holding `jobs/`.
        #[arg(long)]
        workspace: PathBuf,
        /// The job to read.
        job_id: String,
    },
}

/// What can be done to the cache.
#[derive(Debug, Subcommand)]
enum CacheCommand {
    /// Report the cache entries no retention root refers to.
    ///
    /// Reports only. `AGENTS.md` §Ask first puts deleting a cache entry behind
    /// the operator, and E2-S5 task 6 makes that the default rather than an
    /// option: a command that deleted unless told otherwise would put an
    /// irreversible act behind a forgotten flag.
    /// Report how many cache entries a retention root keeps alive.
    Verify {
        /// The governed workspace holding `cache/` and `previews/`.
        #[arg(long)]
        workspace: PathBuf,

        /// Accepted takes documents whose selected artifacts are retention
        /// roots, per ADR-0001 §12.2.
        #[arg(long = "takes")]
        takes: Vec<PathBuf>,
    },
    Prune {
        /// The governed workspace holding `cache/` and `previews/`.
        #[arg(long)]
        workspace: PathBuf,

        /// Accepted takes documents whose selected artifacts must be kept.
        ///
        /// ADR-0001 §12.2 makes an accepted takes selection a retention root.
        /// Without one, every entry it alone keeps alive is reported as a
        /// candidate, which is safe to read and would not be safe to delete.
        #[arg(long = "takes")]
        takes: Vec<PathBuf>,

        /// Report what would be deleted and delete nothing. The default, and
        /// accepted explicitly because ADR-0001 §7.3 spells the command that
        /// way.
        #[arg(long)]
        dry_run: bool,
    },
}

impl Command {
    /// How the operator spelled this command, for the record it produces.
    ///
    /// A `match` rather than a derive: the envelope's `command` field is read
    /// by whoever parses `--json`, so the spelling is part of what this
    /// binary promises and should change only deliberately.
    const fn name(&self) -> &'static str {
        match self {
            Self::Lesson(LessonCommand::New { .. }) => "lesson new",
            Self::Lesson(LessonCommand::Validate { .. }) => "lesson validate",
            Self::Publish { .. } => "publish",
            Self::Cache(CacheCommand::Prune { .. }) => "cache prune",
            Self::Cache(CacheCommand::Verify { .. }) => "cache verify",
            Self::Inspect { .. } => "inspect",
            Self::Report { .. } => "report",
            Self::Review { .. } => "review",
            Self::Render { .. } => "render",
            Self::Resume { .. } => "resume",
        }
    }
}

/// Where the governed worker, model, and voice roots live.
///
/// Flattened into each command that synthesises rather than declared globally:
/// `report`, `inspect`, and `review` never launch a worker, and an argument
/// they would refuse to use should not appear in their help.
#[derive(Debug, clap::Args)]
struct WorkerRoots {
    /// Root holding `worker/`, whose bundle identity feeds every synthesis key.
    #[arg(long)]
    bundle_root: PathBuf,
    /// Governed model root, checked against its acquisition record.
    #[arg(long)]
    model_root: PathBuf,
    /// Governed voice-profile root.
    #[arg(long)]
    voice_root: PathBuf,
    /// The qualified machine this render runs on, recorded in the run report.
    ///
    /// `docs/operations/REFERENCE-ENVIRONMENT.md` records the value for the
    /// reference machine. Accepted ADR-0002's waiver retains it, and
    /// `HardwareEnvironmentId` refuses anything path-shaped, so a directory
    /// name cannot become provenance.
    #[arg(long, value_parser = parse_hardware_environment)]
    hardware_environment: HardwareEnvironmentId,
}

/// What can be done to a lesson document.
#[derive(Debug, Subcommand)]
enum LessonCommand {
    /// Write a new lesson scaffold that already validates.
    New {
        /// The stable lesson identity stored in the lesson document.
        lesson_id: String,
        /// Where to write the scaffold; an existing file is never replaced.
        #[arg(long)]
        out: PathBuf,
    },
    /// Check a lesson document the way the build that renders it will.
    Validate {
        /// The lesson document to check.
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let name = cli.command.name();
    match run(cli.command) {
        Ok(report) => {
            if cli.json {
                println!("{}", CommandOutput::succeeded(name, report).to_json());
            } else {
                println!("{report}");
            }
            ExitCode::SUCCESS
        }
        // The exit code names what kind of failure this was, so a caller can
        // tell a lesson it should fix from a tool it should install.
        // [`ExitClass::of`] is the mapping and ADR-0001 §7.3 is its vocabulary.
        Err(refusal) => {
            if cli.json {
                // The refusal alone. A structured reader gets the command from
                // `recovery_command`, and repeating it inside `message` would
                // give two places for one answer to drift.
                let record = CommandOutput::refused(name, &refusal, describe_refusal(&refusal));
                println!("{}", record.to_json());
            } else {
                eprintln!("{}", describe(&refusal));
            }
            ExitClass::of(&refusal).code()
        }
    }
}

/// Runs one command, returning what to tell the author on success.
fn run(command: Command) -> Result<String, BuildError> {
    match command {
        Command::Lesson(lesson) => run_lesson(lesson),
        Command::Publish { .. } => Err(refuse_publication()),
        Command::Cache(cache) => run_cache(cache),
        Command::Inspect { workspace, job_id } => {
            inspect_job(&workspace_root(&workspace)?, &job_id)
        }
        Command::Report {
            workspace,
            lesson_id,
        } => report_run(&workspace_root(&workspace)?, &lesson_id),
        Command::Review {
            workspace,
            lesson_id,
            manifest,
            reviewer,
            reviewer_role,
            playback_environment,
            disposition,
        } => record_review(
            &workspace_root(&workspace)?,
            &lesson_id,
            &manifest,
            &reviewer,
            &reviewer_role,
            &playback_environment,
            disposition.into(),
        ),
        Command::Render {
            lesson,
            workspace,
            roots,
            retakes,
        } => render_lesson(&lesson, &workspace_root(&workspace)?, &roots, retakes),
        Command::Resume {
            job_id,
            workspace,
            roots,
        } => resume_job(&job_id, &workspace_root(&workspace)?, &roots),
    }
}

/// Parses one `--retake <segment-id>=<take>` pair at argument time.
fn parse_retake(value: &str) -> Result<(String, u32), String> {
    let (segment_id, take) = value
        .split_once('=')
        .ok_or_else(|| format!("`--retake` takes `<segment-id>=<take>`, got `{value}`"))?;
    let take = take
        .parse()
        .map_err(|_| format!("`{take}` is not a take number"))?;
    Ok((segment_id.to_owned(), take))
}

/// Parses the qualified-machine label at argument time.
fn parse_hardware_environment(value: &str) -> Result<HardwareEnvironmentId, String> {
    HardwareEnvironmentId::parse(value).map_err(|error| error.to_string())
}

/// Starts the worker every synthesising command needs.
///
/// Every gate a build passes runs here rather than at the call site: the bundle
/// is identified, the model's bytes are verified, and the voice is resolved at
/// [`VoiceUse::PrivateSynthesis`] — a preview renders a lesson, not a voice
/// qualification. `WorkerConfiguration::for_bundle` owns all three, which is
/// why this function is four lines and not forty.
fn start_worker(workspace: &Path, roots: &WorkerRoots) -> Result<WorkerTtsExecutor, BuildError> {
    let configuration = WorkerConfiguration::for_bundle(
        &workspace_root(&roots.bundle_root)?,
        &workspace_root(&roots.model_root)?,
        &workspace_root(&roots.voice_root)?,
        workspace,
        VoiceUse::PrivateSynthesis,
        roots.hardware_environment.clone(),
    )?;
    WorkerTtsExecutor::start(&configuration)
}

/// Renders one lesson into a package.
fn render_lesson(
    lesson: &Path,
    workspace: &Path,
    roots: &WorkerRoots,
    retakes: Vec<(String, u32)>,
) -> Result<String, BuildError> {
    let executor = start_worker(workspace, roots)?;
    let result = build_preview(
        BuildRequest {
            lesson_path: lesson.to_path_buf(),
            workspace: workspace.to_path_buf(),
            ffmpeg_executable: PathBuf::from("ffmpeg"),
            ffprobe_executable: PathBuf::from("ffprobe"),
            voice_profile_root: workspace_root(&roots.voice_root)?,
            retakes: retakes.into_iter().collect(),
        },
        &executor,
    )?;
    Ok(describe_package(&result))
}

/// Resumes an interrupted job.
fn resume_job(job_id: &str, workspace: &Path, roots: &WorkerRoots) -> Result<String, BuildError> {
    let executor = start_worker(workspace, roots)?;
    let result = resume_preview(
        ResumeRequest {
            job_id: job_id.to_owned(),
            workspace: workspace.to_path_buf(),
            ffmpeg_executable: PathBuf::from("ffmpeg"),
            ffprobe_executable: PathBuf::from("ffprobe"),
            voice_profile_root: workspace_root(&roots.voice_root)?,
        },
        &executor,
    )?;
    Ok(describe_package(&result))
}

/// Names what a build published, and where.
///
/// The package directory is named by its own manifest digest, so naming the
/// directory names the identity — there is nothing to print twice.
fn describe_package(result: &BuildResult) -> String {
    [
        format!("package: {}", result.package_dir.display()),
        format!("  master: {}", result.master_wav.display()),
        format!("  manifest: {}", result.manifest.display()),
    ]
    .join("\n")
}

/// Records a listening decision beside the package it judges.
///
/// Both the decision and the digest are already validated: `clap` refuses an
/// unknown disposition and a malformed digest before this runs, which is the
/// earliest authoritative boundary for an argument and keeps a wrong value
/// from reaching the governed root at all.
fn record_review(
    workspace: &Path,
    lesson_id: &str,
    digest: &ManifestDigest,
    reviewer: &str,
    reviewer_role: &str,
    playback_environment: &str,
    decision: ApprovalDisposition,
) -> Result<String, BuildError> {
    let record = approve_preview(
        workspace,
        &ApprovalRequest {
            lesson_id,
            manifest_blake3: digest,
            reviewer,
            reviewer_role,
            playback_environment,
            disposition: decision,
        },
    )?;
    Ok(format!(
        "recorded {:?} for `{}` package {}",
        record.disposition,
        record.lesson_id,
        record.manifest_blake3.as_str()
    ))
}

/// Reports what the current package's sealed run report measured.
///
/// The six measures accepted ADR-0002 retains while its waiver stands, plus
/// the completion state. Not every field the report holds: ADR-0001 §14 says
/// "human terminal output remains concise", and the document itself is beside
/// the package for a reader who wants all of it.
fn report_run(workspace: &Path, lesson_id: &str) -> Result<String, BuildError> {
    let Some(report) = current_run_report(workspace, lesson_id)? else {
        return Ok(format!("no current package for `{lesson_id}`"));
    };
    Ok([
        format!("lesson: {}", report.lesson_id),
        format!("  job: {}", report.job_id),
        format!("  completion: {:?}", report.completion),
        format!("  wall: {} us", report.wall_micros),
        format!("  worker bundle: {}", report.worker_bundle_hash.as_str()),
        format!("  hardware: {}", report.hardware_environment_id.as_str()),
        format!("  segments: {}", report.segments.len()),
    ]
    .join("\n"))
}

/// What a reviewer decided, as an operator spells it on the command line.
///
/// A CLI-local mirror of [`ApprovalDisposition`] rather than a `ValueEnum`
/// derived on the core type: argument vocabulary is this crate's concern, and
/// deriving it there would put a `clap` dependency in the crate that owns
/// durable domain decisions. [`From`] is the one place the two spellings meet,
/// and it is exhaustive, so a third disposition cannot be silently untypeable.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum Disposition {
    /// The reviewer accepted this package.
    Accepted,
    /// The reviewer rejected it.
    Rejected,
}

impl From<Disposition> for ApprovalDisposition {
    fn from(disposition: Disposition) -> Self {
        match disposition {
            Disposition::Accepted => Self::Accepted,
            Disposition::Rejected => Self::Rejected,
        }
    }
}

/// Parses a manifest digest at argument time.
///
/// `clap` renders the message, so a mistyped package is refused before the
/// command runs rather than from inside the governed root — validation at the
/// earliest authoritative boundary, which is what `crates/AGENTS.md` asks for.
fn parse_manifest_digest(value: &str) -> Result<ManifestDigest, String> {
    ManifestDigest::try_from(value.to_owned())
        .map_err(|error| format!("not a manifest digest: {error}"))
}

/// Resolves a workspace an operator typed into one the runtime can contain.
///
/// `managed::` measures containment against the root it is given, so a relative
/// root makes every path beneath it look like an escape. `package-render`
/// absolutizes for the same reason, one layer up, and the operator gets to keep
/// typing `--workspace .`.
///
/// # Errors
///
/// [`study_tts_runtime::IoError::FileSystem`] when the current directory
/// cannot be read, which is the only way `absolute` fails for a path this has
/// already accepted as an argument.
fn workspace_root(workspace: &Path) -> Result<PathBuf, BuildError> {
    std::path::absolute(workspace).map_err(|source| {
        study_tts_runtime::IoError::FileSystem {
            path: workspace.to_path_buf(),
            source,
        }
        .into()
    })
}

/// Reports what a job's durable record says, and nothing it does not hold.
///
/// A read, not a claim: `JobRepository::load` returns `None` for a job this
/// workspace never owned, and saying so is the honest answer rather than an
/// empty record that reads like a job with nothing in it.
fn inspect_job(workspace: &Path, job_id: &str) -> Result<String, BuildError> {
    let Some(document) = FileSystemJobRepository.load(workspace, job_id)? else {
        return Ok(format!("no job `{job_id}` in this workspace"));
    };
    Ok([
        format!("job: {}", document.job_id),
        format!("  state: {:?}", document.state),
        format!(
            "  last successful state: {:?}",
            document.last_successful_state
        ),
        format!("  build attempt: {}", document.build_attempt),
        format!("  plan hash: {}", document.plan_hash.as_str()),
    ]
    .join("\n"))
}

/// Runs one cache command.
fn run_cache(command: CacheCommand) -> Result<String, BuildError> {
    match command {
        CacheCommand::Verify { workspace, takes } => {
            let accepted = load_accepted_takes(&takes)?;
            let live = live_cache_keys(&workspace_root(&workspace)?, &accepted)?;
            Ok(format!(
                "{} cache {} referenced by a retention root",
                live.len(),
                if live.len() == 1 {
                    "entry is"
                } else {
                    "entries are"
                }
            ))
        }
        CacheCommand::Prune {
            workspace,
            takes,
            dry_run: _,
        } => {
            // `dry_run` is read and discarded: this command has one behavior
            // and the flag exists so ADR-0001 §7.3's spelling is accepted
            // rather than refused. Deleting is not implemented here at all,
            // which is stronger than defaulting to not deleting.
            let accepted = load_accepted_takes(&takes)?;
            let candidates = prune_candidates(&workspace_root(&workspace)?, &accepted)?;
            if candidates.is_empty() {
                return Ok("no cache entry is unreferenced".to_owned());
            }
            let mut lines = vec![format!(
                "{} cache {} unreferenced; nothing was deleted:",
                candidates.len(),
                if candidates.len() == 1 {
                    "entry is"
                } else {
                    "entries are"
                }
            )];
            lines.extend(
                candidates
                    .iter()
                    .map(|candidate| format!("  - {}", candidate.cache_key.as_str())),
            );
            Ok(lines.join("\n"))
        }
    }
}

/// Refuses publication, and says what production would have required.
///
/// `DELIVERY-PLAN.md` §11 settles that `publish` writes nothing in version 1.0
/// and that the refusal is the whole of its behavior. The refusal itself is
/// the runtime's — [`ReleaseClaim::validate_as_production`] returns
/// `PrivateProfileCannotClaimProduction` for anything that is not already a
/// production release, and it stays correct once the gates exist, because a
/// preview is not the artifact that earned them.
///
/// The gate list is added by [`describe`] rather than here, so that every
/// refusal this binary reports is printed in exactly one place.
fn refuse_publication() -> BuildError {
    BuildError::from(
        ReleaseClaim::private_preview()
            .validate_as_production()
            .expect_err("a private preview is never a production release"),
    )
}

/// Runs one lesson command, returning what to tell the author on success.
fn run_lesson(lesson: LessonCommand) -> Result<String, BuildError> {
    match lesson {
        LessonCommand::New { lesson_id, out } => {
            scaffold_lesson(&lesson_id, &out)?;
            Ok(format!("created lesson scaffold: {}", out.display()))
        }
        LessonCommand::Validate { path } => {
            load_lesson(&path)?;
            Ok(format!("valid lesson: {}", path.display()))
        }
    }
}

/// Reads every accepted takes document a prune must treat as a retention root.
///
/// Bounded before the parse, in the discipline `rust-production` states and
/// `pipeline.rs` already applies to the same document: a file read into memory
/// to discover it was too large has already cost what the ceiling exists to
/// refuse.
fn load_accepted_takes(paths: &[PathBuf]) -> Result<Vec<ValidatedTakes>, BuildError> {
    paths
        .iter()
        .map(|path| {
            let bytes = std::fs::read(path).map_err(|source| {
                BuildError::from(study_tts_runtime::IoError::ReadFile {
                    path: path.clone(),
                    source,
                })
            })?;
            if bytes.len() > MAX_TAKES_JSON_BYTES {
                return Err(BuildError::Takes(Box::new(
                    study_tts_core::TakesError::TakesJsonTooLarge {
                        max_bytes: MAX_TAKES_JSON_BYTES,
                    },
                )));
            }
            ValidatedTakes::from_json(&bytes).map_err(|error| BuildError::Takes(Box::new(error)))
        })
        .collect()
}

/// Renders one refusal for the person who has to act on it.
///
/// Only a lesson refusal is reshaped, and only because
/// [`LessonDiagnostic`]'s own `Display` cannot know that a reader sitting in an
/// editor wants the segment identity as well as the JSON Pointer: the pointer
/// names a position, and a segment that moved is still found by its identity.
/// Every other refusal already names its artifact, its invariant, and its
/// remedy owner, so restating it here would only give the two spellings room to
/// disagree.
fn describe(error: &BuildError) -> String {
    let described = describe_refusal(error);
    match recovery::command_for(error) {
        Some(command) => format!("{described}\n  try: {command}"),
        None => described,
    }
}

/// The refusal itself, before any recovery advice is appended.
fn describe_refusal(error: &BuildError) -> String {
    match error {
        BuildError::Lesson(diagnostic) => describe_lesson(diagnostic),
        BuildError::Publication(PublicationError::Release(
            ReleaseError::PrivateProfileCannotClaimProduction,
        )) => describe_refused_publication(error),
        other => other.to_string(),
    }
}

/// States the refusal, then what a production claim would have required.
///
/// The runtime's refusal names a claim; an operator also needs to know what
/// claim they could ever make, and [`REQUIRED_PRODUCTION_GATES`] is that
/// answer. It is composed here rather than carried in the error because which
/// gates are outstanding is presentation — the refusal is identical whether
/// twelve are missing or none, since a preview is not the artifact that earns
/// them. `DELIVERY-PLAN.md` §11 settles that.
fn describe_refused_publication(error: &BuildError) -> String {
    let mut lines = vec![
        error.to_string(),
        "production requires evidence for every gate below, and a preview holds none:".to_owned(),
    ];
    lines.extend(
        REQUIRED_PRODUCTION_GATES
            .iter()
            .map(|gate| format!("  - {gate}")),
    );
    lines.join("\n")
}

/// Locates a lesson refusal by document, field path, and segment.
///
/// The lines are what an author edits, in the order they narrow: which file,
/// which field of it, which segment that field belongs to, and only then what
/// was wrong. `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and
/// access excludes spoken text from diagnostics, which is why the offending
/// value is never among them — the pointer is how the author finds it.
fn describe_lesson(diagnostic: &LessonDiagnostic) -> String {
    let mut lines = vec![format!("refused `{}`", diagnostic.document())];
    if !diagnostic.field_path().is_empty() {
        lines.push(format!("  field: {}", diagnostic.field_path()));
    }
    if let Some(segment) = diagnostic.segment_id() {
        lines.push(format!("  segment: {segment}"));
    }
    lines.push(format!("  reason: {}", diagnostic.error()));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use study_tts_core::ValidatedLesson;

    use super::*;

    const DOCUMENT: &str = "lessons/e1-s5.json";

    /// A lesson whose one recall prompt leaves `pause_after_ms` of silence.
    fn lesson_with_pause(pause_after_ms: u32) -> Vec<u8> {
        format!(
            r#"{{
              "schema_version": "3.1",
              "lesson_id": "e1-s5-diagnostic",
              "title": "e1-s5-diagnostic",
              "language": "en",
              "speakers": {{ "instructor": {{ "voice_profile": "owner-fallback-v1" }} }},
              "segments": [
                {{
                  "id": "seg-0001",
                  "speaker": "instructor",
                  "role": "recall_prompt",
                  "source_refs": [],
                  "display_text": "A prompt.",
                  "spoken_text": "A prompt.",
                  "style": "calm_explanatory",
                  "pause_after_ms": {pause_after_ms},
                  "review_status": "approved",
                  "editorial": true
                }}
              ]
            }}"#
        )
        .into_bytes()
    }

    #[test]
    fn t1_e1_validation_error_names_the_offending_field_path() {
        let refusal = ValidatedLesson::from_json(DOCUMENT, &lesson_with_pause(10))
            .expect_err("a recall prompt leaving 10 ms is refused");

        let rendered = describe(&BuildError::Lesson(refusal));

        assert!(
            rendered.contains(&format!("refused `{DOCUMENT}`")),
            "the refusal names the document it read: {rendered}"
        );
        assert!(
            rendered.contains("  field: /segments/0/pause_after_ms"),
            "the refusal names the RFC 6901 pointer to the offending field: {rendered}"
        );
        assert!(
            rendered.contains("  segment: seg-0001"),
            "the refusal names the segment the field belongs to: {rendered}"
        );
    }

    #[test]
    fn t1_e1_a_refusal_about_a_whole_document_names_no_field_path() {
        let refusal = ValidatedLesson::from_json(DOCUMENT, b"{")
            .expect_err("bytes that are not JSON are refused");

        let rendered = describe(&BuildError::Lesson(refusal));

        assert!(
            !rendered.contains("  field:"),
            "RFC 6901's empty pointer is said by saying nothing: {rendered}"
        );
        assert!(
            !rendered.contains("  segment:"),
            "a whole-document refusal belongs to no segment: {rendered}"
        );
    }
}
