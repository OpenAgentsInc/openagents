use openagents_relay::CodexSessionAutonomy;

use crate::config::PylonConfig;

#[test]
fn test_default_codex_config() {
    let config = PylonConfig::default();
    assert!(config.codex.enabled);
    assert_eq!(config.codex.model, "codex-sonnet-4-20250514");
    assert_eq!(config.codex.autonomy, CodexSessionAutonomy::Supervised);
    assert_eq!(
        config.codex.approval_required_tools,
        vec!["Write", "Edit", "Bash"]
    );
}

#[test]
fn test_codex_config_roundtrip() {
    let config = PylonConfig::default();
    let toml_str = toml::to_string(&config).expect("should serialize");
    let parsed: PylonConfig = toml::from_str(&toml_str).expect("should deserialize");
    assert_eq!(parsed.codex.model, config.codex.model);
    assert_eq!(parsed.codex.autonomy, config.codex.autonomy);
    assert_eq!(
        parsed.codex.approval_required_tools,
        config.codex.approval_required_tools
    );
}
