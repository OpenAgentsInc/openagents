//! Local event cache for efficient storage and retrieval

use crate::error::Result;
use nostr::Event;
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuration for event cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of events to store
    pub max_events: usize,
    /// Whether to enable caching
    pub enabled: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_events: 10000,
            enabled: true,
        }
    }
}

/// Local event cache with LRU eviction
pub struct EventCache {
    /// Configuration
    config: CacheConfig,
    /// Events by ID
    events: HashMap<String, Event>,
    /// LRU queue (event IDs in access order, oldest first)
    lru_queue: VecDeque<String>,
    /// Index by kind
    by_kind: HashMap<u16, HashSet<String>>,
    /// Index by author (pubkey)
    by_author: HashMap<String, HashSet<String>>,
    /// Index by tag name -> tag value -> event IDs
    by_tag: HashMap<String, HashMap<String, HashSet<String>>>,
    /// Replaceable events by author + kind (for NIP-16)
    replaceable: HashMap<(String, u16), String>,
    /// Parameterized replaceable events by author + kind + d-tag (for NIP-33)
    param_replaceable: HashMap<(String, u16, String), String>,
}

impl EventCache {
    /// Create a new event cache with default config
    pub fn new() -> Self {
        Self::with_config(CacheConfig::default())
    }

    /// Create a new event cache with custom config
    pub fn with_config(config: CacheConfig) -> Self {
        Self {
            config,
            events: HashMap::new(),
            lru_queue: VecDeque::new(),
            by_kind: HashMap::new(),
            by_author: HashMap::new(),
            by_tag: HashMap::new(),
            replaceable: HashMap::new(),
            param_replaceable: HashMap::new(),
        }
    }

    /// Insert an event into the cache
    pub fn insert(&mut self, event: Event) -> Result<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let event_id = event.id.clone();

        // Handle replaceable events (kinds 0, 3, 10000-19999)
        if self.is_replaceable_kind(event.kind) {
            let key = (event.pubkey.clone(), event.kind);

            // Remove old replaceable event if it exists
            if let Some(old_id) = self.replaceable.get(&key).cloned()
                && old_id != event_id
            {
                // Only replace if new event is newer
                if let Some(old_event) = self.events.get(&old_id) {
                    if event.created_at > old_event.created_at {
                        self.remove_internal(&old_id);
                    } else {
                        // Ignore older replaceable event
                        return Ok(());
                    }
                }
            }

            self.replaceable.insert(key, event_id.clone());
        }

        // Handle parameterized replaceable events (kinds 30000-39999)
        if self.is_param_replaceable_kind(event.kind)
            && let Some(d_tag) = self.get_d_tag(&event)
        {
            let key = (event.pubkey.clone(), event.kind, d_tag);

            // Remove old param replaceable event if it exists
            if let Some(old_id) = self.param_replaceable.get(&key).cloned()
                && old_id != event_id
                && let Some(old_event) = self.events.get(&old_id)
            {
                if event.created_at > old_event.created_at {
                    self.remove_internal(&old_id);
                } else {
                    // Ignore older param replaceable event
                    return Ok(());
                }
            }

            self.param_replaceable.insert(key, event_id.clone());
        }

        // Check if event already exists
        if self.events.contains_key(&event_id) {
            // Update LRU by moving to end
            self.lru_queue.retain(|id| id != &event_id);
            self.lru_queue.push_back(event_id.clone());
            return Ok(());
        }

        // Evict oldest event if cache is full
        if self.events.len() >= self.config.max_events
            && let Some(old_id) = self.lru_queue.pop_front()
        {
            self.remove_internal(&old_id);
        }

        // Add to indexes
        self.by_kind
            .entry(event.kind)
            .or_default()
            .insert(event_id.clone());

        self.by_author
            .entry(event.pubkey.clone())
            .or_default()
            .insert(event_id.clone());

        // Index tags
        for tag in &event.tags {
            if tag.is_empty() {
                continue;
            }
            let tag_name = &tag[0];
            if tag.len() >= 2 {
                let tag_value = &tag[1];
                self.by_tag
                    .entry(tag_name.clone())
                    .or_default()
                    .entry(tag_value.clone())
                    .or_default()
                    .insert(event_id.clone());
            }
        }

        // Add to main storage and LRU
        self.events.insert(event_id.clone(), event);
        self.lru_queue.push_back(event_id);

