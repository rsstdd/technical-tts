//! Checked, normalized BCP 47 tags used by synthesis identities.
//!
//! ADR-0001 §12.5 makes language a synthesis-key input. RFC 5646 §2.1.1
//! defines tags as case-insensitive, so parsing applies conventional casing
//! before a tag reaches a cache key.
//!
//! This build accepts a primary language, optional script, optional region,
//! and distinct variants. It excludes extlangs because RFC 5646 §2.2.2 gives
//! each an identical primary-language record and recommends that form. It
//! excludes extensions and private use because no supported backend interprets
//! them.
//!
//! This is a syntax check, not an IANA registry lookup. Backend support remains
//! a runtime decision in `study_tts_runtime::validate_executor_request`.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::digest::json_schema_as_string;

/// Maximum accepted language-tag length in bytes.
///
/// Mirrors `docs/architecture/WALKING-SKELETON.md` §Provisional resource
/// ceilings, which names this constant in return. The limit bounds variant
/// stacking because RFC 5646 bounds individual subtags but not their count.
pub const MAX_LANGUAGE_TAG_BYTES: usize = 64;

/// A BCP 47 language tag validated and case-normalized on construction.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct LanguageTag(String);

impl LanguageTag {
    /// The tag as it is written into an identity, a lesson, and a request.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The primary language subtag, without script, region, or variants.
    pub fn primary(&self) -> &str {
        self.0
            .split_once('-')
            .map_or(self.as_str(), |(primary, _)| primary)
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl From<LanguageTag> for String {
    fn from(tag: LanguageTag) -> Self {
        tag.0
    }
}

impl TryFrom<String> for LanguageTag {
    type Error = MalformedLanguageTag;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl FromStr for LanguageTag {
    type Err = MalformedLanguageTag;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_language_tag(value).ok_or_else(|| MalformedLanguageTag(value.to_owned()))
    }
}

// `lesson.rs::language_json_schema` publishes a deliberately looser spelling
// of this grammar; it must never reject a tag this parser accepts.
fn parse_language_tag(value: &str) -> Option<LanguageTag> {
    if value.is_empty() || value.len() > MAX_LANGUAGE_TAG_BYTES {
        return None;
    }

    let mut subtags = value.split('-').peekable();
    let mut normalized = String::with_capacity(value.len());

    let language = subtags.next()?;
    if !matches!(language.len(), 2..=8) || !language.bytes().all(|byte| byte.is_ascii_alphabetic())
    {
        return None;
    }
    normalized.push_str(&language.to_ascii_lowercase());

    if let Some(script) = subtags.next_if(|subtag| is_script(subtag)) {
        let mut characters = script.chars();
        normalized.push('-');
        normalized.push(characters.next()?.to_ascii_uppercase());
        normalized.extend(characters.map(|character| character.to_ascii_lowercase()));
    }
    if let Some(region) = subtags.next_if(|subtag| is_region(subtag)) {
        normalized.push('-');
        normalized.push_str(&region.to_ascii_uppercase());
    }
    let mut seen: Vec<String> = Vec::new();
    for variant in subtags {
        if !is_variant(variant) {
            return None;
        }
        let variant = variant.to_ascii_lowercase();
        if seen.contains(&variant) {
            return None;
        }
        normalized.push('-');
        normalized.push_str(&variant);
        seen.push(variant);
    }

    Some(LanguageTag(normalized))
}

fn is_script(subtag: &str) -> bool {
    subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_region(subtag: &str) -> bool {
    match subtag.len() {
        2 => subtag.bytes().all(|byte| byte.is_ascii_alphabetic()),
        3 => subtag.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

fn is_variant(subtag: &str) -> bool {
    let alphanumeric = subtag.bytes().all(|byte| byte.is_ascii_alphanumeric());
    match subtag.len() {
        4 => alphanumeric && subtag.starts_with(|first: char| first.is_ascii_digit()),
        5..=8 => alphanumeric,
        _ => false,
    }
}

/// A language tag rejected by the accepted BCP 47 subset.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error(
    "language `{0}` is not a BCP 47 language tag this build accepts; the document's author must \
     write a language subtag with an optional script, an optional region, and distinct variants, \
     such as `en` or `en-US`, and must not repeat a variant or use an extlang, extension, or \
     private-use subtag — an extlang has a recommended identical primary-language form to use \
     instead, and this build has no backend that reads the other two"
)]
pub struct MalformedLanguageTag(String);

json_schema_as_string!(
    LanguageTag,
    "LanguageTag",
    "A BCP 47 language tag: the RFC 5646 2.1 langtag production without \
     extlang, extension, or private-use subtags and with no repeated variant, \
     case-normalized on parse."
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t1_e1_language_tags_are_case_normalized_to_one_spelling() {
        for (authored, normalized) in [
            ("en", "en"),
            ("EN", "en"),
            ("en-us", "en-US"),
            ("EN-US", "en-US"),
            ("en-latn-us", "en-Latn-US"),
            ("EN-LATN-US", "en-Latn-US"),
            ("sr-cyrl-rs", "sr-Cyrl-RS"),
            ("es-419", "es-419"),
            ("de-DE-1901", "de-DE-1901"),
            ("sl-rozaj-biske", "sl-rozaj-biske"),
            ("cel-gaulish", "cel-gaulish"),
        ] {
            let parsed: LanguageTag = authored.parse().expect("a valid tag parses");

            assert_eq!(parsed.as_str(), normalized, "normalizing `{authored}`");
        }
    }

    #[test]
    fn t1_e1_tags_outside_the_accepted_grammar_are_refused() {
        let boundary = "en-abcdefg1-abcdefg2-abcdefg3-abcdefg4-abcdefg5-abcdefg6-abcdefg";
        assert_eq!(boundary.len(), MAX_LANGUAGE_TAG_BYTES);
        boundary
            .parse::<LanguageTag>()
            .expect("the byte boundary must be accepted");

        let too_long = format!("{boundary}h");
        let cases = [
            ("", "an absent tag"),
            (" ", "whitespace"),
            ("e", "a one-letter language"),
            ("abcdefghi", "a nine-letter language"),
            ("en_US", "an underscore separator"),
            ("en-", "a trailing separator"),
            ("-en", "a leading separator"),
            ("en--US", "an empty subtag"),
            ("en-US-", "a trailing separator after a region"),
            ("en-u-co-phonebk", "an extension subtag"),
            ("en-x-private", "a private-use subtag"),
            ("x-private", "a private-use tag"),
            ("en-USA", "a three-letter region"),
            ("en-1", "a one-character variant"),
            ("en-日本", "a non-ASCII subtag"),
            (
                "zh-yue",
                "an excluded extlang with a recommended primary form",
            ),
            (
                "de-1901-1901",
                "a variant repeated, which RFC 5646 2.2.5 forbids",
            ),
            ("de-DE-1901-1901", "a variant repeated after a region"),
            (
                "de-1901-1996-1901",
                "a variant repeated after another variant",
            ),
            ("sl-ROZAJ-rozaj", "a variant repeated in a second spelling"),
            (&too_long, "a tag one byte past the ceiling"),
        ];

        for (malformed, why) in cases {
            assert_eq!(
                malformed.parse::<LanguageTag>(),
                Err(MalformedLanguageTag(malformed.to_owned())),
                "`{malformed}` must be refused: {why}"
            );
            assert!(
                serde_json::from_value::<LanguageTag>(serde_json::Value::String(
                    malformed.to_owned()
                ))
                .is_err(),
                "a recorded language `{malformed}` must not deserialize"
            );
        }
    }

    #[test]
    fn t1_e1_a_regional_tag_reports_its_primary_language() {
        for (tag, primary) in [("en", "en"), ("en-US", "en"), ("sr-Cyrl-RS", "sr")] {
            let parsed: LanguageTag = tag.parse().expect("a valid tag parses");

            assert_eq!(parsed.primary(), primary);
        }
    }
}
