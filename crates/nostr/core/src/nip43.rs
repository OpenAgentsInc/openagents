//! NIP-43: Relay Access Metadata and Requests
//!
//! This module implements NIP-43, which defines a way for relays to advertise membership
//! lists and for clients to request admission to relays.
//!
//! ## Event Kinds
//!
//! - `13534`: Membership list (published by relay, indicates current members)
//! - `8000`: Add user event (published by relay when member added)
//! - `8001`: Remove user event (published by relay when member removed)
//! - `28934`: Join request (sent by user requesting admission)
//! - `28935`: Invite request (ephemeral, relay provides invite codes)
//! - `28936`: Leave request (sent by user requesting access revocation)
//!
//! ## Use Cases
//!
//! - Private relay access control
//! - Membership management
//! - Invite code distribution
//! - Self-service admission/departure
//!
//! # Example
//!
//! ```
//! use nostr_core::nip43::{MembershipListEvent, validate_membership_list};
//! use nostr_core::Event;
//!
//! # fn example(event: &Event, relay_pubkey: &str) {
//! // Validate a membership list event
//! match validate_membership_list(event, relay_pubkey) {
//!     Ok(members) => println!("Valid membership list with {} members", members.len()),
//!     Err(e) => println!("Invalid: {}", e),
//! }
//! # }
//! ```

use crate::nip01::Event;
use crate::nip70::PROTECTED_TAG as NIP70_PROTECTED_TAG;
use thiserror::Error;

/// Event kind for membership lists (published by relay)
pub const MEMBERSHIP_LIST_KIND: u16 = 13534;

/// Event kind for add user events (published by relay)
pub const ADD_USER_KIND: u16 = 8000;

/// Event kind for remove user events (published by relay)
pub const REMOVE_USER_KIND: u16 = 8001;

/// Event kind for join requests (sent by user)
pub const JOIN_REQUEST_KIND: u16 = 28934;

/// Event kind for invite requests (ephemeral, published by relay)
pub const INVITE_REQUEST_KIND: u16 = 28935;

/// Event kind for leave requests (sent by user)
pub const LEAVE_REQUEST_KIND: u16 = 28936;

/// Tag name for member pubkeys
pub const MEMBER_TAG: &str = "member";

/// Tag name for invite claim codes
pub const CLAIM_TAG: &str = "claim";

/// Errors that can occur during NIP-43 operations.
#[derive(Debug, Error)]
pub enum Nip43Error {
    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("missing required protected tag (NIP-70 '-' tag)")]
    MissingProtectedTag,

    #[error("missing required tag: {0}")]
    MissingTag(String),

    #[error("pubkey mismatch: expected relay pubkey {expected}, got {actual}")]
    PubkeyMismatch { expected: String, actual: String },

    #[error("invalid timestamp: event must be recent (within ~10 minutes)")]
    InvalidTimestamp,

    #[error("invalid member tag format: {0}")]
    InvalidMemberTag(String),
}

/// Maximum acceptable time difference for time-sensitive events (10 minutes in seconds)
pub const MAX_TIME_DIFF: u64 = 600;

/// A membership list event (kind 13534).
///
/// Published by relays to advertise which pubkeys have access.
/// Must be signed by the relay's pubkey (from NIP-11 self field).
#[derive(Debug, Clone)]
pub struct MembershipListEvent {
    /// The underlying Nostr event
    pub event: Event,
    /// List of member pubkeys (hex format)
    pub members: Vec<String>,
}

impl MembershipListEvent {
    /// Create a new membership list event.
    ///
    /// Note: The event still needs to be signed by the relay's private key.
    pub fn new(relay_pubkey: String, members: Vec<String>) -> Event {
        let mut tags = vec![vec![NIP70_PROTECTED_TAG.to_string()]];

        for member in members {
            tags.push(vec![MEMBER_TAG.to_string(), member]);
        }

        Event {
            id: String::new(),
            pubkey: relay_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: MEMBERSHIP_LIST_KIND,
            tags,
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse a membership list event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != MEMBERSHIP_LIST_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: MEMBERSHIP_LIST_KIND,
                actual: event.kind,
            });
        }

        // Check for protected tag (NIP-70)
        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        // Extract member pubkeys
        let members = event
            .tags
            .iter()
            .filter(|tag| tag.len() >= 2 && tag[0] == MEMBER_TAG)
            .map(|tag| tag[1].clone())
            .collect();

        Ok(Self { event, members })
    }
}

