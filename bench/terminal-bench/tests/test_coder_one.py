"""The Coder One adapter: the v0.5 contract under its own arm and path."""

import asyncio
import hashlib

import pytest

from tbench.coder_one import CoderOne, CoderOneDelegate
from tbench.coder_v05 import ArtifactIdentityError, CoderV05, EpisodeContractError


def _binary(tmp_path, payload: bytes = b"#!/bin/sh\n") -> tuple[str, str]:
    path = tmp_path / "coder-one"
    path.write_bytes(payload)
    return str(path), hashlib.sha256(payload).hexdigest()


def test_arm_and_install_path_are_its_own(tmp_path):
    path, digest = _binary(tmp_path)
    agent = CoderOne(logs_dir=tmp_path, artifact_path=path, artifact_sha256=digest)
    assert CoderOne.name() == "coder-one"
    assert str(agent.BINARY_PATH) == "/opt/openagents/bin/coder-one"
    assert str(CoderV05.BINARY_PATH) == "/opt/openagents/bin/coder-v05"
    assert agent.get_version_command() == "/opt/openagents/bin/coder-one --version"


def test_jev_key_reaches_the_episode_by_name_only(tmp_path):
    assert "TYPESAFE_API_KEY" in CoderOne.EPISODE_ENV
    assert "CODER_ONE_JEV" in CoderOne.EPISODE_ENV
    assert "TYPESAFE_API_KEY" not in CoderV05.EPISODE_ENV


def test_the_pin_is_still_enforced(tmp_path):
    path, _ = _binary(tmp_path)
    with pytest.raises(ArtifactIdentityError):
        CoderOne(logs_dir=tmp_path, artifact_path=path, artifact_sha256="00" * 32)


def _delegate(tmp_path, **kwargs) -> CoderOneDelegate:
    path, digest = _binary(tmp_path)
    return CoderOneDelegate(
        logs_dir=tmp_path, artifact_path=path, artifact_sha256=digest, **kwargs
    )


def test_delegate_arm_needs_a_mode(tmp_path):
    with pytest.raises(EpisodeContractError, match="delegate=always or delegate=auto"):
        _delegate(tmp_path)
    with pytest.raises(EpisodeContractError, match="delegate=always"):
        _delegate(tmp_path, delegate="off")


def test_delegate_arm_refuses_a_claude_code_the_api_refuses(tmp_path):
    with pytest.raises(EpisodeContractError, match="older than 2.1.280"):
        _delegate(tmp_path, delegate="always", claude_code_version="2.1.278")
    assert _delegate(tmp_path, delegate="always", claude_code_version="2.1.281")


def test_delegate_env_sets_the_mode_and_forwards_claude_credentials_by_name(
    tmp_path, monkeypatch
):
    monkeypatch.setenv("CLAUDE_CODE_OAUTH_TOKEN", "tok-secret")
    monkeypatch.delenv("ANTHROPIC_API_KEY", raising=False)
    agent = _delegate(
        tmp_path, delegate="auto", delegate_timeout_sec=480, explore_steps=6
    )
    assert CoderOneDelegate.name() == "coder-one-delegate"
    assert str(agent.BINARY_PATH) == "/opt/openagents/bin/coder-one"
    assert "CLAUDE_CODE_OAUTH_TOKEN" in CoderOneDelegate.EPISODE_ENV
    assert "CLAUDE_CODE_OAUTH_TOKEN" not in CoderOne.EPISODE_ENV
    agent._claude_bin = "/root/.local/share/claude/versions/2.1.280"
    env = agent._episode_env()
    assert env["CODER_ONE_DELEGATE"] == "auto"
    assert env["CODER_ONE_DELEGATE_MODEL"] == "claude-opus-5-5"
    assert env["CODER_ONE_CLAUDE_BIN"] == "/root/.local/share/claude/versions/2.1.280"
    assert env["CODER_ONE_DELEGATE_TIMEOUT"] == "480"
    assert env["CODER_ONE_EXPLORE_STEPS"] == "6"
    assert env["CLAUDE_CODE_OAUTH_TOKEN"] == "tok-secret"
    assert "ANTHROPIC_API_KEY" not in env


def test_delegate_install_uses_harbors_pinned_installer(tmp_path):
    command = _delegate(tmp_path, delegate="always").claude_install_command()
    assert "bootstrap.sh | bash -s -- 2.1.280" in command
    assert "@anthropic-ai/claude-code@2.1.280" in command
    assert command.endswith("claude --version")


