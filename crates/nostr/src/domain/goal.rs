//! NIP-75 zap goals.
//!
//! Kind `9041` describes a fundraising goal. `amount` is the target in
//! millisats. `relays` lists where zaps for the goal are sent and
//! tallied. `closed_at` is the last second that still counts. `image`
//! and `summary` are optional. `r` and `a` tags point at a URL or an
//! addressable event. `zap` tags name beneficiaries.
//!
//! A zap request covers the goal when its `relays` tag includes every
//! goal relay. A zap at `closed_at` counts. A later zap does not. The
//! relay does not send or tally Lightning payments. Kind `9041` is a
//! regular event, so a newer goal does not replace an older one.
//! NIP-75 is a draft, so this kind stays off the NIP-11 list.

use std::str::FromStr;

use super::zap::{ZapShare, zap_split};
use super::{DomainError, Event, ReplacementAddress};

const GOAL_KIND: u16 = 9_041;

/// A URL or addressable event linked from a goal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GoalSubject {
    Url(String),
    Address(ReplacementAddress),
}

/// A kind `9041` fundraising goal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZapGoal {
    pub content: String,
    pub amount_msat: u64,
    pub relays: Vec<String>,
    pub closed_at: Option<u64>,
    pub image: Option<String>,
    pub summary: Option<String>,
    pub subjects: Vec<GoalSubject>,
    pub beneficiaries: Vec<ZapShare>,
}

/// A `goal` tag on another event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalReference {
    pub event_id: String,
    pub relay: Option<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn whole(value: &str, reason: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid(reason));
    }
    value.parse().map_err(|_| invalid(reason))
}

fn http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

fn relay_url(value: &str) -> bool {
    (value.starts_with("wss://") || value.starts_with("ws://"))
        && value.len() > "wss://".len()
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn one<'a>(event: &'a Event, name: &str) -> Result<Option<&'a super::Tag>, DomainError> {
    let mut found = event.tags.iter().filter(|tag| tag.name() == Some(name));
    let tag = found.next();
    if found.next().is_some() {
        return Err(invalid("a goal repeats an optional tag"));
    }
    Ok(tag)
}

fn relays(event: &Event) -> Result<Vec<String>, DomainError> {
    let tags: Vec<_> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("relays"))
        .collect();
    if tags.len() != 1 {
        return Err(invalid("a goal lists relays"));
    }
    let mut relays = Vec::new();
    for value in tags[0].as_slice().iter().skip(1) {
        if !relay_url(value) || relays.iter().any(|seen| seen == value) {
            return Err(invalid("a goal relay is ws:// or wss://"));
        }
        relays.push(value.clone());
    }
    if relays.is_empty() {
        return Err(invalid("a goal lists relays"));
    }
    Ok(relays)
}

fn subjects(event: &Event) -> Result<Vec<GoalSubject>, DomainError> {
    let mut subjects = Vec::new();
    for tag in &event.tags {
        match tag.name() {
            Some("r") => {
                let Some(value) = tag.value().filter(|value| http_url(value)) else {
                    return Err(invalid("a goal link is an http:// or https:// URL"));
                };
                subjects.push(GoalSubject::Url(value.to_owned()));
            }
            Some("a") => {
                let Some(value) = tag.value() else {
                    return Err(invalid("a goal link is an addressable event"));
                };
                let address = ReplacementAddress::from_str(value)
                    .map_err(|_| invalid("a goal link is an addressable event"))?;
                if let Some(relay) = tag.as_slice().get(2)
                    && !relay_url(relay)
                {
                    return Err(invalid("a goal address hint is ws:// or wss://"));
                }
                subjects.push(GoalSubject::Address(address));
            }
            _ => {}
        }
    }
    Ok(subjects)
}

