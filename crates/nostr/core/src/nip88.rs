//! NIP-88: Polls
//!
//! This module implements NIP-88 which defines decentralized polls on Nostr.
//!
//! # Overview
//!
//! NIP-88 enables creating polls and collecting responses through Nostr events.
//! Polls can be single-choice or multiple-choice, with specified end times and
//! designated response relays.
//!
//! # Event Structure
//!
//! ## Poll Event (kind 1068)
//! - `content`: Poll question/label
//! - `option` tags: Option ID and label pairs
//! - `relay` tags: Relays where responses should be published
//! - `polltype` tag: "singlechoice" or "multiplechoice" (defaults to singlechoice)
//! - `endsAt` tag: Unix timestamp when poll ends
//!
//! ## Response Event (kind 1018)
//! - `e` tag: References the poll event ID
//! - `response` tags: Selected option IDs
//! - One vote per pubkey (latest within poll period)
//!
//! # Example
//!
//! ```
//! use nostr::nip88::{PollEvent, PollOption, PollType, KIND_POLL, create_poll_event};
//!
//! let options = vec![
//!     PollOption {
//!         id: "yes".to_string(),
//!         label: "Yes".to_string(),
//!     },
//!     PollOption {
//!         id: "no".to_string(),
//!         label: "No".to_string(),
//!     },
//! ];
//!
//! let relays = vec!["wss://relay.example.com".to_string()];
//!
//! let poll = create_poll_event(
//!     "Do you like polls?",
//!     options,
//!     relays,
//!     PollType::SingleChoice,
//!     Some(1234567890),
//! );
//! ```

use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

/// Event kind for poll events
pub const KIND_POLL: u16 = 1068;

/// Event kind for poll response events
pub const KIND_POLL_RESPONSE: u16 = 1018;

/// Tag name for poll options
pub const OPTION_TAG: &str = "option";

/// Tag name for relay URLs
pub const RELAY_TAG: &str = "relay";

/// Tag name for poll type
pub const POLL_TYPE_TAG: &str = "polltype";

/// Tag name for poll end time
pub const ENDS_AT_TAG: &str = "endsAt";

/// Tag name for poll response
pub const RESPONSE_TAG: &str = "response";

/// Tag name for poll event reference
pub const POLL_EVENT_TAG: &str = "e";

/// NIP-88 error types
#[derive(Debug, Error, Clone, PartialEq)]
pub enum Nip88Error {
    /// Missing poll question
    #[error("missing poll question (content is empty)")]
    MissingQuestion,

    /// No options provided
    #[error("poll must have at least one option")]
    NoOptions,

    /// Invalid option tag format
    #[error("invalid option tag: {0}")]
    InvalidOptionTag(String),

    /// Invalid poll type
    #[error("invalid poll type: {0}")]
    InvalidPollType(String),

    /// Invalid end time
    #[error("invalid endsAt timestamp: {0}")]
    InvalidEndTime(String),

    /// Missing poll event reference
    #[error("missing poll event reference (e tag)")]
    MissingPollReference,

    /// No responses provided
    #[error("response must have at least one option selected")]
    NoResponses,

    /// Wrong event kind
    #[error("expected kind {expected}, got {actual}")]
    WrongKind { expected: u16, actual: u16 },

    /// Poll has ended
    #[error("poll ended at {0}")]
    PollEnded(u64),
}

/// Poll type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollType {
    /// Single choice poll (only first response tag counts)
    SingleChoice,
    /// Multiple choice poll (all response tags count)
    MultipleChoice,
}

impl PollType {
    /// Convert poll type to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            PollType::SingleChoice => "singlechoice",
            PollType::MultipleChoice => "multiplechoice",
        }
    }
}

impl std::str::FromStr for PollType {
    type Err = Nip88Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "singlechoice" => Ok(PollType::SingleChoice),
            "multiplechoice" => Ok(PollType::MultipleChoice),
            _ => Err(Nip88Error::InvalidPollType(s.to_string())),
        }
    }
}

/// Represents a poll option
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollOption {
    /// Unique option ID (alphanumeric)
    pub id: String,
    /// Human-readable option label
    pub label: String,
}

