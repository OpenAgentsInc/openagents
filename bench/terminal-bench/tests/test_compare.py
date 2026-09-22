"""Comparison reports: pairing on identical pins, unknown honesty."""

import json

from tbench.compare import SMALL_SAMPLE_LABEL, compare, render_table


def _attempt(tmp_path, job, name, arm, task, reward, status="completed",
             commit="abc123", checksum="sum1", cost=None):
    record = {
        "schema": "openagents.tbench.attempt.v1",
        "attempt": {
            "id": name,
            "job": job,
            "trial": name,
            "arm": arm,
            "profile": "smoke",
            "auth_mode": None,
            "kind": "fresh",
        },
        "task": {
            "name": task,
            "path": f"archive/{task}",
            "git_commit_id": commit,
            "checksum": checksum,
        },
        "agent": {"selector": arm},
        "outcome": {"reward": reward, "terminal_status": status},
        "timing": {"agent_execution_ms": 1000, "total_ms": 2000},
        "usage": {"input_tokens": 10, "output_tokens": 5, "coverage": "full"},
        "cost": {"amount_usd": cost, "provenance": "unknown" if cost is None else "price_estimate"},
        "counts": {"atif_steps": 4, "tool_calls": 3, "semantics": "test"},
        "completeness": {"trace": "present", "usage": "full", "cost": "reported", "artifacts": "present"},
        "evidence": {},
    }
    dest = tmp_path / job / "tbench" / "attempts"
    dest.mkdir(parents=True, exist_ok=True)
    (dest / f"{name}.json").write_text(json.dumps(record))


def test_compare_groups_by_task_and_arm(tmp_path):
    _attempt(tmp_path, "j1", "a1", "claude-code", "fix-git", 1.0, cost=0.01)
    _attempt(tmp_path, "j1", "a2", "claude-code", "fix-git", 0.0, cost=0.02)
    _attempt(tmp_path, "j2", "b1", "codex", "fix-git", 1.0)
    report = compare(tmp_path)
    assert report["schema"] == "openagents.tbench.report.v1"
    assert report["label"] == SMALL_SAMPLE_LABEL
    group = report["tasks"][0]
    assert group["task"] == "fix-git"
    arms = {a["arm"]: a for a in group["arms"]}
    assert arms["claude-code"]["attempts"] == 2
    assert arms["claude-code"]["reward_mean"] == 0.5
    assert arms["codex"]["attempts"] == 1


def test_mixed_pins_flagged_not_pooled(tmp_path):
    _attempt(tmp_path, "j1", "a1", "claude-code", "fix-git", 1.0, commit="aaa")
    _attempt(tmp_path, "j2", "b1", "codex", "fix-git", 1.0, commit="bbb")
    report = compare(tmp_path)
    assert any("mixed commits" in w for w in report["pin_warnings"])


def test_failed_attempts_stay_visible(tmp_path):
    _attempt(tmp_path, "j1", "a1", "claude-code", "fix-git", None, status="timeout")
    report = compare(tmp_path)
    group = report["tasks"][0]["arms"][0]
    assert group["terminal_statuses"] == ["timeout"]
    assert group["rewards"] == [None]
    table = render_table(report)
    assert "timeout" in table
    assert "unknown" in table


def _pinned(tmp_path, job, name, arm, reward, *, version="1.0", model="m",
            sha=None, kind="fresh", cost=None, agent_ms=1000):
    _attempt(tmp_path, job, name, arm, "fix-git", reward, cost=cost,
             status="completed" if reward is not None else "timeout")
    path = tmp_path / job / "tbench" / "attempts" / f"{name}.json"
    record = json.loads(path.read_text())
    record["agent"].update(
        {"observed_version": version, "observed_model": model, "artifact_sha256": sha}
    )
    record["attempt"]["kind"] = kind
    record["timing"]["agent_execution_ms"] = agent_ms
    path.write_text(json.dumps(record))


def test_wilson_interval_values():
    from tbench.compare import wilson_interval

    assert wilson_interval(1, 1) is None
    assert wilson_interval(0, 0) is None
    low, high = wilson_interval(2, 2)
    assert round(low, 3) == 0.342 and high == 1.0
    low, high = wilson_interval(0, 3)
    assert low == 0.0 and round(high, 3) == 0.561
    low, high = wilson_interval(2, 3)
    assert round(low, 3) == 0.208 and round(high, 3) == 0.939


def test_repetitions_pool_across_job_names(tmp_path):
    _pinned(tmp_path, "smoke--x--fix-git", "t1", "x", 1.0, cost=0.10, agent_ms=1000)
    _pinned(tmp_path, "smoke--x--fix-git-2", "t2", "x", 0.0, cost=0.30, agent_ms=3000)
    _pinned(tmp_path, "smoke--x--fix-git-3", "t3", "x", 1.0, cost=0.20, agent_ms=2000)
    report = compare(tmp_path)
    [cell] = report["tasks"][0]["arms"]
    assert cell["attempts"] == 3
    assert cell["passes"] == 2 and cell["scored"] == 3
    assert cell["jobs"] == [
        "smoke--x--fix-git",
        "smoke--x--fix-git-2",
        "smoke--x--fix-git-3",
    ]
    assert cell["pass_rate_interval"]["method"] == "wilson"
    assert round(cell["pass_rate_interval"]["low"], 3) == 0.208
    spread = cell["cost_usd_spread"]
    assert spread["n"] == 3 and round(spread["mean"], 6) == 0.2
    assert spread["min"] == 0.1 and spread["max"] == 0.3
    assert cell["agent_wall_ms_spread"] == {
        "n": 3, "mean": 2000, "min": 1000, "max": 3000,
    }
    table = render_table(report)
    assert "2/3 [0.21, 0.94]" in table
    assert "2.0s (1.0s-3.0s)" in table
    assert "$0.2000 ($0.1000-$0.3000) price_estimate" in table


def test_different_pins_never_pool(tmp_path):
    _pinned(tmp_path, "j1", "a", "x", 1.0, sha="aa" * 32)
    _pinned(tmp_path, "j2", "b", "x", 0.0, sha="bb" * 32)
    _pinned(tmp_path, "j3", "c", "x", 1.0, version="2.0", sha="aa" * 32)
    _pinned(tmp_path, "j4", "d", "x", 1.0, model="other", sha="aa" * 32)
    report = compare(tmp_path)
    cells = report["tasks"][0]["arms"]
    assert len(cells) == 4
    assert all(c["attempts"] == 1 and c["arm_split_by_pin"] for c in cells)
    table = render_table(report)
    assert "artifact sha256 aaaaaaaaaaaa" in table
    assert "artifact sha256 bbbbbbbbbbbb" in table


def test_single_trial_is_labelled_without_interval(tmp_path):
    _pinned(tmp_path, "j1", "a", "x", 1.0)
    report = compare(tmp_path)
    [cell] = report["tasks"][0]["arms"]
    assert cell["single_trial"] is True
    assert cell["pass_rate_interval"] is None
    assert "1/1 single trial" in render_table(report)


def test_interrupted_attempts_counted_apart(tmp_path):
    _pinned(tmp_path, "j1", "a", "x", 1.0)
    _pinned(tmp_path, "j1", "b", "x", None, kind="interrupted")
    _pinned(tmp_path, "j1", "c", "x", None)
    report = compare(tmp_path)
    [cell] = report["tasks"][0]["arms"]
    assert cell["attempts"] == 2
    assert cell["interrupted"] == 1
    assert cell["scored"] == 1
    assert "+1 interrupted" in render_table(report)
