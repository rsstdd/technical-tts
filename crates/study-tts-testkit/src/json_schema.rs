//! A bounded JSON Schema validator, for checking this project's own published
//! schemas against this project's own examples.
//!
//! **Why this is not a dependency.** `jsonschema` was measured against this
//! workspace rather than guessed at: with default features off it still adds 86
//! crates, taking `Cargo.lock` from 53 entries to 139, for a check that runs
//! against seven files that never leave the repository. Nearly all of it —
//! ICU tables, `time`, `uuid`, `iso8601` — serves `format` assertions and
//! remote-`$ref` URL resolution that these schemas never use, and a validator
//! that can fetch a remote `$ref` introduces exactly the risk ADR-0001 §14
//! spends a network-namespace test removing. Under ADR-0001 §7.2 — a crate is
//! added "only when it removes more risk than it introduces" — that trade does
//! not pay, so the keyword subset is implemented here, in the test crate, where
//! it can never reach a production path.
//!
//! **Why a small validator is not a weak one.** The usual failure of a
//! hand-rolled validator is that it silently ignores what it does not
//! understand, so it agrees with every schema including a wrong one. This one
//! refuses instead, and "does not understand" has two halves that leave a
//! constraint equally unchecked: [`validate_against_schema`] reports a keyword
//! it has never implemented, and [`malformed_schema`] reports a keyword it
//! implements whose schema value it cannot read — a `required` that is a
//! string, an `enum` that is not an array. Both are violations of the
//! *schema*, not of the instance. An inapplicable *instance* is neither, and
//! stays silent: `minLength` says nothing about a number, and the
//! specification has it apply to strings alone.
//!
//! Adding a keyword to a generated schema therefore fails the suite until this
//! file learns it, and mistyping one in a hand-edited `schemas/` file fails it
//! rather than quietly switching the constraint off. That is the only
//! arrangement under which "the examples validate" means anything.
//!
//! The dialect is JSON Schema 2020-12, restricted to the keywords `schemars`
//! emits for the types in `study_tts_runtime::PUBLISHED_SCHEMAS`. The one
//! keyword not implemented here is `pattern`, which that dialect defines as an
//! ECMA-262 regular expression: matching one is the part of this job that is
//! genuinely hard to get right, and it is the part where the failure above —
//! quietly accepting what was not understood — is hardest to notice, so it goes
//! to `regex` rather than to a second hand-rolled matcher. That crate is four
//! transitive dependencies and reads no files and no network. Its parser,
//! `regex-syntax`, is named directly as well: deciding whether a pattern is
//! anchored is a question about its structure, and the two crates are one
//! acquisition.
//!
//! `regex` is not ECMA-262, and the gap is worth naming rather than assuming.
//! Backreferences and every lookaround except the fixed absolute-end guard are
//! refused loudly. `\d`, `\w`, `\s`, and `\b` are Unicode-aware here and
//! ASCII-only in ECMA-262, so a body using one would be checked against a wider
//! class than a conforming editor applies. No published body uses one. The
//! guard deliberately uses `[\s\S]`, whose union is every character under both
//! meanings, and [`check_pattern`] translates only that exact suffix.

use std::collections::BTreeSet;

use regex::Regex;
use regex_syntax::hir::Look;
use serde_json::Value;

/// Keywords that carry documentation rather than a constraint.
///
/// Listed rather than skipped by default: every keyword is either enforced or
/// named here, so a keyword nobody thought about cannot pass as an annotation.
///
/// `$id` is identity, not a constraint on any instance. It changes how a
/// relative `$ref` resolves, and every `$ref` this project publishes is the
/// absolute `#/$defs/...` form that `resolve` handles, so naming it here says
/// what it is rather than leaving it to the unimplemented-keyword arm.
const ANNOTATIONS: [&str; 6] = ["$schema", "$defs", "$id", "description", "title", "format"];

/// ECMAScript's absolute-end guard, translated to Rust regex `\z` below.
const ABSOLUTE_END_GUARD: &str = r"$(?![\s\S])";

