"""The early-stopping rule, and the scheduler applying it."""

import json
from pathlib import Path

import pytest

from tbench import stop_rule
from tbench.cli import build_parser
from tbench.suite import FINISHED, SKIPPED

from test_experiment import _experiment, _finish, _session

CASES = Path(__file__).parent / "fixtures" / "stop-rule" / "cases.json"


@pytest.mark.parametrize(
    "case", json.loads(CASES.read_text())["cases"], ids=lambda case: case["name"]
)
def test_every_shared_case_gives_the_expected_verdict(case):
    # crates/gym/src/terminal_bench_stop.rs reads the same file.
    data, alpha, bar = stop_rule.Input.from_json(case["input"])
    verdict = stop_rule.evaluate(data, alpha=alpha, accept_pass_rate=bar)
    expect = case["expect"]
    assert verdict["ended"] == expect["ended"]
    assert verdict["verdict"] == expect["verdict"]
    states = {arm["arm"]: arm["state"] for arm in verdict["arms"]}
    for arm, state in expect["arms"].items():
        assert states[arm] == state, arm


def test_the_exact_binomial_test_matches_known_values():
    assert stop_rule.binomial_two_sided(0, 0) == 1.0
    assert stop_rule.binomial_two_sided(5, 5) == pytest.approx(0.0625)
    assert stop_rule.binomial_two_sided(2, 10) == pytest.approx(112 / 1024)


def _run(scheduler, launcher, outcome, cost=None):
    """Run the schedule two trials at a time; ``outcome`` gives each arm's
    reward and ``cost`` each arm's Claude cost, $1 by default."""
    scheduler.reconcile()
    while not scheduler.done():
        for trial in scheduler.launch_ready():
            usd = (cost or {}).get(trial.arm, 1.0)
            _finish(launcher, trial.job, reward=outcome[trial.arm], stream=_session(usd))
        scheduler.poll()


def test_a_decided_experiment_stops_and_records_why(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=10, max_concurrent=2)
    _run(scheduler, launcher, {"plain": 0.0, "coder": 1.0})
    states = [t.state for t in scheduler.trials]
    # Nine pairs all one way separate the arms whatever the tenth does:
    # 2 × P(X ≤ 1 | 10) = 0.021.
    assert states.count(FINISHED) == 18
    assert states.count(SKIPPED) == 2
    assert all("stopped early" in t.reason for t in scheduler.trials if t.state == SKIPPED)
    ledger = [json.loads(line) for line in (tmp_path / "experiment/ledger.jsonl").read_text().splitlines()]
    stops = [r for r in ledger if r["event"] == "stop"]
    assert [(r["scope"], r["arm"], r["state"]) for r in stops] == [
        ("arm", "coder", "decided"),
        ("experiment", None, "decided"),
    ]
    assert stops[0]["after_graded"] == 18
    assert len(stops[0]["skipped"]) == 1 and len(stops[1]["skipped"]) == 1
    status = scheduler.status()
    assert status["state"] == "done"
    assert status["stop_early"]["enabled"] is True
    assert status["stop_early"]["stopped_arms"]["coder"]["state"] == "decided"
    assert status["stop_early"]["ended"]["state"] == "decided"
    # A restart keeps the stop: the skipped trials stay skipped.
    again, _ = _experiment(tmp_path, tasks=("alpha",), attempts=10, max_concurrent=2)
    again.reconcile()
    assert [t.state for t in again.trials].count(SKIPPED) == 2
    assert again.launch_ready() == []


def test_no_stop_early_runs_every_planned_attempt(tmp_path):
    scheduler, launcher = _experiment(
        tmp_path, tasks=("alpha",), attempts=10, max_concurrent=2, stop_early=False
    )
    _run(scheduler, launcher, {"plain": 0.0, "coder": 1.0})
    assert [t.state for t in scheduler.trials].count(FINISHED) == 20
    assert not (tmp_path / "experiment/ledger.jsonl").exists()
    assert scheduler.status()["stop_early"]["enabled"] is False


def test_an_arm_below_the_acceptance_bar_stops(tmp_path):
    scheduler, launcher = _experiment(
        tmp_path, tasks=("alpha",), attempts=10, max_concurrent=2, accept_pass_rate=0.5
    )
    # The candidate is cheaper, so falling behind doesn't dominate it.
    _run(scheduler, launcher, {"plain": 1.0, "coder": 0.0}, cost={"coder": 0.5})
    ledger = [json.loads(line) for line in (tmp_path / "experiment/ledger.jsonl").read_text().splitlines()]
    assert ("coder", "below_bar") in [(r["arm"], r["state"]) for r in ledger if r["event"] == "stop"]
    # Six failures leave at most four of ten: under half.
    assert [t.state for t in scheduler.trials].count(FINISHED) == 12


def test_an_arm_that_cant_catch_up_and_costs_no_less_is_dominated(tmp_path):
    scheduler, launcher = _experiment(tmp_path, tasks=("alpha",), attempts=10, max_concurrent=2)
    _run(scheduler, launcher, {"plain": 1.0, "coder": 0.0}, cost={"coder": 2.0})
    ledger = [json.loads(line) for line in (tmp_path / "experiment/ledger.jsonl").read_text().splitlines()]
    assert [(r["arm"], r["state"]) for r in ledger if r["event"] == "stop"] == [
        ("coder", "dominated"),
        (None, "dominated"),
    ]


def test_the_flags_default_to_stopping_early():
    parser = build_parser()
    args = parser.parse_args(["experiment", "run", "--id", "x"])
    assert args.stop_early is True and args.stop_alpha == 0.05 and args.accept_pass_rate is None
    args = parser.parse_args(["experiment", "plan", "--id", "x", "--no-stop-early",
                              "--accept-pass-rate", "0.6"])
    assert args.stop_early is False and args.accept_pass_rate == 0.6
