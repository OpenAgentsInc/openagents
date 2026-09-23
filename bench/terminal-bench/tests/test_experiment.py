"""Targeted experiments: interleaving, credentials, losses, and the quota budget."""

import json
import os

import pytest

from tbench import credentials
from tbench.experiment import (
    ExperimentError,
    ExperimentScheduler,
    Spec,
    interleave,
    job_name,
    pin,
    status_lines,
)
from tbench.suite import FINISHED, PENDING, RUNNING, Budget, inspect_job

from test_suite import FakeHost, FakeLauncher, _task

REVOKED = {
    "type": "result",
    "subtype": "success",
    "is_error": True,
    "api_error_status": 401,
    "result": "Failed to authenticate. API Error: 401 OAuth access token has been revoked.",
    "total_cost_usd": 0.0,
}


def _session(cost: float) -> str:
    return "\n".join(
        json.dumps(event)
        for event in (
            {"type": "system", "subtype": "init", "claude_code_version": "2.1.280"},
            {"type": "result", "is_error": False, "total_cost_usd": cost,
             "usage": {"output_tokens": 100}},
        )
    )


def _finish(launcher, job, *, reward=None, stream=None, exception=None):
    trial = launcher.jobs / job / "trial__abc"
    (trial / "agent").mkdir(parents=True, exist_ok=True)
    if stream is not None:
        (trial / "agent" / "claude-code.txt").write_text(stream)
    result = {
        "exception_info": {"exception_type": exception} if exception else None,
        "verifier_result": {"rewards": {"reward": reward}} if reward is not None else None,
    }
    (trial / "result.json").write_text(json.dumps(result))
    launcher.exit(job)


def _experiment(tmp_path, *, tasks=("alpha", "beta"), attempts=3, quota=None,
                providers=None, slots=0):
    spec = Spec(id="x1", profile="tb4", arms=["plain", "coder"], tasks=list(tasks),
                attempts=attempts, quota_usd=quota)
    jobs = tmp_path / "jobs"
    jobs.mkdir(parents=True, exist_ok=True)
    launcher = FakeLauncher(jobs, tmp_path / "logs")
    host = FakeHost()
    host.lock = tmp_path / "gpu.lock"
    host.claude_slots = tmp_path / "claude-slots"
    # Each task's image is built, so both arms of a task may start at once.
    host.images = {f"{name}__built" for name in tasks}
    scheduler = ExperimentScheduler(
        spec=spec,
        tasks={name: _task(name) for name in tasks},
        arm_providers=providers or {"plain": frozenset({"anthropic"}),
                                    "coder": frozenset({"anthropic"})},
        credential_source="setup-token",
        budget=Budget(max_cpus=24, max_mem_gb=100, order="listed",
                      max_claude_concurrent=slots),
        jobs_dir=jobs,
        directory=tmp_path / "experiment",
        launcher=launcher,
        host_=host.build(),
        clock=lambda: 0.0,
        find_running={}.get,
        failed=tmp_path / "failed",
        pauses=tmp_path / "usage-pauses.json",
        wall=lambda: 1_790_163_000.0,
    )
    return scheduler, launcher


def test_the_schedule_interleaves_arms_and_rotates_who_goes_first():
    order = interleave(["a", "b"], ["plain", "coder"], 3)
    assert len(order) == 12
    assert order[:4] == [(1, "a", "plain"), (1, "a", "coder"),
                         (1, "b", "coder"), (1, "b", "plain")]
    assert order[4:6] == [(2, "a", "coder"), (2, "a", "plain")]
    # Each arm goes first in half the task slots.
    firsts = [order[i][2] for i in range(0, 12, 2)]
    assert firsts.count("plain") == firsts.count("coder") == 3


def test_job_names_keep_the_profile_and_arm_the_evidence_reader_parses():
    assert job_name("tb4", "coder", "cad-model", "x1", 2) == "tb4--coder--cad-model--x1-r2"


def test_a_spec_needs_two_arms_and_is_pinned_on_the_first_start(tmp_path):
    with pytest.raises(ExperimentError, match="at least two arms"):
        Spec(id="x", profile="tb4", arms=["a"], tasks=["t"]).validate()
    with pytest.raises(ExperimentError, match="without '--'"):
        Spec(id="a--b", profile="tb4", arms=["a", "b"], tasks=["t"]).validate()
    spec = Spec(id="x", profile="tb4", arms=["a", "b"], tasks=["t"])
    assert spec.attempts == 3
    pin(spec, tmp_path)
    # The quota budget may change on a restart; the design may not.
    pin(Spec(id="x", profile="tb4", arms=["a", "b"], tasks=["t"], quota_usd=9), tmp_path)
    with pytest.raises(ExperimentError, match="attempts"):
        pin(Spec(id="x", profile="tb4", arms=["a", "b"], tasks=["t"], attempts=1), tmp_path)


