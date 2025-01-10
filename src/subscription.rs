use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::event::Event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub filters: Vec<ReqFilter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReqFilter {
    pub ids: Option<Vec<String>>,
    pub authors: Option<Vec<String>>,
    pub kinds: Option<Vec<i32>>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: Option<u64>,
    #[serde(flatten)]
    pub tags: HashMap<String, Vec<String>>,
}

impl Subscription {
    pub fn interested_in_event(&self, event: &Event) -> bool {
        self.filters.iter().any(|f| f.matches_event(event))
    }
}

impl ReqFilter {
    pub fn matches_event(&self, event: &Event) -> bool {
        // Check basic fields
        if let Some(ids) = &self.ids {
            if !ids.iter().any(|id| event.id.starts_with(id)) {
                return false;
            }
        }

        if let Some(authors) = &self.authors {
            if !authors.iter().any(|author| event.pubkey.starts_with(author)) {
                return false;
            }
        }

        if let Some(kinds) = &self.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }

        if let Some(since) = self.since {
            if event.created_at < since {
                return false;
            }
        }

        if let Some(until) = self.until {
            if event.created_at > until {
                return false;
            }
        }

        // Check tags
        for (tag_name, tag_values) in &self.tags {
            if !tag_name.starts_with('#') {
                continue;
            }

            let tag_char = tag_name.chars().nth(1).unwrap();
            let tag_set: HashSet<_> = tag_values.iter().cloned().collect();

            if !event.generic_tag_val_intersect(tag_char, &tag_set) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_matching() {
        let mut event = Event {
            id: "abc123".into(),
            pubkey: "def456".into(),
            created_at: 1000,
            kind: 1,
            tags: vec![vec!["e".into(), "789".into()]],
            content: "test".into(),
            sig: "".into(),
            tagidx: None,
        };
        event.build_index();

        let filter = ReqFilter {
            ids: Some(vec!["abc".into()]),
            authors: Some(vec!["def".into()]),
            kinds: Some(vec![1]),
            since: Some(500),
            until: Some(1500),
            limit: None,
            tags: {
                let mut map = HashMap::new();
                map.insert("#e".into(), vec!["789".into()]);
                map
            },
        };

        assert!(filter.matches_event(&event));
    }

    #[test]
    fn test_filter_non_matching() {
        let mut event = Event {
            id: "abc123".into(),
            pubkey: "def456".into(),
            created_at: 1000,
            kind: 1,
            tags: vec![vec!["e".into(), "789".into()]],
            content: "test".into(),
            sig: "".into(),
            tagidx: None,
        };
        event.build_index();

        let filter = ReqFilter {
            ids: Some(vec!["xyz".into()]), // Won't match
            authors: Some(vec!["def".into()]),
            kinds: Some(vec![1]),
            since: Some(500),
            until: Some(1500),
            limit: None,
            tags: HashMap::new(),
        };

        assert!(!filter.matches_event(&event));
    }
}
