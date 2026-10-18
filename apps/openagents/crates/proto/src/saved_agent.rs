//! Saved-agent chats (OpenAgents #11966). A chat that runs a saved agent is
//! marked in its config, and every surface draws it as that agent (the
//! OpenAgents mark and the agent's name), not as the runtime underneath.
use crate::{ChatConfig, HarnessId};

/// The assignment a saved-agent chat belongs to.
pub const ASSIGNMENT: &str = "openagentsAssignment";
/// The saved agent's name, kept on the chat config for display.
pub const AGENT_NAME: &str = "openagentsAgentName";
/// The saved agent's definition ID, kept on the chat config so hosts key
/// what they do for the agent by its definition, never by its name.
pub const AGENT: &str = "openagentsAgent";
/// What the agent's definition declares about running on its own
/// ([`Declared`]), kept on the chat config so the host can read it offline.
pub const AUTONOMY: &str = "openagentsAutonomy";
/// The longest one shell command of a saved agent's turn may run, in
/// milliseconds, from its definition's autonomy settings (#12029). A run
/// option: the runtime applies it (Claude Code's `BASH_DEFAULT_TIMEOUT_MS`
/// and `BASH_MAX_TIMEOUT_MS`).
pub const SHELL_TIMEOUT_MS: &str = "openagentsShellTimeoutMs";

/// The part of a saved agent's declared autonomy a host acts on (#12030):
/// turn check-ins on for its chat without being asked, how often, and
/// whether it asks for standing authority with them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Declared {
    pub check_ins: bool,
    pub every_minutes: Option<u32>,
    pub authority: bool,
}

/// What a saved-agent chat's definition declares, from its config. `None`
/// for ordinary chats and agents that declare nothing.
pub fn declared(config: Option<&ChatConfig>) -> Option<Declared> {
    let config = config.filter(|c| is_agent_chat(Some(c)))?;
    serde_json::from_value(config.model_options.get(AUTONOMY)?.clone()).ok()
}

/// Whether the chat's agent asks hosts to turn check-ins on for it.
pub fn declares_check_ins(config: Option<&ChatConfig>) -> bool {
    declared(config).is_some_and(|d| d.check_ins)
}

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

/// Where people find and manage their saved agents (#12027).
pub const AGENT_LAB_URL: &str = "https://openagents.com/labs/agent";

/// The line a saved agent's chat shows beside or under its title, so the
/// chat reads as an agent rather than a plain runtime chat (#12027):
/// "Foreman · custom agent · Claude Code".
pub fn identity_line(agent: &str, harness: HarnessId) -> String {
    format!("{agent} · custom agent · {}", runtime_label(harness))
}

/// [`identity_line`] for a chat, or `None` for an ordinary chat.
pub fn chat_identity_line(config: Option<&ChatConfig>, title: Option<&str>) -> Option<String> {
    let name = agent_name(config, title)?;
    Some(identity_line(&name, config?.harness))
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

    #[test]
    fn check_ins_come_from_the_declaration_not_the_name() {
        let declaring = config(serde_json::json!({
            ASSIGNMENT: "a",
            AGENT_NAME: "Night shift",
            AUTONOMY: {"checkIns": true, "everyMinutes": 10, "authority": true},
        }));
        assert_eq!(
            declared(Some(&declaring)),
            Some(Declared {
                check_ins: true,
                every_minutes: Some(10),
                authority: true,
            })
        );
        assert!(declares_check_ins(Some(&declaring)));
        let named = config(serde_json::json!({ASSIGNMENT: "a", AGENT_NAME: "Foreman"}));
        assert!(!declares_check_ins(Some(&named)));
        // Only saved-agent chats count.
        let plain = config(serde_json::json!({AUTONOMY: {"checkIns": true}}));
        assert!(!declares_check_ins(Some(&plain)));
    }

    #[test]
    fn agent_chats_say_they_run_a_custom_agent() {
        let named = config(serde_json::json!({ASSIGNMENT: "a", AGENT_NAME: "Foreman"}));
        assert_eq!(
            chat_identity_line(Some(&named), Some("Foreman")).as_deref(),
            Some("Foreman · custom agent · Claude Code")
        );
        let plain = config(serde_json::json!({}));
        assert_eq!(chat_identity_line(Some(&plain), Some("Fix CI")), None);
        assert_eq!(chat_identity_line(None, Some("Foreman")), None);
    }
}
