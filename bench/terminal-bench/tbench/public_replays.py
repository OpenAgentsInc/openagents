"""Acquire the public Fable 5.1 TB4 trajectories for Gym replay.

Run with ``uv run python -m tbench.public_replays``. Downloads are resumable,
verified against the retained manifest, and never execute trace contents.
Only public trajectory bytes and allowlisted trial metadata are retained;
full trial archives, credentials, and provider configuration are not needed.
"""
from __future__ import annotations

import argparse
import asyncio
import hashlib
import json
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from .reference import HarborHubReader, TB4_LEADERBOARD_ID, HUB_URL

SCHEMA = "openagents.gym.public-replays.v1"
DEFAULT_MANIFEST = Path(__file__).resolve().parents[1] / "reference/fable-5.1-replays.json"
DEFAULT_CACHE = Path.home() / ".openagents/terminal-bench/public-replays"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    pending = path.with_suffix(".tmp")
    pending.write_text(json.dumps(value, indent=2) + "\n")
    pending.replace(path)


def trial_record(detail: dict, row: dict) -> dict:
    """Keep only the public identity, outcome, and clock, not configuration."""
    execution = detail.get("agent_execution") or {}
    return {
        "id": detail["id"], "task": detail["task_name"].rsplit("/", 1)[-1],
        "trial_name": detail["trial_name"], "job_id": detail["job_id"],
        "row_id": row["id"], "rank": row.get("rank"),
        "model": "Fable 5.1", "agent": "Claude Code",
        "effort": row["metadata"]["reasoning_effort"],
        "agent_version": (detail.get("agent_info") or {}).get("version"),
        "source_url": f"{HUB_URL}/trials/{detail['id']}",
        "job_url": f"{HUB_URL}/jobs/{detail['job_id']}",
        "trajectory_path": detail.get("trajectory_path"),
        "started_at": execution.get("started_at"),
        "finished_at": execution.get("finished_at"),
        "reward": ((detail.get("verifier_result") or {}).get("rewards") or {}).get("reward"),
        "exception": (detail.get("exception_info") or {}).get("exception_type"),
        "file": f"{detail['id']}.json",
    }


async def acquire(manifest: dict, cache: Path, concurrency: int) -> None:
    from harbor.upload.storage import UploadStorage

    for record in manifest["trials"]:
        uuid.UUID(record["id"])
        if record["file"] != f"{record['id']}.json":
            raise ValueError("Invalid replay file identity")
    cache.mkdir(parents=True, exist_ok=True)
    semaphore = asyncio.Semaphore(concurrency)
    done = 0

    async def one(record: dict) -> None:
        nonlocal done
        async with semaphore:
            dest = cache / record["file"]
            if dest.is_file() and record.get("sha256") == digest(dest):
                record["available"] = True
            elif record.get("trajectory_path"):
                temp = dest.with_suffix(".download")
                for attempt in range(4):
                    try:
                        await UploadStorage().download_file(record["trajectory_path"], temp)
                        value = json.loads(temp.read_bytes())
                        if not isinstance(value.get("steps"), list):
                            raise ValueError("trajectory has no steps array")
                        actual = digest(temp)
                        if record.get("sha256") and actual != record["sha256"]:
                            raise ValueError("published bytes differ from the pinned digest")
                        temp.replace(dest)
                        record.update(sha256=actual, bytes=dest.stat().st_size,
                                      steps=len(value["steps"]), available=True,
                                      timestamped_steps=sum(bool(s.get("timestamp")) for s in value["steps"]),
                                      cost_usd=(value.get("final_metrics") or {}).get("total_cost_usd"))
                        record.pop("error", None)
                        break
                    except Exception as exc:
                        record.update(available=False, error=f"{type(exc).__name__}: acquisition failed")
                        if attempt < 3:
                            await asyncio.sleep(2 ** attempt)
                temp.unlink(missing_ok=True)
            else:
                record.update(available=False, error="No published trajectory")
            done += 1
            if done % 25 == 0 or done == len(manifest["trials"]):
                print(f"{done}/{len(manifest['trials'])} checked", flush=True)
                write_json(cache / "manifest.json", manifest)

    await asyncio.gather(*(one(t) for t in manifest["trials"]))
    manifest["coverage"] = {
        "attempts": len(manifest["trials"]),
        "available": sum(t.get("available", False) for t in manifest["trials"]),
        "published": sum(bool(t.get("trajectory_path")) for t in manifest["trials"]),
        "tasks": len({t["task"] for t in manifest["trials"]}),
        "bytes": sum(t.get("bytes", 0) for t in manifest["trials"]),
    }
    write_json(cache / "manifest.json", manifest)


def discover() -> dict:
    reader = HarborHubReader()
    rows = [r for r in reader.leaderboard_rows(TB4_LEADERBOARD_ID)
            if (r.get("metadata", {}).get("model_display") or {}).get("label") == "Fable 5.1"]
    if not rows:
        raise RuntimeError("The public leaderboard returned no Fable 5.1 rows")
    selections = [(row, reader.row_trial_ids(row["id"])) for row in rows]

    async def details() -> list[dict]:
        from harbor.auth.client import create_authenticated_client
        client = await create_authenticated_client()
        semaphore = asyncio.Semaphore(6)
        async def one(row: dict, trial: str) -> dict:
            async with semaphore:
                for attempt in range(4):
                    try:
                        response = await client.rpc("get_trial_detail", {"p_trial_id": trial}).execute()
                        return trial_record(response.data, row)
                    except Exception:
                        if attempt == 3:
                            raise
                        await asyncio.sleep(2 ** attempt)
            raise AssertionError("unreachable")
        return await asyncio.gather(*(one(row, trial) for row, ids in selections for trial in ids))

    print(f"Discovered {sum(len(ids) for _, ids in selections)} attempts in {len(rows)} rows", flush=True)
    trials = asyncio.run(details())
    return {"schema": SCHEMA, "retrieved_at": datetime.now(timezone.utc).isoformat(),
            "leaderboard_id": TB4_LEADERBOARD_ID,
            "source_url": "https://www.tbench.ai/", "rows": rows, "trials": trials}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--discover", action="store_true", help="Refresh leaderboard membership before acquisition")
    parser.add_argument("--concurrency", type=int, default=6)
    args = parser.parse_args()
    if not 1 <= args.concurrency <= 12:
        parser.error("concurrency must be between 1 and 12")
    manifest = discover() if args.discover or not args.manifest.exists() else json.loads(args.manifest.read_text())
    if manifest.get("schema") != SCHEMA:
        parser.error("unsupported manifest schema")
    cached = args.cache / "manifest.json"
    if cached.is_file():
        previous = {t["id"]: t for t in json.loads(cached.read_text()).get("trials", [])}
        for trial in manifest["trials"]:
            prior = previous.get(trial["id"], {})
            if not trial.get("sha256") and prior.get("trajectory_path") == trial.get("trajectory_path"):
                for key in ["sha256", "bytes", "steps", "timestamped_steps", "cost_usd"]:
                    if key in prior: trial[key] = prior[key]
    # Persist membership before downloading, so an interruption is resumable.
    write_json(args.manifest, manifest)
    asyncio.run(acquire(manifest, args.cache, args.concurrency))
    write_json(args.manifest, manifest)
    print(json.dumps(manifest["coverage"]), flush=True)
    if manifest["coverage"]["available"] != manifest["coverage"]["published"]:
        raise SystemExit("Some trajectories are unavailable; see the manifest")


if __name__ == "__main__":
    main()
