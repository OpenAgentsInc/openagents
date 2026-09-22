"""Attempt records: provenance, unknown semantics, terminal statuses."""

import json

from tbench.counts import count_trajectory, unknown_counts
from tbench.results import attempt_record


def _trial_result(**overrides):
    base = {
        "id": "trial-1",
        "trial_name": "fix-git__abc__attempt-1",
        "task_name": "fix-git",
        "started_at": "2026-09-22T10:00:00Z",
        "finished_at": "2026-09-22T10:10:00Z",
        "task_checksum": "deadbeef" * 8,
        "task_id": {
            "path": "archive/fix-git",
            "git_url": "https://example/t.git",
            "git_commit_id": "c0ffee" * 6,
        },
        "config": {
            "agent": {"name": "claude-code", "model_name": "claude-sonnet-4-5"},
        },
        "agent_info": {
            "name": "claude-code",
            "version": "2.1.278",
            "model_info": {"name": "claude-sonnet-4-5", "provider": "anthropic"},
        },
        "agent_result": {
            "n_input_tokens": 1200,
            "n_cache_tokens": 40,
            "n_output_tokens": 300,
            "cost_usd": 0.0123,
        },
        "verifier_result": {"rewards": {"reward": 1.0}},
        "environment_setup": {
            "started_at": "2026-09-22T10:00:00Z",
            "finished_at": "2026-09-22T10:01:00Z",
        },
        "agent_execution": {
            "started_at": "2026-09-22T10:02:00Z",
            "finished_at": "2026-09-22T10:09:00Z",
        },
    }
    base.update(overrides)
    return base


def _record(result, **kwargs):
    return attempt_record(
        result,
        job_name="smoke--claude-code",
        trial_dir=kwargs.pop("trial_dir", __import__("pathlib").Path("/x")),
        arm="claude-code",
        profile_id="smoke",
        auth_mode="api-key",
        declared_cost_provenance="provider_reported",
        **kwargs,
    )


def test_record_schema_and_identity(tmp_path):
    record = _record(_trial_result(), trial_dir=tmp_path)
    assert record["schema"] == "openagents.tbench.attempt.v1"
    assert record["task"]["git_commit_id"] == "c0ffee" * 6
    assert record["task"]["checksum"] == "deadbeef" * 8
    assert record["outcome"]["reward"] == 1.0
    assert record["outcome"]["terminal_status"] == "completed"


def test_timing_phases_are_separate(tmp_path):
    record = _record(_trial_result(), trial_dir=tmp_path)
    timing = record["timing"]
    assert timing["environment_setup_ms"] == 60_000
    assert timing["agent_execution_ms"] == 420_000
    assert timing["total_ms"] == 600_000


def test_missing_usage_is_unknown_not_zero(tmp_path):
    result = _trial_result(agent_result={})
    record = _record(result, trial_dir=tmp_path)
    assert record["usage"]["input_tokens"] is None
    assert record["usage"]["coverage"] == "unknown"


def test_partial_usage_marked(tmp_path):
    result = _trial_result(agent_result={"n_input_tokens": 5})
    record = _record(result, trial_dir=tmp_path)
    assert record["usage"]["coverage"] == "partial"


def test_missing_cost_overrides_provenance(tmp_path):
    result = _trial_result(agent_result={"cost_usd": None})
    record = _record(result, trial_dir=tmp_path)
    assert record["cost"]["amount_usd"] is None
    assert record["cost"]["provenance"] == "unknown"


def test_timeout_and_refusal_statuses(tmp_path):
    timed_out = _trial_result(
        exception_info={"exception_type": "AgentTimeoutError", "message": "x"}
    )
    assert _record(timed_out, trial_dir=tmp_path)["outcome"]["terminal_status"] == "timeout"

    refused = _trial_result(
        exception_info={"exception_type": "AgentSafetyRefusalError"}
    )
    assert _record(refused, trial_dir=tmp_path)["outcome"]["terminal_status"] == "provider_refusal"

    no_verifier = _trial_result(verifier_result=None)
    assert _record(no_verifier, trial_dir=tmp_path)["outcome"]["terminal_status"] == "unverifiable"


def test_atif_counts_semantics():
    trajectory = {
        "steps": [
            {
                "source": "agent",
                "tool_calls": [
                    {"tool_name": "bash"},
                    {"tool_name": "read_file"},
                ],
            },
            {"source": "system"},
            {
                "source": "agent",
                "tool_calls": [{"tool_name": "bash", "is_retry": True}],
                "extra": {"decision_call": True},
            },
        ]
    }
    counts = count_trajectory(trajectory)
    assert counts["atif_steps"] == 3
    assert counts["tool_calls"] == 3
    assert counts["shell_commands"] == 2
    assert counts["retries"] == 1
    assert counts["typed_decisions"] == 1


def test_unknown_counts_never_zero():
    counts = unknown_counts("no trajectory.json retained")
    assert counts["atif_steps"] == "unknown"
    assert counts["tool_calls"] == "unknown"