/// Checks one document against one schema.
///
/// # Errors
///
/// Every violation found, each as a message beginning with the JSON Pointer of
/// the value it is about. All of them are returned rather than the first,
/// because a schema change usually breaks several examples at once and fixing
/// them one failure per run is how a suite stops being run.
///
/// A schema keyword this validator does not implement is reported as a
/// violation, so an unchecked constraint can never read as a passing example.
///
/// # Examples
///
/// ```rust
/// use serde_json::json;
/// use study_tts_testkit::validate_against_schema;
///
/// let schema = json!({"type": "object", "required": ["id"],
///                     "properties": {"id": {"type": "string"}}});
///
/// assert!(validate_against_schema(&schema, &json!({"id": "seg-1"})).is_ok());
/// assert!(validate_against_schema(&schema, &json!({})).is_err());
/// assert!(validate_against_schema(&schema, &json!({"id": 1})).is_err());
/// ```
pub fn validate_against_schema(schema: &Value, instance: &Value) -> Result<(), Vec<String>> {
    let mut violations = Vec::new();
    check(schema, schema, instance, "", &mut violations);
    if violations.is_empty() {
        return Ok(());
    }
    Err(violations)
}

/// Applies every keyword of `schema` to `instance`.
///
/// `root` is carried separately from `schema` because `$ref` resolves against
/// the document root, and a subschema does not otherwise know where it lives.
fn check(root: &Value, schema: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let object = match schema {
        Value::Object(object) => object,
        // A boolean schema. `true` accepts anything; `false` accepts nothing.
        Value::Bool(true) => return,
        Value::Bool(false) => {
            found.push(format!("{at}: schema accepts nothing"));
            return;
        }
        malformed => {
            found.push(format!(
                "{at}: schema is {malformed}, not an object or boolean; correct the schema"
            ));
            return;
        }
    };

    if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
        match resolve(root, reference) {
            Some(target) => check(root, target, instance, at, found),
            None => found.push(format!(
                "{at}: schema `$ref` `{reference}` does not resolve"
            )),
        }
        // A `$ref` beside other keywords is legal in 2020-12, so the rest of
        // this schema still applies and is checked below.
    }

    for (keyword, expected) in object {
        match keyword.as_str() {
            "$ref" => {}
            keyword if ANNOTATIONS.contains(&keyword) => {}
            "type" => check_type(expected, instance, at, found),
            "const" => {
                if instance != expected {
                    found.push(format!(
                        "{at}: expected the constant {expected}, found {instance}"
                    ));
                }
            }
            "properties" => check_properties(root, expected, instance, at, found),
            "required" => check_required(expected, instance, at, found),
            "additionalProperties" => {
                check_additional_properties(root, object, expected, instance, at, found);
            }
            "items" => check_items(root, expected, instance, at, found),
            "uniqueItems" => check_unique_items(expected, instance, at, found),
            "minimum" | "maximum" => check_bound(keyword, expected, instance, at, found),
            "minLength" | "maxLength" => check_length(keyword, expected, instance, at, found),
            "minItems" | "maxItems" => check_item_count(keyword, expected, instance, at, found),
            "enum" => check_enum(expected, instance, at, found),
            "pattern" => check_pattern(expected, instance, at, found),
            "oneOf" | "anyOf" => check_branches(root, keyword, expected, instance, at, found),
            unimplemented => found.push(format!(
                "{at}: schema uses `{unimplemented}`, which this validator does not implement; \
                 teach it that keyword rather than leaving the constraint unchecked"
            )),
        }
    }
}

/// Resolves a local `#/$defs/Name` pointer.
///
/// Only local pointers are supported, and a remote one is left unresolved on
/// purpose: fetching it is the behavior this module exists to avoid.
fn resolve<'a>(root: &'a Value, reference: &str) -> Option<&'a Value> {
    let mut target = root;
    for segment in reference.strip_prefix("#/")?.split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        target = target.get(&segment)?;
    }
    Some(target)
}

