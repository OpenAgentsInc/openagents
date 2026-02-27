//! Credit Intent (`kind:39240`).

use super::scope_hash::{ScopeHashError, ScopeReference};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Intent.
pub const KIND_CREDIT_INTENT: u16 = 39240;

/// Errors for credit-intent operations.
#[derive(Debug, Error)]
pub enum IntentError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid max sats")]
    InvalidMaxSats,

    #[error("invalid expiry")]
    InvalidExpiry,

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error(transparent)]
    Scope(#[from] ScopeHashError),
}

/// `content` payload for credit intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditIntentContent {
    pub schema: u32,
    pub need: String,
    pub estimate_sats: u64,
    pub deadline: Option<u64>,
    pub notes: Option<String>,
}

impl CreditIntentContent {
    pub fn new(need: impl Into<String>, estimate_sats: u64) -> Self {
        Self {
            schema: 1,
            need: need.into(),
            estimate_sats,
            deadline: None,
            notes: None,
        }
    }

    pub fn with_deadline(mut self, deadline: u64) -> Self {
        self.deadline = Some(deadline);
        self
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// Credit-intent event model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditIntent {
    pub scope: ScopeReference,
    pub max_sats: u64,
    pub expiry: u64,
    pub provider_pubkey: Option<String>,
    pub content: CreditIntentContent,
}

impl CreditIntent {
    pub fn new(
        scope: ScopeReference,
        max_sats: u64,
        expiry: u64,
        content: CreditIntentContent,
    ) -> Self {
        Self {
            scope,
            max_sats,
            expiry,
            provider_pubkey: None,
            content,
        }
    }

    pub fn with_provider(mut self, provider_pubkey: impl Into<String>) -> Self {
        self.provider_pubkey = Some(provider_pubkey.into());
        self
    }

    pub fn validate(&self) -> Result<(), IntentError> {
        self.scope.validate()?;
        if self.max_sats == 0 {
            return Err(IntentError::InvalidMaxSats);
        }
        if self.expiry == 0 {
            return Err(IntentError::InvalidExpiry);
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, IntentError> {
        self.validate()?;
        let mut tags = vec![
            self.scope.to_scope_tag()?,
            vec!["max".to_string(), self.max_sats.to_string()],
            vec!["exp".to_string(), self.expiry.to_string()],
        ];
        if let Some(provider_pubkey) = &self.provider_pubkey {
            tags.push(vec!["provider".to_string(), provider_pubkey.clone()]);
        }
        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, IntentError> {
        self.validate()?;
        let content = serde_json::to_string(&self.content)
            .map_err(|error| IntentError::Serialization(error.to_string()))?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_INTENT,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, IntentError> {
        if event.kind != KIND_CREDIT_INTENT {
            return Err(IntentError::InvalidKind {
                expected: KIND_CREDIT_INTENT,
                actual: event.kind,
            });
        }

        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(IntentError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let max_sats = find_required_tag_value(&event.tags, "max")?
            .parse::<u64>()
            .map_err(|_| IntentError::InvalidMaxSats)?;
        let expiry = find_required_tag_value(&event.tags, "exp")?
            .parse::<u64>()
            .map_err(|_| IntentError::InvalidExpiry)?;
        let provider_pubkey = find_tag_value(&event.tags, "provider");

        let content: CreditIntentContent = serde_json::from_str(&event.content)
            .map_err(|error| IntentError::Deserialization(error.to_string()))?;

        let intent = CreditIntent {
            scope,
            max_sats,
            expiry,
            provider_pubkey,
            content,
        };
        intent.validate()?;
        Ok(intent)
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, IntentError> {
    find_tag_value(tags, tag_name).ok_or(IntentError::MissingRequiredTag(tag_name))
}

fn find_tag_value(tags: &[Vec<String>], tag_name: &str) -> Option<String> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .and_then(|tag| tag.get(1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_ac::scope_hash::ScopeType;

    #[test]
    fn test_credit_intent_roundtrip() {
        let intent = CreditIntent::new(
            ScopeReference::new(ScopeType::Nip90, "job-request-id"),
            35_000,
            1_703_003_600,
            CreditIntentContent::new("compute", 30_000).with_notes("run tests + index"),
        )
        .with_provider("provider-pubkey");

        let template = intent.to_event_template(1_703_003_000).unwrap();
        let event = Event {
            id: "intent-id".to_string(),
            pubkey: "agent-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditIntent::from_event(&event).unwrap();
        assert_eq!(parsed.max_sats, 35_000);
        assert_eq!(parsed.scope.scope_type, ScopeType::Nip90);
        assert_eq!(parsed.provider_pubkey, Some("provider-pubkey".to_string()));
    }
}
