"""Per-task reference results from the public Terminal-Bench 4.0 leaderboard.

The official leaderboard lives on the Harbor Hub. Its rows, each row's
trial list, each trial's task and reward, and each source job's per-task
cost aggregate are public reads: Harbor's own client makes them without a
login. ``fetch_reference`` folds them into one checked document,
``reference/tb4-leaderboard.json``, so the Gym can show the leaderboard's
per-task pass counts beside this host's arms without reaching the network.

Nothing here authenticates, and nothing is written back to the Hub.
"""

from __future__ import annotations

import asyncio
import json
from collections.abc import Iterable
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Protocol

from . import paths

REFERENCE_SCHEMA = "openagents.tbench.reference.v1"
TB4_LEADERBOARD_ID = "9f966760-00f1-424e-90f5-c964fb6f6091"
HUB_URL = "https://hub.harborframework.com"
TRIAL_COLUMNS = (
    "id,job_id,task_name,rewards,exception_type,status,"
    "agent_execution_started_at,agent_execution_finished_at,config"
)
# Trial ids per `in` filter, so each request's URL stays short.
CHUNK = 80


def default_path() -> Path:
    return paths.PACKAGE_DIR / "reference" / "tb4-leaderboard.json"


class HubReader(Protocol):
    """The public Hub reads the fetch needs; tests pass a fake."""

    def leaderboard_rows(self, leaderboard_id: str) -> list[dict[str, Any]]: ...

    def row_trial_ids(self, row_id: str) -> list[str]: ...

    def trials(self, ids: list[str]) -> list[dict[str, Any]]: ...

    def job_tasks(self, job_id: str) -> list[dict[str, Any]]: ...


class HarborHubReader:
    """``HubReader`` over Harbor's anonymous Supabase client."""

    def __init__(self) -> None:
        self._loop = asyncio.new_event_loop()

    def _run(self, coro: Any) -> Any:
        return self._loop.run_until_complete(coro)

    async def _client(self) -> Any:
        from harbor.auth.client import create_authenticated_client

        return await create_authenticated_client()

    def leaderboard_rows(self, leaderboard_id: str) -> list[dict[str, Any]]:
        from harbor.hub.leaderboards import LeaderboardClient

        async def read() -> list[dict[str, Any]]:
            rows: list[dict[str, Any]] = []
            page = 1
            while True:
                _board, result = await LeaderboardClient().list_rows(
                    leaderboard_id=leaderboard_id, page=page, page_size=50
                )
                rows.extend(row.raw for row in result.items)
                if page >= (result.total_pages or 1):
                    return rows
                page += 1

        return self._run(read())

    def row_trial_ids(self, row_id: str) -> list[str]:
        from harbor.hub.leaderboards import LeaderboardClient

        async def read() -> list[str]:
            ids: list[str] = []
            page = 1
            while True:
                result = await LeaderboardClient().list_row_trials(
                    row_id, page=page, page_size=1000
                )
                ids.extend(item.trial_id for item in result.items)
                if page >= (result.total_pages or 1):
                    return ids
                page += 1

        return self._run(read())

    def trials(self, ids: list[str]) -> list[dict[str, Any]]:
        async def read() -> list[dict[str, Any]]:
            client = await self._client()
            rows: list[dict[str, Any]] = []
            for start in range(0, len(ids), CHUNK):
                response = await (
                    client.table("trial")
                    .select(TRIAL_COLUMNS)
                    .in_("id", ids[start : start + CHUNK])
                    .execute()
                )
                rows.extend(response.data or [])
            return rows

        return self._run(read())

    def job_tasks(self, job_id: str) -> list[dict[str, Any]]:
        async def read() -> list[dict[str, Any]]:
            client = await self._client()
            items: list[dict[str, Any]] = []
            page = 1
            while True:
                response = await client.rpc(
                    "get_job_tasks",
                    {"p_job_id": job_id, "p_page": page, "p_page_size": 200},
                ).execute()
                data = response.data or {}
                items.extend(data.get("items") or [])
                if page >= int(data.get("total_pages") or 1):
                    return items
                page += 1

        return self._run(read())


def short_task(name: str) -> str:
    return name.rsplit("/", 1)[-1]


def _label(metadata: dict[str, Any], key: str) -> str | None:
    value = metadata.get(key)
    return value.get("label") if isinstance(value, dict) else None


def _seconds(start: str | None, end: str | None) -> float | None:
    if not start or not end:
        return None
    try:
        return (
            datetime.fromisoformat(end) - datetime.fromisoformat(start)
        ).total_seconds()
    except ValueError:
        return None


def _agent_identity(trial: dict[str, Any]) -> tuple[str | None, str | None, str | None]:
    agent = ((trial.get("config") or {}).get("agent")) or {}
    kwargs = agent.get("kwargs") or {}
    return agent.get("name"), agent.get("model_name"), kwargs.get("version")


