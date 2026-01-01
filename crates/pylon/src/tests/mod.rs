use openagents_relay::ClaudeSessionAutonomy;

use crate::config::PylonConfig;

#[test]
fn test_default_claude_config() {
    let config = PylonConfig::default();
    assert!(config.claude.enabled);
    assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
    assert_eq!(config.claude.autonomy, ClaudeSessionAutonomy::Supervised);
    assert_eq!(
        config.claude.approval_required_tools,
        vec!["Write", "Edit", "Bash"]
    );
}

#[test]
fn test_claude_config_roundtrip() {
    let config = PylonConfig::default();
    let toml_str = toml::to_string(&config).expect("should serialize");
    let parsed: PylonConfig = toml::from_str(&toml_str).expect("should deserialize");
    assert_eq!(parsed.claude.model, config.claude.model);
    assert_eq!(parsed.claude.autonomy, config.claude.autonomy);
    assert_eq!(
        parsed.claude.approval_required_tools,
        config.claude.approval_required_tools
    );
}
