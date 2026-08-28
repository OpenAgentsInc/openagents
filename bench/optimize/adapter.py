"""GEPA adapter: candidate text in, Harbor (or fixture) scores out.

Does not write `surfaces/coder`. Live evaluation of a mutated candidate
would need a surfaces overlay or a CLI rebuild; this packet scores the
current tree (seed) live, and scores fixtures otherwise.
"""

from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from .job import HarborJob, load_job
from .metric import HOLDOUT_SUITE_ID, score_trial
from .surfaces import repo_root_from

try:
    from gepa import EvaluationBatch as GepaEvaluationBatch
except ImportError:  # pragma: no cover - gepa is optional for the fixture path
    GepaEvaluationBatch = None


@dataclass
class GymTask:
    task_id: str
    suite: str
    model: str = ""
    lane: str = ""


@dataclass
class EvaluationBatch:
    outputs: list[Any]
    scores: list[float]
    trajectories: list[Any] | None = None
    objective_scores: list[dict[str, float]] | None = None
    num_metric_calls: int | None = None


def _batch(*, outputs: list[Any], scores: list[float], trajectories: list[Any] | None, calls: int) -> Any:
    payload = EvaluationBatch(
        outputs=outputs,
        scores=scores,
        trajectories=trajectories,
        num_metric_calls=calls,
    )
    if GepaEvaluationBatch is None:
        return payload
    return GepaEvaluationBatch(
        outputs=outputs,
        scores=scores,
        trajectories=trajectories,
        num_metric_calls=calls,
    )


class HoldoutScreenError(RuntimeError):
    """The optimizer was asked to screen on the holdout suite (ledger O3)."""


