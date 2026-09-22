"""JobConfig assembly: pins, defaults, and no secrets."""

import pytest

from tbench.agents import load_agents
from tbench.jobconfig import build_job_config, load_job_profile
from tbench.panel import load_panel


@pytest.fixture()
def panel():
    return load_panel()


@pytest.fixture()
def agents():
    return load_agents()


def test_smoke_profile_tasks():
    profile = load_job_profile("smoke")
    assert profile.task_ids == ("fix-git", "build-cython-ext")
    assert profile.n_attempts == 1
    assert profile.n_concurrent_trials == 1


def test_comparison_profile_repetitions():
    profile = load_job_profile("smoke-comparison")
    assert profile.n_attempts == 3


def test_materialized_config_pins_git(panel, agents, tmp_path):
    profile = load_job_profile("smoke")
    tasks = panel.select(profile.task_ids)
    config = build_job_config(
        panel,
        profile,
        tasks,
        agents["claude-code"],
        checkout=None,
        jobs_dir=tmp_path,
    )
    assert config["n_concurrent_trials"] == 1
    assert config["environment"]["delete"] is True
    assert len(config["tasks"]) == 2
    for entry in config["tasks"]:
        assert entry["git_commit_id"] == panel.git_commit_id
        assert entry["git_url"] == panel.git_url
    agent = config["agents"][0]
    assert agent["name"] == "claude-code"
    assert agent["model_name"] == "claude-sonnet-4-5"


def test_local_checkout_overrides_git_entry(panel, agents, tmp_path):
    profile = load_job_profile("smoke")
    tasks = panel.select(profile.task_ids)
    config = build_job_config(
        panel, profile, tasks, agents["oracle"], checkout=tmp_path
    )
    assert config["tasks"][0]["path"] == str(tmp_path / "archive/fix-git")


def test_v05_requires_artifact_kwargs(panel, agents, tmp_path):
    profile = load_job_profile("smoke")
    tasks = panel.select(profile.task_ids)
    with pytest.raises(ValueError, match="artifact_path"):
        build_job_config(
            panel, profile, tasks, agents["coder-v05"], jobs_dir=tmp_path
        )
    config = build_job_config(
        panel,
        profile,
        tasks,
        agents["coder-v05"],
        agent_kwargs={
            "artifact_path": "/x/coder-v05",
            "artifact_sha256": "ab" * 32,
        },
        jobs_dir=tmp_path,
    )
    agent = config["agents"][0]
    assert agent["import_path"] == "tbench.coder_v05:CoderV05"
    assert agent["kwargs"]["artifact_sha256"] == "ab" * 32


def test_no_secret_values_in_config(panel, agents, tmp_path, monkeypatch):
    monkeypatch.setenv("ANTHROPIC_API_KEY", "sk-live-value")
    profile = load_job_profile("smoke")
    tasks = panel.select(profile.task_ids)
    config = build_job_config(
        panel,
        profile,
        tasks,
        agents["claude-code"],
        auth_mode="api-key",
        jobs_dir=tmp_path,
    )
    assert "sk-live-value" not in repr(config)
    assert config["agents"][0]["env"]["ANTHROPIC_API_KEY"] == (
        "${ANTHROPIC_API_KEY}"
    )
