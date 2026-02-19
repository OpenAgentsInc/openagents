//! Contact list synchronization
//!
//! This module provides automatic contact list synchronization with support for:
//! - Subscribing to kind:3 events (NIP-02)
//! - Maintaining local cache of contact lists
//! - Merge strategies for conflicts
//! - Contact list updates and retrieval

use crate::error::{ClientError, Result};
use nostr::{CONTACT_LIST_KIND, Contact, ContactList, Event};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Merge strategy for contact list conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MergeStrategy {
    /// Replace with newest (by created_at timestamp)
    #[default]
    ReplaceNewest,
    /// Union of both lists (merge contacts from both)
    Union,
    /// Keep local version
    KeepLocal,
    /// Keep remote version
    KeepRemote,
}

/// Contact list manager for synchronizing and caching contact lists
pub struct ContactManager {
    /// Cached contact lists by pubkey
    lists: Arc<RwLock<HashMap<String, ContactList>>>,
    /// Merge strategy for conflicts
    merge_strategy: MergeStrategy,
}

impl ContactManager {
    /// Create a new contact manager with default merge strategy
    pub fn new() -> Self {
        Self::with_strategy(MergeStrategy::default())
    }

    /// Create a new contact manager with a specific merge strategy
    pub fn with_strategy(merge_strategy: MergeStrategy) -> Self {
        Self {
            lists: Arc::new(RwLock::new(HashMap::new())),
            merge_strategy,
        }
    }

    /// Handle an incoming event, updating contact list if applicable
    pub fn handle_event(&self, event: Event) -> Result<bool> {
        if event.kind != CONTACT_LIST_KIND {
            return Ok(false);
        }

        let contact_list = ContactList::from_event(event)
            .map_err(|e| ClientError::Protocol(format!("Failed to parse contact list: {}", e)))?;

        let pubkey = contact_list.event.pubkey.clone();
        let mut lists = self.lists.write().map_err(|e| {
            tracing::error!("Contact lists write lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire write lock on contact lists (poisoned)".to_string(),
            )
        })?;

        // Check if we need to merge or replace
        let should_update = if let Some(existing) = lists.get(&pubkey) {
            match self.merge_strategy {
                MergeStrategy::ReplaceNewest => {
                    contact_list.event.created_at >= existing.event.created_at
                }
                MergeStrategy::KeepLocal => false,
                MergeStrategy::KeepRemote => true,
                MergeStrategy::Union => true,
            }
        } else {
            true
        };

        if should_update {
            match self.merge_strategy {
                MergeStrategy::Union => {
                    if let Some(existing) = lists.get(&pubkey) {
                        let merged = self.merge_contact_lists(existing, &contact_list)?;
                        lists.insert(pubkey, merged);
                    } else {
                        lists.insert(pubkey, contact_list);
                    }
                }
                _ => {
                    lists.insert(pubkey, contact_list);
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Merge two contact lists using union strategy
    fn merge_contact_lists(&self, list1: &ContactList, list2: &ContactList) -> Result<ContactList> {
        let mut contacts_map: HashMap<String, Contact> = HashMap::new();

        // Add all contacts from list1
        for contact in &list1.contacts {
            contacts_map.insert(contact.pubkey.clone(), contact.clone());
        }

        // Add/merge contacts from list2
        for contact in &list2.contacts {
            contacts_map
                .entry(contact.pubkey.clone())
                .and_modify(|existing| {
                    // Prefer non-empty relay_url and petname from newer list
                    if contact.relay_url.is_some() {
                        existing.relay_url = contact.relay_url.clone();
                    }
                    if contact.petname.is_some() {
                        existing.petname = contact.petname.clone();
                    }
                })
                .or_insert_with(|| contact.clone());
        }

        // Use the newer event
        let event = if list2.event.created_at > list1.event.created_at {
            list2.event.clone()
        } else {
            list1.event.clone()
        };

        let contacts: Vec<Contact> = contacts_map.into_values().collect();

        Ok(ContactList { event, contacts })
    }

    /// Get contact list for a specific pubkey
    pub fn get_contact_list(&self, pubkey: &str) -> Result<Option<ContactList>> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists.get(pubkey).cloned())
    }

    /// Get all cached contact lists
    pub fn get_all_contact_lists(&self) -> Result<Vec<ContactList>> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists.values().cloned().collect())
    }

