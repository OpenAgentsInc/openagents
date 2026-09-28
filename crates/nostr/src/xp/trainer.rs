//! NIP-XP trainer profiles (`13193`): a key's opt-in to being shown as a
//! trainer, with the other keys it claims as its own.
//!
//! XP is public by construction: anyone can derive any key's ledger. A
//! profile doesn't change that. It is what a client reads before it
//! advertises a key: name-tag levels and rank boards show only keys whose
//! newest profile says `shown: true`. The profile's `keys` are the
//! trainer's other keys; a listed key counts toward the trainer only when
//! it signs a matching link back, so no one can claim a stranger's key.

use serde_json::{Map, Value, json};

use super::open;
use crate::contracts::ContractError;
use crate::domain::Event;
use crate::kb::{Unsigned, is_hex, malformed, mismatch, reject, require, tag};

/// A trainer profile: replaceable, one per key.
pub const PROFILE_KIND: u16 = crate::kinds::XP_PROFILE;
/// Other keys a profile may list, at most.
pub const MAX_KEYS: usize = 16;

/// A verified `13193`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerProfile {
    /// Whether the trainer asks clients to show its level on name tags and
    /// boards.
    pub shown: bool,
    /// The trainer's other keys, hex, distinct, never the signer, in the
    /// order listed.
    pub keys: Vec<String>,
}

/// The parts of a `13193` signed by `trainer` (hex), which lists `keys`.
///
/// # Errors
///
/// A key that isn't 64 lowercase hex characters, the trainer's own key, a
/// repeated key, or more than [`MAX_KEYS`].
pub fn profile(trainer: &str, shown: bool, keys: &[String]) -> Result<Unsigned, ContractError> {
    check_keys(trainer, keys)?;
    let mut tags = vec![tag(&["t", "oa:xp:profile:v1"])];
    tags.extend(keys.iter().map(|key| tag(&["p", key])));
    Ok(Unsigned {
        kind: PROFILE_KIND,
        tags,
        content: json!({
            "v": 1, "requires": [], "type": "profile",
            "shown": shown,
            "keys": keys,
        })
        .to_string(),
    })
}

/// Checks a signed `13193` and returns the profile it holds.
///
/// # Errors
///
/// A bad signature or body, or `p` tags that disagree with `keys`.
pub fn parse_profile(event: &Event) -> Result<TrainerProfile, ContractError> {
    let object = open(event, PROFILE_KIND, "profile")?;
    reject(&object, &["v", "requires", "type", "shown", "keys"])?;
    let shown = require(&object, "shown")?
        .as_bool()
        .ok_or_else(|| malformed("shown"))?;
    let keys = strings(&object, "keys")?;
    check_keys(&event.pubkey, &keys)?;
    let tagged: Vec<&str> = event.tag_values("p").collect();
    if tagged != keys.iter().map(String::as_str).collect::<Vec<_>>() {
        return Err(mismatch("p tags"));
    }
    Ok(TrainerProfile { shown, keys })
}

fn strings(object: &Map<String, Value>, key: &str) -> Result<Vec<String>, ContractError> {
    require(object, key)?
        .as_array()
        .ok_or_else(|| malformed(key))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| malformed(key))
        })
        .collect()
}

fn check_keys(trainer: &str, keys: &[String]) -> Result<(), ContractError> {
    if keys.len() > MAX_KEYS {
        return Err(malformed(format!("keys: at most {MAX_KEYS}")));
    }
    let mut seen = std::collections::BTreeSet::new();
    for key in keys {
        if !is_hex(key) || key.len() != 64 {
            return Err(malformed("keys"));
        }
        if key == trainer {
            return Err(malformed("keys: a profile doesn't list its own key"));
        }
        if !seen.insert(key) {
            return Err(malformed("keys: a key is listed twice"));
        }
    }
    Ok(())
}

/// The newest of `events` by one author and kind, as NIP-01 orders
/// replaceable events: the latest `created_at`, then the lowest ID.
#[must_use]
pub fn newest<'a>(events: impl IntoIterator<Item = &'a Event>) -> Option<&'a Event> {
    events.into_iter().max_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| b.id.cmp(&a.id))
    })
}

#[cfg(test)]
mod tests;
