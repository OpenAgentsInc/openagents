import io
import json
import tarfile
from pathlib import Path

import pytest

from tbench import replay


def make_trial(jobs: Path, job: str, name: str, reward: float | None, task: Path) -> Path:
    trial = jobs / job / name
    (trial / "agent" / "episode").mkdir(parents=True)
    (trial / "agent" / "episode" / "episode.atif.jsonl").write_text("{}\n")
    (trial / "config.json").write_text(json.dumps({"task": {"path": str(task)}}))
    result = {"finished_at": f"2026-09-23T1{len(name) % 10}:00:00Z"}
    if reward is not None:
        result["verifier_result"] = {"rewards": {"reward": reward}}
    (trial / "result.json").write_text(json.dumps(result))
    return trial


def make_task(root: Path) -> Path:
    task = root / "tasks" / "demo-task"
    (task / "environment").mkdir(parents=True)
    (task / "environment" / "Dockerfile").write_text("FROM debian:stable-slim\n")
    (task / "tests").mkdir()
    (task / "tests" / "test.sh").write_text("#!/bin/bash\n")
    (task / "instruction.md").write_text("Write /app/out.txt.\n")
    (task / "task.toml").write_text(
        'schema_version = "1.0"\n'
        'artifacts = ["/app/out.txt", "/app/missing.txt"]\n'
        '[task]\nname = "terminal-bench/demo-task"\n'
        '[verifier]\nenvironment_mode = "separate"\n'
    )
    return task


def test_trials_are_found_by_path_job_and_name(tmp_path: Path):
    task = make_task(tmp_path)
    jobs = tmp_path / "jobs"
    trial = make_trial(jobs, "tb4--arm--demo", "demo-task__abc", 0.0, task)
    assert replay.find_trial(str(trial)) == trial
    assert replay.find_trial("tb4--arm--demo/demo-task__abc", jobs) == trial
    assert replay.find_trial("demo-task__abc", jobs) == trial
    with pytest.raises(replay.ReplayError):
        replay.find_trial("nothing__here", jobs)


def test_failing_trials_are_graded_coder_one_failures(tmp_path: Path):
    task = make_task(tmp_path)
    jobs = tmp_path / "jobs"
    failed = make_trial(jobs, "tb4--coder-one-x--a", "a__1", 0.0, task)
    (failed / "artifacts").mkdir()
    (failed / "artifacts" / "manifest.json").write_text(
        json.dumps([{"source": "/app/out.txt", "destination": "artifacts/app/out.txt", "status": "ok"}])
    )
    (failed / "artifacts" / "app").mkdir()
    (failed / "artifacts" / "app" / "out.txt").write_text("x")
    # A failure that left nothing to replay is skipped.
    make_trial(jobs, "tb4--coder-one-x--e", "e__1", 0.0, task)
    make_trial(jobs, "tb4--coder-one-x--b", "b__1", 1.0, task)
    make_trial(jobs, "tb4--coder-one-x--c", "c__1", None, task)
    make_trial(jobs, "tb4--claude--d", "d__1", 0.0, task)
    assert replay.failing_trials("tb4--coder-one-x", 10, jobs) == [failed]


def test_a_workspace_comes_from_the_snapshot_first(tmp_path: Path):
    task = make_task(tmp_path)
    trial = make_trial(tmp_path / "jobs", "j", "demo-task__1", 0.0, task)
    snapshot = trial / replay.SNAPSHOT
    snapshot.parent.mkdir(parents=True)
    with tarfile.open(snapshot, "w:gz") as tar:
        data = b"42\n"
        info = tarfile.TarInfo("app/out.txt")
        info.size = len(data)
        tar.addfile(info, io.BytesIO(data))
    workspace = replay.workspace_of(trial, tmp_path / "scratch")
    assert workspace.source == "snapshot" and workspace.full
    assert (workspace.root / "app" / "out.txt").read_text() == "42\n"


def test_without_a_snapshot_the_collected_artifacts_overlay_the_image(tmp_path: Path):
    task = make_task(tmp_path)
    trial = make_trial(tmp_path / "jobs", "j", "demo-task__1", 0.0, task)
    (trial / "artifacts" / "app").mkdir(parents=True)
    (trial / "artifacts" / "app" / "out.txt").write_text("41\n")
    (trial / "artifacts" / "manifest.json").write_text(
        json.dumps(
            [
                {"source": "/logs/artifacts", "destination": "artifacts/logs/artifacts", "type": "directory", "status": "empty", "service": None},
                {"source": "/app/out.txt", "destination": "artifacts/app/out.txt", "type": "file", "status": "ok", "service": None},
            ]
        )
    )
    workspace = replay.workspace_of(trial, tmp_path / "scratch")
    assert workspace.source == "artifacts" and not workspace.full
    assert (workspace.root / "app" / "out.txt").read_text() == "41\n"


def test_a_candidate_becomes_a_source_trial_harbor_can_regrade(tmp_path: Path):
    task = make_task(tmp_path)
    candidate = tmp_path / "candidate"
    candidate.mkdir()
    (candidate / "out.txt").write_text("42\n")
    workspace = replay.candidate_workspace(candidate, "/app", tmp_path / "scratch")
    source = replay._synthetic_source(tmp_path / "out" / "source" / "c", task, workspace, None)
    entries = json.loads((source / "artifacts" / "manifest.json").read_text())
    by_source = {entry["source"]: entry for entry in entries}
    assert by_source["/app/out.txt"]["status"] == "ok"
    assert (source / "artifacts" / "app" / "out.txt").read_text() == "42\n"
    # A declared artifact the candidate lacks is empty, so the verifier
    # sees it missing, as it would have in the trial.
    assert by_source["/app/missing.txt"]["status"] == "empty"
    result = json.loads((source / "result.json").read_text())
    assert result["task_name"] == "terminal-bench/demo-task"
    from harbor.trial.regrade import check_task_regradable

    assert check_task_regradable(task) is None


def test_the_summary_counts_recall_and_false_alarms():
    rows = [
        replay.Replay("a", "checks", "artifacts", 0.0, {"total": 1.0}, {"detected": True}),
        replay.Replay("b", "checks", "artifacts", 0.0, {"total": 1.0}, {"detected": False}),
        replay.Replay("c", "checks", "snapshot", 1.0, {"total": 1.0}, {"detected": True}),
        replay.Replay("d", "checks", "-", 0.0, {}, {}, error="no image"),
    ]
    summary = replay.summary(rows, 3.0)
    assert summary["failing"] == 2 and summary["failing_detected"] == 1
    assert summary["passing"] == 1 and summary["passing_flagged"] == 1
    assert summary["errors"] == 1
    assert "checks detected" in replay.verdict(rows[0])
    assert replay.verdict(rows[3]).startswith("error")
