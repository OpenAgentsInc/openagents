"""The v0.5 adapter's no-fallback contract."""

import hashlib

import pytest

from tbench.coder_v05 import (
    ArtifactIdentityError,
    CoderV05,
    EpisodeContractError,
)


def _binary(tmp_path, payload: bytes = b"#!/bin/sh\n") -> tuple[str, str]:
    path = tmp_path / "coder-v05"
    path.write_bytes(payload)
    return str(path), hashlib.sha256(payload).hexdigest()


def test_missing_sha_refused(tmp_path):
    path, _ = _binary(tmp_path)
    with pytest.raises(EpisodeContractError, match="artifact_sha256"):
        CoderV05(logs_dir=tmp_path, artifact_path=path)


def test_missing_binary_refused(tmp_path):
    with pytest.raises(EpisodeContractError, match="not found"):
        CoderV05(
            logs_dir=tmp_path,
            artifact_path=str(tmp_path / "nope"),
            artifact_sha256="ab" * 32,
        )


def test_digest_mismatch_refused(tmp_path):
    path, _ = _binary(tmp_path)
    with pytest.raises(ArtifactIdentityError):
        CoderV05(
            logs_dir=tmp_path,
            artifact_path=path,
            artifact_sha256="00" * 32,
        )


def test_pinned_artifact_accepted(tmp_path):
    path, digest = _binary(tmp_path)
    agent = CoderV05(
        logs_dir=tmp_path,
        artifact_path=path,
        artifact_sha256=digest,
        artifact_version="0.5.0-test",
    )
    assert agent.version() == "0.5.0-test"
    assert agent.name() == "coder-v05"


def test_unknown_contract_refused(tmp_path):
    path, digest = _binary(tmp_path)
    with pytest.raises(EpisodeContractError, match="implements"):
        CoderV05(
            logs_dir=tmp_path,
            artifact_path=path,
            artifact_sha256=digest,
            contract="openagents.coder.episode.v0",
        )


def test_episode_env_forwards_names(tmp_path, monkeypatch):
    path, digest = _binary(tmp_path)
    monkeypatch.setenv("OPENAGENTS_API_KEY", "oak_secret")
    monkeypatch.setenv("OPENAGENTS_DOOR_URL", "http://localhost:9000")
    agent = CoderV05(
        logs_dir=tmp_path, artifact_path=path, artifact_sha256=digest
    )
    env = agent._episode_env()
    assert env["OPENAGENTS_API_KEY"] == "oak_secret"
    assert env["OPENAGENTS_DOOR_URL"] == "http://localhost:9000"
