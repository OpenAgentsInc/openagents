"""Check that streamed updates don't inflate reported model and tool steps."""

import json

from tbench.matched_report import aggregate, native, seconds
from tbench.matched_regrade import ReplayBatchedArtifacts, digests
import pytest


def test_native_counts_deduplicate_stream_updates(tmp_path, monkeypatch):
    monkeypatch.setattr("tbench.matched_report.PACKAGE_DIR", tmp_path / "bench/package")
    message = {"type": "assistant", "message": {"id": "msg-1", "content": [
        {"type": "tool_use", "id": "tool-1", "name": "Bash", "input": {}}]}}
    path = tmp_path / "stream.jsonl"
    path.write_text("\n".join(json.dumps(e) for e in [message, message,
        {"type": "result", "num_turns": 1, "total_cost_usd": 0.0}]) + "\n")
    result = native(path)
    assert result["model_messages"] == 1 and result["tool_calls"] == 1
    assert result["cost_usd"] == 0.0
    assert result["usage_limit"] is None


def test_missing_time_is_unknown():
    assert seconds({"started_at": "2026-09-23T00:00:00Z"}) is None
    assert seconds({"started_at": "2026-09-23T00:00:00Z",
                    "finished_at": "2026-09-23T00:01:01.5Z"}) == 61.5


def test_unknown_charge_is_not_a_free_trial():
    result = aggregate([{"exception": None, "usage_limited": False, "reward": 1,
                         "cost_usd": None, "agent_seconds": 1, "trial_seconds": 2}])
    assert result["passes"] == 1
    assert result["cost_usd"]["total"] is None


def test_regrade_refuses_changed_candidate(tmp_path):
    source = tmp_path / "candidate"
    source.mkdir()
    path = source / "model.py"
    path.write_text("original candidate")
    expected = digests(source)
    path.write_text("changed candidate")
    with pytest.raises(ValueError, match="changed"):
        ReplayBatchedArtifacts(logs_dir=tmp_path / "logs", source=str(source), expected=expected)
