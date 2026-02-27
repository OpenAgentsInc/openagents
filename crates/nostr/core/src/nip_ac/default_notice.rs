//! Credit Default Notice (`kind:39245`).

use super::scope_hash::{ScopeHashError, ScopeReference};
use crate::nip01::{Event, EventTemplate};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// AC kind: Credit Default Notice.
pub const KIND_CREDIT_DEFAULT_NOTICE: u16 = 39245;

/// `content` payload for default notices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditDefaultContent {
    pub schema: u32,
    pub reason: String,
    pub loss_sats: u64,
}

impl CreditDefaultContent {
    pub fn new(reason: impl Into<String>, loss_sats: u64) -> Self {
        Self {
            schema: 1,
            reason: reason.into(),
            loss_sats,
        }
    }
}

/// Errors for default-notice operations.
#[derive(Debug, Error)]
pub enum DefaultNoticeError {
    #[error("missing required tag: {0}")]
    MissingRequiredTag(&'static str),

    #[error("invalid event kind: expected {expected}, got {actual}")]
    InvalidKind { expected: u16, actual: u16 },

    #[error("invalid status: {0}")]
    InvalidStatus(String),

    #[error("invalid loss sats")]
    InvalidLossSats,

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("deserialization error: {0}")]
    Deserialization(String),

    #[error(transparent)]
    Scope(#[from] ScopeHashError),
}

/// Default-notice event model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditDefaultNotice {
    pub envelope_id: String,
    pub agent_pubkey: String,
    pub scope: ScopeReference,
    pub status: String,
    pub content: CreditDefaultContent,
}

impl CreditDefaultNotice {
    pub fn new(
        envelope_id: impl Into<String>,
        agent_pubkey: impl Into<String>,
        scope: ScopeReference,
        content: CreditDefaultContent,
    ) -> Self {
        Self {
            envelope_id: envelope_id.into(),
            agent_pubkey: agent_pubkey.into(),
            scope,
            status: "defaulted".to_string(),
            content,
        }
    }

    pub fn validate(&self) -> Result<(), DefaultNoticeError> {
        self.scope.validate()?;
        if self.envelope_id.trim().is_empty() {
            return Err(DefaultNoticeError::MissingRequiredTag("credit"));
        }
        if self.agent_pubkey.trim().is_empty() {
            return Err(DefaultNoticeError::MissingRequiredTag("p"));
        }
        if self.status != "defaulted" {
            return Err(DefaultNoticeError::InvalidStatus(self.status.clone()));
        }
        if self.content.loss_sats == 0 {
            return Err(DefaultNoticeError::InvalidLossSats);
        }
        if self.content.reason.trim().is_empty() {
            return Err(DefaultNoticeError::Deserialization(
                "reason cannot be empty".to_string(),
            ));
        }
        Ok(())
    }

    pub fn to_tags(&self) -> Result<Vec<Vec<String>>, DefaultNoticeError> {
        self.validate()?;
        Ok(vec![
            vec!["credit".to_string(), self.envelope_id.clone()],
            vec!["p".to_string(), self.agent_pubkey.clone()],
            self.scope.to_scope_tag()?,
            vec!["status".to_string(), self.status.clone()],
        ])
    }

    pub fn to_event_template(&self, created_at: u64) -> Result<EventTemplate, DefaultNoticeError> {
        self.validate()?;
        let content = serde_json::to_string(&self.content)
            .map_err(|error| DefaultNoticeError::Serialization(error.to_string()))?;
        Ok(EventTemplate {
            created_at,
            kind: KIND_CREDIT_DEFAULT_NOTICE,
            tags: self.to_tags()?,
            content,
        })
    }

    pub fn from_event(event: &Event) -> Result<Self, DefaultNoticeError> {
        if event.kind != KIND_CREDIT_DEFAULT_NOTICE {
            return Err(DefaultNoticeError::InvalidKind {
                expected: KIND_CREDIT_DEFAULT_NOTICE,
                actual: event.kind,
            });
        }

        let envelope_id = find_required_tag_value(&event.tags, "credit")?;
        let agent_pubkey = find_required_tag_value(&event.tags, "p")?;
        let scope_tag = event
            .tags
            .iter()
            .find(|tag| tag.first().map(String::as_str) == Some("scope"))
            .ok_or(DefaultNoticeError::MissingRequiredTag("scope"))?;
        let scope = ScopeReference::from_scope_tag(scope_tag)?;
        let status = find_required_tag_value(&event.tags, "status")?;
        let content: CreditDefaultContent = serde_json::from_str(&event.content)
            .map_err(|error| DefaultNoticeError::Deserialization(error.to_string()))?;

        let notice = CreditDefaultNotice {
            envelope_id,
            agent_pubkey,
            scope,
            status,
            content,
        };
        notice.validate()?;
        Ok(notice)
    }
}

fn find_required_tag_value(
    tags: &[Vec<String>],
    tag_name: &'static str,
) -> Result<String, DefaultNoticeError> {
    tags.iter()
        .find(|tag| tag.first().map(String::as_str) == Some(tag_name))
        .and_then(|tag| tag.get(1))
        .cloned()
        .ok_or(DefaultNoticeError::MissingRequiredTag(tag_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nip_ac::ScopeType;

    #[test]
    fn test_default_notice_roundtrip() {
        let notice = CreditDefaultNotice::new(
            "envelope-1",
            "agent-pubkey",
            ScopeReference::new(ScopeType::Nip90, "job-hash"),
            CreditDefaultContent::new("verification failed", 30_000),
        );

        let template = notice.to_event_template(1_703_003_900).unwrap();
        let event = Event {
            id: "default-id".to_string(),
            pubkey: "issuer-pubkey".to_string(),
            created_at: template.created_at,
            kind: template.kind,
            tags: template.tags.clone(),
            content: template.content.clone(),
            sig: "sig".to_string(),
        };

        let parsed = CreditDefaultNotice::from_event(&event).unwrap();
        assert_eq!(parsed.status, "defaulted");
        assert_eq!(parsed.content.loss_sats, 30_000);
    }
}