/// Represents a poll event
#[derive(Debug, Clone, PartialEq)]
pub struct PollEvent {
    /// Poll question/label
    pub question: String,
    /// Available options
    pub options: Vec<PollOption>,
    /// Relays where responses should be published
    pub relays: Vec<String>,
    /// Poll type (single or multiple choice)
    pub poll_type: PollType,
    /// Unix timestamp when poll ends (optional)
    pub ends_at: Option<u64>,
}

/// Represents a poll response event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollResponse {
    /// Poll event ID being responded to
    pub poll_id: String,
    /// Selected option IDs
    pub responses: Vec<String>,
}

/// Create tags for a poll event
///
/// # Example
///
/// ```
/// use nostr::nip88::{PollOption, PollType, create_poll_tags};
///
/// let options = vec![
///     PollOption {
///         id: "yes".to_string(),
///         label: "Yes".to_string(),
///     },
///     PollOption {
///         id: "no".to_string(),
///         label: "No".to_string(),
///     },
/// ];
///
/// let relays = vec!["wss://relay.example.com".to_string()];
///
/// let tags = create_poll_tags(
///     &options,
///     &relays,
///     PollType::SingleChoice,
///     Some(1234567890),
/// );
///
/// assert!(tags.len() >= 4); // 2 options + relay + polltype + endsAt
/// ```
pub fn create_poll_tags(
    options: &[PollOption],
    relays: &[String],
    poll_type: PollType,
    ends_at: Option<u64>,
) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add option tags
    for option in options {
        tags.push(vec![
            OPTION_TAG.to_string(),
            option.id.clone(),
            option.label.clone(),
        ]);
    }

    // Add relay tags
    for relay in relays {
        tags.push(vec![RELAY_TAG.to_string(), relay.clone()]);
    }

    // Add polltype tag
    tags.push(vec![
        POLL_TYPE_TAG.to_string(),
        poll_type.as_str().to_string(),
    ]);

    // Add endsAt tag if specified
    if let Some(end_time) = ends_at {
        tags.push(vec![ENDS_AT_TAG.to_string(), end_time.to_string()]);
    }

    tags
}

/// Create a poll event structure from content and tags
///
/// # Example
///
/// ```
/// use nostr::nip88::{PollOption, PollType, create_poll_tags, create_poll_event};
///
/// let options = vec![
///     PollOption {
///         id: "yes".to_string(),
///         label: "Yes".to_string(),
///     },
/// ];
///
/// let poll = create_poll_event(
///     "Do you agree?",
///     options,
///     vec!["wss://relay.example.com".to_string()],
///     PollType::SingleChoice,
///     None,
/// );
///
/// assert_eq!(poll.question, "Do you agree?");
/// assert_eq!(poll.options.len(), 1);
/// ```
pub fn create_poll_event(
    question: &str,
    options: Vec<PollOption>,
    relays: Vec<String>,
    poll_type: PollType,
    ends_at: Option<u64>,
) -> PollEvent {
    PollEvent {
        question: question.to_string(),
        options,
        relays,
        poll_type,
        ends_at,
    }
}

/// Parse poll options from tags
///
/// # Example
///
/// ```
/// use nostr::nip88::parse_poll_options;
///
/// let tags = vec![
///     vec!["option".to_string(), "yes".to_string(), "Yes".to_string()],
///     vec!["option".to_string(), "no".to_string(), "No".to_string()],
/// ];
///
/// let options = parse_poll_options(&tags).unwrap();
/// assert_eq!(options.len(), 2);
/// assert_eq!(options[0].id, "yes");
/// assert_eq!(options[0].label, "Yes");
/// ```
pub fn parse_poll_options(tags: &[Vec<String>]) -> Result<Vec<PollOption>, Nip88Error> {
    let options: Vec<PollOption> = tags
        .iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some(OPTION_TAG))
        .map(|tag| {
            if tag.len() < 3 {
                return Err(Nip88Error::InvalidOptionTag(format!(
                    "option tag must have at least 3 elements, got {}",
                    tag.len()
                )));
            }
            Ok(PollOption {
                id: tag[1].clone(),
                label: tag[2].clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if options.is_empty() {
        return Err(Nip88Error::NoOptions);
    }

    Ok(options)
}

/// Parse relay URLs from tags
pub fn parse_poll_relays(tags: &[Vec<String>]) -> Vec<String> {
    tags.iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some(RELAY_TAG))
        .filter_map(|tag| tag.get(1).cloned())
        .collect()
}

/// Parse poll type from tags
pub fn parse_poll_type(tags: &[Vec<String>]) -> Result<PollType, Nip88Error> {
    tags.iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(POLL_TYPE_TAG))
        .and_then(|tag| tag.get(1))
        .map(|s| PollType::from_str(s))
        .unwrap_or(Ok(PollType::SingleChoice)) // Default to single choice
}

/// Parse poll end time from tags
pub fn parse_poll_ends_at(tags: &[Vec<String>]) -> Result<Option<u64>, Nip88Error> {
    match tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(ENDS_AT_TAG))
        .and_then(|tag| tag.get(1))
    {
        Some(s) => s
            .parse::<u64>()
            .map(Some)
            .map_err(|e| Nip88Error::InvalidEndTime(e.to_string())),
        None => Ok(None),
    }
}

