"""The agent phase's network policy: Harbor's resolution, the allowlist the
plugin enforces, and the record each trial keeps (issue #9589)."""

import json
import warnings
from types import SimpleNamespace

import pytest
from harbor.models.task.config import NetworkMode, NetworkPolicy, TaskConfig
from harbor.models.trial.config import AgentConfig, EnvironmentConfig
from harbor.trial.network_policy import (
    resolve_agent_env_baseline,
    resolve_agent_phase_policy,
)
from harbor.trial.trial import Trial

from tbench import netpolicy
from tbench.agents import load_agents
from tbench.netprobe import parse_probe, verdict

HOSTS = ["openagents.com", "api.typesafe.ai", "chatgpt.com"]


@pytest.fixture(autouse=True)
def _restore_harbor():
    yield
    netpolicy.uninstall()


def _task(**environment) -> TaskConfig:
    return TaskConfig.model_validate({"environment": environment} if environment else {})


def _trial(tmp_path, task: TaskConfig, hosts=HOSTS):
    """Just enough of a Harbor trial for ``Trial._network_plan``."""
    return SimpleNamespace(
        task=SimpleNamespace(config=task),
        config=SimpleNamespace(
            agent=AgentConfig(extra_allowed_hosts=hosts),
            environment=EnvironmentConfig(),
        ),
        paths=SimpleNamespace(trial_dir=tmp_path),
    )


def test_harbor_alone_leaves_a_public_task_public_and_ignores_the_hosts():
    task = _task()
    baseline = resolve_agent_env_baseline(task, EnvironmentConfig())
    with pytest.warns(UserWarning, match="ignored because the effective network policy is public"):
        policy = resolve_agent_phase_policy(task, AgentConfig(extra_allowed_hosts=HOSTS), baseline)
    assert policy.network_mode == NetworkMode.PUBLIC
    assert policy.allowed_hosts == []


def test_narrow_turns_a_public_phase_into_exactly_the_allowed_hosts():
    public = NetworkPolicy(network_mode=NetworkMode.PUBLIC)
    narrowed = netpolicy.narrow(public, HOSTS + ["chatgpt.com"])
    assert narrowed.network_mode == NetworkMode.ALLOWLIST
    assert narrowed.allowed_hosts == HOSTS
    # Nothing to allow, or a phase the task already restricts: unchanged.
    assert netpolicy.narrow(public, []) == public
    closed = NetworkPolicy(network_mode=NetworkMode.NO_NETWORK)
    assert netpolicy.narrow(closed, HOSTS) == closed


def test_the_allowlist_mode_holds_the_agent_phase_and_keeps_the_verifier_baseline(tmp_path):
    netpolicy.install("allowlist")
    with warnings.catch_warnings():
        warnings.simplefilter("error")
        plan = Trial._network_plan(_trial(tmp_path, _task()))
    assert plan.agent_phase.network_mode == NetworkMode.ALLOWLIST
    assert plan.agent_phase.allowed_hosts == HOSTS
    # Setup and the verifier keep the task's own public baseline.
    assert plan.agent_env_baseline.network_mode == NetworkMode.PUBLIC
    assert plan.verifier_phase.network_mode == NetworkMode.PUBLIC
    record = json.loads((tmp_path / netpolicy.RECORD_NAME).read_text())
    assert record["schema"] == netpolicy.RECORD_SCHEMA
    assert record["enforce"] == "allowlist"
    assert record["agent_phase"] == {"network_mode": "allowlist", "allowed_hosts": HOSTS}
    assert record["harbor_agent_phase"]["network_mode"] == "public"
    assert record["agent_phase_public"] is False


def test_the_harbor_mode_only_records_and_flags_a_public_agent_phase(tmp_path):
    netpolicy.install("harbor")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        plan = Trial._network_plan(_trial(tmp_path, _task()))
    assert plan.agent_phase.network_mode == NetworkMode.PUBLIC
    summary = netpolicy.trial_network(tmp_path)
    assert summary["recorded"] is True
    assert summary["agent_phase_public"] is True


def test_a_no_network_task_merges_the_hosts_as_harbor_does(tmp_path):
    netpolicy.install("allowlist")
    plan = Trial._network_plan(_trial(tmp_path, _task(network_mode="no-network")))
    assert plan.agent_env_baseline.network_mode == NetworkMode.NO_NETWORK
    assert plan.agent_phase.network_mode == NetworkMode.ALLOWLIST
    assert plan.agent_phase.allowed_hosts == HOSTS


def test_a_trial_without_the_plugin_records_nothing(tmp_path):
    summary = netpolicy.trial_network(tmp_path)
    assert summary["recorded"] is False
    assert summary["agent_phase_public"] is None


def test_the_plugin_arguments_and_modes():
    assert netpolicy.plugin_args("allowlist") == [
        "--plugin",
        "tbench.netpolicy:AgentNetworkPlugin",
        "--plugin-kwarg",
        "enforce=allowlist",
    ]
    with pytest.raises(ValueError):
        netpolicy.plugin_args("open")
    with pytest.raises(ValueError):
        netpolicy.install("open")


def test_every_coder_one_arm_runs_its_agent_phase_on_the_allowlist():
    agents = load_agents()
    coder_one = [a for a in agents.values() if (a.harbor_import_path or "").startswith("tbench.coder_one:")]
    assert coder_one
    assert all(a.agent_network == "allowlist" for a in coder_one)
    assert agents["coder-one-microluna-v6"].extra_allowed_hosts == tuple(HOSTS)
    assert agents["netprobe"].agent_network == "allowlist"
    # Baselines keep Harbor's resolution.
    assert agents["claude-code"].agent_network == "harbor"


def test_an_allowlist_arm_needs_hosts(tmp_path):
    path = tmp_path / "agents.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": "openagents.tbench.agents.v1",
                "agents": {"x": {"harbor_name": "nop", "agent_network": "allowlist"}},
            }
        )
    )
    with pytest.raises(ValueError, match="needs extra_allowed_hosts"):
        load_agents(path)


def test_the_probe_reads_curl_and_python_lines_and_judges_the_allowlist():
    results = parse_probe(
        "curl https://chatgpt.com/ 403 0 \n"
        "curl https://example.com/ 000 35 curl: (35) Recv failure: Connection reset by peer\n"
        "python3 https://pypi.org/simple/ 000 1 URLError timed out\n"
    )
    assert [r["reached"] for r in results] == [True, False, False]
    expected = {
        "https://chatgpt.com/": "reachable",
        "https://example.com/": "unreachable",
        "https://pypi.org/simple/": "unreachable",
    }
    for row in results:
        row["expected"] = expected[row["url"]]
    held = verdict({"network_mode": "allowlist"}, results)
    assert held["allowlist_enforced"] is True
    assert held["leaked"] == []
    results[1]["reached"] = True
    assert verdict({"network_mode": "allowlist"}, results)["leaked"] == ["https://example.com/"]
    assert verdict({"network_mode": "public"}, results)["allowlist_enforced"] is False