    /// Get contacts for a specific pubkey
    pub fn get_contacts(&self, pubkey: &str) -> Result<Vec<Contact>> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists
            .get(pubkey)
            .map(|list| list.contacts.clone())
            .unwrap_or_default())
    }

    /// Check if a pubkey follows another pubkey
    pub fn is_following(&self, follower: &str, followee: &str) -> Result<bool> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists
            .get(follower)
            .map(|list| list.contains(followee))
            .unwrap_or(false))
    }

    /// Get all followers of a specific pubkey (from cached lists)
    pub fn get_followers(&self, pubkey: &str) -> Result<Vec<String>> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        let mut followers = Vec::new();
        for (follower_pubkey, list) in lists.iter() {
            if list.contains(pubkey) {
                followers.push(follower_pubkey.clone());
            }
        }

        Ok(followers)
    }

    /// Get petname for a contact
    pub fn get_petname(&self, owner: &str, contact_pubkey: &str) -> Result<Option<String>> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists
            .get(owner)
            .and_then(|list| list.get_petname(contact_pubkey).map(String::from)))
    }

    /// Clear all cached contact lists
    pub fn clear(&self) -> Result<()> {
        let mut lists = self.lists.write().map_err(|e| {
            tracing::error!("Contact lists write lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire write lock on contact lists (poisoned)".to_string(),
            )
        })?;

        lists.clear();
        Ok(())
    }

    /// Get number of cached contact lists
    pub fn len(&self) -> Result<usize> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists.len())
    }

    /// Check if no contact lists are cached
    pub fn is_empty(&self) -> Result<bool> {
        let lists = self.lists.read().map_err(|e| {
            tracing::error!("Contact lists read lock poisoned: {}", e);
            ClientError::Internal(
                "Failed to acquire read lock on contact lists (poisoned)".to_string(),
            )
        })?;

        Ok(lists.is_empty())
    }

    /// Create a filter for subscribing to contact list events
    pub fn create_subscription_filter(pubkeys: Option<Vec<String>>) -> serde_json::Value {
        let mut filter = serde_json::json!({
            "kinds": [CONTACT_LIST_KIND]
        });

        if let Some(pks) = pubkeys {
            filter["authors"] = serde_json::json!(pks);
        }

        filter
    }
}

impl Default for ContactManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(pubkey: &str, created_at: u64, contacts: Vec<Contact>) -> Event {
        let tags: Vec<Vec<String>> = contacts.iter().map(|c| c.to_tag()).collect();

