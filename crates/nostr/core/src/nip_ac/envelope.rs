//! Credit Envelope (`kind:39242`) and authority state machine.

use super::scope_hash::{ScopeHashError, ScopeReference};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Envelope.
pub const KIND_CREDIT_ENVELOPE: u16 = 39242;

/// Envelope status lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnvelopeStatus {
    Offered,
    Accepted,
    Revoked,
    Spent,
    Settled,
    Defaulted,
}

impl EnvelopeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Revoked => "revoked",
            Self::Spent => "spent",
            Self::Settled => "settled",
            Self::Defaulted => "defaulted",
        }
    }

    pub fn parse(value: &str) -> Result<Self, EnvelopeError> {
        match value.to_lowercase().as_str() {
            "offered" => Ok(Self::Offered),
            "accepted" => Ok(Self::Accepted),
            "revoked" => Ok(Self::Revoked),
            "spent" => Ok(Self::Spent),
            "settled" => Ok(Self::Settled),
            "defaulted" => Ok(Self::Defaulted),
            _ => Err(EnvelopeError::InvalidStatus(value.to_string())),
        }
    }
}

/// Optional repayment rail pointer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepaymentReference {
    pub rail: String,
    pub reference: String,
}

impl RepaymentReference {
    pub fn new(rail: impl Into<String>, reference: impl Into<String>) -> Self {
        Self {
            rail: rail.into(),
            reference: reference.into(),
        }
    }

    pub fn to_tag(&self) -> Vec<String> {
        vec![
            "repay".to_string(),
            self.rail.clone(),
            self.reference.clone(),
        ]
    }
}

/// Errors for envelope/state-machine operations.
#[derive(Debug, Error)]
pub enum EnvelopeError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid envelope status: {0}")]
    InvalidStatus(String),

    #[error("invalid max sats")]
    InvalidMaxSats,

    #[error("invalid expiry")]
    InvalidExpiry,

    #[error("envelope expired at {expiry}, current time {current_time}")]
    Expired { expiry: u64, current_time: u64 },

    #[error("spend amount {amount} exceeds cap {cap}")]
    SpendExceedsCap { amount: u64, cap: u64 },

    #[error("envelope authority mismatch: expected d={expected}, got d={actual}")]
    EnvelopeIdMismatch { expected: String, actual: String },

    #[error("invalid envelope transition: {from:?} -> {to:?}")]
    InvalidTransition {
        from: EnvelopeStatus,
        to: EnvelopeStatus,
    },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error(transparent)]
    Scope(#[from] ScopeHashError),
}

/// AC envelope model (`kind:39242`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditEnvelope {
    pub envelope_id: String,
    pub agent_pubkey: String,
    pub issuer_pubkey: String,
    pub lp_pubkey: Option<String>,
    pub scope: ScopeReference,
    pub provider_pubkey: Option<String>,
    pub max_sats: u64,
    pub expiry: u64,
    pub status: EnvelopeStatus,
    pub repayment: Option<RepaymentReference>,
    /// Stringified terms JSON.
    pub terms_json: String,
}

impl CreditEnvelope {
    pub fn new(
        envelope_id: impl Into<String>,
        agent_pubkey: impl Into<String>,
        issuer_pubkey: impl Into<String>,
        scope: ScopeReference,
        max_sats: u64,
        expiry: u64,
    ) -> Self {
        Self {
            envelope_id: envelope_id.into(),
            agent_pubkey: agent_pubkey.into(),
            issuer_pubkey: issuer_pubkey.into(),
            lp_pubkey: None,
            scope,
            provider_pubkey: None,
            max_sats,
            expiry,
            status: EnvelopeStatus::Accepted,
            repayment: None,
            terms_json: "{}".to_string(),
        }
    }

    pub fn with_lp(mut self, lp_pubkey: impl Into<String>) -> Self {
        self.lp_pubkey = Some(lp_pubkey.into());
        self
    }

    pub fn with_provider(mut self, provider_pubkey: impl Into<String>) -> Self {
        self.provider_pubkey = Some(provider_pubkey.into());
        self
    }

