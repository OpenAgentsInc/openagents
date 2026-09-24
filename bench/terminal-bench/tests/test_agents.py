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
        "coder-one-tunable-luna-pack",
        "coder-one-microluna-v1",
        "coder-one-microluna-v2",
        "coder-one-microluna-v3",
        "coder-one-microluna-v4",
        "coder-one-microluna-v5",
        "coder-one-microluna-v6",
        "coder-one-microluna-v7",
        "coder-one-microluna-v8",
        "coder-one-microluna-v17",
        "coder-one-microluna-v16",
        "coder-one-microluna-v15",
        "coder-one-microluna-v14",
        "coder-one-microluna-v13",
        "coder-one-microluna-v12",
        "coder-one-microluna-evidence-v1",
        "coder-one-microluna-v11",
        "coder-one-microluna-v10",
        "coder-one-microluna-v9",
        "coder-one-microluna-solo",
        "netprobe",
        "coder-one-tunable-v2",
        "coder-one-tunable-luna-v2",
        "coder-one-tunable-luna-snapshot",
        "coder-one-tunable-v3",
        "coder-one-tunable-v4",
        "coder-one-tunable-v5",
        "coder-one-tunable-v6",
        "coder-one-tunable-v7",
        "coder-one-tunable-v8",
        "coder-one-tunable-v9",
        "coder-one-tunable-v9-escalate",
        "coder-one-tunable-v10",
        "coder-one-matched-v8",
        "claude-code-opus-matched",
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
        ("coder-one-tunable-luna-pack", "tunable-luna-pack.json"),
        ("coder-one-tunable-v2", "tunable-v2.json"),
        ("coder-one-tunable-luna-v2", "tunable-luna-v2.json"),
        ("coder-one-tunable-luna-snapshot", "tunable-luna-snapshot.json"),
        ("coder-one-tunable-v3", "tunable-v3.json"),
        ("coder-one-tunable-v4", "tunable-v4.json"),
        ("coder-one-tunable-v5", "tunable-v5.json"),
        ("coder-one-tunable-v6", "tunable-v6.json"),
        ("coder-one-tunable-v7", "tunable-v7.json"),
        ("coder-one-tunable-v8", "tunable-v8.json"),
        ("coder-one-tunable-v9", "tunable-v9.json"),
        ("coder-one-tunable-v9-escalate", "tunable-v9-escalate.json"),
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


def test_the_v5_arm_is_v4_plus_persist(agents):
    import json
    from pathlib import Path

    from tbench.coder_one import manifest_tiers

    root = Path(__file__).resolve().parents[3]
    v4 = json.loads((root / "crates/coder-one/policies/tunable-v4.json").read_text())
    v5 = json.loads((root / agents["coder-one-tunable-v5"].kwargs["policy"]).read_text())
    assert v5["name"] == "coder-one-tunable-v5"
    persist = v5["policy"]["control"].pop("persist")
    assert persist["max_rounds"] == 3
    assert persist["min_remaining_sec"] == 1800
    assert persist["long_only"] is True
    # Everything else is v4, so a screen measures persistence alone.
    assert v5["policy"] == v4["policy"]
    assert v5["protected"] == v4["protected"]
    profile = agents["coder-one-tunable-v5"]
    assert profile.kwargs["executors"] == agents["coder-one-tunable-v4"].kwargs["executors"]
    # An alternate persist executor is installed like any other tier.
    v5["policy"]["control"]["persist"] = dict(
        persist, alternate=[{"agent": "codex", "model": "gpt-6-astra", "version": "0.155.1"}]
    )
    v5["policy"]["verify"].pop("second")
    v5["policy"]["control"]["route"].pop("families")
    assert "gpt-6-astra" in {tier["model"] for tier in manifest_tiers(v5)}
