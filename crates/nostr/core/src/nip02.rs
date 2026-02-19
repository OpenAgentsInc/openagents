//! NIP-02: Follow List (Contact List and Petnames)
//!
//! Defines how users publish their follow/contact lists as kind 3 events.
//! Each followed profile is represented by a "p" tag with optional relay URL and petname.
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/02.md>

use crate::Event;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Event kind for contact lists (follow lists)
pub const CONTACT_LIST_KIND: u16 = 3;

/// Errors that can occur during NIP-02 operations
#[derive(Debug, Error)]
pub enum Nip02Error {
    #[error("invalid event kind: expected 3, got {0}")]
    InvalidKind(u16),

    #[error("invalid p-tag format: {0}")]
    InvalidPTag(String),

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// A single contact in a follow list
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    /// The public key of the followed profile (32-byte hex)
    pub pubkey: String,

    /// Optional relay URL where this profile can be found
    pub relay_url: Option<String>,

    /// Optional local petname for this contact
    pub petname: Option<String>,
}

impl Contact {
    /// Create a new contact with just a public key
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            pubkey: pubkey.into(),
            relay_url: None,
            petname: None,
        }
    }

    /// Create a contact with a relay URL
    pub fn with_relay(pubkey: impl Into<String>, relay_url: impl Into<String>) -> Self {
        let relay_url = relay_url.into();
        Self {
            pubkey: pubkey.into(),
            relay_url: if relay_url.is_empty() {
                None
            } else {
                Some(relay_url)
            },
            petname: None,
        }
    }

    /// Create a contact with both relay URL and petname
    pub fn with_relay_and_petname(
        pubkey: impl Into<String>,
        relay_url: impl Into<String>,
        petname: impl Into<String>,
    ) -> Self {
        let relay_url = relay_url.into();
        let petname = petname.into();
        Self {
            pubkey: pubkey.into(),
            relay_url: if relay_url.is_empty() {
                None
            } else {
                Some(relay_url)
            },
            petname: if petname.is_empty() {
                None
            } else {
                Some(petname)
            },
        }
    }

    /// Convert contact to a p-tag array
    pub fn to_tag(&self) -> Vec<String> {
        let mut tag = vec!["p".to_string(), self.pubkey.clone()];

        // Add relay URL (empty string if None)
        tag.push(self.relay_url.clone().unwrap_or_default());

        // Add petname if present
        if let Some(ref petname) = self.petname {
            tag.push(petname.clone());
        }

        tag
    }

    /// Parse a contact from a p-tag array
    pub fn from_tag(tag: &[String]) -> Result<Self, Nip02Error> {
        if tag.is_empty() || tag[0] != "p" {
            return Err(Nip02Error::InvalidPTag(
                "tag must start with 'p'".to_string(),
            ));
        }

        if tag.len() < 2 {
            return Err(Nip02Error::InvalidPTag(
                "p-tag must have at least pubkey".to_string(),
            ));
        }

        let pubkey = tag[1].clone();

        // Validate pubkey is 64-character hex
        if pubkey.len() != 64 || !pubkey.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Nip02Error::InvalidPublicKey(format!(
                "pubkey must be 64-character hex, got: {}",
                pubkey
            )));
        }

        let relay_url = if tag.len() > 2 && !tag[2].is_empty() {
            Some(tag[2].clone())
        } else {
            None
        };

        let petname = if tag.len() > 3 && !tag[3].is_empty() {
            Some(tag[3].clone())
        } else {
            None
        };

        Ok(Self {
            pubkey,
            relay_url,
            petname,
        })
    }
}

/// Contact list (follow list) event
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContactList {
    pub event: Event,
    pub contacts: Vec<Contact>,
}

impl ContactList {
    /// Create a new contact list from an event
    pub fn from_event(event: Event) -> Result<Self, Nip02Error> {
        if event.kind != CONTACT_LIST_KIND {
            return Err(Nip02Error::InvalidKind(event.kind));
        }

        let mut contacts = Vec::new();

        for tag in &event.tags {
            if !tag.is_empty() && tag[0] == "p" {
                contacts.push(Contact::from_tag(tag)?);
            }
        }

        Ok(Self { event, contacts })
    }

    /// Get all contacts in the list
    pub fn get_contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Get contact by public key
    pub fn get_contact(&self, pubkey: &str) -> Option<&Contact> {
        self.contacts.iter().find(|c| c.pubkey == pubkey)
    }

    /// Check if a public key is in the contact list
    pub fn contains(&self, pubkey: &str) -> bool {
        self.contacts.iter().any(|c| c.pubkey == pubkey)
    }

    /// Get all public keys in the contact list
    pub fn get_pubkeys(&self) -> Vec<String> {
        self.contacts.iter().map(|c| c.pubkey.clone()).collect()
    }