class _Result:
    def __init__(self, stdout: str = "", return_code: int = 0):
        self.stdout = stdout
        self.stderr = ""
        self.return_code = return_code


class _Environment:
    """Records every command; answers the path probe from a script."""

    def __init__(self, probe: str):
        self.commands: list[str] = []
        self.probe = probe

    async def exec(self, command: str, **_: object) -> _Result:
        self.commands.append(command)
        if "readlink" in command:
            return _Result(self.probe)
        return _Result("ok")

    async def upload_file(self, *_: object) -> None:
        pass


def test_delegate_install_refuses_the_wrong_claude_version(tmp_path, monkeypatch):
    agent = _delegate(tmp_path, delegate="always")

    async def nothing(*_: object, **__: object) -> None:
        return None

    monkeypatch.setattr(agent, "ensure_system_dependencies", nothing)
    environment = _Environment(
        "/root/.local/share/claude/versions/2.1.279\n2.1.279 (Claude Code)\n"
    )
    with pytest.raises(EpisodeContractError, match="2.1.279 installed"):
        asyncio.run(agent.install(environment))
    assert any("bootstrap.sh" in command for command in environment.commands)


def test_codex_delegate_needs_no_claude_pin_and_defaults_to_luna(tmp_path):
    agent = _delegate(tmp_path, delegate="always", delegate_agent="codex")
    assert agent._delegate_model == "gpt-6-luna"
    with pytest.raises(EpisodeContractError, match="claude-code or codex"):
        _delegate(tmp_path, delegate="always", delegate_agent="devin")


def test_codex_install_command_pins_the_version(tmp_path):
    command = _delegate(
        tmp_path, delegate="always", delegate_agent="codex"
    ).codex_install_command()
    assert "npm install -g @openai/codex@0.155.1" in command
    assert command.endswith("codex --version")


def test_codex_auth_path_follows_harbors_selectors(tmp_path, monkeypatch):
    agent = _delegate(tmp_path, delegate="always", delegate_agent="codex")
    monkeypatch.delenv("CODEX_AUTH_JSON_PATH", raising=False)
    monkeypatch.delenv("CODEX_FORCE_AUTH_JSON", raising=False)
    assert agent.codex_auth_path() is None
    monkeypatch.setenv("CODEX_FORCE_AUTH_JSON", "1")
    assert str(agent.codex_auth_path()).endswith(".codex/auth.json")
    monkeypatch.setenv("CODEX_AUTH_JSON_PATH", str(tmp_path / "auth.json"))
    assert agent.codex_auth_path() == tmp_path / "auth.json"


class _CodexEnvironment:
    """Answers `codex --version`, records commands, uploads, and envs."""

    default_user = None

    def __init__(self, version: str = "codex-cli 0.155.1"):
        self.version = version
        self.commands: list[str] = []
        self.envs: list[dict] = []
        self.uploads: list[tuple[str, str]] = []

    async def exec(self, command: str, env=None, **_: object) -> _Result:
        self.commands.append(command)
        self.envs.append(dict(env or {}))
        if command.endswith("codex --version") and command.startswith("/usr/local"):
            return _Result(self.version + "\n")
        return _Result("ok")

    async def upload_file(self, source: str, target: str) -> None:
        self.uploads.append((source, target))


def _codex_agent(tmp_path, monkeypatch):
    auth = tmp_path / "auth.json"
    auth.write_text("{}")
    monkeypatch.setenv("CODEX_AUTH_JSON_PATH", str(auth))
    agent = _delegate(tmp_path, delegate="auto", delegate_agent="codex")

    async def nothing(*_: object, **__: object) -> None:
        return None

    monkeypatch.setattr(agent, "ensure_system_dependencies", nothing)
    return agent, auth


def test_codex_install_places_the_auth_file_and_points_the_episode_at_it(
    tmp_path, monkeypatch
):
    agent, auth = _codex_agent(tmp_path, monkeypatch)
    environment = _CodexEnvironment()
    asyncio.run(agent.install(environment))
    assert (str(auth), "/tmp/codex-secrets/auth.json") in environment.uploads
    assert any(
        "chmod 600 /tmp/codex-secrets/auth.json" in command
        and "ln -sf /tmp/codex-secrets/auth.json /tmp/codex-home/auth.json" in command
        for command in environment.commands
    )
    doctor_env = next(
        env
        for command, env in zip(environment.commands, environment.envs)
        if "episode doctor" in command
    )
    assert doctor_env["CODER_ONE_DELEGATE"] == "auto"
    assert doctor_env["CODER_ONE_DELEGATE_AGENT"] == "codex"
    assert doctor_env["CODER_ONE_DELEGATE_MODEL"] == "gpt-6-luna"
    assert doctor_env["CODEX_HOME"] == "/tmp/codex-home"
    assert doctor_env["CODER_ONE_CODEX_BIN"] == "/usr/local/bin/codex"
    assert "CODEX_AUTH_JSON_PATH" not in doctor_env


