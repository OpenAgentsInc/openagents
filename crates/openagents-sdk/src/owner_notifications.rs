//! An agent asks its owner to return to its chat. Grant this separately for each agent.
use crate::{CancellationToken, Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize};

fn valid_chat(chat: &str) -> bool {
    !chat.is_empty()
        && chat.len() <= 128
        && chat
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

/// Return the chat ID from an OpenAgents chat link. Refuse other destinations.
pub fn chat_from_deep_link(link: &str) -> Option<&str> {
    let chat = link.strip_prefix("openagents://chat/")?;
    valid_chat(chat).then_some(chat)
}

/// Private note fetched by the phone; none of these fields enter the push payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerNote {
    /// Saved agent definition ID, used for permission and rate limits.
    pub agent: String,
    /// That agent's chat, opened when the owner taps the notification.
    pub chat: String,
    /// Plain-language title under 120 UTF-8 bytes.
    pub title: String,
    /// What happened or what the agent needs, up to 1,024 UTF-8 bytes.
    pub body: String,
}
impl OwnerNote {
    /// Check identifiers, byte limits, and the app's plain-language copy rules.
    pub fn validate(&self) -> Result<(), Error> {
        let text = |s: &str, limit: usize| {
            !s.trim().is_empty()
                && s.len() < limit
                && !s.chars().any(char::is_control)
                && oa_copy::violations(s, &[]).is_empty()
        };
        if crate::agents::valid_id(&self.agent)
            && valid_chat(&self.chat)
            && text(&self.title, 120)
            && text(&self.body, 1025)
        {
            Ok(())
        } else {
            Err(Error::InvalidReference)
        }
    }
    /// App link generated from the checked chat ID, never a caller-supplied URL.
    pub fn deep_link(&self) -> String {
        format!("openagents://chat/{}", self.chat)
    }
}

/// A notification request whose ID stays the same across retries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotifyOwner {
    pub request_id: String,
    pub note: OwnerNote,
}
/// An account-private note fetched by a phone after an attention wake.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerNotification {
    pub request_id: String,
    pub note: OwnerNote,
    pub deep_link: String,
}

impl Client {
    /// Notify once for `request_id`. Reuse the same ID and note after a failed request.
    /// The account must have enabled notifications for this agent. At most six
    /// new notes per agent per hour are accepted, including across host restarts.
    pub async fn notify_owner(
        &self,
        session: &Session,
        request_id: &str,
        note: OwnerNote,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        self.check_session(session)?;
        note.validate()?;
        if !crate::agents::valid_id(request_id) {
            return Err(Error::InvalidReference);
        }
        let body = serde_json::to_value(NotifyOwner {
            request_id: request_id.into(),
            note,
        })
        .map_err(|_| Error::InvalidReference)?;
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.request::<serde_json::Value>(Method::POST, &["notify"], &[], Some(body), Some(session.token())) => result.map(|_| ()),
        }
    }

    /// Enable or revoke this agent's permission to notify the owner. Disabled by default.
    pub async fn set_owner_notification_grant(
        &self,
        session: &Session,
        agent: &str,
        enabled: bool,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        self.check_session(session)?;
        if !crate::agents::valid_id(agent) {
            return Err(Error::InvalidReference);
        }
        let path = ["notify", "grants", agent];
        tokio::select! { biased;
            _ = cancel.cancelled() => Err(Error::Cancelled),
            result = self.request::<serde_json::Value>(Method::PUT, &path, &[], Some(serde_json::json!({"enabled":enabled})), Some(session.token())) => result.map(|_| ()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notes_have_plain_bounded_copy_and_a_chat_link() {
        let mut n = OwnerNote {
            agent: "a".repeat(64),
            chat: "chat-1".into(),
            title: "I need your help".into(),
            body: "The build stopped. Can you check the computer?".into(),
        };
        n.validate().unwrap();
        assert_eq!(n.deep_link(), "openagents://chat/chat-1");
        for title in [
            "".into(),
            "x".repeat(120),
            "é".repeat(60),
            "Line\nbreak".into(),
            "Reconcile the canonical projection".into(),
        ] {
            n.title = title;
            assert!(n.validate().is_err());
        }
        n.title = "I have a question".into();
        n.chat = "../other".into();
        assert!(n.validate().is_err());
        n.chat = "chat-1".into();
        n.body = "é".repeat(512);
        assert!(n.validate().is_ok());
        n.body.push('!');
        assert!(n.validate().is_err());
    }
}
