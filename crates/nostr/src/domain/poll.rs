//! NIP-88 polls.
//!
//! Kind `1068` is a poll. `content` is the label. Each `option` tag is an
//! alphanumeric id and a label. `polltype` is `singlechoice` or
//! `multiplechoice`; a missing type is single choice. `endsAt` is the
//! unix second when voting stops. Kind `1018` is one response.
//!
//! `tally` keeps one vote per pubkey: the latest `created_at` that is
//! not after `endsAt`. A tie keeps the greater event id. A single-choice
//! vote counts its first response tag. A multiple-choice vote counts
//! the first tag for each option id. The relay does not fetch the
//! poll's relays, does not ignore kind `5` deletions of votes, and does
//! not apply a follow set. These kinds are not added to the NIP-11 list.

use std::collections::BTreeMap;

use super::hex::decode_lower_hex;
use super::{DomainError, Event};

const POLL_KIND: u16 = 1_068;
const RESPONSE_KIND: u16 = 1_018;

/// How more than one response tag is counted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollType {
    SingleChoice,
    MultipleChoice,
}

/// One poll option.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollOption {
    pub id: String,
    pub label: String,
}

/// A kind `1068` poll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Poll {
    pub id: String,
    pub label: String,
    pub options: Vec<PollOption>,
    pub relays: Vec<String>,
    pub poll_type: PollType,
    pub ends_at: Option<u64>,
}

/// A kind `1018` response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PollResponse {
    pub id: String,
    pub voter: String,
    pub created_at: u64,
    pub poll_id: String,
    pub choices: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn option_id(value: &str) -> bool {
    (1..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn unix_seconds(value: &str) -> Result<u64, DomainError> {
    if value.is_empty()
        || value.len() > 20
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(invalid("a poll end is unix seconds"));
    }
    value
        .parse()
        .map_err(|_| invalid("a poll end is unix seconds"))
}

fn tags<'a>(event: &'a Event, name: &str) -> Vec<&'a super::Tag> {
    event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .collect()
}

/// Read a kind `1068` poll.
///
/// # Errors
///
/// Returns a sentence when an option, relay, type, or end time is refused.
pub fn open_poll(event: &Event) -> Result<Poll, DomainError> {
    if event.kind != POLL_KIND {
        return Err(invalid("a poll has kind 1068"));
    }
    let mut options = Vec::new();
    for tag in tags(event, "option") {
        let Some(id) = tag.value().filter(|value| option_id(value)) else {
            return Err(invalid("a poll option id is alphanumeric"));
        };
        let Some(label) = tag
            .as_slice()
            .get(2)
            .filter(|label| !label.is_empty() && label.len() <= 1_024)
        else {
            return Err(invalid("a poll option has a label"));
        };
        if options.iter().any(|option: &PollOption| option.id == id) {
            return Err(invalid("a poll option id is unique"));
        }
        options.push(PollOption {
            id: id.to_owned(),
            label: label.to_owned(),
        });
    }
    if options.is_empty() {
        return Err(invalid("a poll has one option"));
    }
    let mut relays = Vec::new();
    for tag in tags(event, "relay") {
        let Some(value) = tag.value().filter(|value| is_relay(value)) else {
            return Err(invalid("a poll relay is ws:// or wss://"));
        };
        relays.push(value.to_owned());
    }
    if relays.is_empty() {
        return Err(invalid("a poll relay is ws:// or wss://"));
    }
    let poll_type = match tags(event, "polltype").as_slice() {
        [] => PollType::SingleChoice,
        [tag] => match tag.value() {
            Some("singlechoice") => PollType::SingleChoice,
            Some("multiplechoice") => PollType::MultipleChoice,
            _ => return Err(invalid("a poll type is singlechoice or multiplechoice")),
        },
        _ => return Err(invalid("a poll type is singlechoice or multiplechoice")),
    };
    let ends_at = match tags(event, "endsAt").as_slice() {
        [] => None,
        [tag] => {
            let Some(value) = tag.value() else {
                return Err(invalid("a poll end is unix seconds"));
            };
            Some(unix_seconds(value)?)
        }
        _ => return Err(invalid("a poll end is unix seconds")),
    };
    Ok(Poll {
        id: event.id.clone(),
        label: event.content.clone(),
        options,
        relays,
        poll_type,
        ends_at,
    })
}