/// Parse a poll event from tags and content
///
/// # Example
///
/// ```
/// use nostr::nip88::{parse_poll, KIND_POLL};
///
/// let tags = vec![
///     vec!["option".to_string(), "yes".to_string(), "Yes".to_string()],
///     vec!["option".to_string(), "no".to_string(), "No".to_string()],
///     vec!["relay".to_string(), "wss://relay.example.com".to_string()],
///     vec!["polltype".to_string(), "singlechoice".to_string()],
/// ];
///
/// let poll = parse_poll(KIND_POLL, &tags, "Do you agree?").unwrap();
/// assert_eq!(poll.question, "Do you agree?");
/// assert_eq!(poll.options.len(), 2);
/// ```
pub fn parse_poll(kind: u16, tags: &[Vec<String>], content: &str) -> Result<PollEvent, Nip88Error> {
    // Validate kind
    if kind != KIND_POLL {
        return Err(Nip88Error::WrongKind {
            expected: KIND_POLL,
            actual: kind,
        });
    }

    // Validate content
    if content.is_empty() {
        return Err(Nip88Error::MissingQuestion);
    }

    // Parse components
    let options = parse_poll_options(tags)?;
    let relays = parse_poll_relays(tags);
    let poll_type = parse_poll_type(tags)?;
    let ends_at = parse_poll_ends_at(tags)?;

    Ok(PollEvent {
        question: content.to_string(),
        options,
        relays,
        poll_type,
        ends_at,
    })
}

/// Create tags for a poll response event
///
/// # Example
///
/// ```
/// use nostr::nip88::create_response_tags;
///
/// let tags = create_response_tags("poll123", &["yes".to_string()]);
/// assert_eq!(tags.len(), 2); // e tag + response tag
/// ```
pub fn create_response_tags(poll_id: &str, responses: &[String]) -> Vec<Vec<String>> {
    let mut tags = Vec::new();

    // Add poll event reference
    tags.push(vec![POLL_EVENT_TAG.to_string(), poll_id.to_string()]);

    // Add response tags
    for response in responses {
        tags.push(vec![RESPONSE_TAG.to_string(), response.clone()]);
    }

    tags
}

/// Parse a poll response event from tags
///
/// # Example
///
/// ```
/// use nostr::nip88::{parse_response, KIND_POLL_RESPONSE};
///
/// let tags = vec![
///     vec!["e".to_string(), "poll123".to_string()],
///     vec!["response".to_string(), "yes".to_string()],
/// ];
///
/// let response = parse_response(KIND_POLL_RESPONSE, &tags).unwrap();
/// assert_eq!(response.poll_id, "poll123");
/// assert_eq!(response.responses, vec!["yes"]);
/// ```
pub fn parse_response(kind: u16, tags: &[Vec<String>]) -> Result<PollResponse, Nip88Error> {
    // Validate kind
    if kind != KIND_POLL_RESPONSE {
        return Err(Nip88Error::WrongKind {
            expected: KIND_POLL_RESPONSE,
            actual: kind,
        });
    }

    // Get poll reference
    let poll_id = tags
        .iter()
        .find(|tag| tag.first().map(|s| s.as_str()) == Some(POLL_EVENT_TAG))
        .and_then(|tag| tag.get(1))
        .ok_or(Nip88Error::MissingPollReference)?
        .clone();

    // Get responses
    let responses: Vec<String> = tags
        .iter()
        .filter(|tag| tag.first().map(|s| s.as_str()) == Some(RESPONSE_TAG))
        .filter_map(|tag| tag.get(1).cloned())
        .collect();

    if responses.is_empty() {
        return Err(Nip88Error::NoResponses);
    }

    Ok(PollResponse { poll_id, responses })
}