def entry_for_row(
    row: dict[str, Any], reader: HubReader, task_names: Iterable[str]
) -> dict[str, Any]:
    """One leaderboard row with its per-task pass counts and costs."""
    metadata = row.get("metadata") or {}
    metrics = row.get("metrics") or {}
    ids = reader.row_trial_ids(row["id"])
    trials = reader.trials(ids)
    per_task: dict[str, dict[str, Any]] = {
        name: {"successes": 0, "trials": 0, "errors": 0, "reward_sum": 0.0}
        for name in task_names
    }
    agent_seconds: dict[str, list[float]] = {}
    jobs: set[str] = set()
    identity: tuple[str | None, str | None, str | None] = (None, None, None)
    for trial in trials:
        task = short_task(trial.get("task_name") or "")
        cell = per_task.setdefault(
            task, {"successes": 0, "trials": 0, "errors": 0, "reward_sum": 0.0}
        )
        reward = (trial.get("rewards") or {}).get("reward")
        reward = float(reward) if isinstance(reward, (int, float)) else 0.0
        cell["trials"] += 1
        cell["reward_sum"] = round(cell["reward_sum"] + reward, 6)
        if reward >= 1.0:
            cell["successes"] += 1
        if trial.get("exception_type"):
            cell["errors"] += 1
        seconds = _seconds(
            trial.get("agent_execution_started_at"),
            trial.get("agent_execution_finished_at"),
        )
        if seconds is not None:
            agent_seconds.setdefault(task, []).append(seconds)
        if trial.get("job_id"):
            jobs.add(trial["job_id"])
        if identity == (None, None, None):
            identity = _agent_identity(trial)
    agent_name, model_name, agent_version = identity
    # Cost comes from each source job's per-task aggregate, narrowed to the
    # row's agent and model. It covers every trial of that agent in the job,
    # so a task's cost counts only when the aggregate's trial count matches
    # the row's; otherwise it stays unknown rather than misattributed.
    costs: dict[str, dict[str, float]] = {}
    for job_id in sorted(jobs):
        for item in reader.job_tasks(job_id):
            model = item.get("model_name")
            provider = item.get("model_provider")
            full = f"{provider}/{model}" if provider and provider != "unknown" else model
            if agent_name and item.get("agent_name") != agent_name:
                continue
            if model_name and full != model_name and model != model_name:
                continue
            slot = costs.setdefault(
                short_task(item.get("task_name") or ""), {"cost_usd": 0.0, "trials": 0}
            )
            if item.get("cost_usd") is not None:
                slot["cost_usd"] += float(item["cost_usd"])
            slot["trials"] += int(item.get("n_trials") or 0)
    for task, cell in per_task.items():
        cost = costs.get(task)
        cell["cost_usd"] = (
            round(cost["cost_usd"], 6)
            if cost and cost["trials"] == cell["trials"] and cell["trials"]
            else None
        )
        seconds = agent_seconds.get(task) or []
        cell["mean_agent_sec"] = (
            round(sum(seconds) / len(seconds), 1)
            if seconds and len(seconds) == cell["trials"]
            else None
        )
    successes = sum(cell["successes"] for cell in per_task.values())
    counted = sum(cell["trials"] for cell in per_task.values())
    known = [cell["cost_usd"] for cell in per_task.values() if cell["cost_usd"] is not None]
    cost_total = round(sum(known), 2) if len(known) == len(per_task) else None
    row_cost = metrics.get("total_cost_usd")
    return {
        "id": row["id"],
        "rank": row.get("rank"),
        "status": row.get("status"),
        "agent": _label(metadata, "agent_display"),
        "model": _label(metadata, "model_display"),
        "agent_org": _label(metadata, "agent_org"),
        "model_org": _label(metadata, "model_org"),
        "reasoning_effort": metadata.get("reasoning_effort"),
        "date": metadata.get("date"),
        "harbor_agent": agent_name,
        "harbor_model": model_name,
        "agent_version": agent_version,
        "metrics": {
            key: metrics.get(key)
            for key in (
                "accuracy",
                "accuracy_ci95_half_width",
                "successes",
                "n_trials",
                "total_cost_usd",
                "avg_trial_duration_sec",
                "pass_at_2",
                "pass_at_3",
                "pass_at_4",
                "pass_at_5",
            )
        },
        "source_jobs": [f"{HUB_URL}/jobs/{job}" for job in sorted(jobs)],
        "per_task": {
            "trials_counted": counted,
            "successes_counted": successes,
            "consistent": counted == metrics.get("n_trials")
            and successes == metrics.get("successes"),
            "cost_usd_counted": cost_total,
            # The job aggregates can include retried attempts the row
            # dropped, so their sum can exceed the row's own total.
            "cost_consistent": cost_total is not None
            and isinstance(row_cost, (int, float))
            and abs(cost_total - row_cost) < 0.01,
        },
        "tasks": dict(sorted(per_task.items())),
    }


def fetch_reference(
    reader: HubReader,
    *,
    task_names: list[str],
    leaderboard_id: str = TB4_LEADERBOARD_ID,
    now: str | None = None,
) -> dict[str, Any]:
    """The reference document for every displayed row of the leaderboard."""
    rows = reader.leaderboard_rows(leaderboard_id)
    entries = [entry_for_row(row, reader, task_names) for row in rows]
    return {
        "schema": REFERENCE_SCHEMA,
        "benchmark": "Terminal-Bench 4.0",
        "profile": "tb4",
        "dataset_ref": "v4.0.0",
        "leaderboard": f"{HUB_URL}/leaderboards/{leaderboard_id}",
        "leaderboard_id": leaderboard_id,
        "fetched_at": now
        or datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "source": (
            "Harbor Hub public reads without credentials: leaderboard-read, "
            "leaderboard_row_trial, the trial table, and get_job_tasks. A "
            "success is a trial reward of 1.0; an errored trial counts as a "
            "failure, as the leaderboard counts it. A task's cost is the "
            "source job's per-task aggregate for the row's agent and model, "
            "and is null when that aggregate covers a different trial count."
        ),
        "tasks": sorted(task_names),
        "entries": entries,
    }


def write_reference(document: dict[str, Any], path: Path | None = None) -> Path:
    path = path or default_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=1) + "\n")
    return path
