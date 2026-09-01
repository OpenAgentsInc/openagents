"""Post one Harbor job's graded result to the Gym.

Usage:

    OPENAGENTS_TOKEN=... python3 bench/post_gym_run.py \
      <job-dir> [--api-url http://localhost:4000] [--lane proxy] \
      [--run-id <uuid>]

Reads the job's `result.json` and each trial's ATIF `trajectory.json` (for
token totals), derives the recipe digest from the job's `config.json` plus
the agent tarball digest, and posts to the Gym.

With `--run-id` (or `OPENAGENTS_GYM_RUN_ID` in the environment) this is the
finalize step of a run `bench/run-suite.sh` registered against the lifecycle
API (OpenAgentsInc/openagents#38): each trial's final state is upserted to
`POST /api/v1/gym/runs/{id}/trials` — `passed` or `failed` where the
verifier ran, `ungraded` where it never did — and the run is closed with
`PATCH /api/v1/gym/runs/{id}`. A run whose verifier never ran on any trial
is patched `abandoned`: a crashed grader is not a grade, but it should not
be a forever-running row either. A 409 on the PATCH means another run
already holds the digest; that is a replay, reported and treated as success.

Without a run id, this keeps its original shape: one POST to
`/api/v1/gym/runs`, idempotent by recipe digest. Post only runs whose
verifier actually ran: a score is a claim, and a crashed grader is not a
grade.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

# The coder's --plain thread announcement (OpenAgentsInc/openagents#39),
# the same contract the adapter parses live.
_THREAD_LINE = re.compile(r"\[oa:thread ([0-9a-fA-F-]{36})\]")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


_MODEL_LINE = re.compile(r"^Model:\s+(\S+)\s*$", re.MULTILINE)


def request_json(url: str, token: str, payload: dict, method: str) -> tuple[int, dict]:
    request = urllib.request.Request(
        url,
        data=json.dumps(payload).encode(),
        headers={
            "Content-Type": "application/json",
            "Authorization": f"Bearer {token}",
        },
        method=method,
    )
    try:
        with urllib.request.urlopen(request) as response:
            return response.status, json.loads(response.read() or b"{}")
    except urllib.error.HTTPError as error:
        raw = error.read()
        try:
            body = json.loads(raw.decode() or "{}")
        except json.JSONDecodeError:
            body = {"message": raw[:500].decode(errors="replace")}
        print(
            f"{method} {url} -> {error.code}: {json.dumps(body)[:1000]}",
            file=sys.stderr,
        )
        raise


def catalog_model_from_config(config: dict) -> str | None:
    """Harbor stores the model on each agent as provider/name."""
    for agent in config.get("agents") or []:
        if not isinstance(agent, dict):
            continue
        spelled = (agent.get("model_name") or "").strip()
        if not spelled:
            continue
        provider, sep, name = spelled.partition("/")
        return name if sep else provider
    return None


def model_from_coder_log(text: str) -> str | None:
    match = _MODEL_LINE.search(text)
    return match.group(1) if match else None


def thread_id_of(trial_dir: Path) -> str | None:
    """The thread id the coder announced in this trial's captured output."""
    coder_log = trial_dir / "agent" / "coder.txt"
    try:
        text = coder_log.read_text(errors="replace")
    except OSError:
        return None
    match = _THREAD_LINE.search(text)
    return match.group(1) if match else None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("job_dir", type=Path)
    parser.add_argument("--api-url", default="http://localhost:4000")
    parser.add_argument("--lane", default="proxy")
    parser.add_argument("--suite", default="terminal-bench@2.0")
    parser.add_argument(
        "--run-id",
        default=os.environ.get("OPENAGENTS_GYM_RUN_ID") or None,
        help="Finalize this registered Gym run instead of the one-shot POST. "
        "Defaults to OPENAGENTS_GYM_RUN_ID when set.",
    )
    arguments = parser.parse_args()

    token = os.environ.get("OPENAGENTS_TOKEN", "")
    if not token:
        print("OPENAGENTS_TOKEN is not set", file=sys.stderr)
        return 2

    job_dir = arguments.job_dir
    result = json.loads((job_dir / "result.json").read_text())
    config = json.loads((job_dir / "config.json").read_text())

    trials = []
    trial_entries = []
    input_tokens = 0
    output_tokens = 0
    duration_seconds = 0.0
    agent_version = None
    model = None
    for trial_dir in sorted(job_dir.iterdir()):
        trial_result = trial_dir / "result.json"
        if not trial_dir.is_dir() or not trial_result.exists():
            continue
        trial = json.loads(trial_result.read_text())
        trials.append(trial)
        trial_entries.append((trial, trial_dir))
        execution = trial.get("agent_execution") or {}
        started = execution.get("started_at")
        finished = execution.get("finished_at")
        if started and finished:
            from datetime import datetime

            span = datetime.fromisoformat(finished.replace("Z", "+00:00")) - datetime.fromisoformat(started.replace("Z", "+00:00"))
            duration_seconds += span.total_seconds()
        trajectory_path = trial_dir / "agent" / "trajectory.json"
        if trajectory_path.exists():
            trajectory = json.loads(trajectory_path.read_text())
            metrics = trajectory.get("final_metrics") or {}
            input_tokens += metrics.get("total_prompt_tokens") or 0
            output_tokens += metrics.get("total_completion_tokens") or 0
            agent = trajectory.get("agent") or {}
            agent_version = agent.get("version") or agent_version
            model = agent.get("model_name") or model
    if not model:
        model = catalog_model_from_config(config)
    if not model:
        for _trial, trial_dir in trial_entries:
            try:
                log_text = (trial_dir / "agent" / "coder.txt").read_text(errors="replace")
            except OSError:
                continue
            model = model_from_coder_log(log_text)
            if model:
                break

    def verifier_ran(trial: dict) -> bool:
        return trial.get("verifier_result") is not None

    def passed(trial: dict) -> bool:
        verifier = trial.get("verifier_result") or {}
        rewards = verifier.get("rewards") or trial.get("rewards") or {}
        reward = rewards.get("reward")
        if reward is None and isinstance(rewards, dict) and rewards:
            reward = next(iter(rewards.values()))
        return bool(reward) and float(reward) > 0

    def task_name(trial: dict) -> str:
        return trial.get("task_name") or trial.get("trial_name") or "?"

    def task_key(trial_dir: Path) -> str:
        # The same key the adapter reports live: the task half of Harbor's
        # `<task>__<shortuuid>` trial directory name, so the finalize upserts
        # the rows the live reports created rather than writing new ones.
        return trial_dir.name.rsplit("__", 1)[0] or trial_dir.name

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

    report = {
        "job_id": result.get("id"),
        "trials": [
            {
                "task": task_name(trial),
                "passed": passed(trial),
                "exception": (trial.get("exception_info") or {}).get("exception_type")
                if trial.get("exception_info")
                else None,
            }
            for trial in trials
        ],
    }

    if arguments.run_id:
        run_url = f"{arguments.api_url}/api/v1/gym/runs/{arguments.run_id}"

        # Each trial's final state, upserted by task. A trial whose verifier
        # never ran stays a claimless `ungraded` rather than a grade.
        for trial, trial_dir in trial_entries:
            state = ("passed" if passed(trial) else "failed") if verifier_ran(trial) else "ungraded"
            trial_payload = {"task": task_key(trial_dir), "state": state}
            thread_id = thread_id_of(trial_dir)
            if thread_id:
                trial_payload["thread_id"] = thread_id
            try:
                request_json(f"{run_url}/trials", token, trial_payload, "POST")
            except (urllib.error.URLError, OSError) as error:
                print(f"trial upsert failed for {task_key(trial_dir)}: {error}", file=sys.stderr)

        if not any(verifier_ran(trial) for trial in trials):
            # A crashed grader is not a grade, and a run nobody will grade
            # should not stay a forever-running row.
            request_json(run_url, token, {"status": "abandoned"}, "PATCH")
            print(f"run {arguments.run_id} abandoned: no trial's verifier ran")
            return 0

        finalize = {
            "status": "graded",
            "tasks_total": tasks_total,
            "tasks_passed": tasks_passed,
            "input_tokens": input_tokens or None,
            "output_tokens": output_tokens or None,
            "duration_seconds": int(duration_seconds) or None,
            "recipe_digest": recipe_digest,
            "report": report,
            "model": model,
            "agent_version": agent_version,
        }
        try:
            status, body = request_json(run_url, token, finalize, "PATCH")
        except urllib.error.HTTPError as error:
            if error.code == 409:
                # Another run already holds this digest: a replay, not a
                # failure. The registered row is closed by the run that owns
                # the digest.
                print(f"409: run {arguments.run_id} replayed an existing digest")
                return 0
            # A graded PATCH that the server refuses is not a crashed grader.
            # Leave the registered row for an operator; do not abandon it.
            return 1
        run = body.get("run") or {}
        print(
            f"{status}: run {run.get('id') or arguments.run_id} "
            f"status={run.get('status')} passed={tasks_passed}/{tasks_total}"
        )
        return 0

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
        "duration_seconds": int(duration_seconds) or None,
        "recipe_digest": recipe_digest,
        "report": report,
    }

    status, body = request_json(f"{arguments.api_url}/api/v1/gym/runs", token, payload, "POST")
    run = body.get("run") or {}
    print(
        f"{status}: run {run.get('id')} score={run.get('score')} "
        f"replayed={body.get('replayed')}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