def test_an_arm_can_run_a_profile_under_its_own_name(tmp_path):
    from tbench.suite import Launcher

    spec = Spec(
        id="prop-1", profile="tb4", arms=["base", "prop-1"], tasks=["t"],
        arm_profiles={"prop-1": "base"},
    )
    spec.validate()
    assert spec.profile_of("prop-1") == "base" and spec.profile_of("base") == "base"
    pin(spec, tmp_path)
    assert json.loads((tmp_path / "experiment.json").read_text())["arm_profiles"] == {
        "prop-1": "base"
    }
    with pytest.raises(ExperimentError, match="isn't an arm"):
        Spec(id="x", profile="tb4", arms=["a", "b"], tasks=["t"],
             arm_profiles={"c": "a"}).validate()
    # A spec without aliases pins as it did before them.
    assert "arm_profiles" not in Spec(id="x", profile="tb4", arms=["a", "b"], tasks=["t"]).pinned()
    launcher = Launcher(profile="tb4", arm="base", logs=tmp_path, arm_profiles={"prop-1": "base"})
    trial = type("T", (), {"arm": "prop-1", "task": type("K", (), {"id": "t"})(), "job": "j"})()
    command = launcher.command(trial, "run")
    assert command[command.index("--agent") + 1] == "base"


def test_trials_start_in_the_interleaved_order_and_record_claude_quota(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=2)
    scheduler.reconcile()
    started = [t.job for t in scheduler.launch_ready()]
    assert started[:2] == ["tb4--plain--alpha--x1-r1", "tb4--coder--alpha--x1-r1"]
    _finish(launcher, "tb4--plain--alpha--x1-r1", reward=1.0, stream=_session(1.25))
    scheduler.poll()
    status = scheduler.status()
    assert status["schema"] == "openagents.tbench.experiment-status.v1"
    assert status["quota"]["used"]["usd"] == 1.25
    assert status["quota"]["by_arm"]["plain"]["usd"] == 1.25
    row = status["trials"][0]
    assert (row["arm"], row["attempt"], row["state"], row["reward"]) == ("plain", 1, FINISHED, 1.0)
    assert row["claude_usage"]["sessions"] == 1


def test_a_revoked_token_is_a_credential_loss_that_blocks_further_claude_trials(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=1)
    scheduler.reconcile()
    scheduler.launch_ready()
    job = "tb4--plain--alpha--x1-r1"
    # The verifier graded an untouched container: reward 0, not a result.
    _finish(launcher, job, reward=0.0, stream=json.dumps(REVOKED))
    assert inspect_job(launcher.jobs / job).kind == "credentials"
    scheduler.poll()
    trial = scheduler.trials[0]
    assert (trial.state, trial.credential_failures, trial.reward) == (PENDING, 1, None)
    assert "anthropic" in scheduler.blocked
    assert scheduler.launch_ready() == []
    ledger = [json.loads(l) for l in (tmp_path / "experiment/ledger.jsonl").read_text().splitlines()]
    assert [(r["job"], r["cause"]) for r in ledger] == [(job, "credentials")]
    assert "revoked" in ledger[0]["message"]
    # The trial still running finishes; then the held schedule ends.
    _finish(launcher, "tb4--coder--alpha--x1-r1", reward=1.0, stream=_session(0.5))
    scheduler.poll()
    assert scheduler.done()
    assert scheduler.status()["state"] == "held"
    assert any("blocked" in line for line in status_lines(scheduler.status()))


def test_losses_survive_a_restart_from_the_ledger(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=1)
    scheduler.reconcile()
    scheduler.launch_ready()
    _finish(launcher, "tb4--plain--alpha--x1-r1", stream=json.dumps(REVOKED),
            exception="NonZeroAgentExitCodeError")
    scheduler.poll()
    again, _ = _experiment(tmp_path, tasks=("alpha",), attempts=1)
    assert [loss["cause"] for loss in again.losses["tb4--plain--alpha--x1-r1"]] == ["credentials"]