/// Read a kind `9041` zap goal.
pub fn open_zap_goal(event: &Event) -> Result<ZapGoal, DomainError> {
    if event.kind != GOAL_KIND {
        return Err(invalid("a zap goal has kind 9041"));
    }
    if event.content.is_empty() {
        return Err(invalid("a zap goal describes itself"));
    }
    let amount_msat = whole(
        one(event, "amount")?
            .and_then(super::Tag::value)
            .ok_or_else(|| invalid("a goal amount is a number of millisats"))?,
        "a goal amount is a number of millisats",
    )?;
    let closed_at = match one(event, "closed_at")? {
        None => None,
        Some(tag) => Some(whole(
            tag.value()
                .ok_or_else(|| invalid("a goal closed_at is a unix timestamp"))?,
            "a goal closed_at is a unix timestamp",
        )?),
    };
    let image = match one(event, "image")? {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value().filter(|value| http_url(value)) else {
                return Err(invalid("a goal image is an http:// or https:// URL"));
            };
            Some(value.to_owned())
        }
    };
    let summary = match one(event, "summary")? {
        None => None,
        Some(tag) => {
            let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                return Err(invalid("a goal summary is non-empty"));
            };
            Some(value.to_owned())
        }
    };
    Ok(ZapGoal {
        content: event.content.clone(),
        amount_msat,
        relays: relays(event)?,
        closed_at,
        image,
        summary,
        subjects: subjects(event)?,
        beneficiaries: zap_split(event, amount_msat)?,
    })
}

/// Read `goal` tags. Each value is a 32-byte event id and an optional relay.
pub fn open_goal_references(event: &Event) -> Result<Vec<GoalReference>, DomainError> {
    let mut references = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("goal")) {
        let parts = tag.as_slice();
        if parts.len() < 2 || parts.len() > 3 {
            return Err(invalid("a goal reference is an event id"));
        }
        super::hex::decode_lower_hex::<32>(&parts[1], "goal event id")
            .map_err(|_| invalid("a goal reference is an event id"))?;
        let relay = match parts.get(2) {
            None => None,
            Some(value) if relay_url(value) => Some(value.clone()),
            Some(_) => return Err(invalid("a goal reference relay is ws:// or wss://")),
        };
        references.push(GoalReference {
            event_id: parts[1].clone(),
            relay,
        });
    }
    Ok(references)
}

/// A zap at `closed_at` counts. A later zap does not.
pub fn zap_counts_toward_goal(goal: &ZapGoal, created_at: u64) -> bool {
    match goal.closed_at {
        Some(closed_at) => created_at <= closed_at,
        None => true,
    }
}

/// The request lists every relay the goal named.
pub fn zap_request_covers_goal(goal: &ZapGoal, request_relays: &[String]) -> bool {
    goal.relays
        .iter()
        .all(|relay| request_relays.iter().any(|candidate| candidate == relay))
}

