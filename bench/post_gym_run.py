"""Post one Harbor job's graded result to the Gym.

Usage:

    OPENAGENTS_TOKEN=... python3 bench/post_gym_run.py \
      <job-dir> [--api-url http://localhost:4000] [--lane proxy]

Reads the job's `result.json` and each trial's ATIF `trajectory.json` (for
token totals), derives the recipe digest from the job's `config.json` plus
the agent tarball digest, and POSTs to `/api/v3/gym/runs`. Idempotent: the
server replays a repeated digest rather than duplicating the row.
"""

import argparse
import hashlib
import json
import os
import sys
import urllib.request
from pathlib import Path


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("job_dir", type=Path)
    parser.add_argument("--api-url", default="http://localhost:4000")
    parser.add_argument("--lane", default="proxy")
    parser.add_argument("--suite", default="terminal-bench@2.0")
    arguments = parser.parse_args()

    token = os.environ.get("OPENAGENTS_TOKEN", "")
    if not token:
        print("OPENAGENTS_TOKEN is not set", file=sys.stderr)
        return 2

    job_dir = arguments.job_dir
    result = json.loads((job_dir / "result.json").read_text())
    config = json.loads((job_dir / "config.json").read_text())

    trials = []
    input_tokens = 0
    output_tokens = 0
    agent_version = None
    model = None
    for trial_dir in sorted(job_dir.iterdir()):
        trial_result = trial_dir / "result.json"
        if not trial_dir.is_dir() or not trial_result.exists():
            continue
        trial = json.loads(trial_result.read_text())
        trials.append(trial)
        trajectory_path = trial_dir / "agent" / "trajectory.json"
        if trajectory_path.exists():
            trajectory = json.loads(trajectory_path.read_text())
            metrics = trajectory.get("final_metrics") or {}
            input_tokens += metrics.get("total_prompt_tokens") or 0
            output_tokens += metrics.get("total_completion_tokens") or 0
            agent = trajectory.get("agent") or {}
            agent_version = agent.get("version") or agent_version
            model = agent.get("model_name") or model

    def passed(trial: dict) -> bool:
        verifier = trial.get("verifier_result") or {}
        rewards = verifier.get("rewards") or trial.get("rewards") or {}
        reward = rewards.get("reward")
        if reward is None and isinstance(rewards, dict) and rewards:
            reward = next(iter(rewards.values()))
        return bool(reward) and float(reward) > 0

    tasks_total = len(trials)
    tasks_passed = sum(1 for trial in trials if passed(trial))

    recipe_source = json.dumps(
        {
            "config": config,
            "suite": arguments.suite,
            "lane": arguments.lane,
            "job_id": result.get("id"),
        },
        sort_keys=True,
    ).encode()
    recipe_digest = "harbor:" + hashlib.sha256(recipe_source).hexdigest()

    payload = {
        "suite": arguments.suite,
        "agent": "openagents-coder",
        "agent_version": agent_version,
        "model": model or "unknown",
        "lane": arguments.lane,
        "tasks_total": tasks_total,
        "tasks_passed": tasks_passed,
        "input_tokens": input_tokens or None,
        "output_tokens": output_tokens or None,
        "recipe_digest": recipe_digest,
        "report": {
            "job_id": result.get("id"),
            "trials": [
                {
                    "task": (trial.get("task_name") or trial.get("trial_name") or "?"),
                    "passed": passed(trial),
                    "exception": (trial.get("exception_info") or {}).get("exception_type")
                    if trial.get("exception_info")
                    else None,
                }
                for trial in trials
            ],
        },
    }

    request = urllib.request.Request(
        f"{arguments.api_url}/api/v3/gym/runs",
        data=json.dumps(payload).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {token}",
        },
        method="POST",
    )
    with urllib.request.urlopen(request) as response:
        body = json.loads(response.read())
        run = body.get("run") or {}
        print(
            f"{response.status}: run {run.get('id')} score={run.get('score')} "
            f"replayed={body.get('replayed')}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
