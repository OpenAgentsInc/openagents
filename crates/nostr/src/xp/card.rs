//! NIP-XP trainer cards (`30194`): a trainer's signed summary of its own
//! level, the keys it sums, the trust list it read under, and the awards
//! behind it.
//!
//! A card is a claim, not a proof. Its signature shows who published it;
//! whether it's right is for a reader to re-derive from the relays, under
//! the card's own trust list and curve (`openagents xp verify-card`). The
//! level and the curve are the client's reading of the ledger, carried so a
//! reader can compare, never trusted.

use serde_json::{Map, Value, json};

use super::open;
use crate::contracts::{ContractError, RefusalCode};
use crate::domain::Event;
use crate::kb::{
    Unsigned, is_hex, malformed, mismatch, number, one_tag, reject, require, tag, text,
};

/// A trainer card: addressable, one per trainer at [`CARD_ADDRESS`].
pub const CARD_KIND: u16 = crate::kinds::XP_CARD;
/// The card's `d` tag.
pub const CARD_ADDRESS: &str = "trainer-card";
/// Keys a card may sum, at most: the trainer and 16 linked keys.
pub const MAX_CARD_KEYS: usize = 17;
/// Awards a card may list, at most.
pub const MAX_CARD_AWARDS: usize = 1_000;
/// Referees, runners, or relays a card may name, each, at most.
pub const MAX_CARD_LIST: usize = 32;

/// One counted award a card lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAward {
    /// The `3193` event ID.
    pub id: String,
    /// The awardee key the XP went to.
    pub pubkey: String,
    pub role: String,
    pub xp: u64,
    /// The quest version's address, `<id>@<version>`.
    pub quest: String,
}

/// A verified `30194`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrainerCard {
    /// The curve the level uses, such as `trainer-curve-v1`.
    pub curve: String,
    /// Where to read the events, in order.
    pub relays: Vec<String>,
    /// The referees whose awards the card counts.
    pub referees: Vec<String>,
    /// The runners the card trusts; empty for any.
    pub runners: Vec<String>,
    /// The keys whose XP the card sums, the signer first.
    pub keys: Vec<String>,
    pub xp: u64,
    pub level: u64,
    pub awards: Vec<CardAward>,
    pub issued_at: u64,
}

/// The parts of a `30194` for `card`, signed by `card.keys[0]`.
///
/// # Errors
///
/// When the card isn't one [`parse_card`] would accept.
pub fn card(card: &TrainerCard) -> Result<Unsigned, ContractError> {
    let body = json!({
        "v": 1, "requires": [], "type": "card",
        "curve": card.curve,
        "relays": card.relays,
        "trust": {"referees": card.referees, "runners": card.runners},
        "keys": card.keys,
        "xp": card.xp,
        "level": card.level,
        "awards": card.awards.iter().map(|a| json!({
            "id": a.id, "pubkey": a.pubkey, "role": a.role, "xp": a.xp, "quest": a.quest,
        })).collect::<Vec<_>>(),
        "issued_at": card.issued_at,
    });
    let object = body.as_object().ok_or_else(|| malformed("card"))?;
    let parsed = card_body(object)?;
    if &parsed != card {
        return Err(malformed("card"));
    }
    Ok(Unsigned {
        kind: CARD_KIND,
        tags: vec![tag(&["d", CARD_ADDRESS]), tag(&["t", "oa:xp:card:v1"])],
        content: body.to_string(),
    })
}

/// Checks a signed `30194` and returns the card it holds: its signature,
/// its body, and that its first key is its signer.
///
/// # Errors
///
/// A bad signature or body, another `d` tag, or a card whose first key
/// isn't its signer.
pub fn parse_card(event: &Event) -> Result<TrainerCard, ContractError> {
    let object = open(event, CARD_KIND, "card")?;
    if one_tag(event, "d")? != CARD_ADDRESS {
        return Err(mismatch("d tag"));
    }
    let card = card_body(&object)?;
    if card.keys.first() != Some(&event.pubkey) {
        return Err(mismatch("the card's first key isn't its signer"));
    }
    Ok(card)
}

fn card_body(object: &Map<String, Value>) -> Result<TrainerCard, ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "curve",
            "relays",
            "trust",
            "keys",
            "xp",
            "level",
            "awards",
            "issued_at",
        ],
    )?;
    let curve = text(object, "curve")?;
    if curve.is_empty() || curve.len() > 64 {
        return Err(malformed("curve"));
    }
    let relays = strings(object, "relays", MAX_CARD_LIST)?;
    if relays.is_empty()
        || relays
            .iter()
            .any(|r| !(r.starts_with("wss://") || r.starts_with("ws://")) || r.len() > 256)
    {
        return Err(malformed("relays"));
    }
    let trust = require(object, "trust")?
        .as_object()
        .ok_or_else(|| malformed("trust"))?;
    reject(trust, &["referees", "runners"])?;
    let referees = hex_list(trust, "referees", MAX_CARD_LIST)?;
    if referees.is_empty() {
        return Err(malformed("trust.referees"));
    }
    let runners = hex_list(trust, "runners", MAX_CARD_LIST)?;
    let keys = hex_list(object, "keys", MAX_CARD_KEYS)?;
    if keys.is_empty() {
        return Err(malformed("keys"));
    }
    let values = require(object, "awards")?
        .as_array()
        .ok_or_else(|| malformed("awards"))?;
    if values.len() > MAX_CARD_AWARDS {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "awards"));
    }
    let mut awards = Vec::new();
    for value in values {
        let item = value.as_object().ok_or_else(|| malformed("awards"))?;
        reject(item, &["id", "pubkey", "role", "xp", "quest"])?;
        let award = CardAward {
            id: text(item, "id")?,
            pubkey: text(item, "pubkey")?,
            role: text(item, "role")?,
            xp: number(item, "xp")?,
            quest: text(item, "quest")?,
        };
        if !is_hex(&award.id)
            || !keys.contains(&award.pubkey)
            || award.role.is_empty()
            || award.role.len() > 64
            || award.quest.is_empty()
            || award.quest.len() > 160
        {
            return Err(malformed("awards"));
        }
        awards.push(award);
    }
    Ok(TrainerCard {
        curve,
        relays,
        referees,
        runners,
        keys,
        xp: number(object, "xp")?,
        level: number(object, "level")?,
        awards,
        issued_at: number(object, "issued_at")?,
    })
}

fn strings(
    object: &Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Vec<String>, ContractError> {
    let items = require(object, key)?
        .as_array()
        .ok_or_else(|| malformed(key))?;
    if items.len() > max {
        return Err(ContractError::new(RefusalCode::LimitExceeded, key));
    }
    items
        .iter()
        .map(|v| v.as_str().map(str::to_owned).ok_or_else(|| malformed(key)))
        .collect()
}

fn hex_list(
    object: &Map<String, Value>,
    key: &str,
    max: usize,
) -> Result<Vec<String>, ContractError> {
    let list = strings(object, key, max)?;
    let mut seen = std::collections::BTreeSet::new();
    if list.iter().any(|k| !is_hex(k) || !seen.insert(k)) {
        return Err(malformed(key));
    }
    Ok(list)
}

#[cfg(test)]
mod tests;
