import json
import threading
import time
from pathlib import Path

import pytest
from harbor.models.task.task import Task

from tbench import candidates, replay
from test_replay import make_task, make_trial


def retained(tmp_path, contents=("same", "same")):
    task = make_task(tmp_path)
    trial = make_trial(tmp_path / "jobs", "j", "demo__1", 0.0, task)
    result = json.loads((trial / "result.json").read_text())
    result["task_checksum"] = Task(task).checksum
    (trial / "result.json").write_text(json.dumps(result))
    moves = []
    parent = trial / "agent/episode/artifacts/lean-1"
    for number, content in enumerate(contents, 1):
        path = parent / f"session-{number}"
        path.mkdir(parents=True)
        (path / "out.txt").write_text(content)
        moves.append({"kind": "lean", "after_session": number, "candidate": str(path),
                      "workspace_files": candidates.recorded_files(candidates.inventory(path)),
                      "score": {"correct": 1, "total": 1}, "kept": True})
    (parent / "selection.json").write_text(json.dumps(moves))
    return trial, task, parent


def grade(task, workspace, out):
    assert task.name == "demo-task"
    assert (workspace.root / "app/out.txt").read_text()
    return {"exit": 0, "exception": None, "reward": 0.0}


def test_duplicates_keep_attribution_and_are_opt_in(tmp_path):
    trial, _, _ = retained(tmp_path)
    first = candidates.batch([trial], tmp_path / "full", runner=grade)
    second = candidates.batch([trial], tmp_path / "dedup", deduplicate=True, runner=grade)
    assert first["verifier_executions"] == 2 and first["reused_grades"] == 0
    assert second["verifier_executions"] == 1 and second["reused_grades"] == 1
    assert second["candidates"][1]["reused_from"] == "candidate-0001"
    assert second["oracle"][0]["any_candidate_passes"] is False
    assert len(second["candidates"]) == 2


@pytest.mark.parametrize("failure", ["exception", "partial"])
def test_infrastructure_failure_is_not_reused_or_counted_as_failure(tmp_path, failure):
    trial, _, _ = retained(tmp_path)
    calls = []

    def flaky(*args):
        calls.append(1)
        if len(calls) == 1:
            if failure == "exception":
                raise RuntimeError("Docker unavailable")
            return {"exit": 1, "reward": 0.0}
        return grade(*args)

    result = candidates.batch([trial], tmp_path / "out", deduplicate=True, runner=flaky)
    assert len(calls) == result["verifier_executions"] == 2
    assert result["reused_grades"] == 0 and result["invalid_candidates"] == 1
    assert result["oracle"][0]["any_candidate_passes"] is None


@pytest.mark.parametrize("change", ["candidate", "task", "unfinished", "symlink"])
def test_changed_or_unavailable_inputs_are_refused(tmp_path, change):
    trial, task, parent = retained(tmp_path)
    if change == "candidate":
        (parent / "session-1/out.txt").write_text("changed")
    elif change == "task":
        (task / "tests/test.sh").write_text("changed")
    elif change == "symlink":
        (parent / "session-1/link").symlink_to("out.txt")
    else:
        (trial / "result.json").write_text("{}")
    with pytest.raises(replay.ReplayError):
        candidates.discover(trial)


def test_content_and_modes_change_the_deduplication_key(tmp_path):
    trial, _, parent = retained(tmp_path, ("one", "two", "one"))
    (parent / "session-3/out.txt").chmod(0o700)
    rows = candidates.discover(trial)
    assert len({row["input_digest"] for row in rows}) == 3


def test_parallel_workers_are_bounded_and_outputs_cannot_change_inputs(tmp_path):
    trial, task, _ = retained(tmp_path, ("one", "two", "three", "four"))
    lock = threading.Lock()
    active = peak = 0

    def slow(*args):
        nonlocal active, peak
        with lock:
            active += 1
            peak = max(peak, active)
        time.sleep(0.04)
        with lock:
            active -= 1
        return grade(*args)

    result = candidates.batch([trial], tmp_path / "out", jobs=2, runner=slow)
    assert peak == 2 and result["verifier_executions"] == 4
    for output in [trial / "grades", task / "grades"]:
        with pytest.raises(replay.ReplayError):
            candidates.batch([trial], output, runner=grade)
        assert not output.exists()


def test_failed_discovery_retains_an_unknown_oracle(tmp_path):
    trial, _, parent = retained(tmp_path)
    (parent / "session-1/out.txt").unlink()
    result = candidates.batch([trial], tmp_path / "out", runner=grade)
    assert result["errors"] and result["verifier_executions"] == 0
    assert result["oracle"][0]["any_candidate_passes"] is None
