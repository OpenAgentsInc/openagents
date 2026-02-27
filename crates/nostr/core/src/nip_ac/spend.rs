//! Credit Spend Authorization (`kind:39243`).

use super::envelope::{CreditEnvelope, EnvelopeError};
use super::scope_hash::{ScopeHashError, ScopeReference};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Spend Authorization.
pub const KIND_CREDIT_SPEND_AUTH: u16 = 39243;

/// `content` payload for spend authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditSpendContent {
    pub schema: u32,
    pub spend_sats: u64,
    pub reason: Option<String>,
}

impl CreditSpendContent {
    pub fn new(spend_sats: u64) -> Self {
        Self {
            schema: 1,
            spend_sats,
            reason: None,
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Errors for spend-authorization operations.
#[derive(Debug, Error)]
pub enum SpendError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid max sats")]
    InvalidMaxSats,

    #[error("invalid expiry")]
    InvalidExpiry,

    #[error("invalid spend amount")]
    InvalidSpendAmount,

    #[error("envelope id mismatch: spend references {spend_envelope}, envelope is {envelope}")]
    EnvelopeIdMismatch {
        spend_envelope: String,
        envelope: String,
    },

    #[error(transparent)]
    Scope(#[from] ScopeHashError),

    #[error(transparent)]
    Envelope(#[from] EnvelopeError),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),
}

/// Spend-authorization event model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditSpendAuthorization {
    pub issuer_pubkey: String,
    pub envelope_id: String,
    pub scope: ScopeReference,
    pub max_sats: u64,
    pub expiry: u64,
    pub content: CreditSpendContent,
}

impl CreditSpendAuthorization {
    pub fn new(
        issuer_pubkey: impl Into<String>,
        envelope_id: impl Into<String>,
        scope: ScopeReference,
        max_sats: u64,
        expiry: u64,
        content: CreditSpendContent,
    ) -> Self {
        Self {
            issuer_pubkey: issuer_pubkey.into(),
            envelope_id: envelope_id.into(),
            scope,
            max_sats,
            expiry,
            content,
        }
    }

    pub fn validate(&self) -> Result<(), SpendError> {
        self.scope.validate()?;
        if self.issuer_pubkey.trim().is_empty() {
            return Err(SpendError::MissingRequiredTag("p"));
        }
        if self.envelope_id.trim().is_empty() {
            return Err(SpendError::MissingRequiredTag("credit"));
        }
        if self.max_sats == 0 {
            return Err(SpendError::InvalidMaxSats);
        }
        if self.expiry == 0 {
            return Err(SpendError::InvalidExpiry);
        }
        if self.content.spend_sats == 0 {
            return Err(SpendError::InvalidSpendAmount);
        }
        if self.content.spend_sats > self.max_sats {
            return Err(SpendError::InvalidSpendAmount);
        }
        Ok(())
    }

    /// Validate spend against an authoritative envelope.
    pub fn validate_against_envelope(
        &self,
        envelope: &CreditEnvelope,
        current_time: u64,
    ) -> Result<(), SpendError> {
        self.validate()?;
        if self.envelope_id != envelope.envelope_id {
            return Err(SpendError::EnvelopeIdMismatch {
                spend_envelope: self.envelope_id.clone(),
                envelope: envelope.envelope_id.clone(),
            });
        }
        envelope.validate_spend(self.content.spend_sats, current_time)?;
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, SpendError> {
        self.validate()?;
        Ok(vec![
            vec!["p".to_string(), self.issuer_pubkey.clone()],
            vec!["credit".to_string(), self.envelope_id.clone()],
            self.scope.to_scope_tag()?,
            vec!["max".to_string(), self.max_sats.to_string()],
            vec!["exp".to_string(), self.expiry.to_string()],
        ])
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, SpendError> {
        self.validate()?;
        let content = serde_json::to_string(&self.content)
            .map_err(|error| SpendError::Serialization(error.to_string()))?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_SPEND_AUTH,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, SpendError> {
        if event.kind != KIND_CREDIT_SPEND_AUTH {
            return Err(SpendError::InvalidKind {
                expected: KIND_CREDIT_SPEND_AUTH,
                actual: event.kind,
            });
        }

        let issuer_pubkey = find_required_tag_value(&event.tags, "p")?;
        let envelope_id = find_required_tag_value(&event.tags, "credit")?;
        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(SpendError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let max_sats = find_required_tag_value(&event.tags, "max")?
            .parse::<u64>()
            .map_err(|_| SpendError::InvalidMaxSats)?;
        let expiry = find_required_tag_value(&event.tags, "exp")?
            .parse::<u64>()
            .map_err(|_| SpendError::InvalidExpiry)?;
        let content: CreditSpendContent = serde_json::from_str(&event.content)
            .map_err(|error| SpendError::Deserialization(error.to_string()))?;

        let spend = CreditSpendAuthorization {
            issuer_pubkey,
            envelope_id,
            scope,
            max_sats,
            expiry,
            content,
        };
        spend.validate()?;
        Ok(spend)
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, SpendError> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .and_then(|tag| tag.get(1))
        .cloned()
        .ok_or(SpendError::MissingRequiredTag(tag_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_ac::{ScopeType, scope_hash::ScopeReference};

    #[test]
    fn test_credit_spend_roundtrip() {
        let spend = CreditSpendAuthorization::new(
            "issuer-pubkey",
            "envelope-1",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            35_000,
            1_703_003_600,
            CreditSpendContent::new(30_000).with_reason("run nip90 job"),
        );

        let template = spend.to_event_template(1_703_003_300).unwrap();
        let event = Event {
            id: "spend-id".to_string(),
            pubkey: "agent-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditSpendAuthorization::from_event(&event).unwrap();
        assert_eq!(parsed.envelope_id, "envelope-1");
        assert_eq!(parsed.content.spend_sats, 30_000);
    }
}
