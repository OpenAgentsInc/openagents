"""The host's oracle step: written before the trial, delivered read-only,
and counted in the trial's cost. A fake writer stands in for coder-one:
no Docker, no model."""

import asyncio
import hashlib
import json
import subprocess
import sys
from decimal import Decimal
from pathlib import Path
from types import SimpleNamespace

import pytest

from tbench import cohort, oracle_host
from tbench.coder_one import CoderOneTunable

REPO = Path(__file__).resolve().parents[3]
DIGEST = "d" * 64


def _step(out: Path, *, digest=DIGEST, status="written", luna_known=True, luna=0.004, jev=0.0003,
          oracle_digest=None, exit_code=0):
    """What a fake coder-one leaves in ``out``."""
    out.mkdir(parents=True, exist_ok=True)
    record = {
        "schema": "openagents.coder-one.oracle-host.v1",
        "status": status,
        "why": None if digest else "the writer left no oracle.py",
        "digest": digest,
        "cost": {"luna_usd": luna, "luna_known": luna_known, "luna_bound_usd": 0.08, "jev_usd": jev},
    }
    (out / "record.json").write_text(json.dumps(record))
    if digest:
        (out / "oracle.json").write_text(json.dumps({"digest": oracle_digest or digest, "files": {}}))
        (out / "spec.json").write_text(json.dumps({"digest": "s" * 64}))
    return subprocess.CompletedProcess([], exit_code, "", "")


def _task(tmp_path: Path) -> Path:
    task = tmp_path / "tasks" / "sum-task"
    task.mkdir(parents=True)
    (task / "instruction.md").write_text("Print the sum.\n")
    return task


def _writer(tmp_path: Path) -> str:
    path = tmp_path / "coder-one-host"
    path.write_text("#!/bin/sh\n")
    return str(path)


def test_settings_come_from_the_lean_oracle_switch():
    policy = {"policy": {"executor": {"microluna": {"lean": {"oracle": {"writer_usd": 0.05}}}}}}
    assert oracle_host.settings(policy) == {"writer_usd": 0.05}
    assert oracle_host.settings({"policy": {"executor": {"microluna": {"lean": {}}}}}) is None
    assert oracle_host.settings(None) is None


def test_a_written_oracle_is_delivered_with_its_digest_and_known_cost(tmp_path):
    calls = []

    def run(command, **kwargs):
        calls.append((command, kwargs))
        return _step(tmp_path / "out")

    record = oracle_host.write(_writer(tmp_path), _task(tmp_path), "hb__sum-task", tmp_path / "out",
                               {"writer_usd": 0.08, "writer_turns": 20, "writer_sec": 600}, 600.0,
                               {"TYPESAFE_API_KEY": "x"}, run=run)
    assert record["status"] == "delivered"
    assert record["digest"] == DIGEST
    command, kwargs = calls[0]
    assert command[1:5] == ["checks", "oracle", "write", str(tmp_path / "tasks" / "sum-task")]
    assert command[command.index("--image") + 1] == "hb__sum-task"
    assert command[command.index("--session-usd") + 1] == "0.08"
    assert command[command.index("--writer-turns") + 1] == "20"
    # The writer's bound fits inside the agent's setup time.
    assert int(command[command.index("--writer-sec") + 1]) == 600 - 150 - 60
    assert kwargs["timeout"] == 390 + 60
    assert set(record["files"]) == {"oracle.json", "spec.json"}
    assert record["files"]["oracle.json"] == hashlib.sha256(
        (tmp_path / "out" / "oracle.json").read_bytes()).hexdigest()
    assert record["cost"] == {"rule": "oracle-host-v1", "recorded_usd": "0.0043",
                              "counted_usd": "0.0043", "known": True}
    assert oracle_host.episode_env(record) == {
        "CODER_ONE_ORACLE_DIR": "/opt/openagents/oracle",
        "CODER_ONE_ORACLE_DIGEST": DIGEST,
    }


def test_a_step_without_an_oracle_is_unavailable_and_still_counted(tmp_path):
    record = oracle_host.write(_writer(tmp_path), _task(tmp_path), "img", tmp_path / "out",
                               {"writer_usd": 0.08}, 900.0, {},
                               run=lambda *a, **k: _step(tmp_path / "out", digest=None, exit_code=1))
    assert record["status"] == "unavailable"
    assert record["why"] == "the writer left no oracle.py"
    assert record["cost"]["counted_usd"] == "0.0043"
    assert oracle_host.episode_env(record) == {"CODER_ONE_ORACLE_DIGEST": "unavailable"}


