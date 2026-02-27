//! Credit Offer (`kind:39241`).

use super::scope_hash::{ScopeHashError, ScopeReference, validate_skill_scope_links};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Offer.
pub const KIND_CREDIT_OFFER: u16 = 39241;

/// Offer lifecycle status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OfferStatus {
    Offered,
    Accepted,
    Revoked,
}

impl OfferStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
        }
    }

    pub fn parse(value: &str) -> Result<Self, OfferError> {
        match value.to_lowercase().as_str() {
            "offered" => Ok(Self::Offered),
            "accepted" => Ok(Self::Accepted),
            "revoked" => Ok(Self::Revoked),
            _ => Err(OfferError::InvalidStatus(value.to_string())),
        }
    }
}

/// `content` payload for credit offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditOfferContent {
    pub schema: u32,
    pub max_sats: u64,
    pub fee: String,
    pub requires_verifier: bool,
}

impl CreditOfferContent {
    pub fn new(max_sats: u64, fee: impl Into<String>) -> Self {
        Self {
            schema: 1,
            max_sats,
            fee: fee.into(),
            requires_verifier: false,
        }
    }

    pub fn requires_verifier(mut self, requires_verifier: bool) -> Self {
        self.requires_verifier = requires_verifier;
        self
    }
}

/// Errors for credit-offer operations.
#[derive(Debug, Error)]
pub enum OfferError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid max sats")]
    InvalidMaxSats,

    #[error("invalid expiry")]
    InvalidExpiry,

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error(transparent)]
    Scope(#[from] ScopeHashError),
}

/// Credit-offer event model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditOffer {
    pub agent_pubkey: String,
    pub scope: ScopeReference,
    pub skill_address: Option<String>,
    pub manifest_event_id: Option<String>,
    pub max_sats: u64,
    pub fee: String,
    pub expiry: u64,
    pub issuer_pubkey: String,
    pub lp_pubkey: Option<String>,
    pub status: OfferStatus,
    pub content: CreditOfferContent,
}

impl CreditOffer {
    pub fn new(
        agent_pubkey: impl Into<String>,
        scope: ScopeReference,
        max_sats: u64,
        fee: impl Into<String>,
        expiry: u64,
        issuer_pubkey: impl Into<String>,
        content: CreditOfferContent,
    ) -> Self {
        Self {
            agent_pubkey: agent_pubkey.into(),
            scope,
            skill_address: None,
            manifest_event_id: None,
            max_sats,
            fee: fee.into(),
            expiry,
            issuer_pubkey: issuer_pubkey.into(),
            lp_pubkey: None,
            status: OfferStatus::Offered,
            content,
        }
    }

    pub fn with_lp(mut self, lp_pubkey: impl Into<String>) -> Self {
        self.lp_pubkey = Some(lp_pubkey.into());
        self
    }

    pub fn with_status(mut self, status: OfferStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_skill_reference(
        mut self,
        skill_address: impl Into<String>,
        manifest_event_id: impl Into<String>,
    ) -> Self {
        self.skill_address = Some(skill_address.into());
        self.manifest_event_id = Some(manifest_event_id.into());
        self
    }

    pub fn validate(&self) -> Result<(), OfferError> {
        self.scope.validate()?;
        validate_skill_scope_links(
            &self.scope,
            self.skill_address.as_deref(),
            self.manifest_event_id.as_deref(),
        )?;
        if self.max_sats == 0 {
            return Err(OfferError::InvalidMaxSats);
        }
        if self.expiry == 0 {
            return Err(OfferError::InvalidExpiry);
        }
        if self.agent_pubkey.trim().is_empty() {
            return Err(OfferError::MissingRequiredTag("p"));
        }
        if self.issuer_pubkey.trim().is_empty() {
            return Err(OfferError::MissingRequiredTag("issuer"));
        }
        if self.fee.trim().is_empty() {
            return Err(OfferError::MissingRequiredTag("fee"));
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, OfferError> {
        self.validate()?;
        let mut tags = vec![
            vec!["p".to_string(), self.agent_pubkey.clone()],
            self.scope.to_scope_tag()?,
            vec!["max".to_string(), self.max_sats.to_string()],
            vec!["fee".to_string(), self.fee.clone()],
            vec!["exp".to_string(), self.expiry.to_string()],
            vec!["issuer".to_string(), self.issuer_pubkey.clone()],
            vec!["status".to_string(), self.status.as_str().to_string()],
        ];
        if let Some(lp_pubkey) = &self.lp_pubkey {
            tags.push(vec!["lp".to_string(), lp_pubkey.clone()]);
        }
        if let Some(skill_address) = &self.skill_address {
            tags.push(vec!["a".to_string(), skill_address.clone()]);
        }
        if let Some(manifest_event_id) = &self.manifest_event_id {
            tags.push(vec!["e".to_string(), manifest_event_id.clone()]);
        }
        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, OfferError> {
        self.validate()?;
        let content = serde_json::to_string(&self.content)
            .map_err(|error| OfferError::Serialization(error.to_string()))?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_OFFER,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, OfferError> {
        if event.kind != KIND_CREDIT_OFFER {
            return Err(OfferError::InvalidKind {
                expected: KIND_CREDIT_OFFER,
                actual: event.kind,
            });
        }

        let agent_pubkey = find_required_tag_value(&event.tags, "p")?;
        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(OfferError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let max_sats = find_required_tag_value(&event.tags, "max")?
            .parse::<u64>()
            .map_err(|_| OfferError::InvalidMaxSats)?;
        let fee = find_required_tag_value(&event.tags, "fee")?;
        let expiry = find_required_tag_value(&event.tags, "exp")?
            .parse::<u64>()
            .map_err(|_| OfferError::InvalidExpiry)?;
        let issuer_pubkey = find_required_tag_value(&event.tags, "issuer")?;
        let lp_pubkey = find_tag_value(&event.tags, "lp");
        let skill_address = find_tag_value(&event.tags, "a");
        let manifest_event_id = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("e"))
            .and_then(|tag| tag.get(1))
            .cloned();
        let status = OfferStatus::parse(&find_required_tag_value(&event.tags, "status")?)?;
        let content: CreditOfferContent = serde_json::from_str(&event.content)
            .map_err(|error| OfferError::Deserialization(error.to_string()))?;

        let offer = CreditOffer {
            agent_pubkey,
            scope,
            skill_address,
            manifest_event_id,
            max_sats,
            fee,
            expiry,
            issuer_pubkey,
            lp_pubkey,
            status,
            content,
        };
        offer.validate()?;
        Ok(offer)
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, OfferError> {
    find_tag_value(tags, tag_name).ok_or(OfferError::MissingRequiredTag(tag_name))
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
    fn test_credit_offer_roundtrip() {
        let offer = CreditOffer::new(
            "agent-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            35_000,
            "200bps",
            1_703_003_400,
            "issuer-pubkey",
            CreditOfferContent::new(35_000, "200bps"),
        )
        .with_lp("lp-pubkey");

        let template = offer.to_event_template(1_703_003_200).unwrap();
        let event = Event {
            id: "offer-id".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditOffer::from_event(&event).unwrap();
        assert_eq!(parsed.status, OfferStatus::Offered);
        assert_eq!(parsed.agent_pubkey, "agent-pubkey");
        assert_eq!(parsed.lp_pubkey, Some("lp-pubkey".to_string()));
    }
}