/// Read a kind `1018` poll response.
///
/// # Errors
///
/// Returns a sentence when the poll id or a response id is refused.
pub fn open_poll_response(event: &Event) -> Result<PollResponse, DomainError> {
    if event.kind != RESPONSE_KIND {
        return Err(invalid("a poll response has kind 1018"));
    }
    let events = tags(event, "e");
    if events.len() != 1 {
        return Err(invalid("a poll response names one poll"));
    }
    let Some(poll_id) = events[0].value() else {
        return Err(invalid("a poll response names one poll"));
    };
    decode_lower_hex::<32>(poll_id, "poll event")
        .map_err(|_| invalid("a poll response names one poll"))?;
    if let Some(relay) = events[0].as_slice().get(2)
        && !relay.is_empty()
        && !is_relay(relay)
    {
        return Err(invalid("a poll relay is ws:// or wss://"));
    }
    let mut choices = Vec::new();
    for tag in tags(event, "response") {
        let Some(value) = tag.value().filter(|value| option_id(value)) else {
            return Err(invalid("a poll response id is alphanumeric"));
        };
        choices.push(value.to_owned());
    }
    if choices.is_empty() {
        return Err(invalid("a poll response selects an option"));
    }
    Ok(PollResponse {
        id: event.id.clone(),
        voter: event.pubkey.clone(),
        created_at: event.created_at,
        poll_id: poll_id.to_owned(),
        choices,
    })
}

fn chosen(poll: &Poll, response: &PollResponse) -> Vec<String> {
    let known = |id: &str| poll.options.iter().any(|option| option.id == id);
    match poll.poll_type {
        PollType::SingleChoice => response
            .choices
            .iter()
            .find(|choice| known(choice))
            .cloned()
            .into_iter()
            .collect(),
        PollType::MultipleChoice => {
            let mut seen = Vec::new();
            for choice in &response.choices {
                if known(choice) && !seen.iter().any(|kept: &String| kept == choice) {
                    seen.push(choice.clone());
                }
            }
            seen
        }
    }
}

