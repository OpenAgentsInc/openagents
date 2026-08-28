"""Read a Harbor job directory: `result.json`, trial `result.json`, ATIF."""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class HarborTrial:
    path: Path
    task: str
    result: dict[str, Any]
    trajectory: dict[str, Any] | None
    coder_log: str | None

    @property
    def verifier_ran(self) -> bool:
        return self.result.get("verifier_result") is not None

    @property
    def passed(self) -> bool:
        verifier = self.result.get("verifier_result") or {}
        rewards = verifier.get("rewards") or self.result.get("rewards") or {}
        reward = rewards.get("reward")
        if reward is None and isinstance(rewards, dict) and rewards:
            reward = next(iter(rewards.values()))
        try:
            return bool(reward) and float(reward) > 0
        except (TypeError, ValueError):
            return False


@dataclass(frozen=True)
class HarborJob:
    path: Path
    job_id: str | None
    result: dict[str, Any]
    config: dict[str, Any]
    trials: tuple[HarborTrial, ...]

    def trial_for(self, task: str) -> HarborTrial | None:
        for trial in self.trials:
            if trial.task == task:
                return trial
        return None


def _task_name(trial: dict[str, Any], directory: Path) -> str:
    return trial.get("task_name") or trial.get("trial_name") or directory.name.rsplit("__", 1)[0]


def wall_seconds_of(trial: HarborTrial) -> float:
    execution = trial.result.get("agent_execution") or {}
    started = execution.get("started_at")
    finished = execution.get("finished_at")
    if not started or not finished:
        return 0.0
    start = datetime.fromisoformat(str(started).replace("Z", "+00:00"))
    end = datetime.fromisoformat(str(finished).replace("Z", "+00:00"))
    return max(0.0, (end - start).total_seconds())


def load_job(job_dir: Path) -> HarborJob:
    """Load one Harbor job. Missing ATIF is allowed; missing result.json is not."""
    job_dir = Path(job_dir)
    result_path = job_dir / "result.json"
    if not result_path.is_file():
        raise FileNotFoundError(f"no result.json in {job_dir}")
    result = json.loads(result_path.read_text())
    config_path = job_dir / "config.json"
    config = json.loads(config_path.read_text()) if config_path.is_file() else {}
    trials: list[HarborTrial] = []
    for child in sorted(job_dir.iterdir()):
        trial_result = child / "result.json"
        if not child.is_dir() or not trial_result.is_file():
            continue
        trial = json.loads(trial_result.read_text())
        trajectory = None
        trajectory_path = child / "agent" / "trajectory.json"
        if trajectory_path.is_file():
            trajectory = json.loads(trajectory_path.read_text())
        coder_log = None
        coder_path = child / "agent" / "coder.txt"
        if coder_path.is_file():
            coder_log = coder_path.read_text(errors="replace")
        trials.append(
            HarborTrial(
                path=child,
                task=_task_name(trial, child),
                result=trial,
                trajectory=trajectory,
                coder_log=coder_log,
            )
        )
    job_id = result.get("id") if isinstance(result, dict) else None
    return HarborJob(
        path=job_dir,
        job_id=str(job_id) if job_id else None,
        result=result if isinstance(result, dict) else {},
        config=config if isinstance(config, dict) else {},
        trials=tuple(trials),
    )