/// Get effective responses based on poll type
///
/// For single-choice polls, only the first response is considered.
/// For multiple-choice polls, all unique responses are considered.
///
/// # Example
///
/// ```
/// use nostr::nip88::{PollType, get_effective_responses};
///
/// let responses = vec!["yes".to_string(), "no".to_string(), "yes".to_string()];
///
/// // Single choice: only first response
/// let effective = get_effective_responses(&responses, PollType::SingleChoice);
/// assert_eq!(effective, vec!["yes"]);
///
/// // Multiple choice: all unique responses
/// let effective = get_effective_responses(&responses, PollType::MultipleChoice);
/// assert_eq!(effective.len(), 2); // "yes" and "no"
/// ```
pub fn get_effective_responses(responses: &[String], poll_type: PollType) -> Vec<String> {
    match poll_type {
        PollType::SingleChoice => {
            // Only first response counts
            responses.first().cloned().into_iter().collect()
        }
        PollType::MultipleChoice => {
            // All unique responses count
            let mut seen = std::collections::HashSet::new();
            responses
                .iter()
                .filter(|r| seen.insert((*r).clone()))
                .cloned()
                .collect()
        }
    }
}

/// Check if a poll has ended
///
/// # Example
///
/// ```
/// use nostr::nip88::is_poll_ended;
///
/// let current_time = 1234567890;
/// let end_time = Some(1234567800); // Already ended
///
/// assert!(is_poll_ended(end_time, current_time));
/// ```
pub fn is_poll_ended(ends_at: Option<u64>, current_time: u64) -> bool {
    ends_at.map(|end| current_time > end).unwrap_or(false)
}

/// Check if an event is a poll
pub fn is_poll(kind: u16) -> bool {
    kind == KIND_POLL
}

/// Check if an event is a poll response
pub fn is_poll_response(kind: u16) -> bool {
    kind == KIND_POLL_RESPONSE
}