/// Count one vote per pubkey.
///
/// A response after `endsAt` is ignored. The latest remaining response
/// wins. An equal timestamp keeps the greater event id. The returned
/// counts follow the poll's option order.
#[must_use]
pub fn tally(poll: &Poll, responses: &[PollResponse]) -> Vec<u64> {
    let mut best: BTreeMap<&str, &PollResponse> = BTreeMap::new();
    for response in responses {
        if response.poll_id != poll.id {
            continue;
        }
        if poll.ends_at.is_some_and(|end| response.created_at > end) {
            continue;
        }
        match best.get(response.voter.as_str()) {
            Some(current)
                if current.created_at > response.created_at
                    || (current.created_at == response.created_at && current.id >= response.id) => {
            }
            _ => {
                best.insert(response.voter.as_str(), response);
            }
        }
    }
    let mut counts = vec![0_u64; poll.options.len()];
    for response in best.values() {
        for choice in chosen(poll, response) {
            if let Some(index) = poll.options.iter().position(|option| option.id == choice) {
                counts[index] += 1;
            }
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    fn signer(byte: &str) -> RelaySigner {
        RelaySigner::from_secret_hex(&byte.repeat(32)).unwrap()
    }

    #[test]
    fn a_poll_counts_one_vote_per_pubkey_inside_its_window() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/88.md"
        ))
        .unwrap();
        assert!(text.contains("kind:1068"));
        assert!(text.contains("kind:1018"));
        assert!(text.contains("singlechoice"));
        assert!(text.contains("multiplechoice"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "88.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "88.md")
        );

        let author = signer("88");
        let poll_event = author.sign(
            1_719_888_496,
            POLL_KIND,
            vec![
                Tag::new(vec!["option".into(), "qj518h583".into(), "Yay".into()]),
                Tag::new(vec!["option".into(), "gga6cdnqj".into(), "Nay".into()]),
                Tag::new(vec!["relay".into(), "wss://relay.example".into()]),
                Tag::new(vec!["relay".into(), "wss://other.example".into()]),
                Tag::new(vec!["polltype".into(), "singlechoice".into()]),
                Tag::new(vec!["endsAt".into(), "1720100000".into()]),
            ],
            "Pineapple on pizza".into(),
        );
        poll_event.validate_structure().unwrap();
        assert_eq!(poll_event.class(), EventClass::Regular);
        let poll = open_poll(&poll_event).unwrap();
        assert_eq!(poll.label, "Pineapple on pizza");
        assert_eq!(poll.poll_type, PollType::SingleChoice);
        assert_eq!(poll.ends_at, Some(1_720_100_000));
        assert_eq!(poll.options.len(), 2);
        let again = author.sign(
            1_719_888_500,
            POLL_KIND,
            vec![
                Tag::new(vec!["option".into(), "qj518h583".into(), "Yay".into()]),
                Tag::new(vec!["relay".into(), "wss://relay.example".into()]),
            ],
            "Again".into(),
        );
        assert!(matches!(
            compare_replacement(&poll_event, &again),
            Err(DomainError::NotReplaceable)
        ));

        let alice = signer("a8");
        let bob = signer("b8");
        let first = alice.sign(
            1_720_097_000,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["e".into(), poll_event.id.clone()]),
                Tag::new(vec!["response".into(), "gga6cdnqj".into()]),
                Tag::new(vec!["response".into(), "qj518h583".into()]),
            ],
            String::new(),
        );
        first.validate_structure().unwrap();
        let later = alice.sign(
            1_720_098_000,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["e".into(), poll_event.id.clone()]),
                Tag::new(vec!["response".into(), "qj518h583".into()]),
            ],
            String::new(),
        );
        let late = alice.sign(
            1_720_100_001,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["e".into(), poll_event.id.clone()]),
                Tag::new(vec!["response".into(), "gga6cdnqj".into()]),
            ],
            String::new(),
        );
        let bob_vote = bob.sign(
            1_720_097_500,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["e".into(), poll_event.id.clone()]),
                Tag::new(vec!["response".into(), "gga6cdnqj".into()]),
            ],
            String::new(),
        );
        let votes = [first, later, late, bob_vote]
            .into_iter()
            .map(|event| open_poll_response(&event).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(tally(&poll, &votes), vec![1, 1]);

        let multi_event = author.sign(
            1_719_888_600,
            POLL_KIND,
            vec![
                Tag::new(vec!["option".into(), "qj518h583".into(), "Yay".into()]),
                Tag::new(vec!["option".into(), "gga6cdnqj".into(), "Nay".into()]),
                Tag::new(vec!["relay".into(), "wss://relay.example".into()]),
                Tag::new(vec!["polltype".into(), "multiplechoice".into()]),
            ],
            "Both".into(),
        );
        let multi = open_poll(&multi_event).unwrap();
        assert_eq!(multi.poll_type, PollType::MultipleChoice);
        let both = alice.sign(
            1_720_097_100,
            RESPONSE_KIND,
            vec![
                Tag::new(vec!["e".into(), multi_event.id.clone()]),
                Tag::new(vec!["response".into(), "qj518h583".into()]),
                Tag::new(vec!["response".into(), "gga6cdnqj".into()]),
                Tag::new(vec!["response".into(), "qj518h583".into()]),
            ],
            String::new(),
        );
        both.validate_structure().unwrap();
        assert_eq!(
            tally(&multi, &[open_poll_response(&both).unwrap()]),
            vec![1, 1]
        );
        let bare = author.sign(
            1_719_888_700,
            POLL_KIND,
            vec![Tag::new(vec!["relay".into(), "wss://relay.example".into()])],
            "No options".into(),
        );
        assert!(bare.validate_structure().is_err());
    }
}