/// Reports a keyword whose *schema* value is not the shape the keyword needs.
///
/// The counterpart to [`check`]'s unimplemented-keyword arm, and there for the
/// same reason: a keyword this validator recognized but could not read leaves
/// its constraint unchecked, and an unchecked constraint must never read as a
/// passing example.
///
/// An *instance* of an inapplicable type is not this — `minLength` says nothing
/// about a number, and the specification has it apply to strings alone — so
/// those keep returning quietly.
fn malformed_schema(
    keyword: &str,
    expected: &Value,
    shape: &str,
    at: &str,
    found: &mut Vec<String>,
) {
    found.push(format!(
        "{at}: schema `{keyword}` is {expected}, not {shape}; correct the schema rather than \
         leaving the constraint unchecked"
    ));
}

fn check_type(expected: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let accepted: Vec<&str> = match expected {
        Value::String(name) => vec![name.as_str()],
        Value::Array(names)
            if !names.is_empty() && names.iter().all(|name| name.as_str().is_some()) =>
        {
            names.iter().filter_map(Value::as_str).collect()
        }
        other => {
            found.push(format!(
                "{at}: schema `type` is {other}, not a name or non-empty list of names; correct \
                 the schema"
            ));
            return;
        }
    };
    if let Some(unknown) = accepted.iter().find(|name| {
        !matches!(
            **name,
            "null" | "boolean" | "object" | "array" | "string" | "number" | "integer"
        )
    }) {
        found.push(format!(
            "{at}: schema `type` names unknown type `{unknown}`; correct the schema"
        ));
        return;
    }
    if !accepted.iter().any(|name| matches_type(name, instance)) {
        found.push(format!(
            "{at}: expected type {}, found {}",
            accepted.join(" or "),
            type_name(instance)
        ));
    }
}

/// Whether `instance` is of the named JSON Schema type.
///
/// `integer` accepts a float with no fractional part, which the specification
/// requires and which matters here: `serde_json` reads `2400` from a manifest
/// as an integer but a round-tripped `2400.0` is the same number.
fn matches_type(name: &str, instance: &Value) -> bool {
    match name {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => {
            instance.is_i64()
                || instance.is_u64()
                || instance
                    .as_f64()
                    .is_some_and(|number| number.fract() == 0.0)
        }
        _ => false,
    }
}

fn type_name(instance: &Value) -> &'static str {
    match instance {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn check_properties(
    root: &Value,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(properties) = expected.as_object() else {
        malformed_schema("properties", expected, "an object of subschemas", at, found);
        return;
    };
    let Some(object) = instance.as_object() else {
        return;
    };
    for (name, subschema) in properties {
        if let Some(value) = object.get(name) {
            check(root, subschema, value, &format!("{at}/{name}"), found);
        }
    }
}

fn check_required(expected: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let Some(names) = expected.as_array() else {
        malformed_schema(
            "required",
            expected,
            "an array of property names",
            at,
            found,
        );
        return;
    };
    let Some(object) = instance.as_object() else {
        return;
    };
    for name in names {
        let Some(name) = name.as_str() else {
            malformed_schema("required", name, "a property name", at, found);
            continue;
        };
        if !object.contains_key(name) {
            found.push(format!("{at}: required property `{name}` is absent"));
        }
    }
}

/// Applies `additionalProperties` to the members `properties` did not name.
///
/// The sibling `properties` has to be read here rather than trusted to have run
/// first: `additionalProperties` is defined in terms of it, and keyword order
/// in a JSON object is not something a schema author controls.
fn check_additional_properties(
    root: &Value,
    schema: &serde_json::Map<String, Value>,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(object) = instance.as_object() else {
        return;
    };
    let named: BTreeSet<&str> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| properties.keys().map(String::as_str).collect())
        .unwrap_or_default();

    for (name, value) in object {
        if named.contains(name.as_str()) {
            continue;
        }
        if expected.as_bool() == Some(false) {
            found.push(format!("{at}: property `{name}` is not permitted"));
            continue;
        }
        check(root, expected, value, &format!("{at}/{name}"), found);
    }
}

fn check_items(
    root: &Value,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(elements) = instance.as_array() else {
        return;
    };
    for (position, element) in elements.iter().enumerate() {
        check(root, expected, element, &format!("{at}/{position}"), found);
    }
}

