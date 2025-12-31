//! NIP-62: Request to Vanish
//!
//! This module implements NIP-62, which defines a Nostr-native way to request complete
//! deletion of a key's events from relays. This is legally binding in some jurisdictions
//! (e.g., GDPR right to be forgotten).
//!
//! ## Event Kind
//!
//! - `62`: Request to Vanish (delete all events from pubkey up to created_at)
//!
//! ## Usage
//!
//! Users can request deletion from specific relays or globally from all relays using
//! the special "ALL_RELAYS" tag value.
//!
//! ## Legal Implications
//!
//! This NIP offers a legally binding procedure in some jurisdictions. Relay operators
//! who support this NIP should truly delete events from their database, including:
//! - All events from the pubkey up to the request's created_at
//! - NIP-09 deletion events
//! - NIP-59 gift wraps that p-tagged the pubkey
//!
//! Relays must ensure deleted events cannot be re-broadcasted.
//!
//! # Example
//!
//! ```
//! use nostr_core::nip62::{RequestToVanish, validate_request_to_vanish};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event) {
//! // Validate a request to vanish
//! match validate_request_to_vanish(event) {
//!     Ok(relays) => println!("Request targets {} relay(s)", relays.len()),
//!     Err(e) => println!("Invalid: {}", e),
//! }
//! # }
//! ```

use crate::nip01::Event;
use thiserror::Error;

/// Event kind for request to vanish
pub const REQUEST_TO_VANISH_KIND: u16 = 62;

/// Tag name for relay URLs
pub const RELAY_TAG: &str = "relay";

/// Special tag value to request deletion from all relays
pub const ALL_RELAYS: &str = "ALL_RELAYS";

/// Errors that can occur during NIP-62 operations.
#[derive(Debug, Error)]
pub enum Nip62Error {
    #[error("invalid event kind: expected 62, got {0}")]
    InvalidKind(u16),

    #[error("missing relay tag: request must include at least one 'relay' tag")]
    MissingRelayTag,

    #[error("empty relay URL in tag")]
    EmptyRelayUrl,
}

/// A request to vanish event (kind 62).
///
/// Requests a relay (or all relays) to delete all events from the pubkey
/// up to the created_at timestamp.
#[derive(Debug, Clone)]
pub struct RequestToVanish {
    /// The underlying Nostr event
    pub event: Event,
    /// List of relay URLs to delete from (or vec!["ALL_RELAYS"] for global)
    pub relays: Vec<String>,
    /// Optional reason or legal notice
    pub reason: Option<String>,
}

impl RequestToVanish {
    /// Create a new request to vanish for specific relays.
    pub fn new(user_pubkey: String, relay_urls: Vec<String>, reason: Option<String>) -> Event {
        let mut tags = Vec::new();

        for url in relay_urls {
            tags.push(vec![RELAY_TAG.to_string(), url]);
        }

        Event {
            id: String::new(),
            pubkey: user_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: REQUEST_TO_VANISH_KIND,
            tags,
            content: reason.unwrap_or_default(),
            sig: String::new(),
        }
    }

    /// Create a new global request to vanish (targets all relays).
    pub fn new_global(user_pubkey: String, reason: Option<String>) -> Event {
        Event {
            id: String::new(),
            pubkey: user_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: REQUEST_TO_VANISH_KIND,
            tags: vec![vec![RELAY_TAG.to_string(), ALL_RELAYS.to_string()]],
            content: reason.unwrap_or_default(),
            sig: String::new(),
        }
    }

    /// Parse a request to vanish from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip62Error> {
        if event.kind != REQUEST_TO_VANISH_KIND {
            return Err(Nip62Error::InvalidKind(event.kind));
        }

