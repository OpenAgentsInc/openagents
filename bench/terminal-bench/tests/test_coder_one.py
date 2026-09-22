"""The Coder One adapter: the v0.5 contract under its own arm and path."""

import hashlib

import pytest

from tbench.coder_one import CoderOne
from tbench.coder_v05 import ArtifactIdentityError, CoderV05


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
