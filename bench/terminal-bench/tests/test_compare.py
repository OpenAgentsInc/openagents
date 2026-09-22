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