        let relays: Vec<String> = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == RELAY_TAG)
            .map(|tag| tag[1].clone())
            .collect();

        if relays.is_empty() {
            return Err(Nip62Error::MissingRelayTag);
        }

        // Check for empty relay URLs
        if relays.iter().any(|r| r.is_empty()) {
            return Err(Nip62Error::EmptyRelayUrl);
        }

        let reason = if event.content.is_empty() {
            None
        } else {
            Some(event.content.clone())
        };

        Ok(Self {
            event,
            relays,
            reason,
        })
    }

    /// Check if this is a global request (targets all relays).
    pub fn is_global(&self) -> bool {
        self.relays.iter().any(|r| r == ALL_RELAYS)
    }

    /// Get the target relay URLs.
    ///
    /// Returns None if this is a global request.
    pub fn target_relays(&self) -> Option<Vec<String>> {
        if self.is_global() {
            None
        } else {
            Some(self.relays.clone())
        }
    }
}

/// Validate a request to vanish event.
///
/// Returns the list of target relay URLs if valid.
/// If the request is global, returns vec!["ALL_RELAYS"].
pub fn validate_request_to_vanish(event: &Event) -> Result<Vec<String>, Nip62Error> {
    if event.kind != REQUEST_TO_VANISH_KIND {
        return Err(Nip62Error::InvalidKind(event.kind));
    }

    let relays: Vec<String> = event
        .tags
        .iter()
        .filter(|tag| tag.len() >= 2 && tag[0] == RELAY_TAG)
        .map(|tag| tag[1].clone())
        .collect();

    if relays.is_empty() {
        return Err(Nip62Error::MissingRelayTag);
    }

    if relays.iter().any(|r| r.is_empty()) {
        return Err(Nip62Error::EmptyRelayUrl);
    }

    Ok(relays)
}

/// Check if an event is a request to vanish.
pub fn is_request_to_vanish(event: &Event) -> bool {
    event.kind == REQUEST_TO_VANISH_KIND
}

/// Check if a request to vanish is global (targets all relays).
pub fn is_global_request(event: &Event) -> Result<bool, Nip62Error> {
    let relays = validate_request_to_vanish(event)?;
    Ok(relays.iter().any(|r| r == ALL_RELAYS))
}

