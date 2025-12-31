//! NIP-C7: Chats
//!
//! Defines a simple chat protocol using kind 9 events. Chat messages can be
//! standalone or replies that quote parent messages using a `q` tag.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/C7.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for chat messages
pub const CHAT_KIND: u16 = 9;

/// Tag name for quote references
pub const QUOTE_TAG: &str = "q";

/// Errors that can occur during NIP-C7 operations
#[derive(Debug, Error)]
pub enum NipC7Error {
    #[error("event is not a chat message (kind {0})")]
    InvalidKind(u16),

    #[error("invalid quote tag format: {0}")]
    InvalidQuoteTag(String),

    #[error("missing required field: {0}")]
    MissingField(String),
}

/// A quote reference to a parent chat message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteReference {
    /// Event ID being quoted
    pub event_id: String,
    /// Relay URL where the event can be found
    pub relay_url: Option<String>,
    /// Public key of the quoted event author
    pub pubkey: Option<String>,
}

/// A chat message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ChatMessage {
    /// Message content
    pub content: String,
    /// Quote reference if this is a reply
    pub quote: Option<QuoteReference>,
}

#[allow(dead_code)]
impl ChatMessage {
    /// Create a new standalone chat message
    pub fn new(content: String) -> Self {
        Self {
            content,
            quote: None,
        }
    }

    /// Create a new chat message replying to another
    pub fn reply(content: String, quote: QuoteReference) -> Self {
        Self {
            content,
            quote: Some(quote),
        }
    }

    /// Parse a chat message from an event
    pub fn from_event(event: &Event) -> Result<Self, NipC7Error> {
        if event.kind != CHAT_KIND {
            return Err(NipC7Error::InvalidKind(event.kind));
        }

        let quote = get_quote_reference(event)?;

        Ok(Self {
            content: event.content.clone(),
            quote,
        })
    }

    /// Check if this message is a reply
    pub fn is_reply(&self) -> bool {
        self.quote.is_some()
    }
}

impl QuoteReference {
    /// Create a new quote reference
    pub fn new(event_id: String, relay_url: Option<String>, pubkey: Option<String>) -> Self {
        Self {
            event_id,
            relay_url,
            pubkey,
        }
    }

    /// Create from a minimal event ID
    pub fn from_event_id(event_id: String) -> Self {
        Self {
            event_id,
            relay_url: None,
            pubkey: None,
        }
    }
}

/// Check if an event is a chat message
pub fn is_chat_kind(kind: u16) -> bool {
    kind == CHAT_KIND
}

/// Get the quote reference from a chat event, if present
pub fn get_quote_reference(event: &Event) -> Result<Option<QuoteReference>, NipC7Error> {
    for tag in &event.tags {
        if tag.is_empty() || tag[0] != QUOTE_TAG {
            continue;
        }

        if tag.len() < 2 {
            return Err(NipC7Error::InvalidQuoteTag(
                "quote tag must have at least event ID".to_string(),
            ));
        }

        let event_id = tag[1].clone();
        let relay_url = tag.get(2).cloned();
        let pubkey = tag.get(3).cloned();

        return Ok(Some(QuoteReference {
            event_id,
            relay_url,
            pubkey,
        }));
    }

    Ok(None)
}

/// Create a quote tag
#[allow(dead_code)]
pub fn create_quote_tag(quote: &QuoteReference) -> Vec<String> {
    let mut tag = vec![QUOTE_TAG.to_string(), quote.event_id.clone()];

    if let Some(ref relay_url) = quote.relay_url {
        tag.push(relay_url.clone());

        if let Some(ref pubkey) = quote.pubkey {
            tag.push(pubkey.clone());
        }
    } else if let Some(ref pubkey) = quote.pubkey {
        // If we have pubkey but no relay, add empty string for relay
        tag.push("".to_string());
        tag.push(pubkey.clone());
    }

    tag
}

/// Add a quote tag to an event's tags
#[allow(dead_code)]
pub fn add_quote_tag(tags: &mut Vec<Vec<String>>, quote: &QuoteReference) {
    tags.push(create_quote_tag(quote));
}