/// An add user event (kind 8000).
///
/// Published by relays when a member is added.
#[derive(Debug, Clone)]
pub struct AddUserEvent {
    /// The underlying Nostr event
    pub event: Event,
    /// The pubkey of the added member
    pub member_pubkey: String,
}

impl AddUserEvent {
    /// Create a new add user event.
    pub fn new(relay_pubkey: String, member_pubkey: String) -> Event {
        Event {
            id: String::new(),
            pubkey: relay_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: ADD_USER_KIND,
            tags: vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec!["p".to_string(), member_pubkey],
            ],
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse an add user event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != ADD_USER_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: ADD_USER_KIND,
                actual: event.kind,
            });
        }

        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        let member_pubkey = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "p")
            .map(|tag| tag[1].clone())
            .ok_or_else(|| Nip43Error::MissingTag("p".to_string()))?;

        Ok(Self {
            event,
            member_pubkey,
        })
    }
}

/// A remove user event (kind 8001).
///
/// Published by relays when a member is removed.
#[derive(Debug, Clone)]
pub struct RemoveUserEvent {
    /// The underlying Nostr event
    pub event: Event,
    /// The pubkey of the removed member
    pub member_pubkey: String,
}

impl RemoveUserEvent {
    /// Create a new remove user event.
    pub fn new(relay_pubkey: String, member_pubkey: String) -> Event {
        Event {
            id: String::new(),
            pubkey: relay_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: REMOVE_USER_KIND,
            tags: vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec!["p".to_string(), member_pubkey],
            ],
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse a remove user event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != REMOVE_USER_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: REMOVE_USER_KIND,
                actual: event.kind,
            });
        }

        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        let member_pubkey = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == "p")
            .map(|tag| tag[1].clone())
            .ok_or_else(|| Nip43Error::MissingTag("p".to_string()))?;

        Ok(Self {
            event,
            member_pubkey,
        })
    }
}

/// A join request event (kind 28934).
///
/// Sent by users to request admission to a relay.
#[derive(Debug, Clone)]
pub struct JoinRequestEvent {
    /// The underlying Nostr event
    pub event: Event,
    /// The invite code claimed by the user
    pub claim: String,
}

impl JoinRequestEvent {
    /// Create a new join request event.
    pub fn new(user_pubkey: String, claim: String) -> Event {
        Event {
            id: String::new(),
            pubkey: user_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: JOIN_REQUEST_KIND,
            tags: vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec![CLAIM_TAG.to_string(), claim],
            ],
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse a join request event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != JOIN_REQUEST_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: JOIN_REQUEST_KIND,
                actual: event.kind,
            });
        }

        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        let claim = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == CLAIM_TAG)
            .map(|tag| tag[1].clone())
            .ok_or_else(|| Nip43Error::MissingTag(CLAIM_TAG.to_string()))?;

        Ok(Self { event, claim })
    }
}

/// An invite request event (kind 28935).
///
/// Ephemeral event published by relays containing invite codes.
#[derive(Debug, Clone)]
pub struct InviteRequestEvent {
    /// The underlying Nostr event
    pub event: Event,
    /// The invite code
    pub claim: String,
}

impl InviteRequestEvent {
    /// Create a new invite request event.
    pub fn new(relay_pubkey: String, claim: String) -> Event {
        Event {
            id: String::new(),
            pubkey: relay_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: INVITE_REQUEST_KIND,
            tags: vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec![CLAIM_TAG.to_string(), claim],
            ],
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse an invite request event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != INVITE_REQUEST_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: INVITE_REQUEST_KIND,
                actual: event.kind,
            });
        }

        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        let claim = event
            .tags
            .iter()
            .find(|tag| tag.len() >= 2 && tag[0] == CLAIM_TAG)
            .map(|tag| tag[1].clone())
            .ok_or_else(|| Nip43Error::MissingTag(CLAIM_TAG.to_string()))?;

        Ok(Self { event, claim })
    }
}

