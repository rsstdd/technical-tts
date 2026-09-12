//! The command an operator can run next, when there is one.
//!
//! E2-S5 task 3 asks for "documented exit codes and safe recovery commands".
//! [`crate::exit`] owns the first. This owns the second, and it lives in the
//! CLI rather than beside [`study_tts_runtime::RemedyAdvice`] for the reason
//! `crates/AGENTS.md` gives: the CLI surface is this crate's, and a runtime
//! error type that named `study-tts resume` would know a command it cannot
//! call and could not keep true.
//!
//! What the runtime already supplies is better than a command anyway.
//! `RemedyAdvice` carries the *action* — "record the finding and retake or
//! accept it with authority" — and the §Failure routing row that establishes
//! it. This adds only the invocation, keyed by that row, so the two cannot
//! disagree: a row with no runnable answer gets none rather than an invented
//! one.

use study_tts_runtime::BuildError;

/// A runnable answer for each `docs/governance/ROUTING-TABLES.md` §Failure
/// routing row that has one.
///
/// **Two-sided.** The left column is transcribed from that table's `Failure`
/// column character for character, and the table names this file in return. A
/// row whose spelling drifts stops matching and stops advising, which is a
/// visible failure rather than a silent one — and
/// `t1_e2_every_recovery_row_names_a_row_the_routing_table_has` refuses a
/// spelling the document does not carry.
///
/// Rows deliberately absent, because no command answers them:
///
/// - `Voice consent/checksum mismatch` and `Missing rights classification` —
///   the remedy is a rights record a person writes, not an invocation.
/// - `Failed release gate` — `publish` already refuses and names every gate.
/// - `ASR verifier failure` — E4 owns the verifier and its commands.
/// - `Invalid lesson or schema` — **a row no refusal routes to.** The document
///   carries it and `BuildError::Lesson` returns no remedy, so a command here
///   could never be offered. That one-sided gap is the runtime's to close or
///   to record; naming it from this side would only hide it.
const RECOVERY: [(&str, &str); 3] = [
    (
        "Worker protocol or containment failure",
        "study-tts resume <job-id> --workspace <workspace>",
    ),
    (
        "Invalid or over-range audio",
        "study-tts resume <job-id> --workspace <workspace>",
    ),
    (
        "State or checksum corruption",
        "study-tts inspect <job-id> --workspace <workspace>",
    ),
];

/// The command that answers this refusal, when the routing table has one.
///
/// `None` twice over, and the difference matters to nobody downstream but is
/// worth stating: a refusal with no governed remedy at all, and a governed one
/// whose row no command answers. Both leave the operator with the refusal's
/// own message, which already names its remedy owner.
#[must_use]
pub fn command_for(error: &BuildError) -> Option<&'static str> {
    let row = error.remedy()?.routing()?;
    RECOVERY
        .iter()
        .find(|(failure, _)| *failure == row)
        .map(|(_, command)| *command)
}

#[cfg(test)]
mod tests {
    use super::{BuildError, RECOVERY, command_for};

    /// Every row this file advises on exists in the table **and is reachable**.
    ///
    /// The document half is the cheap one. The half that matters is the
    /// second: the first version of this table carried `Invalid lesson or
    /// schema`, a real row in the document that no refusal routes to, so the
    /// command could never be offered and the spelling check passed anyway.
    /// A representative refusal per row is what proves the entry is alive.
    #[test]
    fn t1_e2_every_recovery_row_names_a_row_the_routing_table_has() {
        let table = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/governance/ROUTING-TABLES.md"
        ))
        .expect("read the routing table");

        for (row, command) in RECOVERY {
            assert!(
                table.contains(&format!("| {row} |")),
                "`{row}` is not a §Failure routing row; the command `{command}` \
                 would never be offered"
            );
        }

        // One refusal per row, so an entry nothing routes to cannot survive.
        let reaching: [BuildError; 3] = [
            study_tts_runtime::ManagedPathError::ManagedPathEscape {
                root: std::path::PathBuf::from("workspace"),
                path: std::path::PathBuf::from("/etc"),
            }
            .into(),
            study_tts_runtime::AudioError::UnusableAudio {
                path: std::path::PathBuf::from("segment.wav"),
                fault: study_tts_runtime::AudioFault::NonCanonical {
                    channels: 2,
                    sample_rate: 44_100,
                    bits_per_sample: 16,
                    sample_format: "int",
                    required_channels: 1,
                    required_sample_rate: 24_000,
                    required_bits_per_sample: 32,
                },
            }
            .into(),
            study_tts_runtime::DurableStateError::PackagePlanMismatch {
                path: std::path::PathBuf::from("previews/lesson/packages/manifest.json"),
            }
            .into(),
        ];

        for error in reaching {
            assert!(
                command_for(&error).is_some(),
                "`{error}` routes to a row this file should answer"
            );
        }
    }

    /// A refusal routed to a row with a command gets it.
    ///
    /// The wrong implementation this rejects is advice keyed by error class:
    /// `LoudnessNotLinear` and a missing FFmpeg are both `Tool`, and telling
    /// someone whose audio failed an invariant to install a binary is worse
    /// than telling them nothing.
    #[test]
    fn t1_e2_a_governed_refusal_offers_the_command_its_row_answers() {
        let corruption =
            BuildError::from(study_tts_runtime::DurableStateError::PackagePlanMismatch {
                path: std::path::PathBuf::from("previews/lesson/packages/manifest.json"),
            });

        assert_eq!(
            command_for(&corruption),
            Some("study-tts inspect <job-id> --workspace <workspace>"),
            "state corruption routes to the row whose command inspects the job"
        );
    }

    /// A refusal the routing table does not govern offers nothing rather than
    /// something.
    ///
    /// `MissingTool`'s own doc says it is deliberately unrouted because "its
    /// own message already names the remedy". Inventing a command here would
    /// contradict the document that decided not to.
    #[test]
    fn t1_e2_an_ungoverned_refusal_offers_no_command() {
        let missing = BuildError::from(study_tts_runtime::ToolError::MissingTool {
            tool: "ffmpeg".to_owned(),
            requested: std::path::PathBuf::from("ffmpeg"),
        });

        assert_eq!(
            command_for(&missing),
            None,
            "an unrouted refusal keeps its own message rather than gaining advice here"
        );
    }
}
