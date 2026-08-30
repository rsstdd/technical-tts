//! One deserializer: a JSON object that binds no name twice.
//!
//! `serde`'s derived struct parsers already refuse a repeated field, so this
//! covers the shape they cannot reach — a map field, where the standard
//! [`BTreeMap`] deserializer keeps the last value and drops the earlier one
//! without a word. Both map fields this crate parses carry identities, and a
//! record whose meaning depends on which reader parsed it is not one.
//! `worker/study_tts_worker/protocol.py`'s `_distinct_keys` is the same rule at
//! the other end of the worker protocol, applied to every object it reads.
//!
//! The rule is proven where it is applied rather than here:
//! `t1_e1_a_response_naming_one_voice_profile_twice_is_refused` in
//! [`crate::worker_protocol`] and
//! `t1_e1_a_cache_record_naming_one_generation_parameter_twice_is_not_reused`
//! in [`crate::cache`] each fail if this function collects entries the way the
//! derived deserializer does.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{Error, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

/// Reads one JSON object into a [`BTreeMap`], refusing a name bound twice.
///
/// # Errors
///
/// Whatever `D` raises for input that is not a map or for an entry that does
/// not parse, and a custom refusal when one name appears more than once.
pub(crate) fn deserialize<'de, D, K, V>(deserializer: D) -> Result<BTreeMap<K, V>, D::Error>
where
    D: Deserializer<'de>,
    K: Deserialize<'de> + Ord,
    V: Deserialize<'de>,
{
    struct Distinct<K, V>(PhantomData<(K, V)>);

    impl<'de, K, V> Visitor<'de> for Distinct<K, V>
    where
        K: Deserialize<'de> + Ord,
        V: Deserialize<'de>,
    {
        type Value = BTreeMap<K, V>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an object binding each name once")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut entries: A) -> Result<Self::Value, A::Error> {
            let mut map = BTreeMap::new();
            while let Some((name, value)) = entries.next_entry::<K, V>()? {
                if map.insert(name, value).is_some() {
                    return Err(A::Error::custom("a JSON object names one field twice"));
                }
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(Distinct(PhantomData))
}