/// Check if an event has a quote tag
pub fn has_quote_tag(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| !tag.is_empty() && tag[0] == QUOTE_TAG)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(kind: u16, content: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1707409439,
            kind,
            tags,
            content: content.to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_is_chat_kind() {
        assert!(is_chat_kind(9));
        assert!(!is_chat_kind(1));
        assert!(!is_chat_kind(0));
    }

    #[test]
    fn test_chat_message_new() {
        let msg = ChatMessage::new("GM".to_string());
        assert_eq!(msg.content, "GM");
        assert!(msg.quote.is_none());
        assert!(!msg.is_reply());
    }

    #[test]
    fn test_chat_message_reply() {
        let quote = QuoteReference::new(
            "event123".to_string(),
            Some("wss://relay.example.com".to_string()),
            Some("pubkey456".to_string()),
        );
        let msg = ChatMessage::reply("yes".to_string(), quote);

        assert_eq!(msg.content, "yes");
        assert!(msg.quote.is_some());
        assert!(msg.is_reply());
        assert_eq!(msg.quote.unwrap().event_id, "event123");
    }

    #[test]
    fn test_quote_reference_from_event_id() {
        let quote = QuoteReference::from_event_id("abc123".to_string());
        assert_eq!(quote.event_id, "abc123");
        assert!(quote.relay_url.is_none());
        assert!(quote.pubkey.is_none());
    }

    #[test]
    fn test_chat_message_from_event_simple() {
        let event = create_test_event(9, "GM", vec![]);
        let msg = ChatMessage::from_event(&event).unwrap();

        assert_eq!(msg.content, "GM");
        assert!(msg.quote.is_none());
    }

    #[test]
    fn test_chat_message_from_event_with_quote() {
        let event = create_test_event(
            9,
            "nostr:nevent1...\nyes",
            vec![vec![
                "q".to_string(),
                "event123".to_string(),
                "wss://relay.example.com".to_string(),
                "pubkey456".to_string(),
            ]],
        );
        let msg = ChatMessage::from_event(&event).unwrap();

        assert_eq!(msg.content, "nostr:nevent1...\nyes");
        assert!(msg.quote.is_some());

        let quote = msg.quote.unwrap();
        assert_eq!(quote.event_id, "event123");
        assert_eq!(quote.relay_url, Some("wss://relay.example.com".to_string()));
        assert_eq!(quote.pubkey, Some("pubkey456".to_string()));
    }

    #[test]
    fn test_chat_message_from_event_invalid_kind() {
        let event = create_test_event(1, "test", vec![]);
        let result = ChatMessage::from_event(&event);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), NipC7Error::InvalidKind(1)));
    }

    #[test]
    fn test_get_quote_reference_minimal() {
        let event = create_test_event(9, "test", vec![vec!["q".to_string(), "abc123".to_string()]]);
        let quote = get_quote_reference(&event).unwrap();

        assert!(quote.is_some());
        let quote = quote.unwrap();
        assert_eq!(quote.event_id, "abc123");
        assert!(quote.relay_url.is_none());
        assert!(quote.pubkey.is_none());
    }

    #[test]
    fn test_get_quote_reference_with_relay() {
        let event = create_test_event(
            9,
            "test",
            vec![vec![
                "q".to_string(),
                "abc123".to_string(),
                "wss://relay.example.com".to_string(),
            ]],
        );
        let quote = get_quote_reference(&event).unwrap();

        assert!(quote.is_some());
        let quote = quote.unwrap();
        assert_eq!(quote.event_id, "abc123");
        assert_eq!(quote.relay_url, Some("wss://relay.example.com".to_string()));
        assert!(quote.pubkey.is_none());
    }

    #[test]
    fn test_get_quote_reference_full() {
        let event = create_test_event(
            9,
            "test",
            vec![vec![
                "q".to_string(),
                "abc123".to_string(),
                "wss://relay.example.com".to_string(),
                "pubkey456".to_string(),
            ]],
        );
        let quote = get_quote_reference(&event).unwrap();

        assert!(quote.is_some());
        let quote = quote.unwrap();
        assert_eq!(quote.event_id, "abc123");
        assert_eq!(quote.relay_url, Some("wss://relay.example.com".to_string()));
        assert_eq!(quote.pubkey, Some("pubkey456".to_string()));
    }

    #[test]
    fn test_get_quote_reference_none() {
        let event = create_test_event(9, "test", vec![]);
        let quote = get_quote_reference(&event).unwrap();
        assert!(quote.is_none());
    }

    #[test]
    fn test_get_quote_reference_invalid() {
        let event = create_test_event(9, "test", vec![vec!["q".to_string()]]);
        let result = get_quote_reference(&event);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NipC7Error::InvalidQuoteTag(_)
        ));
    }

    #[test]
    fn test_create_quote_tag_minimal() {
        let quote = QuoteReference::from_event_id("abc123".to_string());
        let tag = create_quote_tag(&quote);

        assert_eq!(tag, vec!["q".to_string(), "abc123".to_string()]);
    }

    #[test]
    fn test_create_quote_tag_with_relay() {
        let quote = QuoteReference::new(
            "abc123".to_string(),
            Some("wss://relay.example.com".to_string()),
            None,
        );
        let tag = create_quote_tag(&quote);

        assert_eq!(
            tag,
            vec![
                "q".to_string(),
                "abc123".to_string(),
                "wss://relay.example.com".to_string()
            ]
        );
    }

    #[test]
    fn test_create_quote_tag_full() {
        let quote = QuoteReference::new(
            "abc123".to_string(),
            Some("wss://relay.example.com".to_string()),
            Some("pubkey456".to_string()),
        );
        let tag = create_quote_tag(&quote);

        assert_eq!(
            tag,
            vec![
                "q".to_string(),
                "abc123".to_string(),
                "wss://relay.example.com".to_string(),
                "pubkey456".to_string()
            ]
        );
    }

    #[test]
    fn test_create_quote_tag_pubkey_no_relay() {
        let quote = QuoteReference::new("abc123".to_string(), None, Some("pubkey456".to_string()));
        let tag = create_quote_tag(&quote);

        assert_eq!(
            tag,
            vec![
                "q".to_string(),
                "abc123".to_string(),
                "".to_string(),
                "pubkey456".to_string()
            ]
        );
    }

    #[test]
    fn test_has_quote_tag() {
        let event_with_quote =
            create_test_event(9, "test", vec![vec!["q".to_string(), "abc123".to_string()]]);
        assert!(has_quote_tag(&event_with_quote));

        let event_without_quote = create_test_event(9, "test", vec![]);
        assert!(!has_quote_tag(&event_without_quote));
    }

    #[test]
    fn test_add_quote_tag() {
        let mut tags = vec![vec!["p".to_string(), "somepubkey".to_string()]];
        let quote = QuoteReference::new(
            "abc123".to_string(),
            Some("wss://relay.example.com".to_string()),
            Some("pubkey456".to_string()),
        );

        add_quote_tag(&mut tags, &quote);

        assert_eq!(tags.len(), 2);
        assert_eq!(
            tags[1],
            vec![
                "q".to_string(),
                "abc123".to_string(),
                "wss://relay.example.com".to_string(),
                "pubkey456".to_string()
            ]
        );
    }

    #[test]
    fn test_example_from_nip() {
        // Simple message example
        let simple = create_test_event(9, "GM", vec![]);
        let msg = ChatMessage::from_event(&simple).unwrap();
        assert_eq!(msg.content, "GM");
        assert!(!msg.is_reply());

        // Reply example
        let reply = create_test_event(
            9,
            "nostr:nevent1...\nyes",
            vec![vec![
                "q".to_string(),
                "event123".to_string(),
                "wss://relay.example.com".to_string(),
                "pubkey456".to_string(),
            ]],
        );
        let msg = ChatMessage::from_event(&reply).unwrap();
        assert!(msg.content.contains("nostr:nevent1"));
        assert!(msg.is_reply());
        assert_eq!(msg.quote.as_ref().unwrap().event_id, "event123");
    }
}