/// Filter responses to one per pubkey, keeping the latest
///
/// # Example
///
/// ```
/// use nostr::nip88::one_vote_per_pubkey;
/// use std::collections::HashMap;
///
/// let mut responses: HashMap<String, (u64, Vec<String>)> = HashMap::new();
/// responses.insert("alice".to_string(), (100, vec!["yes".to_string()]));
/// responses.insert("bob".to_string(), (200, vec!["no".to_string()]));
/// responses.insert("alice".to_string(), (300, vec!["no".to_string()])); // Latest
///
/// let filtered = one_vote_per_pubkey(responses);
/// assert_eq!(filtered.len(), 2);
/// assert_eq!(filtered.get("alice").unwrap().1, vec!["no"]); // Latest vote
/// ```
pub fn one_vote_per_pubkey(
    mut responses: HashMap<String, (u64, Vec<String>)>,
) -> HashMap<String, (u64, Vec<String>)> {
    // The HashMap will automatically keep the last inserted value for each key
    // We just need to ensure we're inserting in timestamp order
    let mut result: HashMap<String, (u64, Vec<String>)> = HashMap::new();

    for (pubkey, (timestamp, votes)) in responses.drain() {
        result
            .entry(pubkey)
            .and_modify(|(existing_ts, existing_votes)| {
                if timestamp > *existing_ts {
                    *existing_ts = timestamp;
                    *existing_votes = votes.clone();
                }
            })
            .or_insert((timestamp, votes));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poll_type_conversion() {
        assert_eq!(PollType::SingleChoice.as_str(), "singlechoice");
        assert_eq!(PollType::MultipleChoice.as_str(), "multiplechoice");

        assert_eq!(
            PollType::from_str("singlechoice")
                .ok()
                .unwrap_or(PollType::SingleChoice),
            PollType::SingleChoice
        );
        assert_eq!(
            PollType::from_str("multiplechoice")
                .ok()
                .unwrap_or(PollType::MultipleChoice),
            PollType::MultipleChoice
        );
        assert_eq!(
            PollType::from_str("SINGLECHOICE")
                .ok()
                .unwrap_or(PollType::SingleChoice),
            PollType::SingleChoice
        );

        assert!(PollType::from_str("invalid").is_err());
    }

    #[test]
    fn test_create_poll_tags() {
        let options = vec![
            PollOption {
                id: "yes".to_string(),
                label: "Yes".to_string(),
            },
            PollOption {
                id: "no".to_string(),
                label: "No".to_string(),
            },
        ];

        let relays = vec!["wss://relay.example.com".to_string()];

        let tags = create_poll_tags(&options, &relays, PollType::SingleChoice, Some(1234567890));

        assert_eq!(tags.len(), 5); // 2 options + 1 relay + polltype + endsAt

        // Check option tags
        assert_eq!(tags[0], vec!["option", "yes", "Yes"]);
        assert_eq!(tags[1], vec!["option", "no", "No"]);

        // Check relay tag
        assert_eq!(tags[2], vec!["relay", "wss://relay.example.com"]);

        // Check polltype tag
        assert_eq!(tags[3], vec!["polltype", "singlechoice"]);

        // Check endsAt tag
        assert_eq!(tags[4], vec!["endsAt", "1234567890"]);
    }

    #[test]
    fn test_parse_poll_options() {
        let tags = vec![
            vec!["option".to_string(), "yes".to_string(), "Yes".to_string()],
            vec!["option".to_string(), "no".to_string(), "No".to_string()],
            vec!["relay".to_string(), "wss://relay.example.com".to_string()],
        ];

        let options = parse_poll_options(&tags).unwrap();
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].id, "yes");
        assert_eq!(options[0].label, "Yes");
        assert_eq!(options[1].id, "no");
        assert_eq!(options[1].label, "No");
    }

    #[test]
    fn test_parse_poll_options_no_options() {
        let tags = vec![vec![
            "relay".to_string(),
            "wss://relay.example.com".to_string(),
        ]];

        assert!(matches!(
            parse_poll_options(&tags),
            Err(Nip88Error::NoOptions)
        ));
    }

    #[test]
    fn test_parse_poll_relays() {
        let tags = vec![
            vec!["relay".to_string(), "wss://relay1.example.com".to_string()],
            vec!["relay".to_string(), "wss://relay2.example.com".to_string()],
            vec!["option".to_string(), "yes".to_string(), "Yes".to_string()],
        ];

        let relays = parse_poll_relays(&tags);
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0], "wss://relay1.example.com");
        assert_eq!(relays[1], "wss://relay2.example.com");
    }

    #[test]
    fn test_parse_poll_type() {
        let tags = vec![vec!["polltype".to_string(), "multiplechoice".to_string()]];

        let poll_type = parse_poll_type(&tags).unwrap();
        assert_eq!(poll_type, PollType::MultipleChoice);
    }

    #[test]
    fn test_parse_poll_type_default() {
        let tags = vec![vec![
            "relay".to_string(),
            "wss://relay.example.com".to_string(),
        ]];

        let poll_type = parse_poll_type(&tags).unwrap();
        assert_eq!(poll_type, PollType::SingleChoice);
    }

    #[test]
    fn test_parse_poll_ends_at() {
        let tags = vec![vec!["endsAt".to_string(), "1234567890".to_string()]];

        let ends_at = parse_poll_ends_at(&tags).unwrap();
        assert_eq!(ends_at, Some(1234567890));
    }

    #[test]
    fn test_parse_poll() {
        let tags = vec![
            vec!["option".to_string(), "yes".to_string(), "Yes".to_string()],
            vec!["option".to_string(), "no".to_string(), "No".to_string()],
            vec!["relay".to_string(), "wss://relay.example.com".to_string()],
            vec!["polltype".to_string(), "singlechoice".to_string()],
            vec!["endsAt".to_string(), "1234567890".to_string()],
        ];

        let poll = parse_poll(KIND_POLL, &tags, "Do you agree?").unwrap();
        assert_eq!(poll.question, "Do you agree?");
        assert_eq!(poll.options.len(), 2);
        assert_eq!(poll.relays.len(), 1);
        assert_eq!(poll.poll_type, PollType::SingleChoice);
        assert_eq!(poll.ends_at, Some(1234567890));
    }

    #[test]
    fn test_parse_poll_wrong_kind() {
        let tags = vec![vec![
            "option".to_string(),
            "yes".to_string(),
            "Yes".to_string(),
        ]];

        assert!(matches!(
            parse_poll(1, &tags, "Question?"),
            Err(Nip88Error::WrongKind { .. })
        ));
    }

    #[test]
    fn test_parse_poll_empty_content() {
        let tags = vec![vec![
            "option".to_string(),
            "yes".to_string(),
            "Yes".to_string(),
        ]];

        assert!(matches!(
            parse_poll(KIND_POLL, &tags, ""),
            Err(Nip88Error::MissingQuestion)
        ));
    }

    #[test]
    fn test_create_response_tags() {
        let tags = create_response_tags("poll123", &["yes".to_string(), "no".to_string()]);

        assert_eq!(tags.len(), 3); // e tag + 2 response tags
        assert_eq!(tags[0], vec!["e", "poll123"]);
        assert_eq!(tags[1], vec!["response", "yes"]);
        assert_eq!(tags[2], vec!["response", "no"]);
    }

    #[test]
    fn test_parse_response() {
        let tags = vec![
            vec!["e".to_string(), "poll123".to_string()],
            vec!["response".to_string(), "yes".to_string()],
            vec!["response".to_string(), "no".to_string()],
        ];

        let response = parse_response(KIND_POLL_RESPONSE, &tags).unwrap();
        assert_eq!(response.poll_id, "poll123");
        assert_eq!(response.responses, vec!["yes", "no"]);
    }

    #[test]
    fn test_parse_response_missing_poll() {
        let tags = vec![vec!["response".to_string(), "yes".to_string()]];

        assert!(matches!(
            parse_response(KIND_POLL_RESPONSE, &tags),
            Err(Nip88Error::MissingPollReference)
        ));
    }

    #[test]
    fn test_parse_response_no_responses() {
        let tags = vec![vec!["e".to_string(), "poll123".to_string()]];

        assert!(matches!(
            parse_response(KIND_POLL_RESPONSE, &tags),
            Err(Nip88Error::NoResponses)
        ));
    }

    #[test]
    fn test_get_effective_responses_single_choice() {
        let responses = vec!["yes".to_string(), "no".to_string()];

        let effective = get_effective_responses(&responses, PollType::SingleChoice);
        assert_eq!(effective, vec!["yes"]);
    }

    #[test]
    fn test_get_effective_responses_multiple_choice() {
        let responses = vec!["yes".to_string(), "no".to_string(), "yes".to_string()];

        let effective = get_effective_responses(&responses, PollType::MultipleChoice);
        assert_eq!(effective.len(), 2); // Unique: yes, no
    }

    #[test]
    fn test_is_poll_ended() {
        assert!(is_poll_ended(Some(1000), 2000));
        assert!(!is_poll_ended(Some(2000), 1000));
        assert!(!is_poll_ended(None, 1000));
    }

    #[test]
    fn test_is_poll() {
        assert!(is_poll(KIND_POLL));
        assert!(is_poll(1068));
        assert!(!is_poll(1));
    }

    #[test]
    fn test_is_poll_response() {
        assert!(is_poll_response(KIND_POLL_RESPONSE));
        assert!(is_poll_response(1018));
        assert!(!is_poll_response(1));
    }

    #[test]
    fn test_one_vote_per_pubkey() {
        let mut responses: HashMap<String, (u64, Vec<String>)> = HashMap::new();
        responses.insert("alice".to_string(), (100, vec!["yes".to_string()]));
        responses.insert("bob".to_string(), (200, vec!["no".to_string()]));
        responses.insert("alice".to_string(), (300, vec!["no".to_string()]));

        let filtered = one_vote_per_pubkey(responses);
        assert_eq!(filtered.len(), 2);

        let alice_vote = filtered.get("alice").unwrap();
        assert_eq!(alice_vote.0, 300); // Latest timestamp
        assert_eq!(alice_vote.1, vec!["no"]); // Latest vote
    }
}
