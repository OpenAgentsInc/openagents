//! Saved-agent chats (OpenAgents #11966). A chat that runs a saved agent is
//! marked in its config, and every surface draws it as that agent (the
//! OpenAgents mark and the agent's name), not as the runtime underneath.
use crate::{ChatConfig, HarnessId};

/// The assignment a saved-agent chat belongs to.
pub const ASSIGNMENT: &str = "openagentsAssignment";
/// The saved agent's name, kept on the chat config for display.
pub const AGENT_NAME: &str = "openagentsAgentName";

/// Whether the chat runs a saved agent.
pub fn is_agent_chat(config: Option<&ChatConfig>) -> bool {
    config.is_some_and(|c| c.model_options.contains_key(ASSIGNMENT))
}

/// The saved agent's name for a chat that runs one: the recorded name, else
/// the chat title (agent chats are created with the agent's name), else
/// "Agent". `None` for ordinary chats.
pub fn agent_name(config: Option<&ChatConfig>, title: Option<&str>) -> Option<String> {
    let config = config.filter(|c| is_agent_chat(Some(c)))?;
    let clean = |s: &str| {
        let s = s.trim();
        (!s.is_empty() && !s.chars().any(char::is_control)).then(|| s.chars().take(80).collect())
    };
    Some(
        config
            .model_options
            .get(AGENT_NAME)
            .and_then(|v| v.as_str())
            .and_then(clean)
            .or_else(|| title.and_then(clean))
            .unwrap_or_else(|| "Agent".into()),
    )
}

/// The runtime's name as people know it.
pub fn runtime_label(harness: HarnessId) -> &'static str {
    match harness {
        HarnessId::ClaudeCode => "Claude Code",
        HarnessId::Codex => "Codex",
        HarnessId::Cursor => "Cursor",
        HarnessId::Devin => "Devin",
        HarnessId::Grok => "Grok",
        HarnessId::Hermes => "Hermes",
        HarnessId::Pi => "Pi",
        HarnessId::Opencode => "OpenCode",
        HarnessId::Antigravity => "Antigravity",
        HarnessId::Mock => "Test runtime",
        HarnessId::Sdk => "SDK runner",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SandboxLevel;

    fn config(options: serde_json::Value) -> ChatConfig {
        ChatConfig {
            harness: HarnessId::ClaudeCode,
            model: None,
            reasoning: None,
            model_options: options.as_object().unwrap().clone(),
            sandbox: SandboxLevel::DangerFullAccess,
        }
    }

    #[test]
    fn agent_chats_are_named_by_the_agent() {
        let plain = config(serde_json::json!({}));
        assert_eq!(agent_name(Some(&plain), Some("Fix CI")), None);
        assert_eq!(agent_name(None, Some("Fix CI")), None);
        let named = config(serde_json::json!({ASSIGNMENT: "a", AGENT_NAME: " Foreman "}));
        assert_eq!(
            agent_name(Some(&named), Some("Other")).as_deref(),
            Some("Foreman")
        );
        let old = config(serde_json::json!({ASSIGNMENT: "a"}));
        assert_eq!(
            agent_name(Some(&old), Some("Foreman")).as_deref(),
            Some("Foreman")
        );
        assert_eq!(agent_name(Some(&old), Some("  ")).as_deref(), Some("Agent"));
    }
}