fn check_unique_items(expected: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let Some(unique) = expected.as_bool() else {
        malformed_schema("uniqueItems", expected, "a boolean", at, found);
        return;
    };
    let (true, Some(elements)) = (unique, instance.as_array()) else {
        return;
    };
    // Compared by serialized form because `serde_json::Value` is not `Ord` and
    // equality over a quadratic scan would be the only alternative. Object key
    // order is stable here: every value came from `serde_json`'s own parser,
    // which preserves input order, and both sides of a duplicate would have to
    // differ in key order alone to slip through.
    let mut seen = BTreeSet::new();
    for element in elements {
        if !seen.insert(element.to_string()) {
            found.push(format!(
                "{at}: array contains the duplicate element {element}"
            ));
        }
    }
}

fn check_bound(
    keyword: &str,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(bound) = expected.as_f64() else {
        malformed_schema(keyword, expected, "a number", at, found);
        return;
    };
    let Some(number) = instance.as_f64() else {
        return;
    };
    let violated = match keyword {
        "minimum" => number < bound,
        // The caller only routes these two keywords here.
        _ => number > bound,
    };
    if violated {
        found.push(format!("{at}: {number} violates {keyword} {bound}"));
    }
}

/// Applies `minLength` or `maxLength`, which JSON Schema counts in characters.
///
/// Characters rather than bytes matters here: the identifier ceiling this
/// mirrors is a byte count in Rust, and the pattern beside it is what keeps the
/// two the same by admitting only ASCII.
fn check_length(
    keyword: &str,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(bound) = expected.as_u64() else {
        malformed_schema(keyword, expected, "a non-negative integer", at, found);
        return;
    };
    let Some(text) = instance.as_str() else {
        return;
    };
    let length = text.chars().count() as u64;
    let violated = match keyword {
        "minLength" => length < bound,
        // The caller only routes these two keywords here.
        _ => length > bound,
    };
    if violated {
        found.push(format!(
            "{at}: `{text}` is {length} characters, violating {keyword} {bound}"
        ));
    }
}

/// Applies `minItems` or `maxItems`.
///
/// The takes document is what needs it: ADR-0001 §12.2 requires one selection
/// per lesson segment, so its list is bounded by the lesson's own segment
/// ceiling and an author's editor should say so.
fn check_item_count(
    keyword: &str,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(bound) = expected.as_u64() else {
        malformed_schema(keyword, expected, "a non-negative integer", at, found);
        return;
    };
    let Some(items) = instance.as_array() else {
        return;
    };
    let count = items.len() as u64;
    let violated = match keyword {
        "minItems" => count < bound,
        // The caller only routes these two keywords here.
        _ => count > bound,
    };
    if violated {
        found.push(format!("{at}: {count} items violate {keyword} {bound}"));
    }
}

fn check_enum(expected: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let Some(accepted) = expected.as_array() else {
        malformed_schema("enum", expected, "an array of accepted values", at, found);
        return;
    };
    if !accepted.contains(instance) {
        found.push(format!("{at}: {instance} is not one of {expected}"));
    }
}

