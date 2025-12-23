//! Tests for agent selector component

use autopilot_gui::views::components::{Agent, AgentPreferences};
use std::str::FromStr;

#[test]
fn test_agent_parsing() {
    assert_eq!(Agent::from_str("claude").unwrap(), Agent::Claude);
    assert_eq!(Agent::from_str("codex").unwrap(), Agent::Codex);
    assert_eq!(Agent::from_str("CLAUDE").unwrap(), Agent::Claude);
    assert_eq!(Agent::from_str("Codex").unwrap(), Agent::Codex);
    assert!(Agent::from_str("unknown").is_err());
}

#[test]
fn test_agent_display() {
    assert_eq!(Agent::Claude.to_string(), "claude");
    assert_eq!(Agent::Codex.to_string(), "codex");
    assert_eq!(Agent::Claude.display_name(), "Claude Code");
    assert_eq!(Agent::Codex.display_name(), "Codex");
}

#[test]
fn test_agent_models() {
    let claude_models = Agent::Claude.supported_models();
    assert!(!claude_models.is_empty());
    assert!(claude_models.contains(&"claude-sonnet-4-5-20250929"));

    let codex_models = Agent::Codex.supported_models();
    assert!(!codex_models.is_empty());
    assert!(codex_models.contains(&"gpt-4o"));
}

#[test]
fn test_agent_preferences_default() {
    let prefs = AgentPreferences::default();
    assert_eq!(prefs.agent, Agent::Claude);
    assert_eq!(prefs.model, None);
}

#[test]
fn test_agent_preferences_new() {
    let prefs = AgentPreferences::new(Agent::Codex, Some("gpt-4o".to_string()));
    assert_eq!(prefs.agent, Agent::Codex);
    assert_eq!(prefs.model, Some("gpt-4o".to_string()));
    assert!(prefs.updated_at.is_some());
}

#[test]
fn test_agent_preferences_serialization() {
    let prefs = AgentPreferences::new(Agent::Claude, Some("claude-sonnet-4-5-20250929".to_string()));
    let json = serde_json::to_string(&prefs).unwrap();
    assert!(json.contains("claude"));
    assert!(json.contains("claude-sonnet-4-5-20250929"));

    let deserialized: AgentPreferences = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.agent, Agent::Claude);
    assert_eq!(deserialized.model, Some("claude-sonnet-4-5-20250929".to_string()));
}

#[test]
fn test_agent_preferences_save_load() {
    use tempfile::tempdir;
    use std::env;

    // Create a temporary directory for test config
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().join("openagents");
    std::fs::create_dir_all(&config_dir).unwrap();

    // Override the config directory for this test
    // SAFETY: This is a test environment, and we're setting the HOME env var
    // to control where the config file is saved. No other threads are running.
    unsafe {
        env::set_var("HOME", temp_dir.path().to_str().unwrap());
    }

    // Create preferences
    let original_prefs = AgentPreferences::new(Agent::Codex, Some("gpt-4o".to_string()));

    // Save
    original_prefs.save().unwrap();

    // Load
    let loaded_prefs = AgentPreferences::load();
    assert_eq!(loaded_prefs.agent, Agent::Codex);
    assert_eq!(loaded_prefs.model, Some("gpt-4o".to_string()));
}