/// Extract target relay URLs from a request to vanish.
///
/// Returns None if this is a global request.
pub fn get_target_relays(event: &Event) -> Result<Option<Vec<String>>, Nip62Error> {
    let relays = validate_request_to_vanish(event)?;

    if relays.iter().any(|r| r == ALL_RELAYS) {
        Ok(None)
    } else {
        Ok(Some(relays))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event(kind: u16, pubkey: &str, tags: Vec<Vec<String>>, content: &str) -> Event {
        Event {
            id: String::new(),
            kind,
            pubkey: pubkey.to_string(),
            tags,
            content: content.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            sig: String::new(),
        }
    }

    #[test]
    fn test_request_to_vanish_specific_relays() {
        let user_pubkey = "user123".to_string();
        let relays = vec![
            "wss://relay1.example.com".to_string(),
            "wss://relay2.example.com".to_string(),
        ];
        let reason = Some("GDPR right to be forgotten request".to_string());

        let event = RequestToVanish::new(user_pubkey.clone(), relays.clone(), reason.clone());

        assert_eq!(event.kind, REQUEST_TO_VANISH_KIND);
        assert_eq!(event.pubkey, user_pubkey);
        assert_eq!(event.content, reason.unwrap());
        assert_eq!(event.tags.len(), 2);

        let parsed = RequestToVanish::from_event(event).unwrap();
        assert_eq!(parsed.relays.len(), 2);
        assert!(!parsed.is_global());
        assert_eq!(parsed.target_relays(), Some(relays));
    }

    #[test]
    fn test_request_to_vanish_global() {
        let user_pubkey = "user456".to_string();
        let reason = Some("Complete data deletion request".to_string());

        let event = RequestToVanish::new_global(user_pubkey.clone(), reason.clone());

        assert_eq!(event.kind, REQUEST_TO_VANISH_KIND);
        assert_eq!(event.pubkey, user_pubkey);
        assert_eq!(event.content, reason.unwrap());
        assert_eq!(event.tags.len(), 1);
        assert_eq!(event.tags[0][1], ALL_RELAYS);

        let parsed = RequestToVanish::from_event(event).unwrap();
        assert!(parsed.is_global());
        assert_eq!(parsed.target_relays(), None);
    }

    #[test]
    fn test_request_to_vanish_no_reason() {
        let user_pubkey = "user789".to_string();
        let relays = vec!["wss://relay.example.com".to_string()];

        let event = RequestToVanish::new(user_pubkey.clone(), relays, None);

        assert_eq!(event.content, "");

        let parsed = RequestToVanish::from_event(event).unwrap();
        assert_eq!(parsed.reason, None);
    }

    #[test]
    fn test_validate_request_to_vanish() {
        let event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![
                vec![RELAY_TAG.to_string(), "wss://relay1.com".to_string()],
                vec![RELAY_TAG.to_string(), "wss://relay2.com".to_string()],
            ],
            "Legal notice",
        );

        let relays = validate_request_to_vanish(&event).unwrap();
        assert_eq!(relays.len(), 2);
        assert!(relays.contains(&"wss://relay1.com".to_string()));
        assert!(relays.contains(&"wss://relay2.com".to_string()));
    }

    #[test]
    fn test_validate_request_to_vanish_invalid_kind() {
        let event = mock_event(
            1, // Wrong kind
            "user123",
            vec![vec![RELAY_TAG.to_string(), "wss://relay.com".to_string()]],
            "",
        );

        let result = validate_request_to_vanish(&event);
        assert!(matches!(result, Err(Nip62Error::InvalidKind(1))));
    }

    #[test]
    fn test_validate_request_to_vanish_missing_relay_tag() {
        let event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![], // No relay tags
            "",
        );

        let result = validate_request_to_vanish(&event);
        assert!(matches!(result, Err(Nip62Error::MissingRelayTag)));
    }

    #[test]
    fn test_validate_request_to_vanish_empty_relay_url() {
        let event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![vec![RELAY_TAG.to_string(), String::new()]], // Empty URL
            "",
        );

        let result = validate_request_to_vanish(&event);
        assert!(matches!(result, Err(Nip62Error::EmptyRelayUrl)));
    }

    #[test]
    fn test_is_request_to_vanish() {
        let event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![vec![RELAY_TAG.to_string(), "wss://relay.com".to_string()]],
            "",
        );

        assert!(is_request_to_vanish(&event));
    }

    #[test]
    fn test_is_not_request_to_vanish() {
        let event = mock_event(1, "user123", vec![], "Regular note");

        assert!(!is_request_to_vanish(&event));
    }

    #[test]
    fn test_is_global_request() {
        let global_event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![vec![RELAY_TAG.to_string(), ALL_RELAYS.to_string()]],
            "",
        );

        assert!(is_global_request(&global_event).unwrap());

        let specific_event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![vec![RELAY_TAG.to_string(), "wss://relay.com".to_string()]],
            "",
        );

        assert!(!is_global_request(&specific_event).unwrap());
    }

    #[test]
    fn test_get_target_relays() {
        let specific_event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![
                vec![RELAY_TAG.to_string(), "wss://relay1.com".to_string()],
                vec![RELAY_TAG.to_string(), "wss://relay2.com".to_string()],
            ],
            "",
        );

        let relays = get_target_relays(&specific_event).unwrap().unwrap();
        assert_eq!(relays.len(), 2);

        let global_event = mock_event(
            REQUEST_TO_VANISH_KIND,
            "user123",
            vec![vec![RELAY_TAG.to_string(), ALL_RELAYS.to_string()]],
            "",
        );

        let result = get_target_relays(&global_event).unwrap();
        assert_eq!(result, None);
    }
}