/// Applies `pattern` as the ECMA-262 regular expression the dialect defines.
///
/// Anchors are required rather than optional. `pattern` constrains a
/// *substring* by definition, so an unanchored one in a schema this project
/// publishes almost always means the author forgot them, and the constraint
/// the schema states is then wider than the one they meant to write.
///
/// ECMAScript `$` also matches before a final line terminator. Every published
/// pattern therefore ends with `$(?![\s\S])`: the negative lookahead makes
/// that position the absolute end. Rust's regex engine deliberately omits
/// lookaround, so this one fixed guard is translated to its `\z` assertion
/// before parsing and matching. Any other lookaround remains a schema error.
///
/// Whether the translated expression is anchored is answered by parsing it,
/// not by reading its first and last character. `^a|b$` opens with one anchor
/// and closes with the other while binding neither side of the alternation,
/// and `^price\$` ends in an escaped literal that is not an anchor at all; both
/// pass a textual check and neither constrains the whole string.
///
/// A pattern that will not parse is reported as a fault in the *schema*, for
/// the reason an unimplemented keyword is: a constraint nobody checked must
/// never read as a passing example. The two constructs refused on purpose,
/// backreferences and lookaround, are outside `regex`'s linear-time guarantee
/// and have no place in a published schema an editor has to honor.
fn check_pattern(expected: &Value, instance: &Value, at: &str, found: &mut Vec<String>) {
    let Some(pattern) = expected.as_str() else {
        malformed_schema("pattern", expected, "a regular expression", at, found);
        return;
    };
    let Some(text) = instance.as_str() else {
        return;
    };

    let Some(prefix) = pattern.strip_suffix(ABSOLUTE_END_GUARD) else {
        found.push(format!(
            "{at}: schema pattern `{pattern}` has no ECMAScript absolute-end guard; end it with \
             `{ABSOLUTE_END_GUARD}`"
        ));
        return;
    };
    let translated = format!(r"{prefix}\z");

    let parsed = match regex_syntax::parse(&translated) {
        Ok(parsed) => parsed,
        // Collapsed to one line because a parse error is several, and a
        // violation is read in a terminal beside other violations.
        Err(error) => {
            found.push(format!(
                "{at}: schema pattern `{pattern}` is not a regular expression this validator can \
                 compile ({}); rewrite it rather than leaving the constraint unchecked",
                error
                    .to_string()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
            return;
        }
    };

    let assertions = parsed.properties();
    if !(assertions.look_set_prefix().contains(Look::Start)
        && assertions.look_set_suffix().contains(Look::End))
    {
        found.push(format!(
            "{at}: schema pattern `{pattern}` does not anchor both ends of every alternative; \
             `pattern` constrains a substring, so anchor it as `^...$` rather than publishing a \
             constraint wider than it reads"
        ));
        return;
    }

    // Compiled from the same text rather than from `parsed`, because building a
    // `Regex` from an HIR is `regex-automata`'s meta API rather than this one,
    // and a second parse of a published pattern is not worth reaching for it.
    //
    // ponytail: parsed and compiled once per value checked rather than once per
    // pattern. Measured at 21 us to parse and 102 us to compile the published
    // patterns, against three `pattern` sites in one schema and a suite that
    // finishes in under 5 ms. Thread a `BTreeMap<&str, Regex>` down from
    // `validate` if a schema ever carries enough of them to notice.
    match Regex::new(&translated) {
        Ok(compiled) if compiled.is_match(text) => {}
        Ok(_) => found.push(format!("{at}: `{text}` does not match `{pattern}`")),
        Err(error) => found.push(format!(
            "{at}: schema pattern `{pattern}` parsed but did not compile ({}); rewrite it rather \
             than leaving the constraint unchecked",
            error
                .to_string()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        )),
    }
}

fn check_branches(
    root: &Value,
    keyword: &str,
    expected: &Value,
    instance: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    let Some(branches) = expected.as_array() else {
        malformed_schema(keyword, expected, "an array of subschemas", at, found);
        return;
    };
    // Each branch's own violations are kept rather than counted away. A
    // tagged union publishes one branch per variant, so "matches 0 of 7" is
    // true and useless: it names neither the field that is wrong nor the
    // variant the author meant. The branch that came closest almost always is
    // that variant, and its complaints are the ones worth reading.
    let attempts: Vec<Vec<String>> = branches
        .iter()
        .map(|branch| {
            let mut violations = Vec::new();
            check(root, branch, instance, at, &mut violations);
            violations
        })
        .collect();
    let matched = attempts
        .iter()
        .filter(|violations| violations.is_empty())
        .count();

    let satisfied = match keyword {
        "oneOf" => matched == 1,
        // The caller only routes these two keywords here.
        _ => matched >= 1,
    };
    if satisfied {
        return;
    }

    found.push(format!(
        "{at}: {instance} matches {matched} of {} `{keyword}` branches",
        branches.len()
    ));
    // Only when nothing matched. Two or more matches make the document
    // ambiguous rather than wrong, and no branch has a complaint to add.
    if matched == 0
        && let Some(closest) = attempts
            .iter()
            .min_by_key(|violations| closeness(violations, at))
    {
        found.extend(closest.iter().cloned());
    }
}

/// Ranks one branch's complaints: the more specific, the closer.
///
/// Fewest-violations alone picks the wrong branch for the commonest `anyOf`
/// this project publishes. An `Option<T>` is `[{"$ref": T}, {"type": "null"}]`,
/// and the null branch always has exactly one complaint — "expected type null"
/// — so it beat the `$ref` branch whenever the instance had two fields wrong.
/// The reader was then told the value should have been null, which is true of
/// no document anybody meant to write.
///
/// A complaint about the value as a whole says less than one naming a field
/// inside it, so branches that only manage the former sort last, and violation
/// count decides between the rest.
fn closeness(violations: &[String], at: &str) -> (bool, usize) {
    let whole_value_only = violations
        .iter()
        .all(|violation| violation.starts_with(&format!("{at}: ")));
    (whole_value_only, violations.len())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn t1_e1_a_keyword_this_validator_cannot_check_is_a_failure() {
        // The property that makes every other test here mean something. A
        // validator that ignored `patternProperties` would report an example as
        // valid without having checked the constraint that would have rejected
        // it.
        let schema = json!({"type": "object", "patternProperties": {"^a": {"type": "string"}}});

        let violations = validate_against_schema(&schema, &json!({"ab": 1}))
            .expect_err("an unimplemented keyword must be reported");

        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("patternProperties")),
            "expected the unimplemented keyword to be named, got {violations:?}"
        );
    }

    #[test]
    fn t1_e1_a_keyword_whose_schema_value_cannot_be_read_is_a_failure() {
        // The sibling of the test above, for the other half of the same
        // property. A keyword this validator recognizes but whose value it
        // cannot read leaves the constraint unchecked just as completely as one
        // it has never heard of, and `schemas/` is hand-editable.
        let cases: [(Value, Value); 12] = [
            (json!(5), json!(5)),
            (json!({"properties": "a"}), json!({"a": 1})),
            (json!({"required": "a"}), json!({})),
            (json!({"required": ["a", 5]}), json!({"a": 1})),
            (json!({"uniqueItems": "true"}), json!([1, 1])),
            (json!({"minimum": "0"}), json!(-1)),
            (json!({"maxLength": "2"}), json!("abc")),
            (json!({"enum": "1.0"}), json!("1.2")),
            (json!({"pattern": 5}), json!("aB")),
            (json!({"type": []}), json!("value")),
            (json!({"type": ["string", 5]}), json!("value")),
            (json!({"type": "imaginary"}), json!("value")),
        ];

        for (schema, instance) in cases {
            let violations = validate_against_schema(&schema, &instance)
                .expect_err("a schema value this validator cannot read must be reported");

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains("correct the schema")),
                "expected {schema} to be reported as the schema's fault, got {violations:?}"
            );
        }

        // `uniqueItems: false` is the constraint switched off, not a schema
        // nobody could read, so it stays silent.
        assert!(validate_against_schema(&json!({"uniqueItems": false}), &json!([1, 1])).is_ok());
    }

    #[test]
    fn t1_e1_the_implemented_keywords_accept_and_refuse() {
        // One case per keyword, each with the instance that satisfies it and
        // the instance that does not, so a keyword that silently accepted
        // everything fails here.
        let cases: [(Value, Value, Value); 15] = [
            (json!({"type": "string"}), json!("x"), json!(1)),
            (json!({"type": ["string", "null"]}), json!(null), json!(1)),
            (json!({"type": "integer"}), json!(3), json!(3.5)),
            (
                json!({"const": "approved"}),
                json!("approved"),
                json!("draft"),
            ),
            (
                json!({"properties": {"a": {"type": "string"}}}),
                json!({"a": "x"}),
                json!({"a": 1}),
            ),
            (json!({"required": ["a"]}), json!({"a": 1}), json!({})),
            (
                json!({"properties": {"a": {}}, "additionalProperties": false}),
                json!({"a": 1}),
                json!({"b": 1}),
            ),
            (
                json!({"items": {"type": "string"}}),
                json!(["x"]),
                json!([1]),
            ),
            (json!({"uniqueItems": true}), json!([1, 2]), json!([1, 1])),
            (json!({"minLength": 2}), json!("ab"), json!("a")),
            (json!({"maxLength": 2}), json!("ab"), json!("abc")),
            (json!({"minItems": 2}), json!([1, 2]), json!([1])),
            (json!({"maxItems": 2}), json!([1, 2]), json!([1, 2, 3])),
            (json!({"enum": ["1.0", "1.1"]}), json!("1.1"), json!("1.2")),
            (
                json!({"pattern": "^[a-z]+$(?![\\s\\S])"}),
                json!("ab"),
                json!("aB"),
            ),
        ];

        for (schema, accepted, refused) in cases {
            assert!(
                validate_against_schema(&schema, &accepted).is_ok(),
                "{schema} must accept {accepted}"
            );
            assert!(
                validate_against_schema(&schema, &refused).is_err(),
                "{schema} must refuse {refused}"
            );
        }
    }

    #[test]
    fn t1_e1_bounds_and_branches_are_checked() {
        let bounded = json!({"minimum": 0, "maximum": 10});
        assert!(validate_against_schema(&bounded, &json!(5)).is_ok());
        assert!(validate_against_schema(&bounded, &json!(-1)).is_err());
        assert!(validate_against_schema(&bounded, &json!(11)).is_err());

        // `oneOf` requires exactly one match, which is what makes a closed
        // vocabulary closed: two matching branches is as wrong as none.
        let exclusive = json!({"oneOf": [{"const": "a"}, {"const": "b"}]});
        assert!(validate_against_schema(&exclusive, &json!("a")).is_ok());
        assert!(validate_against_schema(&exclusive, &json!("c")).is_err());

        let ambiguous = json!({"oneOf": [{"type": "string"}, {"type": "string"}]});
        assert!(validate_against_schema(&ambiguous, &json!("a")).is_err());

        // Nothing matched, so the reader is told which branch came closest
        // and why. A tagged union publishes one branch per variant, and
        // "matches 0 of 2" names neither the field that is wrong nor the
        // variant that was meant.
        let tagged = json!({"oneOf": [
            {"properties": {"tag": {"const": "a"}, "value": {"type": "string"}},
             "required": ["tag", "value"]},
            {"properties": {"tag": {"const": "b"}}, "required": ["tag", "extra"]},
        ]});
        let violations = validate_against_schema(&tagged, &json!({"tag": "a", "value": 1}))
            .expect_err("a document matching no branch is refused");
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("/value")),
            "the closest branch's own complaint must be reported: {violations:?}"
        );

        let permissive = json!({"anyOf": [{"type": "string"}, {"type": "integer"}]});
        assert!(validate_against_schema(&permissive, &json!("a")).is_ok());
        assert!(validate_against_schema(&permissive, &json!(true)).is_err());

        // The `Option<T>` shape every published schema uses for an optional
        // record. The null branch is always one complaint long, so ranking by
        // count alone reported "expected type null" and named neither wrong
        // field.
        let optional = json!({"anyOf": [
            {"properties": {"first": {"const": "a"}, "second": {"const": "b"}},
             "required": ["first", "second"]},
            {"type": "null"},
        ]});
        let violations = validate_against_schema(&optional, &json!({"first": "x", "second": "y"}))
            .expect_err("a present-but-wrong optional record is refused");
        assert!(
            violations
                .iter()
                .any(|violation| violation.contains("/first"))
                && violations
                    .iter()
                    .any(|violation| violation.contains("/second")),
            "the record branch's own complaints must be reported, not the null branch's: \
             {violations:?}"
        );
    }

    #[test]
    fn t1_e1_the_published_patterns_accept_and_refuse() {
        // The two patterns the published schemas actually carry, so `pattern`
        // is tested against its real work rather than against invented ones.
        // Carried over from the hand-rolled matcher this keyword replaced: the
        // engine changed, the constraints the schemas state did not.
        const IDENTIFIER: &str = r"^[A-Za-z0-9_-][A-Za-z0-9._-]*$(?![\s\S])";
        const LANGUAGE_TAG: &str = r"^[A-Za-z]{2,8}(-[A-Za-z0-9]{1,8})*$(?![\s\S])";

        let cases = [
            (IDENTIFIER, "e0-s0-two-segment", true),
            (IDENTIFIER, "seg-0001", true),
            (IDENTIFIER, "a", true),
            (IDENTIFIER, "a.b_c-d", true),
            // A leading dot names a hidden directory, which is why the first
            // class differs from the rest.
            (IDENTIFIER, ".hidden", false),
            (IDENTIFIER, "", false),
            (IDENTIFIER, "has space", false),
            (IDENTIFIER, "has/slash", false),
            (IDENTIFIER, "unicod\u{e9}", false),
            (IDENTIFIER, "valid\n", false),
            (LANGUAGE_TAG, "en", true),
            (LANGUAGE_TAG, "en-US", true),
            (LANGUAGE_TAG, "en-Latn-US", true),
            (LANGUAGE_TAG, "sl-rozaj-biske", true),
            (LANGUAGE_TAG, "es-419", true),
            (LANGUAGE_TAG, "en_US", false),
            (LANGUAGE_TAG, "e", false),
            (LANGUAGE_TAG, "en-", false),
            (LANGUAGE_TAG, "-en", false),
            (LANGUAGE_TAG, "en--US", false),
            (LANGUAGE_TAG, "abcdefghi", false),
            (LANGUAGE_TAG, "en-abcdefghi", false),
            (LANGUAGE_TAG, "en\n", false),
        ];

        for (pattern, text, accepted) in cases {
            let schema = json!({ "pattern": pattern });
            assert_eq!(
                validate_against_schema(&schema, &json!(text)).is_ok(),
                accepted,
                "`{text}` against `{pattern}`"
            );
        }
    }

    #[test]
    fn t1_e1_a_pattern_this_validator_cannot_check_is_a_schema_failure() {
        // The property that makes the cases above mean something. Every one of
        // these would leave a published constraint unchecked, so each has to
        // fail the suite rather than pass silently against any instance.
        for unusable in [
            "a$",                  // unanchored at the start
            "^a",                  // unanchored at the end
            r"^a|b$",              // one branch anchored at each end, neither at both
            r"^price\$",           // a trailing escaped literal, not an anchor
            "^(a$",                // an unclosed group
            "^a)$",                // an unopened group
            "^[a$",                // an unclosed class
            "^a{2,1}$",            // an impossible bound
            "^[z-a]$",             // a reversed range
            r"^(a)\1$(?![\s\S])",  // a backreference, outside the linear-time guarantee
            r"^(?=a)a$(?![\s\S])", // lookaround, likewise
        ] {
            let violations = validate_against_schema(&json!({ "pattern": unusable }), &json!("a"))
                .err()
                .unwrap_or_else(|| panic!("`{unusable}` must be reported rather than guessed at"));

            assert!(
                violations
                    .iter()
                    .any(|violation| violation.contains("schema pattern")),
                "`{unusable}` was refused, but as an instance violation: {violations:?}"
            );
        }
    }

    #[test]
    fn t1_e1_local_references_resolve_and_remote_ones_do_not() {
        let schema = json!({
            "$defs": {"Id": {"type": "string"}},
            "properties": {"id": {"$ref": "#/$defs/Id"}},
        });
        assert!(validate_against_schema(&schema, &json!({"id": "x"})).is_ok());
        assert!(validate_against_schema(&schema, &json!({"id": 1})).is_err());

        // A remote reference is reported rather than fetched. ADR-0001 §14
        // denies network egress, so a validator that resolved one would pass
        // where the project promises nothing.
        let remote = json!({"$ref": "https://example.invalid/schema.json"});
        assert!(validate_against_schema(&remote, &json!({})).is_err());
    }
}
