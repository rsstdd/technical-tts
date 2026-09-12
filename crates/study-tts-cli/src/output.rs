//! What every command says, in the two shapes ADR-0001 §7.3 requires.
//!
//! "Every command supports human-readable output and `--json`." This module
//! owns the second, and the first is the `Display` beside it, so a command
//! decides *what* it reports and never *how*.
//!
//! # Why this is not a published schema
//!
//! `crates/study-tts-testkit/tests/schemas.rs::PUBLISHED_SCHEMAS` holds ten
//! entries, and every one is a durable document this build writes and reads
//! back, or the worker wire format it deserializes. This envelope is neither:
//! it goes to a terminal and nothing in this project parses it. The
//! `rust-review` rule that a project-defined format cannot claim the
//! tool-output exemption governs *deserialization* boundaries, which this is
//! not the far side of.
//!
//! So [`CLI_OUTPUT_VERSION`] is a plain constant rather than a
//! `*_SCHEMA_VERSION`. The spelling is load-bearing:
//! `docs/architecture/G1-FREEZE-CHARTER.md` §The inventory is derived promises
//! that every `*_SCHEMA_VERSION` in `crates/*/src/*.rs` appears in one of its
//! two lists, and a constant named that way would silently owe a charter row.
//!
//! Stability is still owed and is proved instead by a committed golden,
//! `fixtures/contracts/e2-s5-cli-output-refusal.json`, which
//! `t3_e2_cli_json_output_matches_its_committed_golden` compares whole.

use serde::Serialize;
use study_tts_runtime::{BuildError, BuildErrorClass};

use crate::exit::ExitClass;

/// The envelope's shape, moved when a field is added or removed.
///
/// Not a `SchemaVersion`: this is not a published document and comparing it is
/// nobody's job but a reader's. See the module header.
pub const CLI_OUTPUT_VERSION: &str = "1.0-cli-output";

/// One command's result, in the shape `--json` emits.
///
/// Carries what `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md` §Storage and
/// access permits a diagnostic to carry and nothing else: "hashes, IDs,
/// timings, states, and error classes". There is deliberately no field for the
/// value a refusal refused, because the most natural refusal to write is one
/// that quotes it.
#[derive(Debug, Serialize)]
pub struct CommandOutput {
    /// The envelope's own version, so a reader can tell which shape it holds.
    output_version: &'static str,
    /// The command that ran, as the operator spelled it.
    command: &'static str,
    /// What happened.
    #[serde(flatten)]
    outcome: Outcome,
}

/// Whether the command did its work, and what to say about it either way.
#[derive(Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum Outcome {
    /// The command did what it was asked.
    Succeeded {
        /// One line for a person, identical to what the human-readable form
        /// prints, so the two cannot disagree about what happened.
        summary: String,
    },
    /// The command refused.
    Refused {
        /// Which boundary refused, from the same closed vocabulary the run
        /// report publishes.
        error_class: BuildErrorClass,
        /// The exit status this refusal produced, so a caller reading JSON and
        /// a caller reading `$?` agree.
        exit_code: u8,
        /// What the refusal says, already redacted by the error type itself.
        message: String,
    },
}

impl CommandOutput {
    /// Records a command that did its work.
    #[must_use]
    pub fn succeeded(command: &'static str, summary: String) -> Self {
        Self {
            output_version: CLI_OUTPUT_VERSION,
            command,
            outcome: Outcome::Succeeded { summary },
        }
    }

    /// Records a refusal, taking its class and exit status from the error
    /// rather than from the caller, so no command can report a class it did
    /// not raise.
    #[must_use]
    pub fn refused(command: &'static str, error: &BuildError, message: String) -> Self {
        Self {
            output_version: CLI_OUTPUT_VERSION,
            command,
            outcome: Outcome::Refused {
                error_class: error.class(),
                exit_code: ExitClass::of(error) as u8,
                message,
            },
        }
    }

    /// Renders the envelope for `--json`.
    ///
    /// # Panics
    ///
    /// Never in practice. Every field is a `String`, a `&'static str`, a `u8`,
    /// or a fieldless enum with a derived `Serialize`, and `serde_json` fails
    /// on none of those: its documented error cases are a map key that is not
    /// a string, a non-finite float, and a failing custom implementation, and
    /// this type has no map, no float, and no hand-written `Serialize`.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the envelope contains only infallible values")
    }
}

#[cfg(test)]
mod tests {
    use super::{CLI_OUTPUT_VERSION, CommandOutput};

    /// The success envelope names the command and carries its summary.
    #[test]
    fn t1_e2_a_success_envelope_names_its_command_and_version() {
        let rendered =
            CommandOutput::succeeded("lesson validate", "valid lesson".to_owned()).to_json();

        let parsed: serde_json::Value =
            serde_json::from_str(&rendered).expect("the envelope is JSON");
        assert_eq!(parsed["output_version"], CLI_OUTPUT_VERSION);
        assert_eq!(parsed["command"], "lesson validate");
        assert_eq!(parsed["outcome"], "succeeded");
        assert_eq!(parsed["summary"], "valid lesson");
    }
}