        Ok(())
    }

    /// Get an event by ID
    pub fn get(&mut self, event_id: &str) -> Option<&Event> {
        if !self.config.enabled {
            return None;
        }

        if self.events.contains_key(event_id) {
            // Update LRU
            self.lru_queue.retain(|id| id != event_id);
            self.lru_queue.push_back(event_id.to_string());
            return self.events.get(event_id);
        }

        None
    }

    /// Get events by kind
    pub fn get_by_kind(&self, kind: u16) -> Vec<&Event> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.by_kind
            .get(&kind)
            .map(|ids| ids.iter().filter_map(|id| self.events.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get events by author (pubkey)
    pub fn get_by_author(&self, pubkey: &str) -> Vec<&Event> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.by_author
            .get(pubkey)
            .map(|ids| ids.iter().filter_map(|id| self.events.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get events by tag
    pub fn get_by_tag(&self, tag_name: &str, tag_value: &str) -> Vec<&Event> {
        if !self.config.enabled {
            return Vec::new();
        }

        self.by_tag
            .get(tag_name)
            .and_then(|values| values.get(tag_value))
            .map(|ids| ids.iter().filter_map(|id| self.events.get(id)).collect())
            .unwrap_or_default()
    }

    /// Remove an event by ID
    pub fn remove(&mut self, event_id: &str) -> Option<Event> {
        if !self.config.enabled {
            return None;
        }

        self.remove_internal(event_id)
    }

    /// Internal remove without enabled check
    fn remove_internal(&mut self, event_id: &str) -> Option<Event> {
        let event = self.events.remove(event_id)?;

        // Remove from LRU
        self.lru_queue.retain(|id| id != event_id);

        // Remove from kind index
        if let Some(ids) = self.by_kind.get_mut(&event.kind) {
            ids.remove(event_id);
        }

        // Remove from author index
        if let Some(ids) = self.by_author.get_mut(&event.pubkey) {
            ids.remove(event_id);
        }

        // Remove from tag indexes
        for tag in &event.tags {
            if tag.len() >= 2 {
                let tag_name = &tag[0];
                let tag_value = &tag[1];
                if let Some(values) = self.by_tag.get_mut(tag_name)
                    && let Some(ids) = values.get_mut(tag_value)
                {
                    ids.remove(event_id);
                }
            }
        }

        // Remove from replaceable indexes
        if self.is_replaceable_kind(event.kind) {
            let key = (event.pubkey.clone(), event.kind);
            if self.replaceable.get(&key) == Some(&event.id) {
                self.replaceable.remove(&key);
            }
        }

        if self.is_param_replaceable_kind(event.kind)
            && let Some(d_tag) = self.get_d_tag(&event)
        {
            let key = (event.pubkey.clone(), event.kind, d_tag);
            if self.param_replaceable.get(&key) == Some(&event.id) {
                self.param_replaceable.remove(&key);
            }
        }

        Some(event)
    }

    /// Clear all events from cache
    pub fn clear(&mut self) {
        self.events.clear();
        self.lru_queue.clear();
        self.by_kind.clear();
        self.by_author.clear();
        self.by_tag.clear();
        self.replaceable.clear();
        self.param_replaceable.clear();
    }

    /// Get cache size (number of events)
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Check if kind is replaceable (NIP-16)
    fn is_replaceable_kind(&self, kind: u16) -> bool {
        kind == 0 || kind == 3 || (10000..=19999).contains(&kind)
    }

    /// Check if kind is parameterized replaceable (NIP-33)
    fn is_param_replaceable_kind(&self, kind: u16) -> bool {
        (30000..=39999).contains(&kind)
    }

    /// Get d-tag value from event
    fn get_d_tag(&self, event: &Event) -> Option<String> {
        for tag in &event.tags {
            if tag.len() >= 2 && tag[0] == "d" {
                return Some(tag[1].clone());
            }
        }
        None
    }
}

impl Default for EventCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(id: &str, pubkey: &str, kind: u16, created_at: u64) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind,
            tags: vec![],
            content: "test".to_string(),
            sig: "sig".to_string(),
        }
    }

    fn create_event_with_tag(
        id: &str,
        pubkey: &str,
        kind: u16,
        tag_name: &str,
        tag_value: &str,
    ) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1234567890,
            kind,
            tags: vec![vec![tag_name.to_string(), tag_value.to_string()]],
            content: "test".to_string(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_cache_insert_and_get() {
        let mut cache = EventCache::new();
        let event = create_test_event("id1", "pubkey1", 1, 1234567890);

        cache.insert(event.clone()).unwrap();
        assert_eq!(cache.len(), 1);

        let retrieved = cache.get("id1").unwrap();
        assert_eq!(retrieved.id, "id1");
    }

    #[test]
    fn test_cache_get_by_kind() {
        let mut cache = EventCache::new();
        cache
            .insert(create_test_event("id1", "pubkey1", 1, 1234567890))
            .unwrap();
        cache
            .insert(create_test_event("id2", "pubkey2", 1, 1234567891))
            .unwrap();
        cache
            .insert(create_test_event("id3", "pubkey3", 3, 1234567892))
            .unwrap();

        let kind1_events = cache.get_by_kind(1);
        assert_eq!(kind1_events.len(), 2);

        let kind3_events = cache.get_by_kind(3);
        assert_eq!(kind3_events.len(), 1);
    }

    #[test]
    fn test_cache_get_by_author() {
        let mut cache = EventCache::new();
        cache
            .insert(create_test_event("id1", "pubkey1", 1, 1234567890))
            .unwrap();
        cache
            .insert(create_test_event("id2", "pubkey1", 3, 1234567891))
            .unwrap();
        cache
            .insert(create_test_event("id3", "pubkey2", 1, 1234567892))
            .unwrap();

        let pubkey1_events = cache.get_by_author("pubkey1");
        assert_eq!(pubkey1_events.len(), 2);

        let pubkey2_events = cache.get_by_author("pubkey2");
        assert_eq!(pubkey2_events.len(), 1);
    }

    #[test]
    fn test_cache_get_by_tag() {
        let mut cache = EventCache::new();
        cache
            .insert(create_event_with_tag("id1", "pubkey1", 1, "e", "event123"))
            .unwrap();
        cache
            .insert(create_event_with_tag("id2", "pubkey2", 1, "e", "event123"))
            .unwrap();
        cache
            .insert(create_event_with_tag("id3", "pubkey3", 1, "p", "pubkey456"))
            .unwrap();

        let e_events = cache.get_by_tag("e", "event123");
        assert_eq!(e_events.len(), 2);

        let p_events = cache.get_by_tag("p", "pubkey456");
        assert_eq!(p_events.len(), 1);
    }

    #[test]
    fn test_cache_remove() {
        let mut cache = EventCache::new();
        let event = create_test_event("id1", "pubkey1", 1, 1234567890);

        cache.insert(event).unwrap();
        assert_eq!(cache.len(), 1);

        let removed = cache.remove("id1");
        assert!(removed.is_some());
        assert_eq!(cache.len(), 0);
        assert!(cache.get("id1").is_none());
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = EventCache::new();
        cache
            .insert(create_test_event("id1", "pubkey1", 1, 1234567890))
            .unwrap();
        cache
            .insert(create_test_event("id2", "pubkey2", 3, 1234567891))
            .unwrap();

        assert_eq!(cache.len(), 2);
        cache.clear();
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_lru_eviction() {
        let config = CacheConfig {
            max_events: 3,
            enabled: true,
        };
        let mut cache = EventCache::with_config(config);

        cache
            .insert(create_test_event("id1", "pubkey1", 1, 1234567890))
            .unwrap();
        cache
            .insert(create_test_event("id2", "pubkey2", 1, 1234567891))
            .unwrap();
        cache
            .insert(create_test_event("id3", "pubkey3", 1, 1234567892))
            .unwrap();
        assert_eq!(cache.len(), 3);

        // Insert 4th event, should evict oldest (id1)
        cache
            .insert(create_test_event("id4", "pubkey4", 1, 1234567893))
            .unwrap();
        assert_eq!(cache.len(), 3);
        assert!(cache.get("id1").is_none());
        assert!(cache.get("id2").is_some());
        assert!(cache.get("id3").is_some());
        assert!(cache.get("id4").is_some());
    }

    #[test]
    fn test_cache_replaceable_events() {
        let mut cache = EventCache::new();

        // Insert metadata event (kind 0)
        let event1 = create_test_event("id1", "pubkey1", 0, 1234567890);
        cache.insert(event1).unwrap();
        assert_eq!(cache.len(), 1);

        // Insert newer metadata event from same pubkey
        let event2 = create_test_event("id2", "pubkey1", 0, 1234567900);
        cache.insert(event2).unwrap();

        // Old event should be replaced
        assert_eq!(cache.len(), 1);
        assert!(cache.get("id1").is_none());
        assert!(cache.get("id2").is_some());
    }

    #[test]
    fn test_cache_replaceable_events_ignore_older() {
        let mut cache = EventCache::new();

        // Insert newer metadata event
        let event1 = create_test_event("id1", "pubkey1", 0, 1234567900);
        cache.insert(event1).unwrap();

        // Try to insert older metadata event from same pubkey
        let event2 = create_test_event("id2", "pubkey1", 0, 1234567890);
        cache.insert(event2).unwrap();

        // Old event should be ignored
        assert_eq!(cache.len(), 1);
        assert!(cache.get("id1").is_some());
        assert!(cache.get("id2").is_none());
    }

    #[test]
    fn test_cache_param_replaceable_events() {
        let mut cache = EventCache::new();

        // Create parameterized replaceable event (kind 30000) with d-tag
        let mut event1 = create_test_event("id1", "pubkey1", 30000, 1234567890);
        event1.tags = vec![vec!["d".to_string(), "article1".to_string()]];
        cache.insert(event1).unwrap();
        assert_eq!(cache.len(), 1);

        // Insert newer version with same d-tag
        let mut event2 = create_test_event("id2", "pubkey1", 30000, 1234567900);
        event2.tags = vec![vec!["d".to_string(), "article1".to_string()]];
        cache.insert(event2).unwrap();

        // Old event should be replaced
        assert_eq!(cache.len(), 1);
        assert!(cache.get("id1").is_none());
        assert!(cache.get("id2").is_some());

        // Different d-tag should not replace
        let mut event3 = create_test_event("id3", "pubkey1", 30000, 1234567910);
        event3.tags = vec![vec!["d".to_string(), "article2".to_string()]];
        cache.insert(event3).unwrap();
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_disabled() {
        let config = CacheConfig {
            max_events: 100,
            enabled: false,
        };
        let mut cache = EventCache::with_config(config);

        cache
            .insert(create_test_event("id1", "pubkey1", 1, 1234567890))
            .unwrap();
        assert_eq!(cache.len(), 0);
        assert!(cache.get("id1").is_none());
    }
}