        Event {
            id: format!("test_{}_{}", pubkey, created_at),
            pubkey: pubkey.to_string(),
            created_at,
            kind: CONTACT_LIST_KIND,
            tags,
            content: String::new(),
            sig: "test_sig".to_string(),
        }
    }

    fn create_contact(pubkey: &str, relay: Option<&str>, petname: Option<&str>) -> Contact {
        let mut contact = Contact::new(pubkey);
        contact.relay_url = relay.map(String::from);
        contact.petname = petname.map(String::from);
        contact
    }

    #[test]
    fn test_handle_event_new_contact_list() {
        let manager = ContactManager::new();
        let pubkey = "a".repeat(64);
        let contacts = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event = create_test_event(&pubkey, 1234567890, contacts);

        let result = manager.handle_event(event).unwrap();
        assert!(result); // Should return true for new list

        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts.len(), 1);
    }

    #[test]
    fn test_handle_event_replace_newest() {
        let manager = ContactManager::with_strategy(MergeStrategy::ReplaceNewest);
        let pubkey = "a".repeat(64);

        // Insert older list
        let contacts1 = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event1 = create_test_event(&pubkey, 1234567890, contacts1);
        manager.handle_event(event1).unwrap();

        // Insert newer list
        let contacts2 = vec![create_contact(&"c".repeat(64), None, Some("bob"))];
        let event2 = create_test_event(&pubkey, 1234567900, contacts2);
        let updated = manager.handle_event(event2).unwrap();
        assert!(updated);

        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts.len(), 1);
        assert_eq!(list.contacts[0].petname, Some("bob".to_string()));
    }

    #[test]
    fn test_handle_event_ignore_older() {
        let manager = ContactManager::with_strategy(MergeStrategy::ReplaceNewest);
        let pubkey = "a".repeat(64);

        // Insert newer list
        let contacts1 = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event1 = create_test_event(&pubkey, 1234567900, contacts1);
        manager.handle_event(event1).unwrap();

        // Try to insert older list
        let contacts2 = vec![create_contact(&"c".repeat(64), None, Some("bob"))];
        let event2 = create_test_event(&pubkey, 1234567890, contacts2);
        let updated = manager.handle_event(event2).unwrap();
        assert!(!updated);

        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts.len(), 1);
        assert_eq!(list.contacts[0].petname, Some("alice".to_string()));
    }

    #[test]
    fn test_handle_event_union_strategy() {
        let manager = ContactManager::with_strategy(MergeStrategy::Union);
        let pubkey = "a".repeat(64);

        // Insert first list
        let contacts1 = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event1 = create_test_event(&pubkey, 1234567890, contacts1);
        manager.handle_event(event1).unwrap();

        // Insert second list with different contact
        let contacts2 = vec![create_contact(&"c".repeat(64), None, Some("bob"))];
        let event2 = create_test_event(&pubkey, 1234567900, contacts2);
        manager.handle_event(event2).unwrap();

        // Should have both contacts
        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts.len(), 2);
    }

    #[test]
    fn test_get_contacts() {
        let manager = ContactManager::new();
        let pubkey = "a".repeat(64);
        let contacts = vec![
            create_contact(&"b".repeat(64), None, Some("alice")),
            create_contact(&"c".repeat(64), None, Some("bob")),
        ];
        let event = create_test_event(&pubkey, 1234567890, contacts);
        manager.handle_event(event).unwrap();

        let retrieved = manager.get_contacts(&pubkey).unwrap();
        assert_eq!(retrieved.len(), 2);
    }

    #[test]
    fn test_is_following() {
        let manager = ContactManager::new();
        let follower = "a".repeat(64);
        let followee = "b".repeat(64);

        let contacts = vec![create_contact(&followee, None, Some("alice"))];
        let event = create_test_event(&follower, 1234567890, contacts);
        manager.handle_event(event).unwrap();

        assert!(manager.is_following(&follower, &followee).unwrap());
        assert!(!manager.is_following(&follower, &"c".repeat(64)).unwrap());
    }

    #[test]
    fn test_get_followers() {
        let manager = ContactManager::new();
        let followee = "a".repeat(64);
        let follower1 = "b".repeat(64);
        let follower2 = "c".repeat(64);

        // Follower1 follows followee
        let contacts1 = vec![create_contact(&followee, None, None)];
        let event1 = create_test_event(&follower1, 1234567890, contacts1);
        manager.handle_event(event1).unwrap();

        // Follower2 follows followee
        let contacts2 = vec![create_contact(&followee, None, None)];
        let event2 = create_test_event(&follower2, 1234567890, contacts2);
        manager.handle_event(event2).unwrap();

        let followers = manager.get_followers(&followee).unwrap();
        assert_eq!(followers.len(), 2);
        assert!(followers.contains(&follower1));
        assert!(followers.contains(&follower2));
    }

    #[test]
    fn test_get_petname() {
        let manager = ContactManager::new();
        let owner = "a".repeat(64);
        let contact_pk = "b".repeat(64);

        let contacts = vec![create_contact(&contact_pk, None, Some("alice"))];
        let event = create_test_event(&owner, 1234567890, contacts);
        manager.handle_event(event).unwrap();

        let petname = manager.get_petname(&owner, &contact_pk).unwrap();
        assert_eq!(petname, Some("alice".to_string()));
    }

    #[test]
    fn test_clear() {
        let manager = ContactManager::new();
        let pubkey = "a".repeat(64);
        let contacts = vec![create_contact(&"b".repeat(64), None, None)];
        let event = create_test_event(&pubkey, 1234567890, contacts);
        manager.handle_event(event).unwrap();

        assert_eq!(manager.len().unwrap(), 1);
        manager.clear().unwrap();
        assert_eq!(manager.len().unwrap(), 0);
        assert!(manager.is_empty().unwrap());
    }

    #[test]
    fn test_create_subscription_filter() {
        let filter = ContactManager::create_subscription_filter(None);
        assert_eq!(filter["kinds"][0], 3);

        let pubkeys = vec!["a".repeat(64), "b".repeat(64)];
        let filter_with_authors = ContactManager::create_subscription_filter(Some(pubkeys));
        assert_eq!(filter_with_authors["kinds"][0], 3);
        assert_eq!(filter_with_authors["authors"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_merge_contact_lists() {
        let manager = ContactManager::new();

        let pubkey = "a".repeat(64);
        let contact1_pk = "b".repeat(64);
        let contact2_pk = "c".repeat(64);

        // First list with contact1
        let contacts1 = vec![create_contact(
            &contact1_pk,
            Some("wss://relay1.com"),
            Some("alice"),
        )];
        let event1 = create_test_event(&pubkey, 1234567890, contacts1.clone());
        let list1 = ContactList::from_event(event1).unwrap();

        // Second list with contact1 (updated relay) and contact2
        let contacts2 = vec![
            create_contact(&contact1_pk, Some("wss://relay2.com"), None),
            create_contact(&contact2_pk, None, Some("bob")),
        ];
        let event2 = create_test_event(&pubkey, 1234567900, contacts2.clone());
        let list2 = ContactList::from_event(event2).unwrap();

        let merged = manager.merge_contact_lists(&list1, &list2).unwrap();

        // Should have both contacts
        assert_eq!(merged.contacts.len(), 2);

        // contact1 should have updated relay from list2 but keep petname from list1
        let contact1 = merged
            .contacts
            .iter()
            .find(|c| c.pubkey == contact1_pk)
            .unwrap();
        assert_eq!(contact1.relay_url, Some("wss://relay2.com".to_string()));
        assert_eq!(contact1.petname, Some("alice".to_string()));

        // contact2 should be present
        let contact2 = merged
            .contacts
            .iter()
            .find(|c| c.pubkey == contact2_pk)
            .unwrap();
        assert_eq!(contact2.petname, Some("bob".to_string()));
    }

    #[test]
    fn test_keep_local_strategy() {
        let manager = ContactManager::with_strategy(MergeStrategy::KeepLocal);
        let pubkey = "a".repeat(64);

        // Insert local list
        let contacts1 = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event1 = create_test_event(&pubkey, 1234567890, contacts1);
        manager.handle_event(event1).unwrap();

        // Try to insert remote list (newer)
        let contacts2 = vec![create_contact(&"c".repeat(64), None, Some("bob"))];
        let event2 = create_test_event(&pubkey, 1234567900, contacts2);
        let updated = manager.handle_event(event2).unwrap();
        assert!(!updated); // Should not update

        // Should still have old list
        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts[0].petname, Some("alice".to_string()));
    }

    #[test]
    fn test_keep_remote_strategy() {
        let manager = ContactManager::with_strategy(MergeStrategy::KeepRemote);
        let pubkey = "a".repeat(64);

        // Insert local list (newer)
        let contacts1 = vec![create_contact(&"b".repeat(64), None, Some("alice"))];
        let event1 = create_test_event(&pubkey, 1234567900, contacts1);
        manager.handle_event(event1).unwrap();

        // Insert remote list (older)
        let contacts2 = vec![create_contact(&"c".repeat(64), None, Some("bob"))];
        let event2 = create_test_event(&pubkey, 1234567890, contacts2);
        let updated = manager.handle_event(event2).unwrap();
        assert!(updated); // Should update despite being older

        // Should have new list
        let list = manager.get_contact_list(&pubkey).unwrap().unwrap();
        assert_eq!(list.contacts[0].petname, Some("bob".to_string()));
    }
}
