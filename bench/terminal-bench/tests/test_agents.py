"""Agent profiles: auth-mode detection and credential templating."""

import pytest

from tbench.agents import agent_config_env, configured_auth_modes, load_agents


@pytest.fixture()
def agents():
    return load_agents()


def test_known_arms(agents):
    assert set(agents) == {
        "claude-code",
        "codex",
        "coder-v05",
        "oracle",
        "nop",
    }
    assert agents["oracle"].is_control
    assert agents["nop"].is_control
    assert agents["coder-v05"].harbor_import_path == "tbench.coder_v05:CoderV05"


def test_claude_api_key_mode_detected(agents):
    modes = configured_auth_modes(
        agents["claude-code"], env={"ANTHROPIC_API_KEY": "redacted"}
    )
    assert [m.name for m in modes] == ["api-key"]


def test_claude_oauth_needs_force_flag(agents):
    env = {"CLAUDE_CODE_OAUTH_TOKEN": "redacted"}
    assert configured_auth_modes(agents["claude-code"], env=env) == []
    env["CLAUDE_FORCE_OAUTH"] = "1"
    modes = configured_auth_modes(agents["claude-code"], env=env)
    assert [m.name for m in modes] == ["subscription-oauth"]


def test_codex_auth_json_mode(agents):
    env = {"CODEX_AUTH_JSON_PATH": "/private/auth.json"}
    modes = configured_auth_modes(agents["codex"], env=env)
    assert [m.name for m in modes] == ["auth-json"]


def test_env_templates_never_carry_values(agents):
    env = {"ANTHROPIC_API_KEY": "sk-secret-value"}
    out = agent_config_env(
        agents["claude-code"], auth_mode="api-key", env=env
    )
    assert out["ANTHROPIC_API_KEY"] == "${ANTHROPIC_API_KEY}"
    assert "sk-secret-value" not in str(out)


def test_forced_values_are_literals(agents):
    env = {
        "CLAUDE_CODE_OAUTH_TOKEN": "tok",
        "CLAUDE_FORCE_OAUTH": "0",
    }
    out = agent_config_env(
        agents["claude-code"], auth_mode="subscription-oauth", env=env
    )
    assert out["CLAUDE_FORCE_OAUTH"] == "1"
    assert out["CLAUDE_CODE_OAUTH_TOKEN"] == "${CLAUDE_CODE_OAUTH_TOKEN}"


def test_unset_vars_are_not_forwarded(agents):
    out = agent_config_env(agents["claude-code"], env={})
    assert "ANTHROPIC_API_KEY" not in out
