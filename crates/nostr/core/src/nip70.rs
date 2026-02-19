//! NIP-70: Protected Events
//!
//! Defines a mechanism for marking events as "protected", meaning they can only
//! be published by their author after authentication. Relays must reject protected
//! events unless the author is authenticated via NIP-42.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/70.md>

use crate::Event;
use thiserror::Error;

/// Tag marker for protected events
pub const PROTECTED_TAG: &str = "-";

/// Errors that can occur during NIP-70 operations
#[derive(Debug, Error)]
pub enum Nip70Error {
    #[error("event is protected and requires authentication")]
    AuthenticationRequired,

    #[error("authenticated pubkey does not match event author")]
    AuthorMismatch { expected: String, actual: String },

    #[error("invalid tag format: {0}")]
    InvalidTag(String),
}

/// Check if an event is protected
///
/// A protected event has a `["-"]` tag. Protected events can only be published
/// to relays by their author after authentication (NIP-42).
pub fn is_protected(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| !tag.is_empty() && tag.len() == 1 && tag[0] == PROTECTED_TAG)
}

/// Add the protected tag to an event's tags
///
/// This marks the event as protected, requiring author authentication for publishing.
pub fn add_protected_tag(tags: &mut Vec<Vec<String>>) {
    // Only add if not already present
    if !tags
        .iter()
        .any(|tag| !tag.is_empty() && tag.len() == 1 && tag[0] == PROTECTED_TAG)
    {
        tags.push(vec![PROTECTED_TAG.to_string()]);
    }
}

/// Remove the protected tag from an event's tags
pub fn remove_protected_tag(tags: &mut Vec<Vec<String>>) {
    tags.retain(|tag| !(tag.len() == 1 && tag[0] == PROTECTED_TAG));
}

/// Validate that an authenticated user can publish a protected event
///
/// Returns Ok if:
/// - Event is not protected, OR
/// - Event is protected AND authenticated_pubkey matches event author
///
/// Returns Err if event is protected but authentication fails
pub fn validate_protected_event(
    event: &Event,
    authenticated_pubkey: Option<&str>,
) -> Result<(), Nip70Error> {
    if !is_protected(event) {
        // Not protected, no authentication required
        return Ok(());
    }

    // Event is protected, check authentication
    match authenticated_pubkey {
        None => Err(Nip70Error::AuthenticationRequired),
        Some(pubkey) => {
            if pubkey == event.pubkey {
                Ok(())
            } else {
                Err(Nip70Error::AuthorMismatch {
                    expected: event.pubkey.clone(),
                    actual: pubkey.to_string(),
                })
            }
        }
    }
}

/// Get the protected tag if present
pub fn get_protected_tag(event: &Event) -> Option<Vec<String>> {
    event
        .tags
        .iter()
        .find(|tag| !tag.is_empty() && tag.len() == 1 && tag[0] == PROTECTED_TAG)
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(pubkey: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test_id".to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1707409439,
            kind: 1,
            tags,
            content: "test content".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_is_protected_true() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![vec!["-".to_string()]],
        );
        assert!(is_protected(&event));
    }

    #[test]
    fn test_is_protected_false() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![],
        );
        assert!(!is_protected(&event));
    }

    #[test]
    fn test_is_protected_with_other_tags() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![
                vec!["p".to_string(), "somepubkey".to_string()],
                vec!["-".to_string()],
                vec!["e".to_string(), "someevent".to_string()],
            ],
        );
        assert!(is_protected(&event));
    }

    #[test]
    fn test_is_protected_not_alone() {
        // Tag must be exactly ["-"], not ["-", "something"]
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![vec!["-".to_string(), "extra".to_string()]],
        );
        assert!(!is_protected(&event));
    }

    #[test]
    fn test_add_protected_tag() {
        let mut tags = vec![vec!["p".to_string(), "somepubkey".to_string()]];
        add_protected_tag(&mut tags);

        assert_eq!(tags.len(), 2);
        assert_eq!(tags[1], vec!["-".to_string()]);
    }

    #[test]
    fn test_add_protected_tag_idempotent() {
        let mut tags = vec![vec!["-".to_string()]];
        add_protected_tag(&mut tags);

        // Should not add duplicate
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["-".to_string()]);
    }

    #[test]
    fn test_remove_protected_tag() {
        let mut tags = vec![
            vec!["p".to_string(), "somepubkey".to_string()],
            vec!["-".to_string()],
            vec!["e".to_string(), "someevent".to_string()],
        ];
        remove_protected_tag(&mut tags);

        assert_eq!(tags.len(), 2);
        assert!(!tags.iter().any(|tag| tag == &vec!["-".to_string()]));
    }

    #[test]
    fn test_remove_protected_tag_none() {
        let mut tags = vec![vec!["p".to_string(), "somepubkey".to_string()]];
        remove_protected_tag(&mut tags);

        // Should not affect other tags
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn test_validate_protected_event_not_protected() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![],
        );

        // Should succeed even without authentication
        assert!(validate_protected_event(&event, None).is_ok());
    }

    #[test]
    fn test_validate_protected_event_no_auth() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![vec!["-".to_string()]],
        );

        let result = validate_protected_event(&event, None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip70Error::AuthenticationRequired
        ));
    }

    #[test]
    fn test_validate_protected_event_auth_mismatch() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![vec!["-".to_string()]],
        );

        let result = validate_protected_event(&event, Some("different_pubkey"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            Nip70Error::AuthorMismatch { .. }
        ));
    }

    #[test]
    fn test_validate_protected_event_auth_success() {
        let pubkey = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
        let event = create_test_event(pubkey, vec![vec!["-".to_string()]]);

        let result = validate_protected_event(&event, Some(pubkey));
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_protected_tag_present() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![
                vec!["p".to_string(), "somepubkey".to_string()],
                vec!["-".to_string()],
            ],
        );

        let tag = get_protected_tag(&event);
        assert!(tag.is_some());
        assert_eq!(tag.unwrap(), vec!["-".to_string()]);
    }

    #[test]
    fn test_get_protected_tag_absent() {
        let event = create_test_event(
            "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
            vec![vec!["p".to_string(), "somepubkey".to_string()]],
        );

        let tag = get_protected_tag(&event);
        assert!(tag.is_none());
    }

    #[test]
    fn test_protected_event_example_from_nip() {
        // Example from NIP-70 specification
        let event = Event {
            id: "cb8feca582979d91fe90455867b34dbf4d65e4b86e86b3c68c368ca9f9eef6f2".to_string(),
            pubkey: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                .to_string(),
            created_at: 1707409439,
            kind: 1,
            tags: vec![vec!["-".to_string()]],
            content: "hello members of the secret group".to_string(),
            sig: "fa163f5cfb75d77d9b6269011872ee22b34fb48d23251e9879bb1e4ccbdd8aaaf4b6dc5f5084a65ef42c52fbcde8f3178bac3ba207de827ec513a6aa39fa684c".to_string(),
        };

        assert!(is_protected(&event));

        // Without auth, should fail
        assert!(validate_protected_event(&event, None).is_err());

        // With wrong auth, should fail
        assert!(validate_protected_event(&event, Some("wrong_pubkey")).is_err());

        // With correct auth, should succeed
        assert!(
            validate_protected_event(
                &event,
                Some("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798")
            )
            .is_ok()
        );
    }
}