/// A leave request event (kind 28936).
///
/// Sent by users to request that their access be revoked.
#[derive(Debug, Clone)]
pub struct LeaveRequestEvent {
    /// The underlying Nostr event
    pub event: Event,
}

impl LeaveRequestEvent {
    /// Create a new leave request event.
    pub fn new(user_pubkey: String) -> Event {
        Event {
            id: String::new(),
            pubkey: user_pubkey,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            kind: LEAVE_REQUEST_KIND,
            tags: vec![vec![NIP70_PROTECTED_TAG.to_string()]],
            content: String::new(),
            sig: String::new(),
        }
    }

    /// Parse a leave request event from a generic event.
    pub fn from_event(event: Event) -> Result<Self, Nip43Error> {
        if event.kind != LEAVE_REQUEST_KIND {
            return Err(Nip43Error::InvalidKind {
                expected: LEAVE_REQUEST_KIND,
                actual: event.kind,
            });
        }

        if !has_protected_tag(&event) {
            return Err(Nip43Error::MissingProtectedTag);
        }

        Ok(Self { event })
    }
}

/// Check if an event has the NIP-70 protected tag.
fn has_protected_tag(event: &Event) -> bool {
    event
        .tags
        .iter()
        .any(|tag| tag.len() == 1 && tag[0] == NIP70_PROTECTED_TAG)
}

/// Validate a membership list event.
///
/// Returns the list of member pubkeys if valid.
pub fn validate_membership_list(
    event: &Event,
    expected_relay_pubkey: &str,
) -> Result<Vec<String>, Nip43Error> {
    if event.kind != MEMBERSHIP_LIST_KIND {
        return Err(Nip43Error::InvalidKind {
            expected: MEMBERSHIP_LIST_KIND,
            actual: event.kind,
        });
    }

    if event.pubkey != expected_relay_pubkey {
        return Err(Nip43Error::PubkeyMismatch {
            expected: expected_relay_pubkey.to_string(),
            actual: event.pubkey.clone(),
        });
    }

    if !has_protected_tag(event) {
        return Err(Nip43Error::MissingProtectedTag);
    }

    let members = event
        .tags
        .iter()
        .filter(|tag| tag.len() >= 2 && tag[0] == MEMBER_TAG)
        .map(|tag| tag[1].clone())
        .collect();

    Ok(members)
}

/// Validate an add user event.
///
/// Returns the added member's pubkey if valid.
pub fn validate_add_user(event: &Event, expected_relay_pubkey: &str) -> Result<String, Nip43Error> {
    if event.kind != ADD_USER_KIND {
        return Err(Nip43Error::InvalidKind {
            expected: ADD_USER_KIND,
            actual: event.kind,
        });
    }

    if event.pubkey != expected_relay_pubkey {
        return Err(Nip43Error::PubkeyMismatch {
            expected: expected_relay_pubkey.to_string(),
            actual: event.pubkey.clone(),
        });
    }

    if !has_protected_tag(event) {
        return Err(Nip43Error::MissingProtectedTag);
    }

    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "p")
        .map(|tag| tag[1].clone())
        .ok_or_else(|| Nip43Error::MissingTag("p".to_string()))
}

/// Validate a remove user event.
///
/// Returns the removed member's pubkey if valid.
pub fn validate_remove_user(
    event: &Event,
    expected_relay_pubkey: &str,
) -> Result<String, Nip43Error> {
    if event.kind != REMOVE_USER_KIND {
        return Err(Nip43Error::InvalidKind {
            expected: REMOVE_USER_KIND,
            actual: event.kind,
        });
    }

    if event.pubkey != expected_relay_pubkey {
        return Err(Nip43Error::PubkeyMismatch {
            expected: expected_relay_pubkey.to_string(),
            actual: event.pubkey.clone(),
        });
    }

    if !has_protected_tag(event) {
        return Err(Nip43Error::MissingProtectedTag);
    }

    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == "p")
        .map(|tag| tag[1].clone())
        .ok_or_else(|| Nip43Error::MissingTag("p".to_string()))
}

