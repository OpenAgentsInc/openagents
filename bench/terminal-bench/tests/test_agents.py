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
        "coder-one-delegate-opus",
        "coder-one-delegate-auto",
        "coder-one-delegate-luna",
        "coder-one-delegate-luna-auto",
        "coder-one-deep",
        "coder-one-jevbrief-luna",
        "coder-one-jevbrief-opus",
        "coder-one-jevbrief-opus-lean",
        "coder-one-jevprobe-opus-lean",
        "coder-one-jevprobe-luna",
        "coder-one-jevprobe2-luna",
        "coder-one-jevprobe3-luna",
        "coder-one-pack-luna",
        "coder-one-jevprobe2-opus-lean-low-5m",
        "coder-one-jevprobe3-opus-lean-low",
        "coder-one-jevprobe2-opus-lean-low",
        "coder-one-jevprobe-luna-low",
        "coder-one-jevprobe-opus-lean-low",
        "coder-one-jevprobe-sonnet-lean-low",
        "coder-one-jevprobe-haiku-lean",
        "coder-one-tunable",
        "coder-one-tunable-opus",
        "coder-one-tunable-luna",
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


def test_delegate_arms_pin_opus_and_a_new_enough_claude_code(agents):
    for arm, mode in (
        ("coder-one-delegate-opus", "always"),
        ("coder-one-delegate-auto", "auto"),
    ):
        profile = agents[arm]
        assert profile.harbor_import_path == "tbench.coder_one:CoderOneDelegate"
        assert profile.kwargs["delegate"] == mode
        assert profile.kwargs["delegate_model"] == "claude-opus-5-5"
        assert profile.kwargs["claude_code_version"] == "2.1.280"
        assert "CLAUDE_CODE_OAUTH_TOKEN" in profile.env_forward


def test_delegate_oauth_mode_forwards_the_token_by_name_only(agents):
    env = {
        "OPENAGENTS_API_KEY": "oak_secret",
        "TYPESAFE_API_KEY": "ts_secret",
        "CLAUDE_CODE_OAUTH_TOKEN": "tok-secret",
        "ANTHROPIC_API_KEY": "sk-secret",
    }
    profile = agents["coder-one-delegate-opus"]
    assert [m.name for m in configured_auth_modes(profile, env=env)] == [
        "subscription-oauth",
        "api-key",
    ]
    out = agent_config_env(profile, auth_mode="subscription-oauth", env=env)
    assert out["CLAUDE_CODE_OAUTH_TOKEN"] == "${CLAUDE_CODE_OAUTH_TOKEN}"
    assert out["TYPESAFE_API_KEY"] == "${TYPESAFE_API_KEY}"
    assert "ANTHROPIC_API_KEY" not in out
    assert "secret" not in str(out)


def test_delegate_modes_need_the_door_keys_too(agents):
    env = {"CLAUDE_CODE_OAUTH_TOKEN": "tok"}
    assert configured_auth_modes(agents["coder-one-delegate-auto"], env=env) == []


def test_luna_arms_delegate_to_codex(agents):
    for arm, mode in (
        ("coder-one-delegate-luna", "always"),
        ("coder-one-delegate-luna-auto", "auto"),
    ):
        profile = agents[arm]
        assert profile.harbor_import_path == "tbench.coder_one:CoderOneDelegate"
        assert profile.kwargs["delegate"] == mode
        assert profile.kwargs["delegate_agent"] == "codex"
        assert profile.kwargs["delegate_model"] == "gpt-6-luna"
        assert profile.kwargs["codex_version"] == "0.155.1"
        assert list(profile.auth_modes) == ["auth-json"]


def test_luna_auth_json_keeps_the_selector_host_side(agents):
    env = {
        "OPENAGENTS_API_KEY": "oak_secret",
        "TYPESAFE_API_KEY": "ts_secret",
        "CODEX_FORCE_AUTH_JSON": "1",
    }
    profile = agents["coder-one-delegate-luna"]
    assert [m.name for m in configured_auth_modes(profile, env=env)] == ["auth-json"]
    out = agent_config_env(profile, auth_mode="auth-json", env=env)
    assert out["CODEX_AUTH_JSON_PATH"].endswith(".codex/auth.json")
    assert "~" not in out["CODEX_AUTH_JSON_PATH"]
    assert "CODEX_FORCE_AUTH_JSON" not in out
    assert out["OPENAGENTS_API_KEY"] == "${OPENAGENTS_API_KEY}"
    assert "secret" not in str(out)
    # Without the door keys the mode is not configured.
    assert configured_auth_modes(profile, env={"CODEX_FORCE_AUTH_JSON": "1"}) == []


def test_unset_vars_are_not_forwarded(agents):
    out = agent_config_env(agents["claude-code"], env={})
    assert "ANTHROPIC_API_KEY" not in out


def test_tunable_arms_install_both_clis_and_forward_both_credentials(agents):
    for arm, policy in (
        ("coder-one-tunable", "tunable.json"),
        ("coder-one-tunable-opus", "tunable-opus.json"),
        ("coder-one-tunable-luna", "tunable-luna.json"),
    ):
        profile = agents[arm]
        assert profile.harbor_import_path == "tbench.coder_one:CoderOneTunable"
        assert profile.kwargs["policy"] == f"crates/coder-one/policies/{policy}"
        assert profile.kwargs["executors"] == {"claude-code": "2.1.280", "codex": "0.155.1"}
        env = {
            "OPENAGENTS_API_KEY": "oak_secret",
            "TYPESAFE_API_KEY": "ts_secret",
            "CLAUDE_CODE_OAUTH_TOKEN": "tok-secret",
            "ANTHROPIC_API_KEY": "sk-secret",
            "CODEX_FORCE_AUTH_JSON": "1",
        }
        assert [m.name for m in configured_auth_modes(profile, env=env)] == [
            "subscription-oauth"
        ]
        out = agent_config_env(profile, auth_mode="subscription-oauth", env=env)
        assert out["CLAUDE_CODE_OAUTH_TOKEN"] == "${CLAUDE_CODE_OAUTH_TOKEN}"
        assert out["CODEX_AUTH_JSON_PATH"].endswith(".codex/auth.json")
        assert "ANTHROPIC_API_KEY" not in out
        assert "CODEX_FORCE_AUTH_JSON" not in out
        assert "secret" not in str(out)
        # Without the Codex credential the mode is not configured.
        del env["CODEX_FORCE_AUTH_JSON"]
        assert configured_auth_modes(profile, env=env) == []