def test_a_writer_whose_cost_is_unknown_counts_its_whole_bound(tmp_path):
    record = oracle_host.write(_writer(tmp_path), _task(tmp_path), "img", tmp_path / "out",
                               {"writer_usd": 0.08}, 900.0, {},
                               run=lambda *a, **k: _step(tmp_path / "out", luna_known=False, luna=0.01))
    assert record["status"] == "delivered"
    assert record["cost"]["known"] is False
    assert record["cost"]["recorded_usd"] is None
    assert Decimal(record["cost"]["counted_usd"]) == Decimal("0.08") + Decimal("0.0003")


def test_a_step_past_its_time_bound_counts_both_bounds(tmp_path):
    def run(command, **kwargs):
        raise subprocess.TimeoutExpired(command, kwargs["timeout"])

    record = oracle_host.write(_writer(tmp_path), _task(tmp_path), "img", tmp_path / "out",
                               {"writer_usd": 0.05}, 900.0, {}, run=run)
    assert record["status"] == "unavailable"
    assert Decimal(record["cost"]["counted_usd"]) == Decimal("0.05") + oracle_host.JEV_BOUND_USD


def test_an_oracle_file_that_disagrees_with_the_record_is_not_delivered(tmp_path):
    record = oracle_host.write(_writer(tmp_path), _task(tmp_path), "img", tmp_path / "out", {}, 900.0, {},
                               run=lambda *a, **k: _step(tmp_path / "out", oracle_digest="e" * 64))
    assert record["status"] == "unavailable"
    assert "digest" in record["why"]


def test_refusals_before_the_step_runs_cost_nothing(tmp_path):
    def never(*_, **__):
        raise AssertionError("the step must not run")

    task = _task(tmp_path)
    writer = _writer(tmp_path)
    cases = [
        (None, task, "img", 900.0, "no coder-one binary"),
        (writer, None, "img", 900.0, "task's directory"),
        (writer, task, None, 900.0, "image isn't known"),
        (writer, task, "img", 250.0, "setup time"),
    ]
    for binary, task_dir, image, setup, why in cases:
        record = oracle_host.write(binary, task_dir, image, tmp_path / "out", {}, setup, {}, run=never)
        assert record["status"] == "unavailable"
        assert why in record["why"]
        assert record["cost"]["counted_usd"] == "0"


def test_the_setup_budget_comes_from_the_lock(tmp_path):
    logs = tmp_path / "trial" / "agent"
    logs.mkdir(parents=True)
    assert oracle_host.setup_budget_sec(logs) == 360.0
    (logs.parent / "lock.json").write_text(json.dumps(
        {"agent": {"override_setup_timeout_sec": 1200}, "agent_setup_timeout_multiplier": 1.5}))
    assert oracle_host.setup_budget_sec(logs) == 1800.0


def test_the_image_is_the_one_harbor_started_the_trial_from():
    prebuilt = SimpleNamespace(_use_prebuilt=True, task_env_config=SimpleNamespace(docker_image="org/task:1"),
                               _main_image_name="hb__x")
    built = SimpleNamespace(_use_prebuilt=False, task_env_config=SimpleNamespace(docker_image=None),
                            _main_image_name="hb__x")
    assert oracle_host.task_image(prebuilt) == "org/task:1"
    assert oracle_host.task_image(built) == "hb__x"
    assert oracle_host.task_image(SimpleNamespace()) is None


class _Environment:
    def __init__(self):
        self.commands: list[str] = []
        self.uploads: list[tuple[str, str]] = []
        self.default_user = None
        self._use_prebuilt = True
        self.task_env_config = SimpleNamespace(docker_image="org/sum-task:1")

    async def exec(self, command: str, **_: object):
        self.commands.append(command)
        return SimpleNamespace(stdout="", stderr="", return_code=0)

    async def upload_file(self, source, target):
        self.uploads.append((str(source), str(target)))


FAKE_WRITER = """#!{python}
import json, sys
args = sys.argv[1:]
out = args[args.index("--out") + 1]
image = args[args.index("--image") + 1]
import pathlib
out = pathlib.Path(out); out.mkdir(parents=True, exist_ok=True)
digest = "f" * 64
(out / "oracle.json").write_text(json.dumps({{"digest": digest, "image": image}}))
(out / "spec.json").write_text(json.dumps({{"digest": "s" * 64}}))
(out / "record.json").write_text(json.dumps({{"status": "written", "digest": digest,
    "cost": {{"luna_usd": 0.01, "luna_known": True, "luna_bound_usd": 0.08, "jev_usd": 0.0005}}}}))
"""


