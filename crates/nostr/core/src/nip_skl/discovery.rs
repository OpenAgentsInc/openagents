//! Optional SKL discovery profile on NIP-90 request/result kinds.
//!
//! This module is intentionally optional and independent from SKL core:
//! - request `kind:5390`
//! - result `kind:6390`
//!
//! Result events MUST include `a` tags pointing to SKL manifests
//! (`33400:<skill-pubkey>:<d-tag>`).

use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

/// NIP-90 optional SKL search request kind.
pub const KIND_SKILL_SEARCH_REQUEST: u16 = 5390;

/// NIP-90 optional SKL search result kind.
pub const KIND_SKILL_SEARCH_RESULT: u16 = 6390;

/// Errors for optional SKL discovery profile.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("missing query parameter")]
    MissingQuery,

    #[error("missing request event reference")]
    MissingRequestReference,

    #[error("invalid SKL address: {0}")]
    InvalidSkillAddress(String),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),
}

/// Skill search request payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillSearchRequest {
    pub query: String,
    pub limit: Option<u32>,
    pub capabilities: Vec<String>,
    pub topics: Vec<String>,
    pub publisher_pubkey: Option<String>,
}

impl SkillSearchRequest {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            limit: None,
            capabilities: Vec::new(),
            topics: Vec::new(),
            publisher_pubkey: None,
        }
    }

    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.push(capability.into());
        self
    }

    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topics.push(topic.into());
        self
    }

    pub fn with_publisher(mut self, publisher_pubkey: impl Into<String>) -> Self {
        self.publisher_pubkey = Some(publisher_pubkey.into());
        self
    }

    pub fn validate(&self) -> Result<(), DiscoveryError> {
        if self.query.trim().is_empty() {
            return Err(DiscoveryError::MissingQuery);
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, DiscoveryError> {
        self.validate()?;

        let mut tags = vec![vec![
            "param".to_string(),
            "q".to_string(),
            self.query.clone(),
        ]];

        if let Some(limit) = self.limit {
            tags.push(vec![
                "param".to_string(),
                "limit".to_string(),
                limit.to_string(),
            ]);
        }

        for capability in canonicalized(&self.capabilities) {
            tags.push(vec![
                "param".to_string(),
                "capability".to_string(),
                capability,
            ]);
        }

        for topic in canonicalized(&self.topics) {
            tags.push(vec!["param".to_string(), "topic".to_string(), topic]);
        }

        if let Some(publisher_pubkey) = &self.publisher_pubkey {
            tags.push(vec!["p".to_string(), publisher_pubkey.clone()]);
        }

        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, DiscoveryError> {
        Ok(EventTemplate {
            created_at,
            kind: KIND_SKILL_SEARCH_REQUEST,
            tags: self.to_tags()?,
            content: String::new(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, DiscoveryError> {
        if event.kind != KIND_SKILL_SEARCH_REQUEST {
            return Err(DiscoveryError::InvalidKind {
                expected: KIND_SKILL_SEARCH_REQUEST,
                actual: event.kind,
            });
        }

        let query = find_param(&event.tags, "q").ok_or(DiscoveryError::MissingQuery)?;
        let limit = find_param(&event.tags, "limit").and_then(|value| value.parse::<u32>().ok());
        let capabilities = find_params(&event.tags, "capability");
        let topics = find_params(&event.tags, "topic");
        let publisher_pubkey = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("p"))
            .and_then(|tag| tag.get(1))
            .cloned();

        Ok(Self {
            query,
            limit,
            capabilities,
            topics,
            publisher_pubkey,
        })
    }
}

/// Single skill hit in search results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillSearchHit {
    pub manifest_address: String,
    pub manifest_event_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub summary: Option<String>,
    pub score: Option<f64>,
}

impl SkillSearchHit {
    pub fn new(manifest_address: impl Into<String>) -> Self {
        Self {
            manifest_address: manifest_address.into(),
            manifest_event_id: None,
            name: None,
            version: None,
            summary: None,
            score: None,
        }
    }

    pub fn with_manifest_event_id(mut self, manifest_event_id: impl Into<String>) -> Self {
        self.manifest_event_id = Some(manifest_event_id.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    pub fn with_score(mut self, score: f64) -> Self {
        self.score = Some(score);
        self
    }

    pub fn validate(&self) -> Result<(), DiscoveryError> {
        if !self.manifest_address.starts_with("33400:") {
            return Err(DiscoveryError::InvalidSkillAddress(
                self.manifest_address.clone(),
            ));
        }
        Ok(())
    }
}

/// Search result payload and tags.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillSearchResult {
    pub request_event_id: String,
    pub hits: Vec<SkillSearchHit>,
    pub total: Option<u32>,
}

impl SkillSearchResult {
    pub fn new(request_event_id: impl Into<String>, hits: Vec<SkillSearchHit>) -> Self {
        Self {
            request_event_id: request_event_id.into(),
            hits,
            total: None,
        }
    }

    pub fn with_total(mut self, total: u32) -> Self {
        self.total = Some(total);
        self
    }

    pub fn validate(&self) -> Result<(), DiscoveryError> {
        if self.request_event_id.trim().is_empty() {
            return Err(DiscoveryError::MissingRequestReference);
        }
        for hit in &self.hits {
            hit.validate()?;
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, DiscoveryError> {
        self.validate()?;

        let mut tags = vec![vec![
            "e".to_string(),
            self.request_event_id.clone(),
            String::new(),
            "request".to_string(),
        ]];

        for hit in &self.hits {
            tags.push(vec!["a".to_string(), hit.manifest_address.clone()]);
            if let Some(manifest_event_id) = &hit.manifest_event_id {
                tags.push(vec![
                    "e".to_string(),
                    manifest_event_id.clone(),
                    String::new(),
                    "manifest".to_string(),
                ]);
            }
        }

        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, DiscoveryError> {
        self.validate()?;
        let content = serde_json::to_string(self)
            .map_err(|error| DiscoveryError::Serialization(error.to_string()))?;

        Ok(EventTemplate {
            created_at,
            kind: KIND_SKILL_SEARCH_RESULT,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, DiscoveryError> {
        if event.kind != KIND_SKILL_SEARCH_RESULT {
            return Err(DiscoveryError::InvalidKind {
                expected: KIND_SKILL_SEARCH_RESULT,
                actual: event.kind,
            });
        }

        let request_event_id = event
            .tags
            .iter()
            .find(|tag| {
                tag.first().map(String::as_str) == Some("e")
                    && tag.get(3).map(String::as_str) == Some("request")
            })
            .and_then(|tag| tag.get(1))
            .cloned()
            .or_else(|| {
                event.tags.iter().find_map(|tag| {
                    if tag.first().map(String::as_str) == Some("e") {
                        tag.get(1).cloned()
                    } else {
                        None
                    }
                })
            })
            .ok_or(DiscoveryError::MissingRequestReference)?;

        let parsed_from_content = if event.content.trim().is_empty() {
            None
        } else {
            serde_json::from_str::<SkillSearchResult>(&event.content).ok()
        };

        let mut hits = parsed_from_content
            .as_ref()
            .map(|result| result.hits.clone())
            .unwrap_or_default();

        if hits.is_empty() {
            hits = event
                .tags
                .iter()
                .filter(|tag| tag.first().map(String::as_str) == Some("a"))
                .filter_map(|tag| tag.get(1))
                .cloned()
                .map(SkillSearchHit::new)
                .collect();
        }

        let total = parsed_from_content.as_ref().and_then(|result| result.total);
        let result = SkillSearchResult {
            request_event_id,
            hits,
            total,
        };
        result.validate()?;
        Ok(result)
    }
}

fn canonicalized(values: &[String]) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for value in values {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            unique.insert(trimmed.to_lowercase());
        }
    }
    unique.into_iter().collect()
}

fn find_param(tags: &[Vec<String>], key: &str) -> Option<String> {
    tags.iter().find_map(|tag| {
        if tag.first().map(String::as_str) == Some("param")
            && tag.get(1).map(String::as_str) == Some(key)
        {
            tag.get(2).cloned()
        } else {
            None
        }
    })
}

fn find_params(tags: &[Vec<String>], key: &str) -> Vec<String> {
    tags.iter()
        .filter_map(|tag| {
            if tag.first().map(String::as_str) == Some("param")
                && tag.get(1).map(String::as_str) == Some(key)
            {
                tag.get(2).cloned()
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_roundtrip() {
        let request = SkillSearchRequest::new("research assistant")
            .with_limit(25)
            .with_capability("http:outbound")
            .with_topic("agent-skill")
            .with_publisher("skillpub");

        let template = request.to_event_template(1_740_500_000).unwrap();
        let event = Event {
            id: "request-event-id".to_string(),
            pubkey: "searcher".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = SkillSearchRequest::from_event(&event).unwrap();
        assert_eq!(parsed.query, "research assistant");
        assert_eq!(parsed.limit, Some(25));
        assert_eq!(parsed.capabilities, vec!["http:outbound"]);
        assert_eq!(parsed.topics, vec!["agent-skill"]);
        assert_eq!(parsed.publisher_pubkey, Some("skillpub".to_string()));
    }

    #[test]
    fn test_result_roundtrip_and_a_tags() {
        let hits = vec![
            SkillSearchHit::new("33400:skillpub:research-assistant")
                .with_manifest_event_id("manifest-event-1")
                .with_name("Research Assistant")
                .with_version("1.4.2"),
            SkillSearchHit::new("33400:skillpub:data-compiler")
                .with_manifest_event_id("manifest-event-2"),
        ];
        let result = SkillSearchResult::new("request-event-id", hits).with_total(2);

        let template = result.to_event_template(1_740_500_100).unwrap();
        assert!(
            template
                .tags
                .iter()
                .any(|tag| tag.first().map(String::as_str) == Some("a")
                    && tag.get(1).map(String::as_str) == Some("33400:skillpub:research-assistant"))
        );

        let event = Event {
            id: "result-event-id".to_string(),
            pubkey: "indexer".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = SkillSearchResult::from_event(&event).unwrap();
        assert_eq!(parsed.request_event_id, "request-event-id");
        assert_eq!(parsed.hits.len(), 2);
        assert_eq!(
            parsed.hits[0].manifest_address,
            "33400:skillpub:research-assistant"
        );
    }

    #[test]
    fn test_result_validation_requires_skl_addresses() {
        let result =
            SkillSearchResult::new("request-event-id", vec![SkillSearchHit::new("bad-address")]);
        assert!(result.validate().is_err());
    }
}
