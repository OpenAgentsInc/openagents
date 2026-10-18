//! OpenAgents (#11966): a chat that runs a saved agent wears the OpenAgents
//! mark and the agent's name in the sidebar row and the session header, not
//! its runtime's brand. The header keeps the chat's title and says beside it
//! which agent runs the chat, that it is a custom agent, and on what runtime
//! (#12027); the composer addresses the agent and carries its chip.
use openagents_proto::{Chat, HarnessId, saved_agent};

use crate::icons;

/// How a chat introduces itself on the desktop.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Identity {
    /// Icon path and brand tint.
    pub mark: Option<(&'static str, Option<gpui::Hsla>)>,
    /// The saved agent's name; `None` for ordinary chats.
    pub agent: Option<String>,
    /// "Foreman · custom agent · Claude Code", shown beside the title.
    pub line: Option<String>,
}

impl Identity {
    /// `harness` is the runtime the caller would show (`None` when the
    /// surface hides runtimes); a saved agent shows its own mark regardless.
    pub(crate) fn of(chat: Option<&Chat>, harness: Option<HarnessId>) -> Self {
        let config = chat.and_then(|c| c.config.as_ref());
        match saved_agent::agent_name(config, chat.and_then(|c| c.title.as_deref())) {
            Some(name) => Self {
                mark: Some((icons::OPENAGENTS_LOGO, None)),
                line: config.map(|c| saved_agent::identity_line(&name, c.harness)),
                agent: Some(name),
            },
            None => Self {
                mark: harness.map(crate::pickers::harness_brand_icon),
                agent: None,
                line: None,
            },
        }
    }

    /// The composer's placeholder: a saved agent is addressed by name.
    pub(crate) fn placeholder(&self) -> gpui::SharedString {
        match &self.agent {
            Some(name) => format!("Message {name}").into(),
            None => DEFAULT_PLACEHOLDER.into(),
        }
    }
}

/// The composer's placeholder for an ordinary chat.
pub(crate) const DEFAULT_PLACEHOLDER: &str = "Do anything…";

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(options: serde_json::Value) -> Chat {
        serde_json::from_value(serde_json::json!({
            "id": "c", "deviceId": "d", "title": "Foreman", "archived": false,
            "createdAt": "2026-10-17T00:00:00Z",
            "config": {"harness": "claude-code", "model": null, "reasoning": null,
                "modelOptions": options, "sandbox": "danger-full-access"}
        }))
        .unwrap()
    }

    #[test]
    fn saved_agent_chats_wear_the_openagents_mark_and_name() {
        let agent = chat(serde_json::json!({"openagentsAssignment": "a"}));
        let id = Identity::of(Some(&agent), Some(HarnessId::ClaudeCode));
        assert_eq!(id.mark.map(|m| m.0), Some(icons::OPENAGENTS_LOGO));
        assert_eq!(id.agent.as_deref(), Some("Foreman"));
        assert_eq!(
            id.line.as_deref(),
            Some("Foreman · custom agent · Claude Code")
        );
        // Shown even where runtime marks are hidden.
        assert!(Identity::of(Some(&agent), None).mark.is_some());

        let plain = chat(serde_json::json!({}));
        let id = Identity::of(Some(&plain), Some(HarnessId::ClaudeCode));
        assert_eq!(id.mark.map(|m| m.0), Some(icons::CLAUDE_MARK));
        assert!(id.agent.is_none());
        assert!(id.line.is_none());
        assert_eq!(id.placeholder().as_ref(), DEFAULT_PLACEHOLDER);
        assert_eq!(
            Identity::of(Some(&agent), None).placeholder().as_ref(),
            "Message Foreman"
        );
        assert_eq!(Identity::of(Some(&plain), None).mark, None);
    }
}
