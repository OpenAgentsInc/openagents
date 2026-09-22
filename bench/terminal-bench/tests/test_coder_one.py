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
