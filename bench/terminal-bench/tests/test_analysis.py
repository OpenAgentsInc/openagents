"""The run-end hook that writes each trial's analysis through the Gym."""

from __future__ import annotations

import subprocess
from pathlib import Path

from tbench import analysis


def _trial(job: Path, name: str, *, finished: bool = True, analyzed: bool = False) -> Path:
    trial = job / name
    trial.mkdir(parents=True)
    (trial / "config.json").write_text("{}")
    if finished:
        (trial / "result.json").write_text("{}")
    if analyzed:
        (trial / analysis.MARKDOWN).write_text("# done\n")
    return trial


class Recorder:
    def __init__(self, returncode: int = 0) -> None:
        self.calls: list[list[str]] = []
        self.returncode = returncode

    def __call__(self, argv, **kwargs):
        self.calls.append(list(argv))
        assert kwargs["timeout"] == analysis.TIMEOUT_SEC
        return subprocess.CompletedProcess(argv, self.returncode, "", "boom")


def test_only_finished_trials_without_an_analysis_are_pending(tmp_path):
    job = tmp_path / "jobs" / "tb4--arm--task"
    _trial(job, "task__done")
    _trial(job, "task__running", finished=False)
    _trial(job, "task__analyzed", analyzed=True)
    (job / "tbench").mkdir()
    assert [p.name for p in analysis.pending(job)] == ["task__done"]


def test_the_hook_asks_gym_to_write_each_pending_analysis(tmp_path, monkeypatch):
    monkeypatch.delenv(analysis.OFF_ENV, raising=False)
    monkeypatch.setenv(analysis.BIN_ENV, "/opt/gym")
    job = tmp_path / "jobs" / "tb4--arm--task"
    _trial(job, "task__a")
    _trial(job, "task__b")
    recorder = Recorder()
    written = analysis.analyze_job(job, runner=recorder)
    assert written == [job / "task__a" / "analysis.md", job / "task__b" / "analysis.md"]
    assert recorder.calls[0] == [
        "/opt/gym",
        "runs",
        "analyze",
        "tb4--arm--task/task__a",
        "--jobs-dir",
        str(job.parent),
        "--no-traces",
        "--write",
    ]


def test_a_failing_gym_only_warns(tmp_path, monkeypatch, capsys):
    monkeypatch.delenv(analysis.OFF_ENV, raising=False)
    monkeypatch.setenv(analysis.BIN_ENV, "/opt/gym")
    job = tmp_path / "jobs" / "tb4--arm--task"
    _trial(job, "task__a")
    assert analysis.analyze_job(job, runner=Recorder(returncode=2)) == []
    assert "gym exited 2" in capsys.readouterr().err

    def missing(argv, **kwargs):
        raise FileNotFoundError(argv[0])

    assert analysis.analyze_job(job, runner=missing) == []
    assert "no analysis written" in capsys.readouterr().err


def test_the_hook_can_be_turned_off(tmp_path, monkeypatch):
    monkeypatch.setenv(analysis.OFF_ENV, "off")
    job = tmp_path / "jobs" / "tb4--arm--task"
    _trial(job, "task__a")
    recorder = Recorder()
    assert analysis.analyze_job(job, runner=recorder) == []
    assert recorder.calls == []
