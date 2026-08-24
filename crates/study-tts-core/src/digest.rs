//! The textual shape of a BLAKE3 digest, shared by every value that records
//! one.

/// Length of a BLAKE3 digest rendered as lowercase hexadecimal.
pub(crate) const BLAKE3_HEX_LENGTH: usize = 64;

/// Whether `value` is exactly the form `blake3::Hash::to_hex` produces.
///
/// "Well formed" has to mean that exact form, because a recorded digest is
/// compared against that output byte for byte and a cache key is used as a
/// directory name. Uppercase hex is rejected rather than normalized: a value
/// that needs normalizing before it can be compared did not come from this
/// program, and silently accepting it hides that.
///
/// # Examples
///
/// ```rust
/// use study_tts_core::is_blake3_hex;
///
/// assert!(is_blake3_hex(&"a".repeat(64)));
/// assert!(!is_blake3_hex(&"A".repeat(64)), "uppercase is not normalized");
/// assert!(!is_blake3_hex("abc"), "a short digest is not a digest");
/// ```
pub fn is_blake3_hex(value: &str) -> bool {
    value.len() == BLAKE3_HEX_LENGTH
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
