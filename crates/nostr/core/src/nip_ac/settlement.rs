//! Credit Settlement Receipt (`kind:39244`).

use super::envelope::RepaymentReference;
use super::scope_hash::{ScopeHashError, ScopeReference};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Settlement Receipt.
pub const KIND_CREDIT_SETTLEMENT: u16 = 39244;

/// `content` payload for settlement receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditSettlementContent {
    pub schema: u32,
    pub spent_sats: u64,
    pub fee_sats: u64,
    pub outcome: String,
    pub notes: Option<String>,
}

impl CreditSettlementContent {
    pub fn new(spent_sats: u64, fee_sats: u64, outcome: impl Into<String>) -> Self {
        Self {
            schema: 1,
            spent_sats,
            fee_sats,
            outcome: outcome.into(),
            notes: None,
        }
    }

    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// Errors for settlement-receipt operations.
#[derive(Debug, Error)]
pub enum SettlementError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid spent sats")]
    InvalidSpentSats,

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error(transparent)]
    Scope(#[from] ScopeHashError),
}

/// Settlement receipt model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditSettlementReceipt {
    pub envelope_id: String,
    pub agent_pubkey: String,
    pub issuer_pubkey: String,
    pub provider_pubkey: Option<String>,
    pub scope: ScopeReference,
    pub outcome_event_id: Option<String>,
    pub repayment: Option<RepaymentReference>,
    pub status: String,
    pub content: CreditSettlementContent,
}

impl CreditSettlementReceipt {
    pub fn new(
        envelope_id: impl Into<String>,
        agent_pubkey: impl Into<String>,
        issuer_pubkey: impl Into<String>,
        scope: ScopeReference,
        content: CreditSettlementContent,
    ) -> Self {
        Self {
            envelope_id: envelope_id.into(),
            agent_pubkey: agent_pubkey.into(),
            issuer_pubkey: issuer_pubkey.into(),
            provider_pubkey: None,
            scope,
            outcome_event_id: None,
            repayment: None,
            status: "settled".to_string(),
            content,
        }
    }

    pub fn with_provider(mut self, provider_pubkey: impl Into<String>) -> Self {
        self.provider_pubkey = Some(provider_pubkey.into());
        self
    }

    pub fn with_outcome_event(mut self, outcome_event_id: impl Into<String>) -> Self {
        self.outcome_event_id = Some(outcome_event_id.into());
        self
    }

    pub fn with_repayment(mut self, repayment: RepaymentReference) -> Self {
        self.repayment = Some(repayment);
        self
    }

    pub fn validate(&self) -> Result<(), SettlementError> {
        self.scope.validate()?;
        if self.envelope_id.trim().is_empty() {
            return Err(SettlementError::MissingRequiredTag("credit"));
        }
        if self.agent_pubkey.trim().is_empty() {
            return Err(SettlementError::MissingRequiredTag("p"));
        }
        if self.issuer_pubkey.trim().is_empty() {
            return Err(SettlementError::MissingRequiredTag("issuer"));
        }
        if self.content.spent_sats == 0 {
            return Err(SettlementError::InvalidSpentSats);
        }
        if self.status != "settled" {
            return Err(SettlementError::InvalidStatus(self.status.clone()));
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, SettlementError> {
        self.validate()?;
        let mut tags = vec![
            vec!["credit".to_string(), self.envelope_id.clone()],
            vec!["p".to_string(), self.agent_pubkey.clone()],
            vec!["issuer".to_string(), self.issuer_pubkey.clone()],
            self.scope.to_scope_tag()?,
            vec!["status".to_string(), self.status.clone()],
        ];

        if let Some(provider_pubkey) = &self.provider_pubkey {
            tags.push(vec!["provider".to_string(), provider_pubkey.clone()]);
        }
        if let Some(outcome_event_id) = &self.outcome_event_id {
            tags.push(vec!["e".to_string(), outcome_event_id.clone()]);
        }
        if let Some(repayment) = &self.repayment {
            tags.push(repayment.to_tag());
        }

        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, SettlementError> {
        self.validate()?;
        let content = serde_json::to_string(&self.content)
            .map_err(|error| SettlementError::Serialization(error.to_string()))?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_SETTLEMENT,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, SettlementError> {
        if event.kind != KIND_CREDIT_SETTLEMENT {
            return Err(SettlementError::InvalidKind {
                expected: KIND_CREDIT_SETTLEMENT,
                actual: event.kind,
            });
        }

        let envelope_id = find_required_tag_value(&event.tags, "credit")?;
        let agent_pubkey = find_required_tag_value(&event.tags, "p")?;
        let issuer_pubkey = find_required_tag_value(&event.tags, "issuer")?;
        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(SettlementError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let status = find_required_tag_value(&event.tags, "status")?;
        let provider_pubkey = find_tag_value(&event.tags, "provider");
        let outcome_event_id = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("e"))
            .and_then(|tag| tag.get(1))
            .cloned();
        let repayment = parse_repay_tag(&event.tags);
        let content: CreditSettlementContent = serde_json::from_str(&event.content)
            .map_err(|error| SettlementError::Deserialization(error.to_string()))?;

        let receipt = CreditSettlementReceipt {
            envelope_id,
            agent_pubkey,
            issuer_pubkey,
            provider_pubkey,
            scope,
            outcome_event_id,
            repayment,
            status,
            content,
        };
        receipt.validate()?;
        Ok(receipt)
    }
}

fn parse_repay_tag(tags: &[Vec<String>]) -> Option<RepaymentReference> {
    tags.iter().find_map(|tag| {
        if tag.first().map(String::as_str) == Some("repay") && tag.len() >= 3 {
            Some(RepaymentReference::new(tag[1].clone(), tag[2].clone()))
        } else {
            None
        }
    })
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, SettlementError> {
    find_tag_value(tags, tag_name).ok_or(SettlementError::MissingRequiredTag(tag_name))
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
    use crate::nip_ac::ScopeType;

    #[test]
    fn test_settlement_roundtrip() {
        let receipt = CreditSettlementReceipt::new(
            "envelope-1",
            "agent-pubkey",
            "issuer-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            CreditSettlementContent::new(31_200, 600, "success")
                .with_notes("objective verification passed"),
        )
        .with_provider("provider-pubkey")
        .with_outcome_event("result-event-id")
        .with_repayment(RepaymentReference::new("bolt11", "invoice-hash"));

        let template = receipt.to_event_template(1_703_003_800).unwrap();
        let event = Event {
            id: "settlement-id".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditSettlementReceipt::from_event(&event).unwrap();
        assert_eq!(parsed.status, "settled");
        assert_eq!(parsed.content.spent_sats, 31_200);
        assert_eq!(parsed.outcome_event_id, Some("result-event-id".to_string()));
    }
}