/// Sum the amounts whose timestamps still fall inside the goal.
pub fn goal_progress(goal: &ZapGoal, zaps: &[(u64, u64)]) -> u64 {
    zaps.iter().fold(0, |total, (created_at, amount)| {
        if zap_counts_toward_goal(goal, *created_at) {
            total.saturating_add(*amount)
        } else {
            total
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, Tag, compare_replacement, open_zap_request,
    };

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_zap_goal_tallies_until_it_closes_and_a_request_lists_its_relays() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/75.md"
        ))
        .unwrap();
        assert!(text.contains("9041"));
        assert!(text.contains("amount"));
        assert!(text.contains("closed_at"));
        assert!(text.contains("relays"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "75.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "75.md")
        );

        let author = signer("75");
        let alice = signer("76");
        let bob = signer("77");
        let alice_relay = "wss://alicerelay.example.com";
        let bob_relay = "wss://bobrelay.example.com";
        let goal = author.sign(
            1_700_000_000,
            GOAL_KIND,
            vec![
                Tag::new(vec!["relays".into(), alice_relay.into(), bob_relay.into()]),
                Tag::new(vec!["amount".into(), "210000".into()]),
                Tag::new(vec!["closed_at".into(), "1700000500".into()]),
                Tag::new(vec!["image".into(), "https://example.com/goal.png".into()]),
                Tag::new(vec!["summary".into(), "Travel".into()]),
                Tag::new(vec!["r".into(), "https://example.com/nostrasia".into()]),
                Tag::new(vec!["a".into(), format!("30023:{}:trip", author.pubkey())]),
                Tag::new(vec![
                    "zap".into(),
                    alice.pubkey().to_owned(),
                    alice_relay.into(),
                    "1".into(),
                ]),
                Tag::new(vec![
                    "zap".into(),
                    bob.pubkey().to_owned(),
                    bob_relay.into(),
                    "1".into(),
                ]),
            ],
            "Nostrasia travel expenses".into(),
        );
        goal.validate_structure().unwrap();
        assert_eq!(goal.class(), EventClass::Regular);
        let opened = open_zap_goal(&goal).unwrap();
        assert_eq!(opened.content, "Nostrasia travel expenses");
        assert_eq!(opened.amount_msat, 210_000);
        assert_eq!(opened.relays, vec![alice_relay, bob_relay]);
        assert_eq!(opened.closed_at, Some(1_700_000_500));
        assert_eq!(
            opened.image.as_deref(),
            Some("https://example.com/goal.png")
        );
        assert_eq!(
            opened
                .beneficiaries
                .iter()
                .map(|share| share.millisats)
                .sum::<u64>(),
            210_000
        );
        assert!(matches!(
            opened.subjects.first(),
            Some(GoalSubject::Url(url)) if url == "https://example.com/nostrasia"
        ));

        let again = author.sign(
            1_700_000_100,
            GOAL_KIND,
            vec![
                Tag::new(vec!["relays".into(), alice_relay.into()]),
                Tag::new(vec!["amount".into(), "1000".into()]),
            ],
            "Another trip".into(),
        );
        again.validate_structure().unwrap();
        assert!(matches!(
            compare_replacement(&goal, &again),
            Err(DomainError::NotReplaceable)
        ));

        let sender = signer("78");
        let early = sender.sign(
            1_700_000_400,
            9_734,
            vec![
                Tag::new(vec!["relays".into(), alice_relay.into(), bob_relay.into()]),
                Tag::new(vec!["amount".into(), "1000".into()]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), goal.id.clone()]),
            ],
            "For the trip".into(),
        );
        early.validate_structure().unwrap();
        let early_request = open_zap_request(&early).unwrap();
        assert!(zap_request_covers_goal(&opened, &early_request.relays));
        let late = sender.sign(
            1_700_000_600,
            9_734,
            vec![
                Tag::new(vec!["relays".into(), alice_relay.into()]),
                Tag::new(vec!["amount".into(), "5000".into()]),
                Tag::new(vec!["p".into(), author.pubkey().to_owned()]),
                Tag::new(vec!["e".into(), goal.id.clone()]),
            ],
            String::new(),
        );
        late.validate_structure().unwrap();
        let late_request = open_zap_request(&late).unwrap();
        assert!(!zap_request_covers_goal(&opened, &late_request.relays));
        assert!(zap_counts_toward_goal(&opened, early.created_at));
        assert!(!zap_counts_toward_goal(&opened, late.created_at));
        assert_eq!(
            goal_progress(
                &opened,
                &[
                    (early.created_at, early_request.amount_msat.unwrap()),
                    (late.created_at, late_request.amount_msat.unwrap()),
                ]
            ),
            1_000
        );

        let linked = author.sign(
            1_700_000_700,
            30_078,
            vec![
                Tag::new(vec!["d".into(), "trip".into()]),
                Tag::new(vec!["goal".into(), goal.id.clone(), alice_relay.into()]),
            ],
            String::new(),
        );
        linked.validate_structure().unwrap();
        let references = open_goal_references(&linked).unwrap();
        assert_eq!(references[0].event_id, goal.id);
        assert_eq!(references[0].relay.as_deref(), Some(alice_relay));

        let missing = author.sign(
            1_700_000_800,
            GOAL_KIND,
            vec![Tag::new(vec!["amount".into(), "210000".into()])],
            "No relays".into(),
        );
        assert!(missing.validate_structure().is_err());
    }
}