/// Validate a join request event.
///
/// Returns the claim code if valid.
/// Optionally checks timestamp freshness (within MAX_TIME_DIFF seconds).
pub fn validate_join_request(event: &Event, check_timestamp: bool) -> Result<String, Nip43Error> {
    if event.kind != JOIN_REQUEST_KIND {
        return Err(Nip43Error::InvalidKind {
            expected: JOIN_REQUEST_KIND,
            actual: event.kind,
        });
    }

    if !has_protected_tag(event) {
        return Err(Nip43Error::MissingProtectedTag);
    }

    if check_timestamp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let diff = now.abs_diff(event.created_at);
        if diff > MAX_TIME_DIFF {
            return Err(Nip43Error::InvalidTimestamp);
        }
    }

    event
        .tags
        .iter()
        .find(|tag| tag.len() >= 2 && tag[0] == CLAIM_TAG)
        .map(|tag| tag[1].clone())
        .ok_or_else(|| Nip43Error::MissingTag(CLAIM_TAG.to_string()))
}

/// Validate a leave request event.
///
/// Optionally checks timestamp freshness (within MAX_TIME_DIFF seconds).
pub fn validate_leave_request(event: &Event, check_timestamp: bool) -> Result<(), Nip43Error> {
    if event.kind != LEAVE_REQUEST_KIND {
        return Err(Nip43Error::InvalidKind {
            expected: LEAVE_REQUEST_KIND,
            actual: event.kind,
        });
    }

    if !has_protected_tag(event) {
        return Err(Nip43Error::MissingProtectedTag);
    }

    if check_timestamp {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let diff = now.abs_diff(event.created_at);
        if diff > MAX_TIME_DIFF {
            return Err(Nip43Error::InvalidTimestamp);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_event(kind: u16, pubkey: &str, tags: Vec<Vec<String>>) -> Event {
        Event {
            id: String::new(),
            kind,
            pubkey: pubkey.to_string(),
            tags,
            content: String::new(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            sig: String::new(),
        }
    }

    #[test]
    fn test_membership_list_creation() {
        let relay_pubkey = "relay123".to_string();
        let members = vec!["member1".to_string(), "member2".to_string()];

        let event = MembershipListEvent::new(relay_pubkey.clone(), members.clone());

        assert_eq!(event.kind, MEMBERSHIP_LIST_KIND);
        assert_eq!(event.pubkey, relay_pubkey);
        assert!(has_protected_tag(&event));

        let parsed = MembershipListEvent::from_event(event).unwrap();
        assert_eq!(parsed.members, members);
    }

    #[test]
    fn test_membership_list_validation() {
        let relay_pubkey = "relay123";
        let event = mock_event(
            MEMBERSHIP_LIST_KIND,
            relay_pubkey,
            vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec![MEMBER_TAG.to_string(), "member1".to_string()],
                vec![MEMBER_TAG.to_string(), "member2".to_string()],
            ],
        );

        let members = validate_membership_list(&event, relay_pubkey).unwrap();
        assert_eq!(members.len(), 2);
        assert!(members.contains(&"member1".to_string()));
        assert!(members.contains(&"member2".to_string()));
    }

    #[test]
    fn test_membership_list_wrong_pubkey() {
        let event = mock_event(
            MEMBERSHIP_LIST_KIND,
            "wrong_relay",
            vec![vec![NIP70_PROTECTED_TAG.to_string()]],
        );

        let result = validate_membership_list(&event, "expected_relay");
        assert!(matches!(result, Err(Nip43Error::PubkeyMismatch { .. })));
    }

    #[test]
    fn test_membership_list_missing_protected_tag() {
        let event = mock_event(MEMBERSHIP_LIST_KIND, "relay123", vec![]);

        let result = validate_membership_list(&event, "relay123");
        assert!(matches!(result, Err(Nip43Error::MissingProtectedTag)));
    }

    #[test]
    fn test_add_user_event() {
        let relay_pubkey = "relay123".to_string();
        let member_pubkey = "member456".to_string();

        let event = AddUserEvent::new(relay_pubkey.clone(), member_pubkey.clone());

        assert_eq!(event.kind, ADD_USER_KIND);
        assert!(has_protected_tag(&event));

        let parsed = AddUserEvent::from_event(event).unwrap();
        assert_eq!(parsed.member_pubkey, member_pubkey);
    }

    #[test]
    fn test_remove_user_event() {
        let relay_pubkey = "relay123".to_string();
        let member_pubkey = "member456".to_string();

        let event = RemoveUserEvent::new(relay_pubkey.clone(), member_pubkey.clone());

        assert_eq!(event.kind, REMOVE_USER_KIND);
        assert!(has_protected_tag(&event));

        let parsed = RemoveUserEvent::from_event(event).unwrap();
        assert_eq!(parsed.member_pubkey, member_pubkey);
    }

    #[test]
    fn test_join_request_event() {
        let user_pubkey = "user789".to_string();
        let claim = "invite_code_123".to_string();

        let event = JoinRequestEvent::new(user_pubkey.clone(), claim.clone());

        assert_eq!(event.kind, JOIN_REQUEST_KIND);
        assert!(has_protected_tag(&event));

        let parsed = JoinRequestEvent::from_event(event.clone()).unwrap();
        assert_eq!(parsed.claim, claim);

        // Test validation without timestamp check
        let validated_claim = validate_join_request(&event, false).unwrap();
        assert_eq!(validated_claim, claim);
    }

    #[test]
    fn test_invite_request_event() {
        let relay_pubkey = "relay123".to_string();
        let claim = "invite_code_456".to_string();

        let event = InviteRequestEvent::new(relay_pubkey.clone(), claim.clone());

        assert_eq!(event.kind, INVITE_REQUEST_KIND);
        assert!(has_protected_tag(&event));

        let parsed = InviteRequestEvent::from_event(event).unwrap();
        assert_eq!(parsed.claim, claim);
    }

    #[test]
    fn test_leave_request_event() {
        let user_pubkey = "user789".to_string();

        let event = LeaveRequestEvent::new(user_pubkey.clone());

        assert_eq!(event.kind, LEAVE_REQUEST_KIND);
        assert!(has_protected_tag(&event));

        let parsed = LeaveRequestEvent::from_event(event.clone()).unwrap();
        assert_eq!(parsed.event.pubkey, user_pubkey);

        // Test validation without timestamp check
        validate_leave_request(&event, false).unwrap();
    }

    #[test]
    fn test_join_request_timestamp_validation() {
        let old_event = Event {
            id: String::new(),
            kind: JOIN_REQUEST_KIND,
            pubkey: "user789".to_string(),
            tags: vec![
                vec![NIP70_PROTECTED_TAG.to_string()],
                vec![CLAIM_TAG.to_string(), "claim123".to_string()],
            ],
            content: String::new(),
            created_at: 1000000000, // Very old timestamp
            sig: String::new(),
        };

        let result = validate_join_request(&old_event, true);
        assert!(matches!(result, Err(Nip43Error::InvalidTimestamp)));
    }

    #[test]
    fn test_leave_request_timestamp_validation() {
        let old_event = Event {
            id: String::new(),
            kind: LEAVE_REQUEST_KIND,
            pubkey: "user789".to_string(),
            tags: vec![vec![NIP70_PROTECTED_TAG.to_string()]],
            content: String::new(),
            created_at: 1000000000, // Very old timestamp
            sig: String::new(),
        };

        let result = validate_leave_request(&old_event, true);
        assert!(matches!(result, Err(Nip43Error::InvalidTimestamp)));
    }
}