def test_codex_install_refuses_the_wrong_version_or_a_missing_auth_file(
    tmp_path, monkeypatch
):
    agent, auth = _codex_agent(tmp_path, monkeypatch)
    with pytest.raises(EpisodeContractError, match="0.154.0 installed"):
        asyncio.run(agent.install(_CodexEnvironment("codex-cli 0.154.0")))
    auth.unlink()
    with pytest.raises(EpisodeContractError, match="existing auth.json"):
        asyncio.run(agent.install(_CodexEnvironment()))


def test_an_explore_bound_of_zero_reaches_the_episode(tmp_path):
    agent = _delegate(tmp_path, delegate="always", explore_steps=0)
    agent._claude_bin = "/root/.local/share/claude/versions/2.1.280"
    assert agent._episode_env()["CODER_ONE_EXPLORE_STEPS"] == "0"


LUNA_POLICY = "crates/coder-one/policies/jevprobe3-luna.json"
OPUS_POLICY = "crates/coder-one/policies/jevprobe2-opus-lean-low-5m.json"


def test_a_policy_manifest_decides_the_executor_and_reaches_the_episode_inline(
    tmp_path,
):
    import json

    agent = _delegate(tmp_path, policy=LUNA_POLICY)
    assert agent._delegate == "always"
    assert agent._delegate_agent == "codex"
    assert agent._codex_version == "0.155.1"
    agent._codex_bin = "/usr/local/bin/codex"
    env = agent._episode_env()
    manifest = json.loads(env["CODER_ONE_POLICY"])
    assert manifest["policy"]["executor"]["model"] == "gpt-6-luna"
    assert manifest["policy"]["brief"]["directions"] == "batch-checked"
    # The manifest carries the configuration; the switches stay unset.
    for name in (
        "CODER_ONE_DELEGATE",
        "CODER_ONE_DELEGATE_AGENT",
        "CODER_ONE_DELEGATE_MODEL",
        "CODER_ONE_EXPLORE_STEPS",
        "CODER_ONE_PROBE_V2",
    ):
        assert name not in env
    assert env["CODER_ONE_CODEX_BIN"] == "/usr/local/bin/codex"

    opus = _delegate(tmp_path, policy=OPUS_POLICY)
    assert opus._delegate_agent == "claude-code"
    assert opus._claude_code_version == "2.1.280"


def test_a_kwarg_that_disagrees_with_the_policy_is_refused(tmp_path):
    with pytest.raises(EpisodeContractError, match="disagrees with the policy"):
        _delegate(tmp_path, policy=LUNA_POLICY, delegate_model="gpt-6-sol")
    with pytest.raises(EpisodeContractError, match="disagrees with the policy"):
        _delegate(tmp_path, policy=OPUS_POLICY, claude_code_version="2.1.281")
    # Agreeing kwargs are fine.
    assert _delegate(tmp_path, policy=LUNA_POLICY, delegate_agent="codex")


def test_a_missing_or_foreign_policy_is_refused(tmp_path):
    with pytest.raises(EpisodeContractError, match="cannot read policy"):
        _delegate(tmp_path, policy=str(tmp_path / "absent.json"))
    foreign = tmp_path / "foreign.json"
    foreign.write_text('{"schema": "something-else"}')
    with pytest.raises(EpisodeContractError, match="is not a"):
        _delegate(tmp_path, policy=str(foreign))


def test_switch_arms_record_the_installed_executor_version(tmp_path):
    agent = _delegate(tmp_path, delegate="always", delegate_agent="codex")
    assert agent._episode_env()["CODER_ONE_EXECUTOR_VERSION"] == "0.155.1"
    agent = _delegate(tmp_path, delegate="always")
    agent._claude_bin = "/root/.local/bin/claude"
    assert agent._episode_env()["CODER_ONE_EXECUTOR_VERSION"] == "2.1.280"


def test_the_reference_arms_point_at_their_manifests():
    from tbench.agents import load_agents

    agents = load_agents()
    assert agents["coder-one-jevprobe3-luna"].kwargs == {"policy": LUNA_POLICY}
    assert agents["coder-one-jevprobe2-opus-lean-low-5m"].kwargs == {
        "policy": OPUS_POLICY
    }