    pub fn with_status(mut self, status: EnvelopeStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_repayment(mut self, repayment: RepaymentReference) -> Self {
        self.repayment = Some(repayment);
        self
    }

    pub fn with_terms_json(mut self, terms_json: impl Into<String>) -> Self {
        self.terms_json = terms_json.into();
        self
    }

    pub fn validate(&self) -> Result<(), EnvelopeError> {
        self.scope.validate()?;
        if self.envelope_id.trim().is_empty() {
            return Err(EnvelopeError::MissingRequiredTag("d"));
        }
        if self.agent_pubkey.trim().is_empty() {
            return Err(EnvelopeError::MissingRequiredTag("p"));
        }
        if self.issuer_pubkey.trim().is_empty() {
            return Err(EnvelopeError::MissingRequiredTag("issuer"));
        }
        if self.max_sats == 0 {
            return Err(EnvelopeError::InvalidMaxSats);
        }
        if self.expiry == 0 {
            return Err(EnvelopeError::InvalidExpiry);
        }
        Ok(())
    }

    /// Enforce envelope cap and expiry at spend time.
    pub fn validate_spend(&self, spend_sats: u64, current_time: u64) -> Result<(), EnvelopeError> {
        if current_time >= self.expiry {
            return Err(EnvelopeError::Expired {
                expiry: self.expiry,
                current_time,
            });
        }
        if spend_sats > self.max_sats {
            return Err(EnvelopeError::SpendExceedsCap {
                amount: spend_sats,
                cap: self.max_sats,
            });
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, EnvelopeError> {
        self.validate()?;
        let mut tags = vec![
            vec!["d".to_string(), self.envelope_id.clone()],
            vec!["p".to_string(), self.agent_pubkey.clone()],
            vec!["issuer".to_string(), self.issuer_pubkey.clone()],
            self.scope.to_scope_tag()?,
            vec!["max".to_string(), self.max_sats.to_string()],
            vec!["exp".to_string(), self.expiry.to_string()],
            vec!["status".to_string(), self.status.as_str().to_string()],
        ];
        if let Some(lp_pubkey) = &self.lp_pubkey {
            tags.push(vec!["lp".to_string(), lp_pubkey.clone()]);
        }
        if let Some(provider_pubkey) = &self.provider_pubkey {
            tags.push(vec!["provider".to_string(), provider_pubkey.clone()]);
        }
        if let Some(repayment) = &self.repayment {
            tags.push(repayment.to_tag());
        }
        Ok(tags)
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, EnvelopeError> {
        self.validate()?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_ENVELOPE,
            tags: self.to_tags()?,
            content: self.terms_json.clone(),
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, EnvelopeError> {
        if event.kind != KIND_CREDIT_ENVELOPE {
            return Err(EnvelopeError::InvalidKind {
                expected: KIND_CREDIT_ENVELOPE,
                actual: event.kind,
            });
        }

        let envelope_id = find_required_tag_value(&event.tags, "d")?;
        let agent_pubkey = find_required_tag_value(&event.tags, "p")?;
        let issuer_pubkey = find_required_tag_value(&event.tags, "issuer")?;
        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(EnvelopeError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let max_sats = find_required_tag_value(&event.tags, "max")?
            .parse::<u64>()
            .map_err(|_| EnvelopeError::InvalidMaxSats)?;
        let expiry = find_required_tag_value(&event.tags, "exp")?
            .parse::<u64>()
            .map_err(|_| EnvelopeError::InvalidExpiry)?;
        let status = EnvelopeStatus::parse(&find_required_tag_value(&event.tags, "status")?)?;
        let lp_pubkey = find_tag_value(&event.tags, "lp");
        let provider_pubkey = find_tag_value(&event.tags, "provider");
        let repayment = parse_repay_tag(&event.tags);

        let envelope = CreditEnvelope {
            envelope_id,
            agent_pubkey,
            issuer_pubkey,
            lp_pubkey,
            scope,
            provider_pubkey,
            max_sats,
            expiry,
            status,
            repayment,
            terms_json: event.content.clone(),
        };
        envelope.validate()?;
        Ok(envelope)
    }
}

/// Authority record for the currently active envelope head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeRecord {
    pub envelope: CreditEnvelope,
    pub event_id: String,
    pub created_at: u64,
}

/// Deterministic authority state for addressable envelope replacement.
#[derive(Debug, Default, Clone)]
pub struct EnvelopeAuthorityState {
    current: Option<EnvelopeRecord>,
}

impl EnvelopeAuthorityState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> Option<&EnvelopeRecord> {
        self.current.as_ref()
    }

    /// Apply a candidate `kind:39242` event.
    ///
    /// Returns:
    /// - `Ok(true)` if current head changed
    /// - `Ok(false)` if candidate is stale and ignored
    pub fn apply_event(&mut self, event: &Event) -> Result<bool, EnvelopeError> {
        let candidate_envelope = CreditEnvelope::from_event(event)?;
        if let Some(current) = &self.current {
            if current.envelope.envelope_id != candidate_envelope.envelope_id {
                return Err(EnvelopeError::EnvelopeIdMismatch {
                    expected: current.envelope.envelope_id.clone(),
                    actual: candidate_envelope.envelope_id,
                });
            }

            if !is_newer(
                event.created_at,
                &event.id,
                current.created_at,
                &current.event_id,
            ) {
                return Ok(false);
            }

            ensure_transition_allowed(current.envelope.status, candidate_envelope.status)?;
        }

        self.current = Some(EnvelopeRecord {
            envelope: candidate_envelope,
            event_id: event.id.clone(),
            created_at: event.created_at,
        });
        Ok(true)
    }
}

fn ensure_transition_allowed(
    from: EnvelopeStatus,
    to: EnvelopeStatus,
) -> Result<(), EnvelopeError> {
    let allowed = match from {
        EnvelopeStatus::Offered => matches!(
            to,
            EnvelopeStatus::Offered | EnvelopeStatus::Accepted | EnvelopeStatus::Revoked
        ),
        EnvelopeStatus::Accepted => matches!(
            to,
            EnvelopeStatus::Accepted
                | EnvelopeStatus::Spent
                | EnvelopeStatus::Revoked
                | EnvelopeStatus::Settled
                | EnvelopeStatus::Defaulted
        ),
        EnvelopeStatus::Spent => {
            matches!(
                to,
                EnvelopeStatus::Spent | EnvelopeStatus::Settled | EnvelopeStatus::Defaulted
            )
        }
        EnvelopeStatus::Settled => matches!(to, EnvelopeStatus::Settled),
        EnvelopeStatus::Defaulted => matches!(to, EnvelopeStatus::Defaulted),
        EnvelopeStatus::Revoked => matches!(to, EnvelopeStatus::Revoked),
    };

    if allowed {
        Ok(())
    } else {
        Err(EnvelopeError::InvalidTransition { from, to })
    }
}

fn is_newer(
    candidate_created_at: u64,
    candidate_event_id: &str,
    current_created_at: u64,
    current_event_id: &str,
) -> bool {
    (candidate_created_at, candidate_event_id) > (current_created_at, current_event_id)
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
) -> Result<String, EnvelopeError> {
    find_tag_value(tags, tag_name).ok_or(EnvelopeError::MissingRequiredTag(tag_name))
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
    fn test_credit_envelope_roundtrip() {
        let envelope = CreditEnvelope::new(
            "envelope-1",
            "agent-pubkey",
            "issuer-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            35_000,
            1_703_003_600,
        )
        .with_lp("lp-pubkey")
        .with_provider("provider-pubkey")
        .with_status(EnvelopeStatus::Accepted)
        .with_repayment(RepaymentReference::new("zap", "zap-ref"));

        let template = envelope.to_event_template(1_703_003_100).unwrap();
        let event = Event {
            id: "event-a".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditEnvelope::from_event(&event).unwrap();
        assert_eq!(parsed.envelope_id, "envelope-1");
        assert_eq!(parsed.status, EnvelopeStatus::Accepted);
        assert_eq!(
            parsed.repayment,
            Some(RepaymentReference::new("zap", "zap-ref"))
        );
    }

    #[test]
    fn test_envelope_authority_state_replacement() {
        let mut state = EnvelopeAuthorityState::new();
        let offered = CreditEnvelope::new(
            "envelope-1",
            "agent-pubkey",
            "issuer-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            35_000,
            1_703_003_600,
        )
        .with_status(EnvelopeStatus::Offered);
        let offered_event = Event {
            id: "event-a".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: 10,
            kind: KIND_CREDIT_ENVELOPE,
            tags: offered.to_tags().unwrap(),
            content: "{}".to_string(),
            sig: "sig".to_string(),
        };

        let accepted = offered.clone().with_status(EnvelopeStatus::Accepted);
        let accepted_event = Event {
            id: "event-b".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: 11,
            kind: KIND_CREDIT_ENVELOPE,
            tags: accepted.to_tags().unwrap(),
            content: "{}".to_string(),
            sig: "sig".to_string(),
        };

        assert!(state.apply_event(&offered_event).unwrap());
        assert!(state.apply_event(&accepted_event).unwrap());
        assert_eq!(
            state.current().unwrap().envelope.status,
            EnvelopeStatus::Accepted
        );
    }

    #[test]
    fn test_envelope_authority_rejects_invalid_transition() {
        let mut state = EnvelopeAuthorityState::new();

        let base = CreditEnvelope::new(
            "envelope-1",
            "agent-pubkey",
            "issuer-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            35_000,
            1_703_003_600,
        )
        .with_status(EnvelopeStatus::Offered);

        let first = Event {
            id: "event-a".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: 10,
            kind: KIND_CREDIT_ENVELOPE,
            tags: base.to_tags().unwrap(),
            content: "{}".to_string(),
            sig: "sig".to_string(),
        };
        state.apply_event(&first).unwrap();

        let invalid = base.with_status(EnvelopeStatus::Settled);
        let invalid_event = Event {
            id: "event-b".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: 11,
            kind: KIND_CREDIT_ENVELOPE,
            tags: invalid.to_tags().unwrap(),
            content: "{}".to_string(),
            sig: "sig".to_string(),
        };

        let error = state.apply_event(&invalid_event).unwrap_err();
        assert!(matches!(error, EnvelopeError::InvalidTransition { .. }));
    }
}
