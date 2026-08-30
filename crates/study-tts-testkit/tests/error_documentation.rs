//! The published `# Errors` contract of each documented entry point.
//!
//! `.claude/skills/rust-comment/SKILL.md` requires a `# Errors` section to name
//! *variants* rather than categories, and `AGENTS.md` §Coding conventions makes
//! a stale doc on a public item a defect rather than a tidy-up. Nothing the
//! compiler runs can tell that an error enum grew a variant while the prose
//! describing it stood still, and three times now it has: E1-S2 added five
//! lesson refusals and three voice-profile refusals that never reached
//! `build_preview`'s documentation, and then a recall-prompt refusal that
//! never reached `AuthoredLesson::validate`'s.
//!
//! This is the check that makes that a failing test, in both directions: every
//! declared variant is accounted for in the prose, and every variant the prose
//! names still exists. It is deliberately narrow — it does not assert that the
//! claim made about a variant is true. A variant the function cannot return is
//! accounted for by the sentence saying so, which is what
//! `ManagedPathError::InvalidManagedName` carries today.
//!
//! `ValidatedLesson::from_json` is deliberately not a row. It names the three
//! refusals it raises itself and delegates the rest to
//! `AuthoredLesson::validate` by rustdoc link, which `-D warnings` already
//! checks; giving it a row would demand a second copy of that list.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The module declaring `LessonError`, cited by both contracts below.
const LESSON: &str = "crates/study-tts-core/src/lesson.rs";

/// One documented entry point whose `# Errors` section is under test.
struct Contract {
    /// The module declaring the item.
    module: &'static str,
    /// The signature the doc comment sits above, unique within `module`.
    item: &'static str,
    /// Every error enum the section must account for, and where each is
    /// declared. A table rather than a scan of `error/`, because the claim is
    /// about the enums this entry point names — a new error module no
    /// document mentions is not this test's business.
    errors: &'static [(&'static str, &'static str)],
}

const CONTRACTS: [Contract; 2] = [
    Contract {
        module: "crates/study-tts-runtime/src/pipeline.rs",
        item: "pub fn build_preview(",
        errors: &[
            ("LessonError", LESSON),
            ("PlanError", "crates/study-tts-core/src/plan.rs"),
            ("VoiceError", "crates/study-tts-core/src/voice.rs"),
            (
                "VoiceProfileError",
                "crates/study-tts-runtime/src/error/voice_profile.rs",
            ),
            ("IoError", "crates/study-tts-runtime/src/error/io_error.rs"),
            ("ToolError", "crates/study-tts-runtime/src/error/tool.rs"),
            (
                "ManagedPathError",
                "crates/study-tts-runtime/src/error/managed_path.rs",
            ),
            ("CacheError", "crates/study-tts-runtime/src/error/cache.rs"),
            ("AudioError", "crates/study-tts-runtime/src/error/audio.rs"),
            (
                "DurableStateError",
                "crates/study-tts-runtime/src/error/state.rs",
            ),
        ],
    },
    // The lesson boundary is documented separately because it is reached
    // directly: a caller holding an `AuthoredLesson` never passes through
    // `build_preview`, so that function's prose does not describe this one.
    Contract {
        module: LESSON,
        item: "pub fn validate(",
        errors: &[("LessonError", LESSON)],
    },
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    fs::read_to_string(repository_root().join(relative))
        .unwrap_or_else(|error| panic!("`{relative}` is readable: {error}"))
}

/// The doc comment attached to `contract.item`.
///
/// Taken as the run of `///` lines immediately above the line the signature
/// starts on, so a mention elsewhere in the module — in another item's
/// documentation, or in a test — cannot satisfy the assertion below. The
/// signature must be unique in its module, or the section under test would be
/// some other item's.
fn documentation(source: &str, contract: &Contract) -> String {
    let module = contract.module;
    let item = contract.item;
    let mut signatures = source.match_indices(item).map(|(at, _)| at);
    let signature = signatures
        .next()
        .unwrap_or_else(|| panic!("`{module}` declares `{item}`"));
    assert!(
        signatures.next().is_none(),
        "`{module}` declares `{item}` more than once, so this test cannot tell \
         which one its documentation belongs to; give the row a longer signature"
    );
    // Back up to the start of the signature's own line: an indented item
    // leaves its leading whitespace as a final partial line, which is not a
    // `///` line and would end the run below before it began.
    let line_start = source[..signature].rfind('\n').map_or(0, |at| at + 1);
    source[..line_start]
        .lines()
        .rev()
        .take_while(|line| line.trim_start().starts_with("///"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every variant one `pub enum` declares.
///
/// Variants are the four-space-indented capitalized names inside the enum
/// body; every other line at that indentation in these files is a doc comment,
/// an attribute, or a field of the variant above it.
fn variants(source: &str, enum_name: &str) -> Vec<String> {
    let declaration = format!("pub enum {enum_name} {{");
    let start = source
        .find(&declaration)
        .unwrap_or_else(|| panic!("`{enum_name}` is declared"))
        + declaration.len();
    let body = &source[start..];
    let end = body
        .find("\n}\n")
        .unwrap_or_else(|| panic!("`{enum_name}` has a closing brace"));
    body[..end]
        .lines()
        .filter_map(|line| {
            let line = line.strip_prefix("    ")?;
            // A variant is the name up to whatever opens its payload; the
            // trailing space of `Variant {` is not part of the name.
            let name = line[..line.find(['(', '{', ','])?].trim_end();
            let is_variant =
                name.starts_with(char::is_uppercase) && name.chars().all(char::is_alphanumeric);
            is_variant.then(|| name.to_owned())
        })
        .collect()
}

#[test]
fn t3_e1_every_documented_error_variant_is_named_by_its_errors_section() {
    for contract in &CONTRACTS {
        let item = contract.item;
        let documentation = documentation(&read(contract.module), contract);

        let mut unaccounted = Vec::new();
        let mut retired = Vec::new();
        for (enum_name, path) in contract.errors {
            let declared = variants(&read(path), enum_name);
            assert!(
                !declared.is_empty(),
                "`{enum_name}` parsed to no variants; the parser in this test has drifted from \
                 `{path}`"
            );
            for variant in &declared {
                let named = format!("{enum_name}::{variant}");
                if !documentation.contains(&named) {
                    unaccounted.push(named);
                }
            }
            // The other direction: a variant deleted from the enum leaves a
            // link to nothing, and the prose around it describes a refusal
            // that can no longer happen.
            let prefix = format!("{enum_name}::");
            for named in documentation.match_indices(&prefix).map(|(at, _)| {
                let rest = &documentation[at + prefix.len()..];
                let end = rest
                    .find(|character: char| !character.is_alphanumeric())
                    .unwrap_or(rest.len());
                rest[..end].to_owned()
            }) {
                if !declared.contains(&named) {
                    retired.push(format!("{prefix}{named}"));
                }
            }
        }

        assert!(
            unaccounted.is_empty(),
            "`{item}` documents neither returning nor excluding {unaccounted:#?}; \
             `.claude/skills/rust-comment/SKILL.md` requires the section to name variants, so add \
             each one to the `# Errors` prose or say there why it cannot be returned"
        );
        assert!(
            retired.is_empty(),
            "`{item}` documents {retired:#?}, which no longer exist; remove the claim rather than \
             leaving a link to a refusal this build cannot make"
        );
    }
}