@dataclass
class GymAdapter:
    """`GEPAAdapter` over the Gym. Fixture jobs make `--max-metric-calls 1` cheap."""

    job: HarborJob | None = None
    fixture_job_dir: Path | None = None
    live: bool = False
    repo_root: Path | None = None
    model: str = "openai/gpt-5.6-luna"
    lane: str = "proxy"
    jobs_parent: Path | None = None
    max_metric_calls: int = 1
    calls_used: int = 0
    allow_holdout_screen: bool = False
    run_suite: Path | None = None
    _job_cache: HarborJob | None = field(default=None, init=False, repr=False)

    def _root(self) -> Path:
        return Path(self.repo_root or repo_root_from())

    def _job(self) -> HarborJob:
        if self.job is not None:
            return self.job
        if self._job_cache is not None:
            return self._job_cache
        if self.fixture_job_dir is None:
            raise RuntimeError("GymAdapter has no fixture job and no live job")
        self._job_cache = load_job(self.fixture_job_dir)
        return self._job_cache

    def evaluate(
        self,
        batch: list[GymTask],
        candidate: dict[str, str],
        capture_traces: bool = False,
    ) -> Any:
        if not batch:
            return _batch(outputs=[], scores=[], trajectories=[] if capture_traces else None, calls=0)
        suites = {task.suite for task in batch}
        if HOLDOUT_SUITE_ID in suites and not self.allow_holdout_screen:
            raise HoldoutScreenError(
                "optimizer must not screen on tb2-cross-section (ledger O3); "
                "pass --confirm-holdout only for a survivor confirmation, never as valset"
            )
        remaining = max(0, self.max_metric_calls - self.calls_used)
        outputs: list[dict[str, Any]] = []
        scores: list[float] = []
        trajectories: list[dict[str, Any]] = []
        calls = 0
        job = None if self.live else self._job()
        for index, task in enumerate(batch):
            if index >= remaining:
                outputs.append({"task": task.task_id, "skipped": "max_metric_calls"})
                scores.append(0.0)
                trajectories.append(
                    {
                        "task": task.task_id,
                        "skipped": "max_metric_calls",
                        "candidate_keys": sorted(candidate.keys()),
                    }
                )
                continue
            trial_job = job
            if self.live:
                trial_job = self._run_live([task])
            trial = trial_job.trial_for(task.task_id) if trial_job is not None else None
            if trial is None:
                outputs.append({"task": task.task_id, "error": "trial missing from job"})
                scores.append(0.0)
                trajectories.append(
                    {
                        "task": task.task_id,
                        "error": "trial missing from job",
                        "candidate_keys": sorted(candidate.keys()),
                    }
                )
                calls += 1
                continue
            scored = score_trial(trial)
            outputs.append(
                {
                    "task": task.task_id,
                    "accepted": scored.accepted,
                    "ungraded": scored.ungraded,
                    "score": scored.score,
                    "job_id": trial_job.job_id if trial_job else None,
                }
            )
            scores.append(scored.score)
            trajectories.append(
                {
                    "task": task.task_id,
                    "accepted": scored.accepted,
                    "ungraded": scored.ungraded,
                    "prompt_tokens": scored.prompt_tokens,
                    "completion_tokens": scored.completion_tokens,
                    "wall_seconds": scored.wall_seconds,
                    "score": scored.score,
                    "trajectory": trial.trajectory,
                    "verifier_result": trial.result.get("verifier_result"),
                    "candidate_keys": sorted(candidate.keys()),
                }
            )
            calls += 1
        self.calls_used += calls
        return _batch(
            outputs=outputs,
            scores=scores,
            trajectories=trajectories if capture_traces else None,
            calls=calls,
        )

    def make_reflective_dataset(
        self,
        candidate: dict[str, str],
        eval_batch: Any,
        components_to_update: list[str],
    ) -> dict[str, list[dict[str, Any]]]:
        traces = eval_batch.trajectories or []
        dataset: dict[str, list[dict[str, Any]]] = {name: [] for name in components_to_update}
        for trace, score, output in zip(traces, eval_batch.scores, eval_batch.outputs):
            trajectory = (trace or {}).get("trajectory") or {}
            steps = trajectory.get("steps") or []
            step_summaries = []
            for step in steps[:12]:
                if not isinstance(step, dict):
                    continue
                step_summaries.append(
                    {
                        "step_id": step.get("step_id"),
                        "source": step.get("source"),
                        "message": (step.get("message") or "")[:400],
                    }
                )
            accepted = (trace or {}).get("accepted")
            feedback = (
                f"verifier accepted={accepted} ungraded={(trace or {}).get('ungraded')} "
                f"score={score} tokens={((trace or {}).get('prompt_tokens') or 0) + ((trace or {}).get('completion_tokens') or 0)} "
                f"wall_seconds={(trace or {}).get('wall_seconds')}"
            )
            record = {
                "Inputs": {
                    "task": (trace or {}).get("task") or (output or {}).get("task"),
                    "suite_role": "dev",
                },
                "Generated Outputs": {
                    "steps": step_summaries,
                    "output": output,
                },
                "Feedback": feedback,
                "score": score,
            }
            for name in components_to_update:
                item = dict(record)
                item["Inputs"] = dict(record["Inputs"])
                item["Inputs"]["component"] = name
                item["Inputs"]["current_text_excerpt"] = (candidate.get(name) or "")[:800]
                dataset[name].append(item)
        return dataset

    def _run_live(self, batch: list[GymTask]) -> HarborJob:
        """Invoke `bench/run-suite.sh` for this batch. Writes only under jobs_parent."""
        root = self._root()
        script = Path(self.run_suite or (root / "bench" / "run-suite.sh"))
        suite = batch[0].suite
        jobs_parent = Path(self.jobs_parent or (root / "bench" / "optimize" / "output" / "jobs"))
        jobs_parent.mkdir(parents=True, exist_ok=True)
        jobs_dir = jobs_parent / f"live-{suite}-{batch[0].task_id}"
        jobs_dir.mkdir(parents=True, exist_ok=True)
        # A plain task list so a max_metric_calls=1 live run does not start a
        # 12-task suite. Writes stay under the optimizer output tree.
        task_list = jobs_dir / "tasks.txt"
        task_list.write_text("".join(f"{task.task_id}\n" for task in batch))
        command = [
            str(script),
            str(task_list),
            "--model",
            self.model,
            "--lane",
            self.lane,
            "--jobs-dir",
            str(jobs_dir),
        ]
        env = os.environ.copy()
        subprocess.run(command, check=True, cwd=str(root), env=env)
        return load_job(_find_job_dir(jobs_dir))


def _find_job_dir(jobs_dir: Path) -> Path:
    if (jobs_dir / "result.json").is_file():
        return jobs_dir
    for child in sorted(jobs_dir.iterdir()):
        if child.is_dir() and (child / "result.json").is_file():
            return child
    raise FileNotFoundError(f"no Harbor job with result.json under {jobs_dir}")
