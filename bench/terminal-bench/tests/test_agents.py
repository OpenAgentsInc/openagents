"""Agent profiles: auth-mode detection and credential templating."""

import pytest

from tbench.agents import agent_config_env, configured_auth_modes, load_agents


@pytest.fixture()
def agents():
    return load_agents()


def test_known_arms(agents):
    assert set(agents) == {
        "claude-code",
        "claude-code-opus",
        "codex",
        "codex-gpt-6-astra",
        "codex-gpt-6-sol",
        "codex-gpt-6-luna",
        "devin",
        "coder-v05",
        "coder-one",
        "coder-one-no-jev",
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


def test_claude_oauth_needs_token_only(agents):
    env = {"CLAUDE_CODE_OAUTH_TOKEN": "redacted"}
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


def test_oauth_excludes_api_key_vars(agents):
    env = {
        "ANTHROPIC_API_KEY": "sk-secret-value",
        "CLAUDE_CODE_OAUTH_TOKEN": "tok",
        "CLAUDE_FORCE_OAUTH": "1",
    }
    out = agent_config_env(
        agents["claude-code"], auth_mode="subscription-oauth", env=env
    )
    assert out["CLAUDE_CODE_OAUTH_TOKEN"] == "${CLAUDE_CODE_OAUTH_TOKEN}"
    assert "ANTHROPIC_API_KEY" not in out
    assert "ANTHROPIC_AUTH_TOKEN" not in out
    # A truthy selector is never forwarded: Harbor scrubs credential-named
    # values from retained evidence, and "1" would redact every digit.
    assert "CLAUDE_FORCE_OAUTH" not in out


def test_codex_auth_json_default_path_override(agents):
    env = {"CODEX_FORCE_AUTH_JSON": "1"}
    out = agent_config_env(agents["codex"], auth_mode="auth-json", env=env)
    assert out["CODEX_AUTH_JSON_PATH"].endswith(".codex/auth.json")
    assert "~" not in out["CODEX_AUTH_JSON_PATH"]
    assert "CODEX_FORCE_AUTH_JSON" not in out


def test_codex_auth_json_host_path_wins(agents):
    env = {"CODEX_AUTH_JSON_PATH": "/private/auth.json"}
    out = agent_config_env(agents["codex"], auth_mode="auth-json", env=env)
    assert out["CODEX_AUTH_JSON_PATH"] == "${CODEX_AUTH_JSON_PATH}"


def test_unset_vars_are_not_forwarded(agents):
    out = agent_config_env(agents["claude-code"], env={})
    assert "ANTHROPIC_API_KEY" not in out
