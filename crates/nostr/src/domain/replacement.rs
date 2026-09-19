use std::{cmp::Ordering, fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

/// NIP-01 storage classification for an event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventClass {
    Regular,
    Replaceable,
    Ephemeral,
    Addressable,
}

impl EventClass {
    pub const fn from_kind(kind: u16) -> Self {
        match kind {
            0 | 3 | 10_000..=19_999 => Self::Replaceable,
            20_000..=29_999 => Self::Ephemeral,
            30_000..=39_999 => Self::Addressable,
            _ => Self::Regular,
        }
    }
}

/// The coordinate on which replaceable versions compete.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplacementAddress {
    pub kind: u16,
    pub pubkey: String,
    /// Empty for ordinary replaceable events and for addressable events with
    /// an omitted or valueless `d` tag.
    pub identifier: String,
}

impl ReplacementAddress {
    pub fn from_event(event: &Event) -> Option<Self> {
        match event.class() {
            EventClass::Replaceable => Some(Self {
                kind: event.kind,
                pubkey: event.pubkey.clone(),
                identifier: String::new(),
            }),
            EventClass::Addressable => Some(Self {
                kind: event.kind,
                pubkey: event.pubkey.clone(),
                identifier: event.distinct_parameter().unwrap_or("").to_owned(),
            }),
            EventClass::Regular | EventClass::Ephemeral => None,
        }
    }
}

impl fmt::Display for ReplacementAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}", self.kind, self.pubkey, self.identifier)
    }
}

impl FromStr for ReplacementAddress {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.splitn(3, ':');
        let invalid = || DomainError::InvalidReplacementAddress(value.to_owned());
        let kind = parts
            .next()
            .ok_or_else(invalid)?
            .parse::<u16>()
            .map_err(|_| invalid())?;
        let pubkey = parts.next().ok_or_else(invalid)?;
        let identifier = parts.next().ok_or_else(invalid)?;
        decode_lower_hex::<32>(pubkey, "replacement pubkey").map_err(|_| invalid())?;

        match EventClass::from_kind(kind) {
            EventClass::Replaceable if identifier.is_empty() => {}
            EventClass::Addressable => {}
            _ => return Err(invalid()),
        }

        Ok(Self {
            kind,
            pubkey: pubkey.to_owned(),
            identifier: identifier.to_owned(),
        })
    }
}

/// Result of comparing a stored replacement head with a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementDecision {
    KeepCurrent,
    ReplaceCurrent,
    Duplicate,
}

/// Apply NIP-01's newest-timestamp, then lexically-lowest-ID rule.
pub fn compare_replacement(
    current: &Event,
    candidate: &Event,
) -> Result<ReplacementDecision, DomainError> {
    let current_address = current
        .replacement_address()
        .ok_or(DomainError::NotReplaceable)?;
    let candidate_address = candidate
        .replacement_address()
        .ok_or(DomainError::NotReplaceable)?;
    if current_address != candidate_address {
        return Err(DomainError::ReplacementAddressMismatch);
    }
    Ok(compare_replacement_order(
        current.created_at,
        &current.id,
        candidate.created_at,
        &candidate.id,
    ))
}

/// Compare replacement versions when the store already has just the head's
/// ordering columns. IDs are assumed to be validated lowercase NIP-01 IDs.
pub fn compare_replacement_order(
    current_created_at: u64,
    current_id: &str,
    candidate_created_at: u64,
    candidate_id: &str,
) -> ReplacementDecision {
    if current_id == candidate_id {
        return ReplacementDecision::Duplicate;
    }

    match candidate_created_at.cmp(&current_created_at) {
        Ordering::Greater => ReplacementDecision::ReplaceCurrent,
        Ordering::Less => ReplacementDecision::KeepCurrent,
        Ordering::Equal if candidate_id < current_id => ReplacementDecision::ReplaceCurrent,
        Ordering::Equal => ReplacementDecision::KeepCurrent,
    }
}