    /// Get petname for a public key, if set
    pub fn get_petname(&self, pubkey: &str) -> Option<&str> {
        self.contacts
            .iter()
            .find(|c| c.pubkey == pubkey)
            .and_then(|c| c.petname.as_deref())
    }

    /// Get relay URL for a public key, if set
    pub fn get_relay(&self, pubkey: &str) -> Option<&str> {
        self.contacts
            .iter()
            .find(|c| c.pubkey == pubkey)
            .and_then(|c| c.relay_url.as_deref())
    }

    /// Build a petname index (pubkey -> petname)
    pub fn petname_index(&self) -> HashMap<String, String> {
        self.contacts
            .iter()
            .filter_map(|c| {
                c.petname
                    .as_ref()
                    .map(|name| (c.pubkey.clone(), name.clone()))
            })
            .collect()
    }

    /// Build a relay index (pubkey -> relay URL)
    pub fn relay_index(&self) -> HashMap<String, String> {
        self.contacts
            .iter()
            .filter_map(|c| {
                c.relay_url
                    .as_ref()
                    .map(|url| (c.pubkey.clone(), url.clone()))
            })
            .collect()
    }

    /// Get total number of contacts
    pub fn len(&self) -> usize {
        self.contacts.len()
    }

    /// Check if contact list is empty
    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }

    /// Validate the contact list structure
    pub fn validate(&self) -> Result<(), Nip02Error> {
        // Ensure event kind is correct
        if self.event.kind != CONTACT_LIST_KIND {
            return Err(Nip02Error::InvalidKind(self.event.kind));
        }

        // Validate all contacts
        for contact in &self.contacts {
            // Pubkey should be 64-character hex
            if contact.pubkey.len() != 64 || !contact.pubkey.chars().all(|c| c.is_ascii_hexdigit())
            {
                return Err(Nip02Error::InvalidPublicKey(contact.pubkey.clone()));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(tags: Vec<Vec<String>>) -> Event {
        Event {
            id: "test".to_string(),
            pubkey: "test_pubkey".to_string(),
            created_at: 1234567890,
            kind: CONTACT_LIST_KIND,
            tags,
            content: "".to_string(),
            sig: "test_sig".to_string(),
        }
    }

    #[test]
    fn test_contact_new() {
        let contact = Contact::new("91cf9a3a3e5ca".to_string() + &"0".repeat(51));
        assert_eq!(contact.pubkey.len(), 64);
        assert!(contact.relay_url.is_none());
        assert!(contact.petname.is_none());
    }

    #[test]
    fn test_contact_with_relay() {
        let pubkey = "91cf9a3a3e5ca".to_string() + &"0".repeat(51);
        let contact = Contact::with_relay(&pubkey, "wss://relay.example.com");
        assert_eq!(contact.pubkey, pubkey);
        assert_eq!(
            contact.relay_url,
            Some("wss://relay.example.com".to_string())
        );
        assert!(contact.petname.is_none());
    }

    #[test]
    fn test_contact_with_relay_and_petname() {
        let pubkey = "91cf9a3a3e5ca".to_string() + &"0".repeat(51);
        let contact = Contact::with_relay_and_petname(&pubkey, "wss://relay.example.com", "alice");
        assert_eq!(contact.pubkey, pubkey);
        assert_eq!(
            contact.relay_url,
            Some("wss://relay.example.com".to_string())
        );
        assert_eq!(contact.petname, Some("alice".to_string()));
    }

    #[test]
    fn test_contact_to_tag() {
        let pubkey = "a".repeat(64);
        let contact = Contact::with_relay_and_petname(&pubkey, "wss://relay.com", "bob");
        let tag = contact.to_tag();
        assert_eq!(tag.len(), 4);
        assert_eq!(tag[0], "p");
        assert_eq!(tag[1], pubkey);
        assert_eq!(tag[2], "wss://relay.com");
        assert_eq!(tag[3], "bob");
    }

    #[test]
    fn test_contact_from_tag() {
        let pubkey = "b".repeat(64);
        let tag = vec![
            "p".to_string(),
            pubkey.clone(),
            "wss://relay.com".to_string(),
            "carol".to_string(),
        ];

        let contact = Contact::from_tag(&tag).unwrap();
        assert_eq!(contact.pubkey, pubkey);
        assert_eq!(contact.relay_url, Some("wss://relay.com".to_string()));
        assert_eq!(contact.petname, Some("carol".to_string()));
    }

    #[test]
    fn test_contact_from_tag_minimal() {
        let pubkey = "c".repeat(64);
        let tag = vec!["p".to_string(), pubkey.clone()];

        let contact = Contact::from_tag(&tag).unwrap();
        assert_eq!(contact.pubkey, pubkey);
        assert!(contact.relay_url.is_none());
        assert!(contact.petname.is_none());
    }

    #[test]
    fn test_contact_from_tag_empty_relay() {
        let pubkey = "d".repeat(64);
        let tag = vec!["p".to_string(), pubkey.clone(), "".to_string()];

        let contact = Contact::from_tag(&tag).unwrap();
        assert_eq!(contact.pubkey, pubkey);
        assert!(contact.relay_url.is_none());
        assert!(contact.petname.is_none());
    }

    #[test]
    fn test_contact_from_tag_invalid_pubkey() {
        let tag = vec!["p".to_string(), "invalid".to_string()];
        let result = Contact::from_tag(&tag);
        assert!(result.is_err());
    }

    #[test]
    fn test_contact_list_from_event() {
        let tags = vec![
            vec![
                "p".to_string(),
                "a".repeat(64),
                "wss://alice.com".to_string(),
                "alice".to_string(),
            ],
            vec![
                "p".to_string(),
                "b".repeat(64),
                "wss://bob.com".to_string(),
                "bob".to_string(),
            ],
        ];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        assert_eq!(contact_list.len(), 2);
        assert_eq!(contact_list.contacts[0].petname, Some("alice".to_string()));
        assert_eq!(contact_list.contacts[1].petname, Some("bob".to_string()));
    }

    #[test]
    fn test_contact_list_get_contact() {
        let pubkey = "a".repeat(64);
        let tags = vec![vec![
            "p".to_string(),
            pubkey.clone(),
            "wss://relay.com".to_string(),
            "alice".to_string(),
        ]];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        let contact = contact_list.get_contact(&pubkey).unwrap();
        assert_eq!(contact.pubkey, pubkey);
        assert_eq!(contact.petname, Some("alice".to_string()));
    }

    #[test]
    fn test_contact_list_contains() {
        let pubkey = "a".repeat(64);
        let tags = vec![vec!["p".to_string(), pubkey.clone()]];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        assert!(contact_list.contains(&pubkey));
        assert!(!contact_list.contains(&"b".repeat(64)));
    }

    #[test]
    fn test_contact_list_get_petname() {
        let pubkey = "a".repeat(64);
        let tags = vec![vec![
            "p".to_string(),
            pubkey.clone(),
            "".to_string(),
            "alice".to_string(),
        ]];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        assert_eq!(contact_list.get_petname(&pubkey), Some("alice"));
    }

    #[test]
    fn test_contact_list_get_relay() {
        let pubkey = "a".repeat(64);
        let tags = vec![vec![
            "p".to_string(),
            pubkey.clone(),
            "wss://relay.com".to_string(),
        ]];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        assert_eq!(contact_list.get_relay(&pubkey), Some("wss://relay.com"));
    }

    #[test]
    fn test_contact_list_petname_index() {
        let pubkey1 = "a".repeat(64);
        let pubkey2 = "b".repeat(64);
        let tags = vec![
            vec![
                "p".to_string(),
                pubkey1.clone(),
                "".to_string(),
                "alice".to_string(),
            ],
            vec![
                "p".to_string(),
                pubkey2.clone(),
                "".to_string(),
                "bob".to_string(),
            ],
        ];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        let index = contact_list.petname_index();
        assert_eq!(index.len(), 2);
        assert_eq!(index.get(&pubkey1), Some(&"alice".to_string()));
        assert_eq!(index.get(&pubkey2), Some(&"bob".to_string()));
    }

    #[test]
    fn test_contact_list_relay_index() {
        let pubkey1 = "a".repeat(64);
        let pubkey2 = "b".repeat(64);
        let tags = vec![
            vec![
                "p".to_string(),
                pubkey1.clone(),
                "wss://alice.com".to_string(),
            ],
            vec![
                "p".to_string(),
                pubkey2.clone(),
                "wss://bob.com".to_string(),
            ],
        ];

        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        let index = contact_list.relay_index();
        assert_eq!(index.len(), 2);
        assert_eq!(index.get(&pubkey1), Some(&"wss://alice.com".to_string()));
        assert_eq!(index.get(&pubkey2), Some(&"wss://bob.com".to_string()));
    }

    #[test]
    fn test_contact_list_validate() {
        let tags = vec![vec!["p".to_string(), "a".repeat(64)]];
        let event = create_test_event(tags);
        let contact_list = ContactList::from_event(event).unwrap();

        assert!(contact_list.validate().is_ok());
    }

    #[test]
    fn test_contact_list_wrong_kind() {
        let mut event = create_test_event(vec![]);
        event.kind = 1;

        let result = ContactList::from_event(event);
        assert!(result.is_err());
    }
}
