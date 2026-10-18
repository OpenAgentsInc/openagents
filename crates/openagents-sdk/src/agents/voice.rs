//! The account's saved agent and chat used for voice conversations.
use super::{AgentRequest, Agents, request_id, valid_id};
use crate::{CancellationToken, Error};
use reqwest::Method;
use serde::{Deserialize, Serialize};

/// A saved agent's chat, identified independently of its display name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VoiceTarget {
    pub agent_id: String,
    pub chat_id: String,
    pub host_id: String,
}
impl VoiceTarget {
    pub fn validate(&self) -> bool {
        valid_id(&self.agent_id)
            && !self.chat_id.is_empty()
            && self.chat_id.len() <= 128
            && !self.host_id.is_empty()
            && self.host_id.len() <= 128
            && self
                .chat_id
                .bytes()
                .chain(self.host_id.bytes())
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    }
}
/// The current voice choice. Revision zero means no choice has been saved.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceAgent {
    pub revision: u64,
    pub target: Option<VoiceTarget>,
}
/// A retryable change to the account's voice choice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetVoiceAgent {
    pub request_id: String,
    pub expected_revision: u64,
    pub target: Option<VoiceTarget>,
}
impl Agents<'_> {
    /// Reads the same voice choice on every device signed in to this account.
    pub async fn voice_agent(&self, cancel: &CancellationToken) -> Result<VoiceAgent, Error> {
        self.send(Method::GET, &["voice-agent"], &[], None, cancel)
            .await
    }
    /// Prepares a change without starting an agent or a call.
    pub fn voice_agent_request(
        &self,
        expected_revision: u64,
        target: Option<VoiceTarget>,
    ) -> Result<AgentRequest<SetVoiceAgent>, Error> {
        if target.as_ref().is_some_and(|t| !t.validate()) {
            return Err(Error::InvalidReference);
        }
        Ok(self.bind(SetVoiceAgent {
            request_id: request_id()?,
            expected_revision,
            target,
        }))
    }
    /// Saves a prepared voice choice. A stale revision must be reloaded before retrying.
    pub async fn set_voice_agent(
        &self,
        request: &AgentRequest<SetVoiceAgent>,
        cancel: &CancellationToken,
    ) -> Result<VoiceAgent, Error> {
        self.send(
            Method::PUT,
            &["voice-agent"],
            &[],
            Some(self.body(request)?),
            cancel,
        )
        .await
    }
}