def test_the_adapter_writes_on_the_host_delivers_read_only_and_counts_the_cost(tmp_path):
    manifest = json.loads((REPO / "crates/coder-one/policies/microluna-v17.json").read_text())
    manifest["policy"]["executor"]["microluna"]["lean"]["oracle"] = {"writer_usd": 0.08}
    policy = tmp_path / "policy.json"
    policy.write_text(json.dumps(manifest))
    task = _task(tmp_path)
    trial = tmp_path / "trial"
    logs = trial / "agent"
    logs.mkdir(parents=True)
    (trial / "config.json").write_text(json.dumps({"task": {"path": str(task)}}))
    (trial / "lock.json").write_text(json.dumps({"agent": {"override_setup_timeout_sec": 900}}))
    writer = tmp_path / "fake-coder-one"
    writer.write_text(FAKE_WRITER.format(python=sys.executable))
    writer.chmod(0o755)
    binary = tmp_path / "coder-one"
    binary.write_bytes(b"#!/bin/sh\n")
    agent = CoderOneTunable(
        logs_dir=logs,
        artifact_path=str(binary),
        artifact_sha256=hashlib.sha256(b"#!/bin/sh\n").hexdigest(),
        policy=str(policy),
        oracle_writer=str(writer),
    )
    environment = _Environment()
    asyncio.run(agent._write_oracle(environment))
    record = json.loads((logs / "oracle-host.json").read_text())
    assert record["status"] == "delivered"
    assert record["image"] == "org/sum-task:1"
    assert json.loads((logs / "oracle" / "oracle.json").read_text())["image"] == "org/sum-task:1"
    asyncio.run(oracle_host.deliver(agent, environment, logs / "oracle", record))
    assert sorted(target for _, target in environment.uploads) == [
        "/opt/openagents/oracle/oracle.json", "/opt/openagents/oracle/spec.json"]
    assert any("chmod 0444" in c and "chown -R 0:0" in c for c in environment.commands)
    env = agent._episode_env()
    assert env["CODER_ONE_ORACLE_DIGEST"] == "f" * 64
    assert env["CODER_ONE_ORACLE_DIR"] == "/opt/openagents/oracle"
    # The trial's cost gains the host step's.
    usage = logs / "episode" / "evaluation"
    usage.mkdir(parents=True)
    (usage / "usage.json").write_text(json.dumps({"cost": {"amount_usd": 0.05}, "tokens": {}}))
    context = SimpleNamespace(cost_usd=None, metadata=None, n_input_tokens=None,
                              n_cache_tokens=None, n_output_tokens=None)
    agent.populate_context_post_run(context)
    assert context.cost_usd == pytest.approx(0.0605)
    assert context.metadata["oracle"]["status"] == "delivered"


def test_the_cohort_counts_the_host_step_with_the_trial():
    rule = {"version": cohort.RULE, "luna_bound_usd": "0.09"}
    oracle = {"status": "delivered", "digest": DIGEST,
              "cost": {"recorded_usd": "0.0043", "counted_usd": "0.0043", "known": True}}
    priced = cohort.price({"cost": {"amount_usd": 0.05}}, rule, oracle=oracle)
    assert priced["counted_usd"] == "0.0543"
    assert priced["recorded_usd"] == "0.0543"
    assert priced["oracle_host"]["digest"] == DIGEST
    # A writer whose cost wasn't known counts its bound; the total isn't
    # recorded, only bounded below by what was.
    unknown = {"status": "unavailable", "digest": None,
               "cost": {"recorded_usd": None, "counted_usd": "0.0803", "known": False}}
    priced = cohort.price({"cost": {"amount_usd": 0.05}}, rule, oracle=unknown)
    assert priced["counted_usd"] == "0.1303"
    assert priced["recorded_usd"] is None
    assert priced["lower_bound_usd"] == "0.05"
    # A trial that ended before the agent ran still paid for the step.
    priced = cohort.price(None, rule, before_agent=True, oracle=oracle)
    assert priced["kind"] == "host-oracle-before-agent"
    assert priced["counted_usd"] == "0.0043"
    # No step, no change.
    assert cohort.price({"cost": {"amount_usd": 0.05}}, rule) == cohort.price(
        {"cost": {"amount_usd": 0.05}}, rule, oracle=None)