def test_the_quota_budget_stops_new_claude_trials(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha", "beta"), attempts=1, quota=3.0)
    scheduler.reconcile()
    started = [t.job for t in scheduler.launch_ready()]
    assert len(started) == 4
    for job in started[:2]:
        _finish(launcher, job, reward=1.0, stream=_session(2.0))
    scheduler.poll()
    status = scheduler.status()
    assert status["quota"]["used"]["usd"] == 4.0
    for job in started[2:]:
        _finish(launcher, job, reward=0.0, stream=_session(2.0))
    scheduler.poll()
    assert scheduler.done()


def test_the_quota_budget_holds_back_trials_the_running_ones_could_exceed(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha", "beta"), attempts=2, quota=5.0)
    scheduler.reconcile()
    scheduler.budget = Budget(max_cpus=24, max_mem_gb=100, order="listed",
                              max_claude_concurrent=0, max_concurrent=1)
    first = scheduler.launch_ready()
    _finish(launcher, first[0].job, reward=1.0, stream=_session(2.0))
    scheduler.poll()
    scheduler.budget = Budget(max_cpus=24, max_mem_gb=100, order="listed",
                              max_claude_concurrent=0)
    # $2 used at $2 a trial: one more fits under $5, a second would not.
    started = scheduler.launch_ready()
    assert len(started) == 1
    pending = next(t for t in scheduler.trials if t.state == PENDING)
    assert "could take" in pending.reason
    assert not scheduler.done()


def test_a_non_claude_experiment_has_no_quota_hold(tmp_path):
    none = frozenset()
    scheduler, launcher = _experiment(
        tmp_path, tasks=("alpha",), attempts=1, quota=0.01,
        providers={"plain": none, "coder": none},
    )
    scheduler.reconcile()
    assert len(scheduler.launch_ready()) == 2


def test_a_usage_limited_trial_is_a_quota_loss(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=1)
    scheduler.reconcile()
    scheduler.launch_ready()
    limited = json.dumps({"type": "result", "is_error": True, "api_error_status": 429,
                          "result": "You've hit your session limit · resets 11:50am (UTC)",
                          "total_cost_usd": 0.75})
    _finish(launcher, "tb4--plain--alpha--x1-r1", reward=0.0, stream=limited)
    scheduler.poll()
    trial = scheduler.trials[0]
    assert (trial.state, trial.usage_limits) == (PENDING, 1)
    loss = scheduler.losses["tb4--plain--alpha--x1-r1"][0]
    assert (loss["cause"], loss["usage"]["usd"]) == ("quota", 0.75)
    assert scheduler.quota_used()["usd"] == 0.75


def test_the_setup_token_status_never_carries_the_value(tmp_path):
    path = tmp_path / "claude-setup-token"
    missing = credentials.setup_token_status(path)
    assert (missing["present"], missing["usable"]) == (False, False)
    path.write_text("sk-ant-oat01-secret\n")
    os.chmod(path, 0o644)
    loose = credentials.setup_token_status(path)
    assert not loose["usable"] and "chmod 600" in loose["problem"]
    os.chmod(path, 0o600)
    good = credentials.setup_token_status(path)
    assert good["usable"] and good["mode"] == "0600"
    assert "secret" not in json.dumps([missing, loose, good])


def test_the_doctor_reports_the_token_without_reading_it_aloud(tmp_path):
    from tbench.doctor import PASS, WARN, Report, check_claude_setup_token

    path = tmp_path / "claude-setup-token"
    report = Report()
    assert check_claude_setup_token(report, path).status == WARN
    path.write_text("sk-ant-oat01-secret")
    os.chmod(path, 0o600)
    check = check_claude_setup_token(report, path)
    assert check.status == PASS and "secret" not in check.line()


def test_the_credential_scan_ignores_a_clean_session():
    assert credentials.scan_stream(_session(1.0)) is None
    assert credentials.scan_stream(json.dumps(REVOKED))["status"] == 401


def test_the_experiment_refuses_without_the_long_lived_token(tmp_path, monkeypatch):
    from tbench import cli
    from tbench.runner import RunError

    monkeypatch.setattr(credentials, "SETUP_TOKEN", tmp_path / "missing")
    monkeypatch.setattr(
        credentials, "setup_token_status",
        lambda path=None: {"usable": False, "present": False,
                           "problem": "no long-lived Claude token"},
    )
    agents = {}
    providers = {"plain": frozenset({"anthropic"})}
    with pytest.raises(RunError, match="claude setup-token"):
        cli._experiment_credentials(agents, providers, allow_login=False, strict=True)
    source, warnings = cli._experiment_credentials(
        agents, providers, allow_login=False, strict=False
    )
    assert source is None and "claude setup-token" in warnings[0]
